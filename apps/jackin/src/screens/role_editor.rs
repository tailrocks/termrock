use crate::screens::Jackin;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use termrock::core::event::{Key, Outcome};
use termrock::core::id::WidgetId;
use termrock::core::intent::Phase;
use termrock::core::pointer::{MouseTarget, PointerEvent};
use termrock::core::scroll::{ScrollState, TrackClick};
use termrock::shell::Page;
use termrock::theme::{StatusKind, Tone};
use termrock::ui::ctx::{RenderCtx, fill, legible_on_fill};
use termrock::ui::layout::{Stack, StackChild, StackScroll};
use termrock::ui::text::{fit, plural, truncate};
use termrock::widgets::button::{Button, row_layout_right};
use termrock::widgets::choice::Checkbox;
use termrock::widgets::dialog::Dialog;
use termrock::widgets::form_dialog::{FieldValue, FormField, FormValues};
use termrock::widgets::info_dialog::InfoDialog;
use termrock::widgets::input::{InputAction, TextInput};
use termrock::widgets::keyhint::{Hint, hint};
use termrock::widgets::notice::Notice;
use termrock::widgets::option_list::Item;
use termrock::widgets::panel::Panel;
use termrock::widgets::picker::Picker;
use termrock::widgets::progress::spinner_frame;
use termrock::widgets::props::Prop;
use termrock::widgets::scrollbar;
use termrock::widgets::segments::Segment;
use termrock::widgets::tabs::{TabAction, TabItem, Tabs};

use super::modals::{FormDialog, unsaved_dialog};
use super::{Cx, Go, LayerTag, ModalResult, Screen};
use crate::backend::{Effect, Ticket};
use crate::domain::role::{RoleCheckState, RoleEnvEntry, RoleFinding, RoleManifestDoc, RoleRecord};
use crate::domain::workspace::{RoleName, RoleSource, env_key_error};
use crate::world::{Msg, Snapshot, World, job};

const TABS: WidgetId = WidgetId::of("roleeditor.tabs");
const NAME: WidgetId = WidgetId::of("roleeditor.name");
const DOCKERFILE: WidgetId = WidgetId::of("roleeditor.dockerfile");
const POLICY: WidgetId = WidgetId::of("roleeditor.policy");
const MODEL: WidgetId = WidgetId::of("roleeditor.model");
const DOCKER: WidgetId = WidgetId::of("roleeditor.docker");
const SETUP: WidgetId = WidgetId::of("roleeditor.setup");
const PREFLIGHT: WidgetId = WidgetId::of("roleeditor.preflight");
const ENV: WidgetId = WidgetId::of("roleeditor.env");
const CHECK: WidgetId = WidgetId::of("roleeditor.check");
const CANCEL: WidgetId = WidgetId::of("roleeditor.cancel");
const SAVE: WidgetId = WidgetId::of("roleeditor.save");
const MANIFEST_BODY: WidgetId = WidgetId::of("roleeditor.manifest");
const HOOKS_BODY: WidgetId = WidgetId::of("roleeditor.hooks");
const TAB_NAMES: [&str; 3] = ["Manifest", "Hooks", "Environment"];

const PATH_MESSAGE: &str = "Use a path inside the role repository, for example hooks/setup.sh.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Manifest,
    Hooks,
    Environment,
}

impl Tab {
    const ALL: [Tab; 3] = [Tab::Manifest, Tab::Hooks, Tab::Environment];

    fn index(self) -> usize {
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pick {
    Dockerfile,
    Policy,
    Setup,
    Preflight,
}

const NAME_MESSAGE: &str = "Use letters, digits, - and _ in the role name.";

fn name_error(v: &str) -> Option<String> {
    let v = v.trim();
    let ok = !v.is_empty()
        && v.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    (!ok).then(|| NAME_MESSAGE.to_owned())
}

fn path_error(v: &str, optional: bool) -> Option<String> {
    let v = v.trim();
    if v.is_empty() {
        return (!optional).then(|| PATH_MESSAGE.to_owned());
    }
    let bad = v.starts_with('/') || v.starts_with('~') || v.split('/').any(|p| p == "..");
    bad.then(|| PATH_MESSAGE.to_owned())
}

fn entry_name_error(v: &str) -> Option<String> {
    let name = v.trim();
    if name.starts_with("JACKIN_") {
        return Some(format!(
            "{name} is reserved by jackin. Choose another name."
        ));
    }
    env_key_error(name)
}

fn path_input(id: WidgetId, label: &str, value: &str, optional: bool) -> TextInput {
    TextInput::new(id, label)
        .required(!optional)
        .required_message(PATH_MESSAGE)
        .value(value)
        .validator(move |v| path_error(v, optional))
}

fn some(v: &str) -> Option<String> {
    let v = v.trim();
    (!v.is_empty()).then(|| v.to_owned())
}

fn on_off(v: bool) -> &'static str {
    if v { "on" } else { "off" }
}

fn shown(v: &Option<String>) -> String {
    v.clone().unwrap_or_else(|| "none".into())
}

pub struct RoleEditorScreen {
    pub role: RoleName,
    readonly: bool,
    fatal: Option<String>,
    original: RoleManifestDoc,
    pending: RoleManifestDoc,
    source: RoleSource,
    files: Vec<String>,
    tab: Tab,
    tabs: Tabs,
    name: TextInput,
    dockerfile: TextInput,
    policy: TextInput,
    model: TextInput,
    docker: Checkbox,
    setup: TextInput,
    preflight: TextInput,
    choose: [Button; 4],
    manifest_stack: StackScroll,
    hooks_stack: StackScroll,
    env_cursor: usize,
    env_scroll: ScrollState,
    removed: Vec<(usize, RoleEnvEntry)>,
    editing_entry: Option<Option<String>>,
    check_scroll: ScrollState,
    cancel: Button,
    save: Button,
    saving: bool,
    check: RoleCheckState,
    checked: Option<RoleManifestDoc>,
    check_ticket: Option<Ticket>,
    save_after_check: bool,
    pick: Option<Pick>,
    pick_targets: Vec<String>,
}

fn empty_doc() -> RoleManifestDoc {
    RoleManifestDoc {
        name: String::new(),
        dockerfile: String::new(),
        policy: String::new(),
        codex_model: None,
        docker: false,
        hook_setup: None,
        hook_preflight: None,
        env: vec![],
    }
}

impl RoleEditorScreen {
    pub fn new(w: &World, role: &str) -> Self {
        let rec = w.surfaces.roles.as_ref().and_then(|a| a.role(role));
        let (doc, fatal) = match rec.map(|r| &r.manifest) {
            Some(Ok(doc)) => (doc.clone(), None),
            Some(Err(message)) => (empty_doc(), Some(message.clone())),
            None => (empty_doc(), Some(format!("Role {role} is not registered."))),
        };
        let readonly = rec.is_none_or(|r| !r.writable());
        let source = rec.map(|r| r.source.clone()).unwrap_or(RoleSource::Local {
            path: String::new(),
        });
        let files = rec.map(|r| r.files.clone()).unwrap_or_default();
        let locked = readonly || fatal.is_some();
        let choose = |id: &'static str| Button::secondary(WidgetId::of(id), "Choose…");
        let mut s = Self {
            role: role.to_owned(),
            readonly,
            fatal,
            original: doc.clone(),
            pending: doc.clone(),
            source,
            files,
            tab: Tab::Manifest,
            tabs: Tabs::new(TABS, &TAB_NAMES),
            name: TextInput::new(NAME, "Name")
                .required(true)
                .required_message(NAME_MESSAGE)
                .value(&doc.name)
                .validator(name_error)
                .disabled(locked),
            dockerfile: path_input(DOCKERFILE, "Dockerfile", &doc.dockerfile, false)
                .disabled(locked),
            policy: path_input(POLICY, "Policy", &doc.policy, false).disabled(locked),
            model: TextInput::new(MODEL, "Codex model")
                .placeholder("Codex default")
                .value(doc.codex_model.as_deref().unwrap_or(""))
                .disabled(locked),
            docker: Checkbox::new(DOCKER, "Docker for every launch of this role", doc.docker),
            setup: path_input(
                SETUP,
                "Setup",
                doc.hook_setup.as_deref().unwrap_or(""),
                true,
            )
            .disabled(locked),
            preflight: path_input(
                PREFLIGHT,
                "Preflight",
                doc.hook_preflight.as_deref().unwrap_or(""),
                true,
            )
            .disabled(locked),
            choose: [
                choose("roleeditor.choose.dockerfile"),
                choose("roleeditor.choose.policy"),
                choose("roleeditor.choose.setup"),
                choose("roleeditor.choose.preflight"),
            ],
            manifest_stack: StackScroll::default(),
            hooks_stack: StackScroll::default(),
            env_cursor: 0,
            env_scroll: ScrollState::default(),
            removed: vec![],
            editing_entry: None,
            check_scroll: ScrollState::default(),
            cancel: Button::subtle(CANCEL, "Cancel"),
            save: Button::primary(SAVE, "Save…"),
            saving: false,
            check: rec.map_or(RoleCheckState::Unchecked, |r| r.check.clone()),
            checked: rec
                .filter(|r| matches!(r.check, RoleCheckState::Done(_)))
                .map(|_| doc.clone()),
            check_ticket: None,
            save_after_check: false,
            pick: None,
            pick_targets: vec![],
        };
        s.docker.disabled = locked;
        s
    }

