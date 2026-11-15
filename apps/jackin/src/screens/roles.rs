use std::collections::{BTreeMap, BTreeSet};

use crate::screens::Jackin;
use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::PointerEvent;
use junie_tui::shell::Page;
use junie_tui::theme::{ButtonKind, StatusKind, Tone};
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layout::Split;
use junie_tui::widgets::dialog::Dialog;
use junie_tui::widgets::empty::{self, EmptyAction, EmptyState};
use junie_tui::widgets::form_dialog::{FieldValue, FormField};
use junie_tui::widgets::info_dialog::InfoDialog;
use junie_tui::widgets::input::TextInput;
use junie_tui::widgets::keyhint::{Hint, hint};
use junie_tui::widgets::notice::Notice;
use junie_tui::widgets::option_list::Item;
use junie_tui::widgets::picker::Picker;
use junie_tui::widgets::props::Prop;
use junie_tui::widgets::segments::Segment;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;

use super::modals::{self, FormDialog};
use super::{Cx, Go, LayerTag, ModalResult, Screen};
use crate::backend::{Effect, Ticket};
use crate::domain::account::AccountId;
use crate::domain::agent::Agent;
use crate::domain::precedence::PrecedenceLevel;
use crate::domain::role::{RoleCheckState, RoleManifestDoc, RoleRecord};
use crate::domain::workspace::{AllowedRoles, RoleEntry, RoleName, RoleSource, usability_of};
use crate::world::{Msg, RoleLoad, SaveSuccess, World, job};
use junie_tui::ui::text::plural;
use junie_tui::widgets::list_detail::{Action, Event, Frame, Line, ListDetail, ListRow};

pub const LIST: WidgetId = WidgetId::of("roles.list");
pub const DETAIL: WidgetId = WidgetId::of("roles.detail");
const ADD: WidgetId = WidgetId::of("roles.empty.add");

const ORDER_LINE: &str = "A launch uses --account, then the workspace binding, then this role binding, then the provider default, then the only account.";

#[derive(Debug, Clone, PartialEq, Eq)]
enum Row {
    Role(RoleName),
    Add,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Bind {
    role: RoleName,
    agent: Agent,
    account: Option<AccountId>,
    previous: Option<AccountId>,
    undo: bool,
}

pub struct RolesScreen {
    ld: ListDetail,
    rows: Vec<Row>,
    filter: Option<String>,
    want: Option<RoleName>,
    pending: Option<Bind>,
    undo: Option<(RoleName, Agent, Option<AccountId>)>,
    pick: Option<(RoleName, Agent, Vec<AccountId>)>,
    removing: Option<RoleName>,
    checking: BTreeMap<RoleName, Ticket>,
    loading: BTreeSet<RoleName>,
    trusting: Option<RoleName>,
    adding: Option<String>,
}

impl Default for RolesScreen {
    fn default() -> Self {
        Self::new()
    }
}

fn check_state(rec: &RoleRecord, running: bool) -> &RoleCheckState {
    if running {
        &RoleCheckState::Running
    } else {
        &rec.check
    }
}

fn check_word(rec: &RoleRecord, running: bool) -> &'static str {
    if rec.manifest.is_err() {
        return "manifest error";
    }
    match check_state(rec, running) {
        RoleCheckState::Unchecked => "not checked",
        RoleCheckState::Running => "checking…",
        RoleCheckState::Done(f) if f.is_empty() => "check passed",
        RoleCheckState::Done(_) => "check failed",
    }
}

fn check_mark(rec: &RoleRecord, running: bool) -> StatusKind {
    if rec.manifest.is_err() {
        return StatusKind::Fail;
    }
    match check_state(rec, running) {
        RoleCheckState::Unchecked => StatusKind::Queued,
        RoleCheckState::Running => StatusKind::Running,
        RoleCheckState::Done(f) if f.is_empty() => StatusKind::Ok,
        RoleCheckState::Done(_) => StatusKind::Fail,
    }
}

fn location_error(v: &str) -> Option<String> {
    let v = v.trim();
    let ok = !v.chars().any(char::is_whitespace)
        && ["/", "~/", "./", "https://", "ssh://", "git@"]
            .iter()
            .any(|p| v.starts_with(p));
    (!ok).then(|| {
        "Use a directory such as ~/roles/sentinel or a git URL such as https://github.com/owner/role.git."
            .to_owned()
    })
}

fn chip(rec: &RoleRecord) -> &'static str {
    match rec.source {
        RoleSource::Git { .. } => "git",
        RoleSource::Local { .. } => "path",
    }
}

fn rule_label(level: PrecedenceLevel) -> &'static str {
    match level {
        PrecedenceLevel::Role => "role binding",
        other => other.label(),
    }
}

impl RolesScreen {
    pub fn new() -> Self {
        Self {
            ld: ListDetail::new(LIST, DETAIL, Split::new(42, 36, 44)),
            rows: vec![Row::Add],
            filter: None,
            want: None,
            pending: None,
            undo: None,
            pick: None,
            removing: None,
            checking: BTreeMap::new(),
            loading: BTreeSet::new(),
            trusting: None,
            adding: None,
        }
    }

    pub fn select(&mut self, role: Option<RoleName>) {
        if role.is_some() {
            self.want = role;
        }
    }

    fn build_rows(&mut self, w: &World) {
        let q = self.filter.as_ref().map(|f| f.to_lowercase());
        let mut rows: Vec<Row> = w
            .surfaces
            .roles
            .as_ref()
            .map(|a| {
                a.roles
                    .iter()
                    .filter(|r| q.as_ref().is_none_or(|q| r.name.to_lowercase().contains(q)))
                    .map(|r| Row::Role(r.name.clone()))
                    .collect()
            })
            .unwrap_or_default();
        rows.push(Row::Add);
        self.rows = rows;
        if let Some(want) = self.want.take()
            && let Some(i) = self
                .rows
                .iter()
                .position(|r| matches!(r, Row::Role(n) if *n == want))
        {
            self.ld.select(i);
        }
        self.ld.clamp(self.rows.len());
    }