    fn record<'a>(&self, w: &'a World) -> Option<&'a RoleRecord> {
        w.surfaces.roles.as_ref()?.role(&self.role)
    }

    fn locked(&self) -> bool {
        self.readonly || self.fatal.is_some()
    }

    fn sync(&mut self) {
        self.pending.name = self.name.text().trim().to_owned();
        self.pending.dockerfile = self.dockerfile.text().trim().to_owned();
        self.pending.policy = self.policy.text().trim().to_owned();
        self.pending.codex_model = some(self.model.text());
        self.pending.docker = self.docker.checked;
        self.pending.hook_setup = some(self.setup.text());
        self.pending.hook_preflight = some(self.preflight.text());
    }

    fn manifest_dirty(&self) -> bool {
        let (a, b) = (&self.pending, &self.original);
        a.name != b.name
            || a.dockerfile != b.dockerfile
            || a.policy != b.policy
            || a.codex_model != b.codex_model
            || a.docker != b.docker
    }

    fn hooks_dirty(&self) -> bool {
        self.pending.hook_setup != self.original.hook_setup
            || self.pending.hook_preflight != self.original.hook_preflight
    }

    fn env_changes(&self) -> usize {
        crate::world::keyed_diff(&self.original.env, &self.pending.env, |e| e.name.clone())
    }

    fn env_dirty(&self) -> bool {
        self.pending.env != self.original.env
    }

    fn change_count(&self) -> usize {
        let (a, b) = (&self.pending, &self.original);
        usize::from(a.name != b.name)
            + usize::from(a.dockerfile != b.dockerfile)
            + usize::from(a.policy != b.policy)
            + usize::from(a.codex_model != b.codex_model)
            + usize::from(a.docker != b.docker)
            + usize::from(a.hook_setup != b.hook_setup)
            + usize::from(a.hook_preflight != b.hook_preflight)
            + self.env_changes()
    }

    fn checking(&self) -> bool {
        matches!(self.check, RoleCheckState::Running)
    }

    fn findings(&self) -> &[RoleFinding] {
        match &self.check {
            RoleCheckState::Done(f) => f,
            _ => &[],
        }
    }

    fn tab_of(finding: &RoleFinding) -> Tab {
        match finding.key.as_deref() {
            Some(k) if k.starts_with("hooks.") => Tab::Hooks,
            Some(k) if k.starts_with("env.") => Tab::Environment,
            _ => Tab::Manifest,
        }
    }

    fn tab_error(&self, tab: Tab) -> bool {
        self.findings().iter().any(|f| Self::tab_of(f) == tab)
            || match tab {
                Tab::Manifest => [&self.name, &self.dockerfile, &self.policy]
                    .iter()
                    .any(|i| i.error().is_some()),
                Tab::Hooks => [&self.setup, &self.preflight]
                    .iter()
                    .any(|i| i.error().is_some()),
                Tab::Environment => self.env_problems().iter().any(Option::is_some),
            }
    }

    fn env_problems(&self) -> Vec<Option<String>> {
        self.pending
            .env
            .iter()
            .enumerate()
            .map(|(i, e)| {
                entry_name_error(&e.name).or_else(|| {
                    self.pending.env[..i]
                        .iter()
                        .any(|o| o.name == e.name)
                        .then(|| format!("An entry named {} already exists.", e.name))
                })
            })
            .collect()
    }

    fn apply_findings(&mut self) {
        let findings: Vec<RoleFinding> = self.findings().to_vec();
        for (input, key) in [
            (&mut self.dockerfile, "role.dockerfile"),
            (&mut self.policy, "role.policy"),
            (&mut self.setup, "hooks.setup"),
            (&mut self.preflight, "hooks.preflight"),
        ] {
            let found = findings
                .iter()
                .find(|f| f.key.as_deref() == Some(key))
                .map(|f| f.message.clone());
            if found.is_some() {
                input.set_error(found);
            }
        }
    }

    fn set_tab(&mut self, tab: Tab, cx: &mut Cx) {
        self.tab = tab;
        self.tabs.set_active(tab.index());
        cx.focus.focus(TABS);
    }

    fn body_focus(&self) -> WidgetId {
        match self.tab {
            Tab::Manifest => NAME,
            Tab::Hooks => SETUP,
            Tab::Environment => ENV,
        }
    }

    fn is_busy(&self) -> bool {
        self.saving || self.checking()
    }

    fn run_check(&mut self, w: &mut World, cx: &mut Cx) {
        if self.checking() {
            cx.status("A check is already running");
            return;
        }
        if self.fatal.is_some() {
            if let Some(r) = self.record(w)
                && let Ok(doc) = &r.manifest
            {
                let doc = doc.clone();
                *self = Self::new(w, &self.role.clone());
                self.original = doc.clone();
                self.pending = doc;
                cx.status("Reloaded jackin.role.toml");
            } else {
                cx.status("jackin.role.toml still does not parse");
            }
            return;
        }
        self.sync();
        if !self.validate_fields() {
            cx.error("Fix the entries marked ! before the check");
            return;
        }
        self.save_after_check = false;
        self.start_check(w, cx);
    }

    fn start_check(&mut self, w: &mut World, cx: &mut Cx) {
        self.check = RoleCheckState::Running;
        self.checked = Some(self.pending.clone());
        let ticket = w.next_ticket();
        self.check_ticket = Some(ticket);
        w.perform(Effect::CheckRole {
            ticket,
            role: self.role.clone(),
            doc: self.pending.clone(),
        });
        cx.progress(job::ROLE_CHECK, format!("Checking {}…", self.role));
    }

    fn is_stale(&self) -> bool {
        self.checked.as_ref() != Some(&self.pending)
    }

    fn stale_text(&self) -> &'static str {
        if self.checked.is_none() {
            "Not checked yet · F5"
        } else {
            "Not checked since your last edit · F5"
        }
    }

    fn validate_fields(&mut self) -> bool {
        let mut ok = true;
        for input in [
            &mut self.name,
            &mut self.dockerfile,
            &mut self.policy,
            &mut self.setup,
            &mut self.preflight,
        ] {
            input.field.error = None;
            ok &= input.validate();
        }
        ok && self.env_problems().iter().all(Option::is_none)
    }

    fn open_docker_risks(&self, w: &World, cx: &mut Cx, confirm: bool) {
        let Some(d) = w.surfaces.docker.as_ref() else {
            return;
        };
        let dialog = super::modals::docker_risk_dialog(
            WidgetId::of("roleeditor.docker.risks"),
            &format!("role {}", self.role),
            &d.risks,
            confirm,
        );
        cx.open(
            dialog,
            LayerTag::new(if confirm { "docker.on" } else { "docker.info" }),
        );
    }

    fn toggle_docker(&mut self, w: &World, cx: &mut Cx) -> Outcome {
        if self.docker.checked || w.surfaces.docker.is_none() {
            let o = self.docker.on_click();
            self.sync();
            return o;
        }
        self.open_docker_risks(w, cx, true);
        Outcome::Changed
    }

    fn request_save(&mut self, w: &mut World, cx: &mut Cx) {
        self.sync();
        if self.locked() {
            cx.status("This role is read-only");
            return;
        }
        if !self.validate_fields() {
            self.reveal_error(cx);
            cx.error("Fix the entries marked ! before saving");
            return;
        }
        if self.is_busy() {
            self.open_check_failed(cx);
            return;
        }
        if self.change_count() == 0 {
            cx.status("Nothing to save");
            return;
        }
        self.save_after_check = true;
        self.start_check(w, cx);
    }

    fn open_preview(&mut self, cx: &mut Cx) {
        let n = self.change_count();
        let scope = match &self.source {
            RoleSource::Local { path } => format!("role manifest · {path}/jackin.role.toml"),
            RoleSource::Git { url, .. } => format!("role manifest · {url}"),
        };
        let (a, b) = (&self.original, &self.pending);
        let mut facts = vec![
            Prop::new("Role", self.role.clone()),
            Prop::new("Scope", scope),
            Prop::new("Changes", plural(n, "change", "changes")),
        ];
        let mut code = vec![];
        let mut field = |label: &str, key: &str, from: String, to: String, tone: Tone| {
            if from != to {
                facts.push(Prop::new(label, format!("{from} → {to}")).tone(tone).wrap());
                code.push(format!("~ {key} {from} → {to}"));
            }
        };
        field("Name", "name", a.name.clone(), b.name.clone(), Tone::Normal);
        field(
            "Dockerfile",
            "role.dockerfile",
            a.dockerfile.clone(),
            b.dockerfile.clone(),
            Tone::Normal,
        );
        field(
            "Policy",
            "role.policy",
            a.policy.clone(),
            b.policy.clone(),
            Tone::Normal,
        );
        field(
            "Codex model",
            "codex.model",
            shown(&a.codex_model),
            shown(&b.codex_model),
            Tone::Normal,
        );
        field(
            "Docker",
            "docker",
            on_off(a.docker).into(),
            on_off(b.docker).into(),
            Tone::Warning,
        );
        field(
            "Setup hook",
            "hooks.setup",
            shown(&a.hook_setup),
            shown(&b.hook_setup),
            Tone::Normal,
        );
        field(
            "Preflight hook",
            "hooks.preflight",
            shown(&a.hook_preflight),
            shown(&b.hook_preflight),
            Tone::Normal,
        );
        for e in &b.env {
            match a.env.iter().find(|o| o.name == e.name) {
                None => {
                    facts.push(Prop::new("Environment", format!("+ {}", e.name)));
                    code.push(format!("+ env.{}", e.name));
                }
                Some(o) if o != e => {
                    facts.push(Prop::new("Environment", format!("~ {}", e.name)));
                    code.push(format!("~ env.{}", e.name));
                }
                _ => {}
            }
        }
        for o in &a.env {
            if !b.env.iter().any(|e| e.name == o.name) {
                facts.push(Prop::new("Environment", format!("− {}", o.name)));
                code.push(format!("- env.{}", o.name));
            }
        }
        let d = Dialog::facts(
            WidgetId::of("roleeditor.preview"),
            "Save role",
            facts,
            code,
            None,
            Button::primary(WidgetId::of("roleeditor.preview.save"), "Save"),
        );
        cx.open(d, LayerTag::new("preview"));
    }

    fn open_check_failed(&self, cx: &mut Cx) {
        let mut props: Vec<Prop> = self
            .findings()
            .iter()
            .map(|f| {
                Prop::new("Error", f.message.clone())
                    .tone(Tone::Error)
                    .wrap()
            })
            .collect();
        if props.is_empty() {
            props.push(Prop::new("Check", "still running"));
        }
        props.push(Prop::new(
            "State",
            "your edits are intact · nothing was written",
        ));
        props.push(Prop::new("Next", "fix the entries marked !, then press F5"));
        let d = InfoDialog::new(
            WidgetId::of("roleeditor.checkfail"),
            &format!("Check failed · {}", self.role),
            props,
        )
        .error()
        .width(76);
        cx.open(d, LayerTag::new("checkfail"));
    }

    fn reveal_error(&mut self, cx: &mut Cx) {
        let first = [
            (Tab::Manifest, &self.name),
            (Tab::Manifest, &self.dockerfile),
            (Tab::Manifest, &self.policy),
            (Tab::Hooks, &self.setup),
            (Tab::Hooks, &self.preflight),
        ]
        .into_iter()
        .find(|(_, i)| i.error().is_some())
        .map(|(t, i)| (t, i.id));
        if let Some((tab, id)) = first {
            self.set_tab(tab, cx);
            cx.focus.focus(id);
        }
    }

    fn start_save(&mut self, w: &mut World, cx: &mut Cx) {
        self.sync();
        self.saving = true;
        self.save.busy = true;
        w.perform(Effect::SaveRole {
            role: self.role.clone(),
            base: Box::new(self.original.clone()),
            doc: Box::new(self.pending.clone()),
        });
        cx.progress(job::ROLE_SAVE, format!("Saving {}…", self.role));
        cx.focus.focus(SAVE);
    }

    fn finish_save(&mut self, result: &Result<Box<Snapshot>, String>, w: &mut World, cx: &mut Cx) {
        self.saving = false;
        self.save.busy = false;
        let snapshot = match result {
            Ok(snapshot) => snapshot,
            Err(error) => {
                let d = InfoDialog::new(
                    WidgetId::of("roleeditor.savefail"),
                    &format!("Save failed · {}", self.role),
                    vec![
                        Prop::new("Error", error.as_str()).wrap(),
                        Prop::new("State", "your edits are intact · nothing was written"),
                        Prop::new("Next", "fix the cause and Save again"),
                    ],
                )
                .error()
                .width(76);
                cx.open(d, LayerTag::new("savefail"));
                return;
            }
        };
        w.apply((**snapshot).clone());
        let name = self.pending.name.clone();
        self.original = self.pending.clone();
        cx.status(format!("Role {name} saved"));
        cx.go(Go::Roles { select: Some(name) });
    }

    fn leave(&mut self, cx: &mut Cx) {
        self.sync();
        if self.change_count() > 0 {
            let d = unsaved_dialog(WidgetId::of("roleeditor.exit"), self.change_count());
            cx.open(d, LayerTag::new("exit"));
        } else {
            cx.go(Go::Roles {
                select: Some(self.role.clone()),
            });
        }
    }

    fn open_choose(&mut self, which: Pick, cx: &mut Cx) {
        if self.locked() {
            return;
        }
        let current = match which {
            Pick::Dockerfile => self.dockerfile.text(),
            Pick::Policy => self.policy.text(),
            Pick::Setup => self.setup.text(),
            Pick::Preflight => self.preflight.text(),
        }
        .to_owned();
        let items: Vec<Item> = self.files.iter().map(Item::new).collect();
        if items.is_empty() {
            cx.status("The role repository has no files to choose from");
            return;
        }
        let mut p = Picker::new(WidgetId::of("roleeditor.choose"), "Choose a file");
        p.set_searchable(true)
            .unwrap_or_else(|error| p.report_item_error(error));
        p.scope = Some(format!("role {}", self.role));
        p.set_items(items)
            .unwrap_or_else(|error| p.report_item_error(error));
        if let Some(i) = self.files.iter().position(|f| *f == current) {
            p.set_chosen(Some(i));
            p.set_cursor(i);
        }
        self.pick = Some(which);
        self.pick_targets = self.files.clone();
        cx.open(p, LayerTag::new("choose"));
    }

    fn input_mut(&mut self, id: WidgetId) -> Option<&mut TextInput> {
        [
            &mut self.name,
            &mut self.dockerfile,
            &mut self.policy,
            &mut self.model,
            &mut self.setup,
            &mut self.preflight,
        ]
        .into_iter()
        .find(|i| i.id == id)
    }

    fn editing_any(&self) -> bool {
        [
            &self.name,
            &self.dockerfile,
            &self.policy,
            &self.model,
            &self.setup,
            &self.preflight,
        ]
        .iter()
        .any(|i| i.editing)
    }

    fn open_entry_form(&mut self, original: Option<String>, cx: &mut Cx) {
        if self.locked() {
            return;
        }
        let e = original
            .as_ref()
            .and_then(|n| self.pending.env.iter().find(|e| &e.name == n))
            .cloned();
        let f = WidgetId::of("roleeditor.entry");
        let fields = vec![
            FormField::input(
                "name",
                TextInput::new(f.sub("name"), "Name")
                    .required(true)
                    .required_message("Key is required")
                    .placeholder("EXAMPLE_NAME")
                    .validator(entry_name_error)
                    .value(e.as_ref().map_or("", |e| e.name.as_str())),
            ),
            FormField::input(
                "default",
                TextInput::new(f.sub("default"), "Default")
                    .placeholder("empty")
                    .value(e.as_ref().and_then(|e| e.default.as_deref()).unwrap_or("")),
            ),
            FormField::input(
                "prompt",
                TextInput::new(f.sub("prompt"), "Prompt")
                    .placeholder("Asked at launch when there is no default")
                    .value(e.as_ref().and_then(|e| e.prompt.as_deref()).unwrap_or("")),
            ),
        ];
        let title = match &e {
            Some(e) => format!("Edit {}", e.name),
            None => "New environment entry".to_owned(),
        };
        let form = FormDialog::new(f, &title, fields)
            .width(70)
            .meta("role environment");
        self.editing_entry = Some(original);
        cx.open(form, LayerTag::new("entry"));
    }

    fn save_entry(&mut self, values: FormValues, cx: &mut Cx) -> Outcome {
        let text = |name: &str| {
            values
                .iter()
                .find(|(n, _)| n == name)
                .and_then(|(_, v)| match v {
                    FieldValue::Text(s) => Some(s.trim().to_owned()),
                    _ => None,
                })
                .unwrap_or_default()
        };
        let name = text("name");
        let original = self.editing_entry.clone().flatten();
        let error = entry_name_error(&name).or_else(|| {
            self.pending
                .env
                .iter()
                .any(|e| e.name == name && Some(&e.name) != original.as_ref())
                .then(|| {
                    format!("An entry named {name} already exists. Edit it or choose another name.")
                })
        });
        if let Some(e) = error {
            cx.with_top::<FormDialog>(move |f| f.error = Some(e));
            return Outcome::Changed;
        }
        let entry = RoleEnvEntry {
            name: name.clone(),
            default: some(&text("default")),
            prompt: some(&text("prompt")),
        };
        match original.and_then(|o| self.pending.env.iter().position(|e| e.name == o)) {
            Some(i) => self.pending.env[i] = entry,
            None => self.pending.env.push(entry),
        }
        self.pending.env.sort_by(|a, b| a.name.cmp(&b.name));
        self.editing_entry = None;
        cx.with_top::<FormDialog>(|_| {});
        cx.status(format!("Entry {name} · save to apply"));
        Outcome::Changed
    }

    fn remove_entry(&mut self, cx: &mut Cx) {
        if self.locked() || self.env_cursor >= self.pending.env.len() {
            return;
        }
        let e = self.pending.env.remove(self.env_cursor);
        cx.status(format!("Removed {} · u restores until save", e.name));
        self.removed.push((self.env_cursor, e));
    }

    fn undo_entry(&mut self, cx: &mut Cx) {
        if let Some((i, e)) = self.removed.pop() {
            let at = i.min(self.pending.env.len());
            cx.status(format!("Restored {}", e.name));
            self.pending.env.insert(at, e);
        } else {
            cx.status("Nothing to undo");
        }
    }

    fn render_field(
        &mut self,
        id: WidgetId,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        choose: Option<usize>,
    ) {
        let bg = ctx.theme.canvas;
        let button_w = 11u16;
        let (input_area, button) = match choose {
            Some(i) => (
                Rect::new(
                    area.x,
                    area.y,
                    area.width.saturating_sub(button_w + 2),
                    area.height,
                ),
                Some((
                    i,
                    Rect::new(
                        area.right().saturating_sub(button_w),
                        area.y + 1,
                        button_w,
                        1,
                    ),
                )),
            ),
            None => (area, None),
        };
        if let Some(input) = self.input_mut(id) {
            input.render(input_area, buf, ctx, bg);
        }
        if let Some((i, r)) = button {
            self.choose[i].disabled = self.readonly || self.fatal.is_some();
            self.choose[i].render(r, buf, ctx, bg);
        }
    }

    fn render_manifest(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        let t = ctx.theme;
        let bg = t.canvas;
        let fw = area.width.saturating_sub(4).min(72);
        let docker = w.surfaces.docker.as_ref();
        let notice = Notice::warning(format!(
            "{} · press i to read them",
            plural(docker.map_or(0, |d| d.risks.len()), "risk", "risks")
        ));
        let docker_h = if docker.is_some() && self.docker.checked {
            2
        } else {
            1
        };
        let kids = [
            StackChild::fixed(3),
            StackChild::fixed(3),
            StackChild::fixed(3),
            StackChild::fixed(3),
            StackChild::fixed(docker_h),
        ];
        let ids = [NAME, DOCKERFILE, POLICY, MODEL, DOCKER];
        let at = ids.iter().position(|id| ctx.interaction.focus == Some(*id));
        let l = Stack::default().layout(area, &kids, &mut self.manifest_stack, at);
        Stack::chrome(&l, area, buf, ctx, MANIFEST_BODY, &self.manifest_stack);
        let col = |r: Rect| Rect::new(r.x + 2, r.y, fw.min(r.width.saturating_sub(2)), r.height);
        let chooses = [None, Some(0), Some(1), None];
        for (i, id) in ids[..4].iter().enumerate() {
            match l.slot(i) {
                Some(r) => self.render_field(*id, col(r), buf, ctx, chooses[i]),
                None => ctx.focus_stop(*id),
            }
        }
        match l.slot(4) {
            Some(r) => {
                let r = col(r);
                self.docker
                    .render(Rect::new(r.x, r.y, fw.min(r.width), 1), buf, ctx, bg);
                if r.height > 1 {
                    notice.render(
                        Rect::new(r.x + 4, r.y + 1, fw.saturating_sub(4), 1),
                        buf,
                        ctx,
                        bg,
                    );
                } else {
                    let hx = r.x + 42;
                    buf.set_string(
                        hx,
                        r.y,
                        termrock::ui::reveal::cut(
                            Rect::new(hx, r.y, area.right().saturating_sub(hx), 1),
                            "Off by default · adds a Docker client and one engine rule",
                        ),
                        t.faint().bg(bg),
                    );
                }
            }
            None => ctx.focus_stop(DOCKER),
        }
    }

    fn render_hooks(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let bg = t.canvas;
        let fw = area.width.saturating_sub(4).min(72);
        let kids = [StackChild::fixed(4), StackChild::fixed(4)];
        let ids = [SETUP, PREFLIGHT];
        let at = ids.iter().position(|id| ctx.interaction.focus == Some(*id));
        let l = Stack::default().layout(area, &kids, &mut self.hooks_stack, at);
        Stack::chrome(&l, area, buf, ctx, HOOKS_BODY, &self.hooks_stack);
        let helps = [
            "Runs once for each launch as user sandbox, with no terminal, in /sandbox/project",
            "Runs before each Codex start",
        ];
        for (i, id) in ids.iter().enumerate() {
            match l.slot(i) {
                Some(r) => {
                    let r = Rect::new(r.x + 2, r.y, fw.min(r.width.saturating_sub(2)), 3);
                    self.render_field(*id, r, buf, ctx, Some(2 + i));
                    buf.set_string(
                        r.x,
                        r.y + 3,
                        termrock::ui::reveal::cut(
                            Rect::new(r.x, r.y + 3, area.right().saturating_sub(r.x), 1),
                            helps[i],
                        ),
                        t.faint().bg(bg),
                    );
                }
                None => ctx.focus_stop(*id),
            }
        }
    }

    fn render_env(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let bg = t.canvas;
        let note = Notice::info("Only non-secret values belong here. Put secrets in an account.");
        let used = note.render(
            Rect::new(area.x + 2, area.y, area.width.saturating_sub(4), 2),
            buf,
            ctx,
            bg,
        );
        let head_y = area.y + used + 1;
        if head_y >= area.bottom() {
            return;
        }
        let n = self.pending.env.len();
        let head = format!("Entries  {n}");
        buf.set_string(
            area.x + 2,
            head_y,
            &head,
            t.secondary().bg(bg).add_modifier(Modifier::BOLD),
        );
        let body = Rect::new(
            area.x,
            head_y + 1,
            area.width,
            area.bottom().saturating_sub(head_y + 1),
        );
        ctx.control(ENV, body, false);
        ctx.scrollable(ENV, body);
        let rows = n + 1;
        self.env_cursor = self.env_cursor.min(rows - 1);
        self.env_scroll.set_content(rows);
        self.env_scroll.set_viewport(body.height as usize);
        self.env_scroll.follow(self.env_cursor);
        let focused = ctx.interaction.focused(ENV);
        let problems = self.env_problems();
        let has_sb = self.env_scroll.overflows();
        let name_w = self
            .pending
            .env
            .iter()
            .map(|e| e.name.len())
            .max()
            .unwrap_or(8)
            .clamp(8, 28);
        for (k, i) in self.env_scroll.visible_range().enumerate() {
            let y = body.y + k as u16;
            let rid = ENV.child(i);
            let mut s = ctx.state(rid);
            s.focused = focused && i == self.env_cursor;
            s.selected = i == self.env_cursor;
            let st = t.row(s, bg);
            let rect = Rect::new(body.x, y, body.width.saturating_sub(u16::from(has_sb)), 1);
            fill(buf, rect, st);
            buf.set_string(
                rect.x,
                y,
                t.gutter_symbol(s),
                t.gutter(s, st.bg.unwrap_or(bg), false),
            );
            if i == n {
                buf.set_string(
                    rect.x + 4,
                    y,
                    "+ Add entry…",
                    st.fg(if s.focused {
                        t.text_primary
                    } else {
                        t.text_secondary
                    }),
                );
            } else {
                let e = &self.pending.env[i];
                let mark = match self.original.env.iter().find(|o| o.name == e.name) {
                    None => "+",
                    Some(o) if o != e => "•",
                    _ => " ",
                };
                buf.set_string(rect.x + 3, y, mark, st.fg(t.warning));
                buf.set_string(
                    rect.x + 5,
                    y,
                    fit(&truncate(&e.name, name_w), name_w),
                    st.fg(if s.selected { t.accent } else { t.text_primary }),
                );
                let dx = rect.x + 7 + name_w as u16;
                let rest = rect.right().saturating_sub(dx + 1) as usize;
                let dw = (rest / 3).max(8);
                let default = e
                    .default
                    .as_deref()
                    .map_or("no default".to_owned(), |d| format!("default {d}"));
                buf.set_string(
                    dx,
                    y,
                    fit(&truncate(&default, dw), dw),
                    st.fg(t.text_secondary),
                );
                let px = dx + dw as u16 + 2;
                let pw = rect.right().saturating_sub(px + 1) as usize;
                let tail = match &problems[i] {
                    Some(p) => (p.clone(), t.error),
                    None => (e.prompt.clone().unwrap_or_default(), t.text_muted),
                };
                buf.set_string(px, y, truncate(&tail.0, pw), st.fg(tail.1));
            }
            legible_on_fill(buf, rect, t, bg);
            ctx.clickable(rid, rect);
        }
        if n == 0 {
            let e = termrock::widgets::empty::EmptyState::new("No environment entries")
                .hint("a adds one");
            let r = Rect::new(body.x + 2, body.y + 2, body.width.saturating_sub(4), 4);
            if body.height > 6 {
                termrock::widgets::empty::render(r, buf, t, &e, bg);
            }
        }
        if has_sb {
            scrollbar::render_with_fade(body, buf, ctx, ENV, &self.env_scroll, focused);
        }
    }

    fn check_lines(&self) -> Vec<Notice> {
        if self.checking() {
            return vec![];
        }
        if self.fatal.is_some() {
            return vec![Notice::info("No check · the manifest does not parse")];
        }
        let findings = self.findings();
        if findings.is_empty() {
            return vec![if self.is_stale() {
                Notice::info(self.stale_text())
            } else {
                Notice::success("Check passed")
            }];
        }
        findings
            .iter()
            .map(|f| {
                let notice = Notice::error(f.message.clone());
                match &f.fix {
                    Some(fix) => notice.hint(fix.clone()),
                    None => notice,
                }
            })
            .collect()
    }

    fn render_check(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let bg = t.canvas;
        let running = self.checking();
        let notices = self.check_lines();
        if area.height < 4 {
            let findings = self.findings().len();
            let (text, tone) = if running {
                ("Checking…".to_owned(), Tone::Active)
            } else if findings == 0 && self.is_stale() {
                (self.stale_text().to_owned(), Tone::Secondary)
            } else if findings == 0 {
                ("Check passed".to_owned(), Tone::Success)
            } else {
                (
                    format!("{} · F5", plural(findings, "error", "errors")),
                    Tone::Error,
                )
            };
            buf.set_string(
                area.x + 2,
                area.y,
                termrock::ui::reveal::cut(
                    Rect::new(area.x + 2, area.y, area.width.saturating_sub(4), 1),
                    &text,
                ),
                ratatui::style::Style::new().fg(t.tone(tone)).bg(bg),
            );
            return;
        }
        let meta = if running {
            "checking…".to_owned()
        } else {
            String::new()
        };
        let inner = Panel::framed(Some("Check"))
            .meta(&meta)
            .render(area, buf, t);
        if running {
            buf.set_string(
                inner.x,
                inner.y,
                format!(
                    "{} Reading jackin.role.toml…",
                    spinner_frame(ctx.interaction.tick)
                ),
                t.secondary().bg(bg),
            );
            return;
        }
        let heights: Vec<u16> = notices.iter().map(|n| n.height(inner.width)).collect();
        let total: u16 = heights.iter().sum();
        self.check_scroll.set_content(total as usize);
        self.check_scroll.set_viewport(inner.height as usize);
        ctx.scrollable(CHECK, inner);
        let mut skip = self.check_scroll.offset as u16;
        let mut y = inner.y;
        for (n, h) in notices.iter().zip(heights) {
            if skip >= h {
                skip -= h;
                continue;
            }
            if y >= inner.bottom() {
                break;
            }
            let room = inner.bottom() - y;
            y += n.render(Rect::new(inner.x, y, inner.width, room), buf, ctx, bg);
        }
    }
}