    fn selected_role<'a>(&self, w: &'a World) -> Option<&'a RoleRecord> {
        match self.rows.get(self.ld.cursor)? {
            Row::Role(name) => w.surfaces.roles.as_ref()?.role(name),
            Row::Add => None,
        }
    }

    fn agents(&self, w: &World) -> Vec<Agent> {
        w.surfaces
            .roles
            .as_ref()
            .map(|a| a.agents.clone())
            .unwrap_or_default()
    }

    fn selected_agent(&self, w: &World) -> Option<Agent> {
        self.agents(w).get(self.ld.item_cursor).copied()
    }

    fn binding_summary(&self, rec: &RoleRecord, w: &World) -> String {
        let parts: Vec<String> = rec
            .bindings
            .iter()
            .filter_map(|(agent, id)| {
                w.accounts
                    .get(id)
                    .map(|a| format!("{} · {}", agent.label(), a.display_name))
            })
            .collect();
        parts.join(", ")
    }

    fn list_rows(&self, w: &World) -> Vec<ListRow> {
        self.rows
            .iter()
            .map(|row| match row {
                Row::Add => ListRow::new("+ Add role…").faint(),
                Row::Role(name) => {
                    let Some(rec) = w.surfaces.roles.as_ref().and_then(|a| a.role(name)) else {
                        return ListRow::new(name);
                    };
                    let summary = self.binding_summary(rec, w);
                    let row = ListRow::new(&rec.name)
                        .mark(check_mark(rec, self.checking.contains_key(&rec.name)))
                        .detail(chip(rec));
                    if summary.is_empty() {
                        row.meta("no binding", Tone::Faint)
                    } else {
                        row.meta(summary, Tone::Secondary)
                    }
                }
            })
            .collect()
    }

    fn binding_line(&self, rec: &RoleRecord, agent: Agent, w: &World) -> Line {
        let pending = self
            .pending
            .as_ref()
            .is_some_and(|p| p.role == rec.name && p.agent == agent);
        let resolved = w.account_for(
            crate::domain::tuple::TupleIdentity::new(agent, agent.provider()),
            None,
            Some(&rec.name),
            None,
        );
        let bound = rec.bindings.get(&agent).and_then(|id| w.accounts.get(id));
        let (cols, mut mark, mut tone) = match (bound, &resolved.account) {
            (Some(a), _) => (
                [
                    agent.label().to_owned(),
                    a.title(),
                    "role binding".to_owned(),
                ],
                None,
                Tone::Normal,
            ),
            (None, Some(id)) => {
                let a = w.accounts.get(id);
                let title = a.map(|a| a.title()).unwrap_or_else(|| id.clone());
                let star = if a.is_some_and(|a| a.default_for_provider) {
                    " ★"
                } else {
                    ""
                };
                (
                    [
                        agent.label().to_owned(),
                        format!("none · uses {title}{star}"),
                        rule_label(resolved.level).to_owned(),
                    ],
                    None,
                    Tone::Muted,
                )
            }
            (None, None) => (
                [
                    agent.label().to_owned(),
                    "none · no account".to_owned(),
                    "no account".to_owned(),
                ],
                Some(StatusKind::Warn),
                Tone::Warning,
            ),
        };
        let mut cols = cols;
        if let Some(a) = bound {
            let usable = usability_of(a);
            if !usable.is_ready() {
                mark = Some(StatusKind::Warn);
                tone = Tone::Warning;
                cols[2] = format!(
                    "role binding · {}",
                    usable.label(w.surfaces.fixed.as_ref(), &w.surfaces.vocabulary)
                );
            }
        }
        if pending {
            mark = Some(StatusKind::Running);
            cols[2] = "saving…".into();
        }
        Line::Item { mark, cols, tone }
    }

    fn detail(&self, w: &World) -> (String, String, Vec<Line>, Vec<Action>) {
        let Some(rec) = self.selected_role(w) else {
            let lines = vec![
                Line::Text(
                    "Register an existing role directory or a git repository.".into(),
                    Tone::Muted,
                ),
                Line::Text(
                    "A role holds the Dockerfile, policy and manifest of an agent.".into(),
                    Tone::Muted,
                ),
            ];
            return (
                "Add role".into(),
                String::new(),
                lines,
                vec![Action::new("add", "Add role…", ButtonKind::Primary)],
            );
        };
        let source = rec.source.label();
        let kind = if rec.writable() { "local" } else { "git" };
        let meta = format!(
            "{kind} · {}",
            check_word(rec, self.checking.contains_key(&rec.name))
        );
        let mut lines = vec![Line::Prop("Source".into(), source, Tone::Normal)];
        match &rec.manifest {
            Ok(doc) => {
                let hooks: Vec<&str> = [
                    doc.hook_setup.as_deref().map(|_| "setup"),
                    doc.hook_preflight.as_deref().map(|_| "preflight"),
                ]
                .into_iter()
                .flatten()
                .collect();
                lines.push(Line::Prop(
                    "Dockerfile".into(),
                    doc.dockerfile.clone(),
                    Tone::Normal,
                ));
                lines.push(Line::Prop(
                    "Policy".into(),
                    doc.policy.clone(),
                    Tone::Normal,
                ));
                lines.push(Line::Prop(
                    "Codex model".into(),
                    doc.codex_model.clone().unwrap_or_else(|| "none".into()),
                    if doc.codex_model.is_some() {
                        Tone::Normal
                    } else {
                        Tone::Muted
                    },
                ));
                lines.push(Line::Prop(
                    "Docker".into(),
                    if doc.docker { "on" } else { "off" }.into(),
                    if doc.docker {
                        Tone::Warning
                    } else {
                        Tone::Muted
                    },
                ));
                lines.push(Line::Prop(
                    "Hooks".into(),
                    if hooks.is_empty() {
                        "none".into()
                    } else {
                        hooks.join(", ")
                    },
                    Tone::Normal,
                ));
                lines.push(Line::Prop(
                    "Environment".into(),
                    if doc.env.is_empty() {
                        "none".into()
                    } else {
                        plural(doc.env.len(), "entry", "entries")
                    },
                    Tone::Normal,
                ));
                let (check, tone) = match check_state(rec, self.checking.contains_key(&rec.name)) {
                    RoleCheckState::Unchecked => ("not checked".to_owned(), Tone::Muted),
                    RoleCheckState::Running => ("checking…".to_owned(), Tone::Active),
                    RoleCheckState::Done(f) if f.is_empty() => ("passed".to_owned(), Tone::Success),
                    RoleCheckState::Done(f) => (plural(f.len(), "error", "errors"), Tone::Error),
                };
                lines.push(Line::Prop("Check".into(), check, tone));
                let findings = rec.findings();
                if !findings.is_empty() {
                    lines.push(Line::Blank);
                    lines.push(Line::Notice(Notice::warning(
                        "The manifest has errors. Launches of this role fail until you fix them.",
                    )));
                    for f in findings {
                        lines.push(Line::Text(f.message.clone(), Tone::Error));
                    }
                }
            }
            Err(message) => {
                lines.push(Line::Blank);
                lines
                    .push(Line::Notice(Notice::error(message.clone()).hint(
                        "Fix the file in your editor, then press v to check again.",
                    )));
            }
        }
        lines.push(Line::Blank);
        lines.push(Line::Heading(
            "Account bindings".into(),
            "default account of this role".into(),
        ));
        for agent in self.agents(w) {
            lines.push(self.binding_line(rec, agent, w));
        }
        let agent = self.selected_agent(w);
        let has_binding = agent.is_some_and(|a| rec.bindings.contains_key(&a));
        let busy = self.pending.is_some();
        let actions = vec![
            Action::new("edit", "Edit manifest…", ButtonKind::Secondary),
            Action::new("bind", "Bind account…", ButtonKind::Secondary).disabled(busy),
            Action::new("clear", "Clear binding", ButtonKind::Secondary)
                .disabled(busy || !has_binding),
            Action::new("check", "Check", ButtonKind::Secondary)
                .disabled(self.checking.contains_key(&rec.name)),
            Action::new("remove", "Remove…", ButtonKind::Danger),
        ];
        (format!("Role · {}", rec.name), meta, lines, actions)
    }

    fn open_pick(&mut self, w: &World, cx: &mut Cx) {
        let (Some(rec), Some(agent)) = (self.selected_role(w), self.selected_agent(w)) else {
            return;
        };
        if self.pending.is_some() {
            return;
        }
        let provider = agent.provider();
        let accounts: Vec<_> = w
            .accounts
            .accounts
            .iter()
            .filter(|a| a.provider == provider && a.enabled)
            .collect();
        if accounts.is_empty() {
            let d = Dialog::confirm(
                WidgetId::of("roles.noaccount"),
                &format!("No {} accounts", agent.label()),
                &format!("Add a {} account first.", provider.label()),
                "Add account…",
            );
            cx.open(d, LayerTag::new("noaccount"));
            return;
        }
        let current = rec.bindings.get(&agent);
        let role = rec.name.clone();
        let mut targets = vec![];
        let mut items = vec![];
        let mut chosen = None;
        for a in accounts {
            if current == Some(&a.id) {
                chosen = Some(items.len());
            }
            let usable = usability_of(a);
            let detail = if usable.is_ready() && a.default_for_provider {
                format!("★ default for {}", provider.label())
            } else {
                usable.label(w.surfaces.fixed.as_ref(), &w.surfaces.vocabulary)
            };
            items.push(Item::new(a.title()).detail(detail));
            targets.push(a.id.clone());
        }
        let mut p = Picker::new(
            WidgetId::of("roles.pick"),
            &format!("Bind account · {role} · {}", agent.label()),
        );
        p.set_searchable(false)
            .expect("disabling picker search is valid");
        p.set_items(items)
            .unwrap_or_else(|error| p.report_item_error(error));
        p.set_chosen(chosen);
        if let Some(i) = chosen {
            p.set_cursor(i);
        }
        self.pick = Some((role, agent, targets));
        cx.open(p, LayerTag::new("bind"));
    }

    fn start_bind(
        &mut self,
        role: RoleName,
        agent: Agent,
        account: Option<AccountId>,
        undo: bool,
        w: &mut World,
        cx: &mut Cx,
    ) {
        let previous = w
            .surfaces
            .roles
            .as_ref()
            .and_then(|a| a.binding(&role, agent).cloned());
        if previous == account {
            cx.status("Binding unchanged");
            return;
        }
        self.pending = Some(Bind {
            role: role.clone(),
            agent,
            account: account.clone(),
            previous: previous.clone(),
            undo,
        });
        w.perform(Effect::BindRole {
            role: role.clone(),
            agent,
            base: previous,
            account,
        });
        cx.progress(job::ROLE_BIND, format!("Binding {role}…"));
    }

    fn clear(&mut self, w: &mut World, cx: &mut Cx) {
        let (Some(rec), Some(agent)) = (self.selected_role(w), self.selected_agent(w)) else {
            return;
        };
        if rec.bindings.contains_key(&agent) && self.pending.is_none() {
            let role = rec.name.clone();
            self.start_bind(role, agent, None, false, w, cx);
        }
    }

    fn undo(&mut self, w: &mut World, cx: &mut Cx) {
        if self.pending.is_some() {
            return;
        }
        match self.undo.take() {
            Some((role, agent, account)) => self.start_bind(role, agent, account, true, w, cx),
            None => cx.status("Nothing to undo"),
        }
    }

    fn finish_bind(&mut self, result: &Result<SaveSuccess, String>, w: &mut World, cx: &mut Cx) {
        let Some(bind) = self.pending.take() else {
            return;
        };
        let saved = match result {
            Ok(saved) => saved,
            Err(reason) => {
                let hint = if reason.contains("permission denied") {
                    " Make ~/.config/jackin/config.toml writable."
                } else {
                    ""
                };
                if bind.undo {
                    self.undo = Some((bind.role, bind.agent, bind.account));
                }
                cx.error(format!(
                    "Could not save the binding: {reason}. Nothing changed. Try again.{hint}"
                ));
                return;
            }
        };
        w.apply((*saved.snapshot).clone());
        if !bind.undo {
            self.undo = Some((bind.role.clone(), bind.agent, bind.previous.clone()));
        }
        let agent = bind.agent.label();
        let role = &bind.role;
        let status = match (&bind.account, bind.undo) {
            (Some(id), true) => {
                let title = w.accounts.get(id).map(|a| a.title()).unwrap_or_default();
                format!("Restored the {agent} binding of {role} to {title}")
            }
            (None, true) => format!("Restored {role} · no {agent} binding"),
            (Some(id), false) => {
                let title = w.accounts.get(id).map(|a| a.title()).unwrap_or_default();
                format!("Bound {title} to {role} for {agent}")
            }
            (None, false) => {
                format!("Cleared the {agent} binding of {role} · u undoes")
            }
        };
        cx.status(match saved.warning {
            Some(warning) => format!("{} · {status}", warning.status()),
            None => status,
        });
    }

    fn check(&mut self, w: &mut World, cx: &mut Cx) {
        let Some(rec) = self.selected_role(w) else {
            return;
        };
        let name = rec.name.clone();
        match self.check_role(&name, w, cx) {
            RoleLoad::Requested => {}
            RoleLoad::NeedsTrust(entry) => self.ask_trust(&entry, w, cx),
            RoleLoad::Unregistered => cx.error(format!("{name} is not in the role registry")),
        }
    }

    fn check_role(&mut self, name: &str, w: &mut World, cx: &mut Cx) -> RoleLoad {
        let Some(rec) = w.surfaces.roles.as_ref().and_then(|a| a.role(name)) else {
            return RoleLoad::Unregistered;
        };
        match rec.manifest.clone() {
            Ok(doc) => {
                self.start_check(name, doc, w, cx);
                RoleLoad::Requested
            }
            Err(_) if self.loading.contains(name) => RoleLoad::Requested,
            Err(_) => {
                let load = w.load_role(name);
                if load == RoleLoad::Requested {
                    self.loading.insert(name.to_owned());
                    cx.progress(job::ROLE_LOAD, format!("Reading {name} again…"));
                }
                load
            }
        }
    }

    fn ask_trust(&mut self, entry: &RoleEntry, w: &World, cx: &mut Cx) {
        self.trusting = Some(entry.name.clone());
        let d = modals::trust_dialog(WidgetId::of("roles.trust"), entry, w);
        cx.open(d, LayerTag::new("trust").key(&entry.name));
    }

    fn start_check(&mut self, name: &str, doc: RoleManifestDoc, w: &mut World, cx: &mut Cx) {
        let ticket = w.next_ticket();
        self.checking.insert(name.to_owned(), ticket);
        w.perform(Effect::CheckRole {
            ticket,
            role: name.to_owned(),
            doc,
        });
        cx.progress(job::ROLE_CHECK, format!("Checking {name}…"));
    }

    fn check_all(&mut self, w: &mut World, cx: &mut Cx) {
        let names: Vec<RoleName> = w
            .surfaces
            .roles
            .as_ref()
            .map(|a| a.roles.iter().map(|r| r.name.clone()).collect())
            .unwrap_or_default();
        for name in names {
            self.check_role(&name, w, cx);
        }
    }

    fn ask_remove(&mut self, w: &World, cx: &mut Cx) {
        let Some(rec) = self.selected_role(w) else {
            return;
        };
        let name = rec.name.clone();
        let users: Vec<String> = w
            .workspaces
            .iter()
            .filter_map(|ws| {
                let mut uses = vec![];
                if ws.roles.default.as_deref() == Some(name.as_str()) {
                    uses.push("default role");
                }
                if matches!(&ws.roles.allowed, AllowedRoles::Custom(list) if list.contains(&name)) {
                    uses.push("allowed role");
                }
                (!uses.is_empty()).then(|| format!("{} ({})", ws.name, uses.join(" and ")))
            })
            .collect();
        if !users.is_empty() {
            let d = InfoDialog::new(
                WidgetId::of("roles.inuse"),
                &format!("Cannot remove role {name}"),
                vec![
                    Prop::new("Role", name.clone()),
                    Prop::new("State", "nothing was removed"),
                    Prop::new(
                        "Next",
                        "choose another default role or take it off the allowed roles in those workspaces, then remove it",
                    ),
                ],
            )
            .error()
            .intro(vec![(
                format!("Role {name} is still used by {}.", users.join(", ")),
                Tone::Normal,
            )])
            .width(70);
            cx.open(d, LayerTag::new("inuse"));
            return;
        }
        let stays = match &rec.source {
            RoleSource::Local { path } => format!("The files in {path} stay."),
            RoleSource::Git { .. } => "The repository stays.".to_owned(),
        };
        let d = Dialog::destructive(
            WidgetId::of("roles.remove"),
            &format!("Remove role {name}?"),
            &format!("jackin forgets the role. {stays}"),
            "Remove",
        );
        self.removing = Some(name);
        cx.open(d, LayerTag::new("remove"));
    }

    fn open_filter(&mut self, cx: &mut Cx) {
        let input = TextInput::new(WidgetId::of("roles.filter.input"), "Filter")
            .placeholder("role name")
            .value(self.filter.as_deref().unwrap_or(""))
            .plain_label();
        let d = Dialog::prompt(WidgetId::of("roles.filter"), "Filter roles", input, "Apply");
        cx.open(d, LayerTag::new("filter"));
    }

    fn edit(&mut self, w: &World, cx: &mut Cx) {
        if let Some(rec) = self.selected_role(w) {
            cx.go(Go::RoleEditor {
                role: rec.name.clone(),
            });
        }
    }

    fn add(&mut self, cx: &mut Cx) {
        if self.adding.is_some() {
            cx.status("Adding role…");
            return;
        }
        self.open_add_form("", cx);
    }

    fn open_add_form(&mut self, location: &str, cx: &mut Cx) {
        let f = WidgetId::of("roles.add");
        let fields = vec![FormField::input(
            "location",
            TextInput::new(f.sub("location"), "Location")
                .required(true)
                .placeholder("~/roles/sentinel or https://github.com/owner/role.git")
                .validator(location_error)
                .value(location),
        )];
        let form = FormDialog::new(f, "Add role", fields)
            .width(76)
            .meta("register a role");
        cx.open(form, LayerTag::new("add"));
    }

    fn start_add(&mut self, values: Vec<(String, FieldValue)>, w: &mut World, cx: &mut Cx) {
        let location = values
            .iter()
            .find_map(|(n, v)| match v {
                FieldValue::Text(s) if n == "location" => Some(s.trim().to_owned()),
                _ => None,
            })
            .unwrap_or_default();
        self.adding = Some(location.clone());
        w.perform(Effect::AddRole { location });
        cx.progress(job::ROLE_ADD, "Adding role…");
    }

    fn finish_add(
        &mut self,
        location: &str,
        result: &Result<(RoleName, SaveSuccess), String>,
        w: &mut World,
        cx: &mut Cx,
    ) {
        self.adding = None;
        match result {
            Ok((name, saved)) => {
                w.apply((*saved.snapshot).clone());
                self.want = Some(name.clone());
                self.filter = None;
                self.build_rows(w);
                let status = format!("Role {name} added");
                cx.status(match saved.warning {
                    Some(warning) => format!("{} · {status}", warning.status()),
                    None => status,
                });
            }
            Err(reason) => {
                cx.error(format!("Could not add role {location}: {reason}"));
                let d = InfoDialog::new(
                    WidgetId::of("roles.addfail"),
                    "Could not add role",
                    vec![
                        Prop::new("Location", location.to_owned()).wrap(),
                        Prop::new("Error", reason.clone()).tone(Tone::Error).wrap(),
                        Prop::new("State", "no role was registered"),
                        Prop::new("Next", "check the location, then add the role again"),
                    ],
                )
                .error()
                .width(76);
                cx.open(d, LayerTag::new("addfail").key(location));
            }
        }
    }

    fn fire(&mut self, name: &str, w: &mut World, cx: &mut Cx) {
        match name {
            "edit" => self.edit(w, cx),
            "bind" => self.open_pick(w, cx),
            "clear" => self.clear(w, cx),
            "check" => self.check(w, cx),
            "remove" => self.ask_remove(w, cx),
            "add" => self.add(cx),
            _ => {}
        }
    }

    fn empty(&self) -> EmptyState {
        EmptyState::new("No roles yet")
            .hint("A role holds the Dockerfile, policy and manifest of an agent. Add one to launch it.")
            .actions(vec![EmptyAction::primary(ADD, "Add role…")])
    }
}