impl Page<Jackin> for RoleEditorScreen {
    fn enter(&mut self, _w: &mut World, cx: &mut Cx) {
        self.apply_findings();
        cx.focus.focus(TABS);
    }

    fn primary_focus(&self) -> Option<WidgetId> {
        Some(TABS)
    }

    fn is_editing(&self) -> bool {
        self.editing_any()
    }

    fn animating(&self, _w: &World) -> bool {
        self.is_busy()
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
            Msg::RoleSaved { role, result } if self.saving && *role == self.role => {
                self.finish_save(result, w, cx);
                Outcome::Changed
            }
            Msg::RoleChecked {
                ticket,
                role,
                result,
            } if self.checking() && self.check_ticket == Some(*ticket) && *role == self.role => {
                self.check_ticket = None;
                let found = match result {
                    Ok(snapshot) => snapshot
                        .surfaces
                        .roles
                        .as_ref()
                        .and_then(|a| a.role(role))
                        .map(|r| r.findings().to_vec())
                        .ok_or_else(|| format!("The check of {role} came back empty")),
                    Err(reason) => Err(reason.clone()),
                };
                let findings = match found {
                    Ok(findings) => findings,
                    Err(reason) => {
                        self.check = RoleCheckState::Unchecked;
                        self.checked = None;
                        self.save_after_check = false;
                        cx.error(reason);
                        return Outcome::Changed;
                    }
                };
                self.sync();
                if self.is_stale() {
                    self.check = RoleCheckState::Unchecked;
                    self.save_after_check = false;
                    cx.status("The role changed during the check · press F5 to check again");
                    return Outcome::Changed;
                }
                self.check = RoleCheckState::Done(findings.clone());
                self.apply_findings();
                cx.status(if findings.is_empty() {
                    "Check passed".to_owned()
                } else {
                    plural(findings.len(), "error", "errors")
                });
                if std::mem::take(&mut self.save_after_check) {
                    if findings.is_empty() {
                        self.open_preview(cx);
                    } else {
                        self.open_check_failed(cx);
                    }
                }
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    fn on_key(&mut self, key: &Key, w: &mut World, cx: &mut Cx) -> Outcome {
        if self.saving {
            return Outcome::Consumed;
        }
        let focus = cx.focus.current();
        let editing = self.editing_any();
        if key.ctrl() && key.code == KeyCode::Char('s') {
            self.request_save(w, cx);
            return Outcome::Changed;
        }
        if key.is(KeyCode::F(5)) {
            self.run_check(w, cx);
            return Outcome::Changed;
        }
        if !editing {
            match key.code {
                KeyCode::Char('[') => {
                    let i = (self.tab.index() + 2) % 3;
                    self.set_tab(Tab::ALL[i], cx);
                    return Outcome::Changed;
                }
                KeyCode::Char(']') => {
                    let i = (self.tab.index() + 1) % 3;
                    self.set_tab(Tab::ALL[i], cx);
                    return Outcome::Changed;
                }
                KeyCode::Char('i') if key.plain() && w.surfaces.docker.is_some() => {
                    self.open_docker_risks(w, cx, false);
                    return Outcome::Changed;
                }
                _ => {}
            }
        }
        if focus == Some(TABS) {
            if let KeyCode::Char(c @ '1'..='3') = key.code
                && key.plain()
            {
                self.set_tab(Tab::ALL[(c as u8 - b'1') as usize], cx);
                return Outcome::Changed;
            }
            if matches!(
                key.code,
                KeyCode::Enter | KeyCode::Down | KeyCode::Char('j')
            ) {
                if self.fatal.is_none() {
                    cx.focus.focus(self.body_focus());
                }
                return Outcome::Changed;
            }
            let (o, ev) = self.tabs.on_key(key);
            if let Some(TabAction::Activated(i)) = ev {
                self.tab = Tab::ALL[i.min(2)];
                return Outcome::Changed;
            }
            if o.consumed() {
                return o;
            }
            if key.is(KeyCode::Esc) {
                self.leave(cx);
                return Outcome::Changed;
            }
            return Outcome::Ignored;
        }
        if focus == Some(CANCEL) {
            let (o, fired) = self.cancel.on_key(key);
            if fired {
                self.leave(cx);
                return Outcome::Changed;
            }
            if o.consumed() {
                return o;
            }
        }
        if focus == Some(SAVE) {
            let (o, fired) = self.save.on_key(key);
            if fired {
                self.request_save(w, cx);
                return Outcome::Changed;
            }
            if o.consumed() {
                return o;
            }
        }
        if matches!(focus, Some(CANCEL) | Some(SAVE)) {
            match key.code {
                KeyCode::Left | KeyCode::Char('h') => cx.focus.focus(CANCEL),
                KeyCode::Right | KeyCode::Char('l') => cx.focus.focus(SAVE),
                KeyCode::Esc | KeyCode::Up | KeyCode::Char('k') => {
                    cx.focus.focus(self.body_focus());
                }
                _ => return Outcome::Ignored,
            }
            return Outcome::Changed;
        }
        if let Some(id) = focus
            && let Some(o) = self.field_key(id, key, cx)
        {
            return o;
        }
        if focus == Some(DOCKER) && (key.is_char(' ') || key.is(KeyCode::Enter)) {
            return self.toggle_docker(w, cx);
        }
        if focus == Some(ENV)
            && self.tab == Tab::Environment
            && let Some(o) = self.env_key(key, cx)
        {
            return o;
        }
        if !editing {
            match key.code {
                KeyCode::Esc => {
                    cx.focus.focus(TABS);
                    return Outcome::Changed;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    cx.focus.next(cx.ring);
                    return Outcome::Changed;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    cx.focus.prev(cx.ring);
                    return Outcome::Changed;
                }
                _ => {}
            }
        }
        Outcome::Ignored
    }

    fn pointer(&mut self, ev: &PointerEvent, w: &mut World, cx: &mut Cx) -> Outcome {
        let click = ev.is(Phase::Click);
        if self.saving && click {
            return Outcome::Consumed;
        }
        if let Some(o) = self.manifest_stack.pointer(MANIFEST_BODY, ev) {
            return o;
        }
        if let Some(o) = self.hooks_stack.pointer(HOOKS_BODY, ev) {
            return o;
        }
        if let Some(r) = self.tabs.route(ev) {
            let (o, actions) = r.into_parts();
            for action in actions {
                if let TabAction::Activated(i) = action {
                    self.tab = Tab::ALL[i.min(2)];
                }
            }
            if !click {
                return o;
            }
            cx.focus.focus(TABS);
            return o.or(Outcome::Changed);
        }
        if let Some((_, delta)) = ev.wheel() {
            if ev.id == ENV || ev.id == scrollbar::id_for(ENV) {
                self.env_scroll.scroll_by(delta as isize);
                return Outcome::Changed;
            }
            if ev.id == CHECK {
                self.check_scroll.scroll_by(delta as isize);
                return Outcome::Changed;
            }
        }
        let id = ev.id;
        for (i, which) in [Pick::Dockerfile, Pick::Policy, Pick::Setup, Pick::Preflight]
            .into_iter()
            .enumerate()
        {
            if id == self.choose[i].id && click {
                cx.focus.focus(id);
                self.open_choose(which, cx);
                return Outcome::Changed;
            }
        }
        if let Some(input) = self.input_mut(id) {
            let r = input.route(ev);
            if let Some(r) = r {
                if click {
                    cx.focus.focus(id);
                }
                return r.outcome();
            }
        }
        if let Some(o) = scrollbar::pointer(ev, ENV, &mut self.env_scroll, TrackClick::Jump) {
            return o;
        }
        if !click {
            return Outcome::Ignored;
        }
        if id == CANCEL {
            cx.focus.focus(CANCEL);
            if self.cancel.on_click() {
                self.leave(cx);
            }
            return Outcome::Changed;
        }
        if id == SAVE {
            cx.focus.focus(SAVE);
            if self.save.on_click() {
                self.request_save(w, cx);
            }
            return Outcome::Changed;
        }
        if id == DOCKER {
            cx.focus.focus(DOCKER);
            return self.toggle_docker(w, cx);
        }
        for i in 0..=self.pending.env.len() {
            if ENV.child(i) == id {
                let same = self.env_cursor == i;
                self.env_cursor = i;
                cx.focus.focus(ENV);
                if same {
                    if i == self.pending.env.len() {
                        self.open_entry_form(None, cx);
                    } else {
                        self.open_entry_form(Some(self.pending.env[i].name.clone()), cx);
                    }
                }
                return Outcome::Changed;
            }
        }
        Outcome::Ignored
    }

    fn on_blur(
        &mut self,
        ch: termrock::core::focus::FocusChange,
        _w: &mut World,
        _cx: &mut Cx,
    ) -> Outcome {
        let Some(input) = self.input_mut(ch.from) else {
            return Outcome::Ignored;
        };
        match input.on_blur() {
            Some(InputAction::Committed) => {
                input.field.error = None;
                input.validate();
                self.sync();
                Outcome::Changed
            }
            Some(InputAction::Cancelled) => Outcome::Changed,
            _ => Outcome::Ignored,
        }
    }

    fn on_paste(&mut self, text: &str, _w: &mut World, cx: &mut Cx) -> Outcome {
        let Some(id) = cx.focus.current() else {
            return Outcome::Ignored;
        };
        match self.input_mut(id) {
            Some(input) if input.editing => {
                let o = input.on_paste(text);
                self.sync();
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
            (
                "docker.on",
                ModalResult::Dialog {
                    action: Some(1), ..
                },
            ) => {
                self.docker.checked = true;
                self.sync();
                cx.focus.focus(DOCKER);
            }
            ("docker.on" | "docker.info", _) => cx.focus.focus(DOCKER),
            (
                "preview",
                ModalResult::Dialog {
                    action: Some(1), ..
                },
            ) => self.start_save(w, cx),
            ("preview", _) => {
                cx.status("Not saved · keep editing");
                cx.focus.focus(SAVE);
            }
            (
                "exit",
                ModalResult::Dialog {
                    action: Some(2), ..
                },
            ) => self.request_save(w, cx),
            (
                "exit",
                ModalResult::Dialog {
                    action: Some(1), ..
                },
            ) => {
                cx.status(format!(
                    "Discarded {}",
                    plural(self.change_count().max(1), "change", "changes")
                ));
                cx.go(Go::Roles {
                    select: Some(self.role.clone()),
                });
            }
            ("checkfail" | "savefail", _) => {
                self.reveal_error(cx);
            }
            ("choose", ModalResult::Picked(i)) => {
                if let (Some(which), Some(file)) = (self.pick.take(), self.pick_targets.get(i)) {
                    let id = match which {
                        Pick::Dockerfile => DOCKERFILE,
                        Pick::Policy => POLICY,
                        Pick::Setup => SETUP,
                        Pick::Preflight => PREFLIGHT,
                    };
                    let file = file.clone();
                    if let Some(input) = self.input_mut(id) {
                        input.set_value(&file);
                    }
                    self.sync();
                    cx.focus.focus(id);
                }
            }
            ("choose", _) => self.pick = None,
            ("entry", ModalResult::Form(Some(values))) => {
                return self.save_entry(values, cx);
            }
            ("entry", _) => self.editing_entry = None,
            _ => {}
        }
        Outcome::Changed
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        let t = ctx.theme;
        let bg = t.canvas;
        self.sync();
        let locked = self.locked();
        let items: Vec<TabItem> = Tab::ALL
            .iter()
            .zip(TAB_NAMES)
            .map(|(tab, name)| {
                let mut it = TabItem::new(name);
                it.dirty = match tab {
                    Tab::Manifest => self.manifest_dirty(),
                    Tab::Hooks => self.hooks_dirty(),
                    Tab::Environment => self.env_dirty(),
                };
                it.error = self.tab_error(*tab);
                if *tab == Tab::Environment && self.fatal.is_none() {
                    it.suffix = Some(self.pending.env.len().to_string());
                }
                it
            })
            .collect();
        let active = self.tabs.active;
        self.tabs.set_items(items);
        self.tabs.set_active(active);
        self.tabs.render(
            Rect::new(area.x + 1, area.y + 1, area.width.saturating_sub(2), 2),
            buf,
            ctx,
            bg,
        );
        let check_h = if area.height < 28 { 1 } else { 5 };
        let bottom = 2 + check_h;
        let mut body = Rect::new(
            area.x + 1,
            area.y + 4,
            area.width.saturating_sub(2),
            area.height.saturating_sub(4 + bottom),
        );
        if self.readonly || self.fatal.is_some() {
            let notice = match &self.fatal {
                Some(message) => Notice::error(message.clone())
                    .hint("Fix the file in your editor, then press F5."),
                None => Notice::info("This role comes from git. Edit its repository and pull."),
            };
            let used = notice.render(
                Rect::new(body.x + 2, body.y, body.width.saturating_sub(4), 3),
                buf,
                ctx,
                bg,
            );
            body = Rect::new(
                body.x,
                body.y + used + 1,
                body.width,
                body.height.saturating_sub(used + 1),
            );
        }
        if self.fatal.is_none() {
            match self.tab {
                Tab::Manifest => self.render_manifest(body, buf, ctx, w),
                Tab::Hooks => self.render_hooks(body, buf, ctx),
                Tab::Environment => self.render_env(body, buf, ctx),
            }
        }
        let check_area = Rect::new(
            area.x + 1,
            area.bottom().saturating_sub(2 + check_h),
            area.width.saturating_sub(2),
            check_h,
        );
        self.render_check(check_area, buf, ctx);

        self.save.disabled = locked || self.change_count() == 0 || self.is_busy();
        self.save.label = if self.saving {
            format!("{} Saving…", spinner_frame(w.now_ms() as u64 / 80))
        } else {
            "Save…".into()
        };
        let widths = [self.cancel.width(), self.save.width()];
        let rects = row_layout_right(
            Rect::new(
                area.x,
                area.bottom().saturating_sub(1),
                area.width.saturating_sub(4),
                1,
            ),
            &widths,
            3,
        );
        self.cancel.render(rects[0], buf, ctx, bg);
        self.save.render(rects[1], buf, ctx, bg);
    }

    fn hints(&self, focus: Option<WidgetId>, w: &World) -> Vec<Hint> {
        if self.saving {
            return vec![hint("", "Saving…")];
        }
        let mut v = vec![];
        if focus == Some(TABS) {
            v.push(hint("←→", "Tab"));
            v.push(hint("1–3", "Jump"));
            v.push(hint("Enter", "Edit fields"));
        } else if focus == Some(ENV) {
            v.push(hint("a", "Add"));
            v.push(hint("e", "Edit"));
            v.push(hint("d", "Remove"));
            v.push(hint("u", "Undo"));
        } else if focus == Some(DOCKER) && w.surfaces.docker.is_some() {
            v.extend(super::modals::docker_row_hints());
        } else if focus == Some(DOCKER) {
            v.push(hint("Space", "Toggle"));
        } else {
            v.push(hint("Tab", "Next"));
        }
        v.push(hint("F5", "Check"));
        v.push(hint("[ ]", "Switch tab"));
        v.push(hint("Ctrl+S", "Save"));
        v.push(hint("Esc", "Back"));
        v
    }

    fn crumb(&self, _w: &World) -> String {
        format!("Roles › {} › edit", self.role)
    }

    fn stakes(&mut self, _w: &World) -> termrock::ui::leave::Stakes {
        self.sync();
        let n = self.change_count();
        if n > 0 {
            termrock::ui::leave::Stakes::Unsaved { changes: n }
        } else {
            termrock::ui::leave::Stakes::Clean
        }
    }

    fn on_esc_top(&mut self, _w: &mut World, cx: &mut Cx) -> Outcome {
        if cx.focus.is(TABS) {
            self.leave(cx);
        } else {
            cx.focus.focus(TABS);
        }
        Outcome::Changed
    }
}

impl RoleEditorScreen {
    fn field_key(&mut self, id: WidgetId, key: &Key, cx: &mut Cx) -> Option<Outcome> {
        let input = self.input_mut(id)?;
        let (o, ev) = input.on_key(key);
        match ev {
            Some(InputAction::Committed) => {
                input.field.error = None;
                input.validate();
                self.sync();
                return Some(Outcome::Changed);
            }
            Some(InputAction::Cancelled) => return Some(Outcome::Changed),
            _ => {}
        }
        if o.consumed() {
            self.sync();
            return Some(o);
        }
        let _ = cx;
        None
    }

    fn env_key(&mut self, key: &Key, cx: &mut Cx) -> Option<Outcome> {
        let n = self.pending.env.len();
        let on_add = self.env_cursor >= n;
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.env_cursor = self.env_cursor.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.env_cursor = (self.env_cursor + 1).min(n);
            }
            KeyCode::Home | KeyCode::Char('g') => self.env_cursor = 0,
            KeyCode::End | KeyCode::Char('G') => self.env_cursor = n,
            KeyCode::Char('a') => self.open_entry_form(None, cx),
            KeyCode::Char('e') | KeyCode::Enter => {
                if on_add {
                    self.open_entry_form(None, cx);
                } else {
                    self.open_entry_form(Some(self.pending.env[self.env_cursor].name.clone()), cx);
                }
            }
            KeyCode::Char('d') | KeyCode::Delete => self.remove_entry(cx),
            KeyCode::Char('u') => self.undo_entry(cx),
            _ => return None,
        }
        Some(Outcome::Changed)
    }
}

impl Screen for RoleEditorScreen {
    fn picker_items(&mut self, tag: &LayerTag, query: &str, _w: &World) -> Option<Vec<Item>> {
        if tag.kind != "choose" {
            return None;
        }
        let q = query.to_lowercase();
        self.pick_targets = self
            .files
            .iter()
            .filter(|f| q.is_empty() || f.to_lowercase().contains(&q))
            .cloned()
            .collect();
        Some(self.pick_targets.iter().map(Item::new).collect())
    }

    fn strip_right(&self, w: &World) -> Vec<Segment> {
        let mut v = vec![];
        if self.saving {
            v.push(
                Segment::new(
                    format!("{} saving…", spinner_frame(w.now_ms() as u64 / 80)),
                    Tone::Secondary,
                )
                .priority(9),
            );
        }
        let n = self.change_count();
        if n > 0 {
            v.push(
                Segment::new(
                    format!("• {}", plural(n, "change", "changes")),
                    Tone::Warning,
                )
                .priority(8),
            );
        }
        if self.checking() {
            v.push(
                Segment::new(
                    format!("{} checking", spinner_frame(w.now_ms() as u64 / 80)),
                    Tone::Secondary,
                )
                .priority(7),
            );
        }
        let _ = StatusKind::Ok;
        v
    }

    fn request_leave(&mut self, _w: &mut World, cx: &mut Cx) -> Outcome {
        self.leave(cx);
        Outcome::Changed
    }
}