impl Page<Jackin> for RolesScreen {
    fn enter(&mut self, w: &mut World, cx: &mut Cx) {
        self.build_rows(w);
        self.ld.close_detail(cx.focus);
    }

    fn primary_focus(&self) -> Option<WidgetId> {
        Some(LIST)
    }

    fn animating(&self, _w: &World) -> bool {
        self.pending.is_some()
            || self.adding.is_some()
            || !self.checking.is_empty()
            || !self.loading.is_empty()
    }

    fn on_tick(&mut self, w: &mut World, _cx: &mut Cx) -> Outcome {
        if self.animating(w) {
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    fn on_msg(&mut self, msg: &Msg, w: &mut World, cx: &mut Cx) -> Outcome {
        match msg {
            Msg::RoleBound {
                role,
                agent,
                result,
            } if self
                .pending
                .as_ref()
                .is_some_and(|p| &p.role == role && p.agent == *agent) =>
            {
                self.finish_bind(result, w, cx);
                Outcome::Changed
            }
            Msg::RoleAdded { location, result } if self.adding.as_deref() == Some(location) => {
                self.finish_add(location, result, w, cx);
                Outcome::Changed
            }
            Msg::RoleTrusted { role, result } if self.trusting.as_ref() == Some(role) => {
                self.trusting = None;
                match result {
                    Ok(snapshot) => {
                        w.apply((**snapshot).clone());
                        match self.check_role(role, w, cx) {
                            RoleLoad::Requested => {}
                            RoleLoad::NeedsTrust(_) | RoleLoad::Unregistered => {
                                cx.error(format!("{role} is still not trusted · nothing was read"));
                            }
                        }
                        self.build_rows(w);
                    }
                    Err(reason) => cx.error(reason.clone()),
                }
                Outcome::Changed
            }
            Msg::RoleLoaded { role, result } if self.loading.remove(role) => {
                match result {
                    Ok(snapshot) => {
                        w.apply((**snapshot).clone());
                        let manifest = w
                            .surfaces
                            .roles
                            .as_ref()
                            .and_then(|a| a.role(role))
                            .map(|r| r.manifest.clone());
                        match manifest {
                            Some(Ok(doc)) => self.start_check(role, doc, w, cx),
                            _ => {
                                cx.status(format!("{role}: jackin.role.toml still does not parse"))
                            }
                        }
                    }
                    Err(reason) => cx.error(format!("Could not read {role}: {reason}")),
                }
                self.build_rows(w);
                Outcome::Changed
            }
            Msg::RoleChecked {
                ticket,
                role,
                result,
            } if self.checking.get(role) == Some(ticket) => {
                self.checking.remove(role);
                let checked = match result {
                    Ok(snapshot) => snapshot
                        .surfaces
                        .roles
                        .as_ref()
                        .and_then(|a| a.role(role))
                        .cloned()
                        .ok_or_else(|| format!("the check of {role} came back empty")),
                    Err(reason) => Err(reason.clone()),
                };
                match checked {
                    Ok(record) => {
                        let found = record.findings().len();
                        w.set_role_check(role, record.check);
                        cx.status(if found == 0 {
                            format!("Role {role}: check passed")
                        } else {
                            format!("Role {role}: {}", plural(found, "error", "errors"))
                        });
                    }
                    Err(reason) => cx.error(format!("Role {role}: {reason}")),
                }
                Outcome::Changed
            }
            Msg::RoleRemoved { role, result } if self.removing.as_ref() == Some(role) => {
                self.removing = None;
                match result {
                    Ok(saved) => {
                        w.apply((*saved.snapshot).clone());
                        let status = format!("Removed role {role}");
                        cx.status(match saved.warning {
                            Some(warning) => format!("{} · {status}", warning.status()),
                            None => status,
                        });
                        self.build_rows(w);
                    }
                    Err(reason) => cx.error(format!(
                        "Could not remove role {role}: {reason}. Nothing changed."
                    )),
                }
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    fn on_key(&mut self, key: &Key, w: &mut World, cx: &mut Cx) -> Outcome {
        self.build_rows(w);
        let focus = cx.focus.current();
        let on_list = focus == Some(LIST);
        let on_detail = focus == Some(DETAIL);
        if key.plain() {
            if on_list {
                match key.code {
                    KeyCode::Char('e') => {
                        self.edit(w, cx);
                        return Outcome::Changed;
                    }
                    KeyCode::Char('a') => {
                        self.add(cx);
                        return Outcome::Changed;
                    }
                    KeyCode::Char('d') | KeyCode::Delete => {
                        self.ask_remove(w, cx);
                        return Outcome::Changed;
                    }
                    KeyCode::Char('v') => {
                        self.check(w, cx);
                        return Outcome::Changed;
                    }
                    KeyCode::Char('/') => {
                        self.open_filter(cx);
                        return Outcome::Changed;
                    }
                    KeyCode::F(5) => {
                        self.check_all(w, cx);
                        return Outcome::Changed;
                    }
                    KeyCode::Enter if matches!(self.rows.get(self.ld.cursor), Some(Row::Add)) => {
                        self.add(cx);
                        return Outcome::Changed;
                    }
                    KeyCode::Esc => {
                        if self.filter.take().is_some() {
                            self.build_rows(w);
                            cx.status("Filter cleared");
                        } else {
                            cx.go(Go::Manager);
                        }
                        return Outcome::Changed;
                    }
                    _ => {}
                }
            }
            if on_detail {
                match key.code {
                    KeyCode::Char('d') | KeyCode::Delete => {
                        self.clear(w, cx);
                        return Outcome::Changed;
                    }
                    KeyCode::Char('u') => {
                        self.undo(w, cx);
                        return Outcome::Changed;
                    }
                    KeyCode::Char('e') => {
                        self.edit(w, cx);
                        return Outcome::Changed;
                    }
                    KeyCode::Char('v') => {
                        self.check(w, cx);
                        return Outcome::Changed;
                    }
                    _ => {}
                }
            }
        }
        let before = self.ld.cursor;
        let (o, ev) = self.ld.on_key(key, cx.focus, self.rows.len());
        if before != self.ld.cursor {
            self.undo = None;
        }
        match ev {
            Some(Event::Item(_)) => self.open_pick(w, cx),
            Some(Event::Action(name)) => self.fire(name, w, cx),
            _ => {}
        }
        o
    }

    fn pointer(&mut self, ev: &PointerEvent, w: &mut World, cx: &mut Cx) -> Outcome {
        if let Some(o) = self.ld.on_pointer(ev) {
            return o;
        }
        if let Some((axis, delta)) = ev.wheel() {
            return match axis {
                junie_tui::core::hit::Axis::Vertical => self.ld.wheel(ev.id, delta),
                junie_tui::core::hit::Axis::Horizontal => Outcome::Ignored,
            };
        }
        self.build_rows(w);
        match ev.phase() {
            Some(Phase::Click) => {
                if self.empty().on_click(ev.id).is_some() {
                    self.add(cx);
                    return Outcome::Changed;
                }
                let (o, event) = self.ld.click(ev.id, cx.focus, self.rows.len());
                match event {
                    Some(Event::Item(_)) => self.open_pick(w, cx),
                    Some(Event::Action(name)) => self.fire(name, w, cx),
                    Some(Event::Row(_)) => self.undo = None,
                    None => {}
                }
                o
            }
            _ => Outcome::Ignored,
        }
    }

    fn on_layer_result(
        &mut self,
        tag: &LayerTag,
        result: ModalResult,
        w: &mut World,
        cx: &mut Cx,
    ) -> Outcome {
        match (tag.kind, result) {
            ("bind", ModalResult::Picked(i)) => {
                if let Some((role, agent, targets)) = self.pick.take()
                    && let Some(id) = targets.get(i).cloned()
                {
                    self.start_bind(role, agent, Some(id), false, w, cx);
                }
            }
            ("bind", _) => self.pick = None,
            (
                "noaccount",
                ModalResult::Dialog {
                    action: Some(1), ..
                },
            ) => cx.go(Go::Accounts { select: None }),
            (
                "remove",
                ModalResult::Dialog {
                    action: Some(1), ..
                },
            ) => {
                if let Some(role) = self.removing.clone() {
                    w.perform(Effect::RemoveRole { role });
                }
            }
            ("remove", _) => self.removing = None,
            (
                "trust",
                ModalResult::Dialog {
                    action: Some(1), ..
                },
            ) => {
                if let Some(role) = self.trusting.clone() {
                    w.perform(Effect::TrustRole { role });
                }
            }
            ("trust", _) => {
                self.trusting = None;
                cx.status("Not trusted · role not read");
            }
            ("add", ModalResult::Form(Some(values))) => self.start_add(values, w, cx),
            ("addfail", _) => self.open_add_form(&tag.key, cx),
            (
                "filter",
                ModalResult::Dialog {
                    action: Some(1),
                    text,
                },
            ) => {
                self.filter = text.filter(|t| !t.trim().is_empty());
                self.build_rows(w);
            }
            _ => {}
        }
        Outcome::Changed
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        self.build_rows(w);
        let t = ctx.theme;
        let only_add = self.rows.len() == 1 && self.filter.is_none();
        if only_add {
            ctx.control(LIST, Rect::new(area.x, area.y, 1, 1), false);
            empty::render_ctx(area, buf, ctx, &self.empty(), t.canvas);
            return;
        }
        let rows = self.list_rows(w);
        let roles = w.surfaces.roles.as_ref().map_or(0, |a| a.roles.len());
        let failing = w.surfaces.roles.as_ref().map_or(0, |a| {
            a.roles
                .iter()
                .filter(|r| check_mark(r, false) == StatusKind::Fail)
                .count()
        });
        let mut meta = plural(roles, "role", "roles");
        if failing > 0 {
            meta.push_str(&format!(" · {failing} !"));
        }
        let title = match &self.filter {
            Some(f) => format!("Roles · filter {f}"),
            None => "Roles".into(),
        };
        let (detail_title, detail_meta, lines, actions) = self.detail(w);
        let frame = Frame {
            title: &title,
            meta,
            head: vec![],
            rows: &rows,
            under: Some(ORDER_LINE),
            detail_title,
            detail_meta,
            lines,
            actions,
        };
        self.ld.render(area, buf, ctx, &frame);
    }

    fn hints(&self, focus: Option<WidgetId>, w: &World) -> Vec<Hint> {
        if let Some(v) = self.ld.hints(
            focus,
            &[hint("Enter", "Pick"), hint("d", "Clear"), hint("u", "Undo")],
        ) {
            return v;
        }
        let mut v = vec![hint("↑↓", "Move"), hint("Enter", "Details")];
        if self.selected_role(w).is_some() {
            v.push(hint("e", "Edit manifest…"));
            v.push(hint("v", "Check"));
            v.push(hint("d", "Remove…"));
        }
        v.push(hint("a", "Add role…"));
        v.push(hint("/", "Filter"));
        v.push(hint("Esc", "Back"));
        v
    }

    fn crumb(&self, w: &World) -> String {
        match self.selected_role(w) {
            Some(rec) => format!("Roles › {}", rec.name),
            None => "Roles".into(),
        }
    }
}

impl Screen for RolesScreen {
    fn strip_right(&self, w: &World) -> Vec<Segment> {
        if self.pending.is_some() {
            return vec![
                Segment::new(
                    format!(
                        "{} binding…",
                        junie_tui::widgets::progress::spinner_frame(w.now_ms() as u64 / 80)
                    ),
                    Tone::Secondary,
                )
                .priority(6),
            ];
        }
        vec![]
    }

    fn job_continues(&self, m: &Msg) -> bool {
        match m {
            Msg::RoleLoaded { role, .. } => self.loading.iter().any(|r| r != role),
            Msg::RoleChecked { ticket, role, .. } => {
                self.checking.iter().any(|(r, t)| r != role || t != ticket)
            }
            _ => false,
        }
    }
}
