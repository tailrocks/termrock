use crate::screens::Jackin;
use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::hit::Axis;
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::{MouseTarget, PointerEvent};
use junie_tui::core::refusal::{Control, Reason, raise};
use junie_tui::core::scroll::{ScrollState, TrackClick};
use junie_tui::shell::Page;
use junie_tui::theme::Tone;
use junie_tui::ui::ctx::{RenderCtx, fill, legible_on_fill};
use junie_tui::ui::layout::{Stack, StackChild, StackScroll};
use junie_tui::ui::text::{fit, truncate, width};
use junie_tui::widgets::button::{Button, row_layout_right};
use junie_tui::widgets::choice::Checkbox;
use junie_tui::widgets::dialog::Dialog;
use junie_tui::widgets::input::{InputAction, TextInput};
use junie_tui::widgets::keyhint::{Hint, hint};
use junie_tui::widgets::notice::Notice;
use junie_tui::widgets::option_list::Item;
use junie_tui::widgets::picker::Picker;
use junie_tui::widgets::progress::spinner_frame;
use junie_tui::widgets::props::Prop;
use junie_tui::widgets::scrollbar;
use junie_tui::widgets::segments::Segment;
use junie_tui::widgets::select::Select;
use junie_tui::widgets::tabs::{TabAction, TabItem, Tabs};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::Modifier;

use super::config::{ConfigTabs, Doc, Scope, Tab as CfgTab};
use super::modals::{self, InfoDialog};
use super::{Cx, Go, LayerTag, ModalResult, Screen};
use crate::backend::Effect;
use crate::domain::account::AccountId;
use crate::domain::agent::Provider;
use crate::domain::docker::{DockerSetting, EngineState};
use crate::domain::fixed::Fixed;
use crate::domain::vocabulary::fill_in;
use crate::domain::workspace::{
    AllowedRoles, DirtyExitPolicy, Effective, RoleEntry, RoleName, RoleSource, Workspace,
    WorkspaceId,
};
use crate::world::{Msg, RoleLoad, SaveFailure, SaveSuccess, Snapshot, World, job};
use junie_tui::ui::text::plural;

fn on_off(v: bool) -> &'static str {
    if v { "on" } else { "off" }
}

pub const TABS: WidgetId = WidgetId::of("editor.tabs");
pub const NAME: WidgetId = WidgetId::of("editor.name");
pub const WORKDIR: WidgetId = WidgetId::of("editor.workdir");

const GENERAL: WidgetId = WidgetId::of("editor.general");
pub const KEEP_AWAKE: WidgetId = WidgetId::of("editor.keep_awake");
/// Columns of the workspace name in the save hint, so the status line fits.
const SAVED_NAME_COLUMNS: usize = 20;

pub const GIT_PULL: WidgetId = WidgetId::of("editor.git_pull");
pub const DOCKER: WidgetId = WidgetId::of("editor.docker");
pub const DIRTY_POLICY: WidgetId = WidgetId::of("editor.dirty_policy");
pub const ROLES: WidgetId = WidgetId::of("editor.roles");
pub const ACCOUNTS: WidgetId = WidgetId::of("editor.accounts");
pub const CANCEL: WidgetId = WidgetId::of("editor.cancel");
pub const SAVE: WidgetId = WidgetId::of("editor.save");
const BASE: WidgetId = WidgetId::of("editor.cfg");

fn tab_names(w: &World) -> [&str; 5] {
    [
        "General",
        w.surfaces.vocabulary.project_tab.as_str(),
        "Roles",
        "Environments",
        "Accounts",
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EdTab {
    General,
    Mounts,
    Roles,
    Environments,
    Accounts,
}

impl EdTab {
    const ALL: [EdTab; 5] = [
        EdTab::General,
        EdTab::Mounts,
        EdTab::Roles,
        EdTab::Environments,
        EdTab::Accounts,
    ];
    fn cfg(self) -> Option<CfgTab> {
        match self {
            EdTab::Mounts => Some(CfgTab::Mounts),
            EdTab::Environments => Some(CfgTab::Environments),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AcctRow {
    Provider(Provider),
    Account(AccountId),
}

#[derive(Default)]
struct AcctState {
    rows: Vec<AcctRow>,
    cursor: usize,
    scroll: ScrollState,
    filter: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RoleRow {
    Role(String),
    Load,
}

pub struct EditorScreen {
    pub workspace: Option<WorkspaceId>,
    pub original: Workspace,
    pub pending: Workspace,
    pub cfg: ConfigTabs,
    tabs: Tabs,
    tab: EdTab,
    pub(crate) name: TextInput,
    workdir_button: Button,
    keep_awake: Checkbox,
    git_pull: Checkbox,
    docker: Checkbox,
    dirty_policy: Select,
    role_rows: Vec<RoleRow>,
    role_cursor: usize,
    role_scroll: ScrollState,
    general_stack: StackScroll,
    role_filter: Option<String>,
    acct: AcctState,
    cancel: Button,
    save: Button,
    saving: bool,
    pending_load: Option<String>,
    picker_targets: Vec<String>,
    row_status: Option<String>,

    taken: Vec<(String, WorkspaceId)>,
}

impl EditorScreen {
    pub fn new(w: &World, workspace: Option<WorkspaceId>, pending: Option<Workspace>) -> Self {
        let create = pending.is_some() && workspace.is_none();
        let original = workspace
            .and_then(|id| w.workspace(id))
            .cloned()
            .unwrap_or_else(|| {
                let mut o = pending
                    .clone()
                    .unwrap_or_else(|| Workspace::new(0, "new", "/workspace"));

                o.name = String::new();
                o
            });
        let pending = pending
            .or_else(|| workspace.and_then(|id| w.workspace(id)).cloned())
            .unwrap_or_else(|| Workspace::new(0, "new", "/workspace"));
        let registry: Vec<RoleName> = w.roles.iter().map(|r| r.name.clone()).collect();
        let cfg = ConfigTabs::new(
            Scope::Workspace,
            Doc::from_workspace(&original),
            Doc::from_workspace(&pending),
            BASE,
            registry,
        );
        let policy_idx = match pending.dirty_policy {
            DirtyExitPolicy::Ask => 0,
            DirtyExitPolicy::Keep => 1,
            DirtyExitPolicy::Discard => 2,
        };
        let control = |pick: fn(&Fixed) -> &Reason| {
            w.surfaces
                .fixed
                .as_ref()
                .map_or(Control::Live, |f| Control::fixed(pick(f).clone()))
        };
        let mut dirty_policy = Select::new(
            DIRTY_POLICY,
            "On dirty exit",
            &[
                "ask · show the exit dialog",
                "keep · preserve changes silently",
                "discard · drop changes silently",
            ],
            policy_idx,
        )
        .control(control(|f| &f.dirty_exit));
        if let Some(f) = &w.surfaces.fixed {
            dirty_policy = dirty_policy.help(&f.dirty_exit.text);
        }
        let mut s = Self {
            workspace,
            name: TextInput::new(NAME, "Name")
                .required(true)
                .required_message("Name is required")
                .help("Directory basename by default")
                .value(&pending.name)
                .validator(|v| {
                    let v = v.trim();
                    if v.is_empty() {
                        Some("Name is required".into())
                    } else if v.contains('/') {
                        Some("Name cannot contain /".into())
                    } else {
                        None
                    }
                }),
            workdir_button: Button::secondary(WORKDIR, "Choose…").control(control(|f| &f.workdir)),
            keep_awake: Checkbox::new(KEEP_AWAKE, "Keep awake", pending.keep_awake)
                .control(control(|f| &f.keep_awake)),
            git_pull: Checkbox::new(GIT_PULL, "Git pull before launch", pending.git_pull)
                .control(control(|f| &f.git_pull)),
            docker: Checkbox::new(DOCKER, "Docker for the agent", pending.docker),
            dirty_policy,
            original,
            pending,
            cfg,
            tabs: Tabs::new(TABS, &tab_names(w)),
            tab: EdTab::General,
            role_rows: vec![],
            role_cursor: 0,
            role_scroll: ScrollState::default(),
            general_stack: StackScroll::default(),
            role_filter: None,
            acct: AcctState::default(),
            cancel: Button::subtle(CANCEL, "Cancel"),
            save: Button::primary(SAVE, "Save…"),
            saving: false,
            pending_load: None,
            picker_targets: vec![],
            row_status: None,
            taken: vec![],
        };
        let _ = create;
        s.build_roles(w);
        s
    }

    pub fn is_create(&self) -> bool {
        self.workspace.is_none()
    }

    fn sync_pending(&mut self) {
        self.pending.name = self.name.text().trim().to_owned();
        self.pending.keep_awake = self.keep_awake.checked;
        self.pending.git_pull = self.git_pull.checked;
        self.pending.docker = self.docker.checked;
        self.pending.dirty_policy = match self.dirty_policy.selected {
            1 => DirtyExitPolicy::Keep,
            2 => DirtyExitPolicy::Discard,
            _ => DirtyExitPolicy::Ask,
        };
        self.cfg.pending.apply_to_workspace(&mut self.pending);
    }

    pub fn change_count(&self) -> usize {
        self.pending.change_count(&self.original)
    }

    fn general_dirty(&self) -> bool {
        self.pending.name != self.original.name
            || self.pending.workdir != self.original.workdir
            || self.pending.keep_awake != self.original.keep_awake
            || self.pending.git_pull != self.original.git_pull
            || self.pending.docker != self.original.docker
            || self.pending.dirty_policy != self.original.dirty_policy
    }

    fn roles_dirty(&self) -> bool {
        self.pending.roles != self.original.roles
    }

    fn accounts_dirty(&self) -> bool {
        self.pending.accounts != self.original.accounts
    }

    fn build_roles(&mut self, w: &World) {
        let q = self.role_filter.as_ref().map(|f| f.to_lowercase());
        let mut rows: Vec<RoleRow> = w
            .roles
            .iter()
            .filter(|r| {
                q.as_ref().is_none_or(|q| {
                    r.name.to_lowercase().contains(q) || r.description.to_lowercase().contains(q)
                })
            })
            .map(|r| RoleRow::Role(r.name.clone()))
            .collect();
        if let AllowedRoles::Custom(list) = &self.pending.roles.allowed {
            for r in list {
                if !rows.iter().any(|x| matches!(x, RoleRow::Role(n) if n == r)) {
                    rows.push(RoleRow::Role(r.clone()));
                }
            }
        }
        if let Some(d) = &self.pending.roles.default
            && !rows.iter().any(|x| matches!(x, RoleRow::Role(n) if n == d))
        {
            rows.push(RoleRow::Role(d.clone()));
        }
        rows.push(RoleRow::Load);
        self.role_rows = rows;
        if self.role_cursor >= self.role_rows.len() {
            self.role_cursor = self.role_rows.len() - 1;
        }
    }

    fn set_tab(&mut self, i: usize, cx: &mut Cx) {
        self.tab = EdTab::ALL[i.min(4)];
        self.tabs.set_active(i);
        cx.focus.focus(TABS);
    }

    fn body_focus(&self) -> WidgetId {
        match self.tab {
            EdTab::General => NAME,
            EdTab::Roles => ROLES,
            EdTab::Accounts => ACCOUNTS,
            t => self.cfg.list_id(t.cfg().unwrap()),
        }
    }

    fn validate(&mut self, cx: &mut Cx) -> bool {
        if !self.name.validate() {
            self.set_tab(0, cx);
            cx.focus.focus(NAME);
            cx.error("Name is required and cannot contain /");
            return false;
        }
        let name = self.name.text().trim().to_owned();
        if let Some(other) = self.taken_by(&name) {
            self.name.field.error = Some(format!("A workspace named {name} already exists").into());
            self.set_tab(0, cx);
            cx.focus.focus(NAME);
            cx.error(format!(
                "A workspace named {name} already exists (#{other})"
            ));
            return false;
        }
        if self.cfg.tab_error(CfgTab::Environments) {
            self.set_tab(3, cx);
            cx.error("An environment key is invalid");
            return false;
        }
        true
    }

    fn taken_by(&self, name: &str) -> Option<WorkspaceId> {
        self.taken
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, id)| *id)
    }

    fn open_preview(&mut self, w: &World, cx: &mut Cx) {
        self.sync_pending();
        if !self.validate(cx) {
            return;
        }
        let n = self.change_count();
        if n == 0 && !self.is_create() {
            cx.status("Nothing to save");
            return;
        }
        let blockers = self.cfg.blockers();
        let mut facts = vec![
            Prop::new(
                "Workspace",
                if self.is_create() {
                    format!("{} · new", self.pending.name)
                } else {
                    self.pending.name.clone()
                },
            ),
            Prop::new(
                "Scope",
                match &w.surfaces.vocabulary.workspace_config {
                    Some(path) => format!(
                        "workspace config · {}",
                        w.tilde(&fill_in(
                            path,
                            &[
                                ("home", &w.home),
                                ("name", &self.pending.name),
                                ("config", &w.config_file()),
                            ]
                        ))
                    ),
                    None => "workspace config".to_owned(),
                },
            ),
            Prop::new("Changes", plural(n.max(1), "change", "changes")),
        ];
        if self.pending.name != self.original.name && !self.is_create() {
            facts.push(Prop::new(
                "Name",
                format!("{} → {}", self.original.name, self.pending.name),
            ));
        }
        if self.pending.workdir != self.original.workdir {
            facts.push(Prop::new(
                "Working dir",
                format!("{} → {}", self.original.workdir, self.pending.workdir),
            ));
        }
        if self.pending.docker != self.original.docker {
            facts.push(
                Prop::new(
                    "Docker",
                    format!(
                        "{} → {}",
                        on_off(self.original.docker),
                        on_off(self.pending.docker)
                    ),
                )
                .tone(Tone::Warning),
            );
        }
        facts.extend(self.cfg.summary_facts());
        if self.pending.roles != self.original.roles {
            facts.push(Prop::new(
                "Roles",
                format!(
                    "{} · default {}",
                    self.pending.roles.summary().to_lowercase(),
                    self.pending.roles.default.clone().unwrap_or("none".into())
                ),
            ));
        }
        for b in &blockers {
            facts.push(Prop::new("Blocker", b.clone()).tone(Tone::Error));
        }
        let mut code = self.cfg.diff_lines();
        if self.is_create() {
            code.insert(
                0,
                format!(
                    "+ workspace {}  {}",
                    self.pending.name, self.pending.workdir
                ),
            );
        }
        if self.pending.keep_awake != self.original.keep_awake {
            code.push(format!(
                "~ keep_awake {} → {}",
                self.original.keep_awake, self.pending.keep_awake
            ));
        }
        if self.pending.git_pull != self.original.git_pull {
            code.push(format!(
                "~ git_pull {} → {}",
                self.original.git_pull, self.pending.git_pull
            ));
        }
        if self.pending.docker != self.original.docker {
            code.push(format!(
                "~ docker {} → {}",
                self.original.docker, self.pending.docker
            ));
        }
        if self.pending.dirty_policy != self.original.dirty_policy {
            code.push(format!(
                "~ dirty_exit {} → {}",
                self.original.dirty_policy.label(),
                self.pending.dirty_policy.label()
            ));
        }
        let confirm = if blockers.is_empty() {
            Button::primary(WidgetId::of("editor.preview.save"), "Save")
        } else {
            Button::primary(WidgetId::of("editor.preview.cleanup"), "Clean up & save…")
        };
        let title = if self.is_create() {
            "Create workspace"
        } else {
            "Save workspace"
        };
        let d = Dialog::facts(
            WidgetId::of("editor.preview"),
            title,
            facts,
            code,
            None,
            confirm,
        );
        cx.open(
            d,
            LayerTag::new(if blockers.is_empty() {
                "preview"
            } else {
                "preview.cleanup"
            }),
        );
    }

    fn start_save(&mut self, w: &mut World, cx: &mut Cx) {
        self.sync_pending();
        self.saving = true;
        self.save.busy = true;
        w.perform(Effect::SaveWorkspace {
            id: self.workspace,
            base: (!self.is_create()).then(|| Box::new(self.original.clone())),
            workspace: Box::new(self.pending.clone()),
        });
        cx.progress(
            job::WORKSPACE_SAVE,
            format!("Saving {}…", self.pending.name),
        );
        cx.focus.focus(SAVE);
    }

    fn finish_save(
        &mut self,
        id: Option<WorkspaceId>,
        result: &Result<SaveSuccess, SaveFailure>,
        w: &mut World,
        cx: &mut Cx,
    ) {
        self.saving = false;
        self.save.busy = false;
        let saved = match result {
            Ok(saved) => saved,
            Err(error) => {
                let title = format!("Save failed · {}", self.pending.name);
                let mut props = vec![
                    Prop::new("Error", error.error.as_str()),
                    Prop::new("State", "your edits are intact · nothing was written"),
                ];
                if let Some(next) = &error.next {
                    props.push(Prop::new("Next", next.as_str()));
                }
                let d = InfoDialog::new(WidgetId::of("editor.savefail"), &title, props)
                    .error()
                    .width(66);
                cx.open(d, LayerTag::new("savefail"));
                return;
            }
        };
        let name = self.pending.name.clone();
        w.apply((*saved.snapshot).clone());
        self.workspace = id.or(self.workspace);
        self.original = self.pending.clone();
        let status = self.saved_status(&name, w);
        cx.status(match saved.warning {
            Some(warning) => format!("{} · {status}", warning.status()),
            None => status,
        });
        cx.go(Go::Manager);
    }

    /// The status after a save. A world that fixes the editor (`Fixed`) says
    /// what to set when the saved workspace has no default role and there are
    /// roles to choose from.
    fn saved_status(&self, name: &str, w: &World) -> String {
        match &w.surfaces.fixed {
            Some(fixed) if self.pending.roles.default.is_none() && w.roles.len() > 1 => fill_in(
                &fixed.save_hint,
                &[("name", &truncate(name, SAVED_NAME_COLUMNS))],
            ),
            _ => format!("Workspace {name} saved"),
        }
    }

    fn leave(&mut self, cx: &mut Cx) {
        self.sync_pending();
        if self.change_count() > 0 || self.is_create() {
            let d = super::modals::unsaved_dialog(WidgetId::of("editor.exit"), self.change_count());
            cx.open(d, LayerTag::new("exit"));
        } else {
            cx.go(Go::Manager);
        }
    }

    fn render_general(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        let t = ctx.theme;
        let bg = t.canvas;
        let fw = area.width.saturating_sub(4).min(72);
        let inline_reason = |r: Rect, reason: &Reason, buf: &mut Buffer| {
            let x = r.x + 30;
            buf.set_string(
                x,
                r.y,
                junie_tui::ui::reveal::cut(
                    Rect::new(x, r.y, area.right().saturating_sub(x), 1),
                    &reason.text,
                ),
                t.faint().bg(bg),
            );
        };

        let docker = w.surfaces.docker.as_ref();
        let docker_notices = self.docker_notices(w, fw.saturating_sub(4));
        let docker_h = 1 + docker_notices
            .iter()
            .map(|n| n.height(fw.saturating_sub(4)))
            .sum::<u16>();
        let mut kids = vec![
            StackChild::fixed(3),
            StackChild::fixed(1),
            StackChild::fixed(3),
            StackChild::fixed(1),
            StackChild::fixed(1),
            StackChild::fixed(1),
        ];
        let mut ids: Vec<Vec<WidgetId>> = vec![
            vec![NAME],
            vec![],
            vec![WORKDIR, self.workdir_button.id],
            vec![],
            vec![self.keep_awake.id],
            vec![self.git_pull.id],
        ];
        let docker_slot = docker.map(|_| {
            kids.push(StackChild::fixed(docker_h));
            ids.push(vec![DOCKER]);
            kids.len() - 1
        });
        kids.push(StackChild::fixed(1));
        ids.push(vec![]);
        let dirty_slot = kids.len();
        kids.push(StackChild::fixed(3));
        ids.push(vec![self.dirty_policy.id]);
        let focus = ctx.interaction.focus;
        let at = ids
            .iter()
            .position(|ids| focus.is_some_and(|f| ids.contains(&f)));
        let l = Stack::default().layout(area, &kids, &mut self.general_stack, at);
        Stack::chrome(&l, area, buf, ctx, GENERAL, &self.general_stack);
        let col = |r: Rect| Rect::new(r.x + 2, r.y, fw.min(r.width.saturating_sub(2)), r.height);
        match l.slot(0) {
            Some(r) => self.name.render(col(r), buf, ctx, bg),
            None => ctx.focus_stop(NAME),
        }
        match l.slot(2) {
            Some(r) => {
                let r = col(r);
                let (x, y, fw) = (r.x, r.y, r.width);
                buf.set_string(
                    x,
                    y,
                    "Working directory *",
                    t.secondary().bg(bg).add_modifier(Modifier::BOLD),
                );
                let wd_focused = ctx.interaction.focused(WORKDIR);
                buf.set_string(
                    x,
                    y + 1,
                    fit(
                        &truncate(&self.pending.workdir, fw.saturating_sub(14) as usize),
                        fw.saturating_sub(14) as usize,
                    ),
                    if wd_focused {
                        t.primary().bg(bg)
                    } else {
                        t.secondary().bg(bg)
                    },
                );
                self.workdir_button.render(
                    Rect::new(x + fw.saturating_sub(11), y + 1, 11, 1),
                    buf,
                    ctx,
                    bg,
                );
                let hint = if let Some(f) = &w.surfaces.fixed {
                    f.workdir.text.clone()
                } else if self.pending.workdir != self.original.workdir {
                    format!(
                        "Inside {} · was {}",
                        w.surfaces.vocabulary.construct, self.original.workdir
                    )
                } else {
                    format!("Inside {}", w.surfaces.vocabulary.construct)
                };
                buf.set_string(
                    x,
                    y + 2,
                    junie_tui::ui::reveal::cut(Rect::new(x, y + 2, fw, 1), &hint),
                    t.faint().bg(bg),
                );
            }
            None => ctx.focus_stop(self.workdir_button.id),
        }
        match l.slot(4) {
            Some(r) => {
                let r = col(r);
                self.keep_awake
                    .render(Rect::new(r.x, r.y, 30, 1), buf, ctx, bg);
                match self.keep_awake.control.reason() {
                    Some(reason) => inline_reason(r, reason, buf),
                    None => buf.set_string(r.x + 30, r.y, "macOS only", t.faint().bg(bg)),
                }
            }
            None => ctx.focus_stop(self.keep_awake.id),
        }
        match l.slot(5) {
            Some(r) => {
                let r = col(r);
                self.git_pull
                    .render(Rect::new(r.x, r.y, 30, 1), buf, ctx, bg);
                if let Some(reason) = self.git_pull.control.reason() {
                    inline_reason(r, reason, buf);
                }
            }
            None => ctx.focus_stop(self.git_pull.id),
        }
        if let (Some(i), Some(d)) = (docker_slot, docker) {
            match l.slot(i) {
                Some(r) => {
                    let r = col(r);
                    self.docker.render(Rect::new(r.x, r.y, 30, 1), buf, ctx, bg);
                    let hint = self.docker_hint(d, w);
                    let hx = r.x + 30;
                    buf.set_string(
                        hx,
                        r.y,
                        junie_tui::ui::reveal::cut(
                            Rect::new(hx, r.y, area.right().saturating_sub(hx), 1),
                            &hint,
                        ),
                        t.faint().bg(bg),
                    );
                    let mut y = r.y + 1;
                    for n in &docker_notices {
                        let nw = r.width.saturating_sub(4);
                        let used = n.render(
                            Rect::new(r.x + 4, y, nw, r.bottom().saturating_sub(y)),
                            buf,
                            ctx,
                            bg,
                        );
                        y += used;
                    }
                }
                None => ctx.focus_stop(DOCKER),
            }
        }
        match l.slot(dirty_slot) {
            Some(r) => {
                let r = col(r);
                self.dirty_policy
                    .render(Rect::new(r.x, r.y, r.width.min(48), 3), buf, ctx, bg);
            }
            None => ctx.focus_stop(self.dirty_policy.id),
        }
    }

    fn fixed_at(&self, focus: Option<WidgetId>) -> bool {
        [
            (KEEP_AWAKE, self.keep_awake.control.is_fixed()),
            (GIT_PULL, self.git_pull.control.is_fixed()),
            (DIRTY_POLICY, self.dirty_policy.control.is_fixed()),
            (WORKDIR, self.workdir_button.control.is_fixed()),
        ]
        .into_iter()
        .any(|(id, fixed)| fixed && focus == Some(id))
    }

    fn docker_hint(&self, d: &DockerSetting, w: &World) -> String {
        if self.docker.checked {
            return match &d.engine {
                EngineState::Configured { endpoint } => {
                    format!(
                        "On · {} · {}",
                        d.gateway,
                        w.surfaces.vocabulary.engine_endpoint(endpoint)
                    )
                }
                EngineState::Missing => format!("On · {} · no engine", d.gateway),
            };
        }
        match self.always_on_role(d) {
            Some(role) => format!("Always on for role {role}"),
            None => "Off by default".into(),
        }
    }

    fn always_on_role<'a>(&'a self, d: &'a DockerSetting) -> Option<&'a str> {
        match &self.pending.roles.allowed {
            AllowedRoles::Custom(list) if !list.is_empty() => list
                .iter()
                .all(|r| d.role_enabled.contains(r))
                .then(|| list[0].as_str()),
            _ => None,
        }
    }

    fn docker_notices(&self, w: &World, _width: u16) -> Vec<Notice> {
        let Some(d) = w.surfaces.docker.as_ref().filter(|_| self.docker.checked) else {
            return vec![];
        };
        let mut v = vec![Notice::warning(format!(
            "{} · press i to read them",
            plural(d.risks.len(), "risk", "risks")
        ))];
        if d.engine == EngineState::Missing {
            v.push(Notice::warning(format!(
                "Gateway {} has no Docker engine. Launches fail until you add one with jackin gateway add.",
                d.gateway
            )));
        }
        v
    }

    fn open_docker_risks(&self, w: &World, cx: &mut Cx, confirm: bool) {
        let Some(d) = w.surfaces.docker.as_ref() else {
            return;
        };
        let dialog = super::modals::docker_risk_dialog(
            WidgetId::of("editor.docker.risks"),
            &self.pending.name,
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
            self.sync_pending();
            return o;
        }
        self.open_docker_risks(w, cx, true);
        Outcome::Changed
    }

    fn render_roles(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        self.build_roles(w);
        let t = ctx.theme;
        let bg = t.canvas;
        let focused = ctx.interaction.focused(ROLES);
        let (allowed_n, total) = match &self.pending.roles.allowed {
            AllowedRoles::All => (w.roles.len(), w.roles.len()),
            AllowedRoles::Custom(list) => (list.len(), w.roles.len()),
        };
        let head = format!(
            "Allowed roles  {}{}",
            if matches!(self.pending.roles.allowed, AllowedRoles::All) {
                "all".to_owned()
            } else {
                format!("{allowed_n} of {total}")
            },
            self.role_filter
                .as_deref()
                .map(|f| format!("  · filter {f}"))
                .unwrap_or_default()
        );
        buf.set_string(
            area.x + 2,
            area.y,
            &head,
            t.secondary().bg(bg).add_modifier(Modifier::BOLD),
        );
        let def = format!(
            "default {}",
            self.pending
                .roles
                .default
                .as_deref()
                .map(|d| format!("★ {d}"))
                .unwrap_or("none".into())
        );
        buf.set_string(
            area.right().saturating_sub(width(&def) as u16 + 2),
            area.y,
            &def,
            t.faint().bg(bg),
        );
        let body = Rect::new(
            area.x,
            area.y + 1,
            area.width,
            area.height.saturating_sub(1),
        );
        ctx.control(ROLES, body, false);
        ctx.scrollable(ROLES, body);
        self.role_scroll.set_content(self.role_rows.len());
        self.role_scroll.set_viewport(body.height as usize);
        self.role_scroll.follow(self.role_cursor);
        let has_sb = self.role_scroll.overflows();
        let name_w = self
            .role_rows
            .iter()
            .map(|r| match r {
                RoleRow::Role(n) => width(n),
                RoleRow::Load => 0,
            })
            .max()
            .unwrap_or(8)
            .clamp(8, 24) as u16;
        for (k, i) in self.role_scroll.visible_range().enumerate() {
            let row = &self.role_rows[i];
            let y = body.y + k as u16;
            if y >= body.bottom() {
                break;
            }
            let rid = ROLES.child(i);
            let mut s = ctx.state(rid);
            s.focused = focused && i == self.role_cursor;
            s.selected = i == self.role_cursor;
            let st = t.row(s, bg);
            let rect = Rect::new(body.x, y, body.width.saturating_sub(u16::from(has_sb)), 1);
            fill(buf, rect, st);
            buf.set_string(
                rect.x,
                y,
                t.gutter_symbol(s),
                t.gutter(s, st.bg.unwrap_or(bg), false),
            );
            buf.set_string(
                rect.x + 1,
                y,
                if s.selected { "›" } else { " " },
                st.fg(if s.focused {
                    t.accent
                } else {
                    t.text_secondary
                }),
            );
            match row {
                RoleRow::Load => {
                    buf.set_string(
                        rect.x + 4,
                        y,
                        "+ Load role…",
                        st.fg(if s.focused {
                            t.text_primary
                        } else {
                            t.text_secondary
                        }),
                    );
                }
                RoleRow::Role(name) => {
                    let entry = w.roles.iter().find(|r| &r.name == name);
                    let allowed = self.pending.roles.allows(name);
                    let is_default = self.pending.roles.default.as_deref() == Some(name.as_str());
                    let changed = self.original.roles.allows(name) != allowed
                        || (self.original.roles.default.as_deref() == Some(name.as_str()))
                            != is_default;
                    buf.set_string(
                        rect.x + 3,
                        y,
                        if changed { "•" } else { " " },
                        st.fg(t.warning),
                    );
                    buf.set_string(
                        rect.x + 5,
                        y,
                        if allowed { "[✓]" } else { "[ ]" },
                        st.fg(if allowed {
                            t.text_primary
                        } else {
                            t.text_muted
                        }),
                    );
                    buf.set_string(
                        rect.x + 9,
                        y,
                        fit(name, name_w as usize),
                        if s.selected {
                            st.add_modifier(Modifier::BOLD)
                        } else {
                            st
                        },
                    );
                    let sx = rect.x + 10 + name_w;
                    buf.set_string(
                        sx,
                        y,
                        if is_default { "★" } else { " " },
                        st.fg(if s.focused {
                            t.text_primary
                        } else {
                            t.text_secondary
                        })
                        .add_modifier(if s.focused {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                    );
                    let (meta, tone) = match entry {
                        Some(RoleEntry {
                            load_error: Some(e),
                            ..
                        }) => (format!("! load error · {e}"), t.error),
                        Some(e) if !e.in_registry => {
                            (format!("{} · not in registry", e.source.label()), t.warning)
                        }
                        Some(e) => (
                            format!(
                                "registry · {} · {}",
                                if e.trusted { "trusted" } else { "untrusted" },
                                e.description
                            ),
                            t.text_muted,
                        ),
                        None => ("not in registry".into(), t.warning),
                    };
                    let mx = sx + 3;
                    buf.set_string(
                        mx,
                        y,
                        junie_tui::ui::reveal::cut(
                            Rect::new(mx, y, rect.right().saturating_sub(mx + 1), 1),
                            &meta,
                        ),
                        st.fg(tone),
                    );
                }
            }

            legible_on_fill(buf, rect, t, bg);
            ctx.clickable(rid, rect);
        }
        if has_sb {
            scrollbar::render_with_fade(
                Rect::new(body.x, body.y, body.width, body.height),
                buf,
                ctx,
                ROLES,
                &self.role_scroll,
                focused,
            );
        }
    }

    fn build_accounts(&mut self, w: &World) {
        let q = self.acct.filter.as_ref().map(|f| f.to_lowercase());
        let mut rows = vec![];
        let mut providers: Vec<Provider> = w.accounts.accounts.iter().map(|a| a.provider).collect();
        providers.sort();
        providers.dedup();
        for p in providers {
            let ids: Vec<AccountId> = w
                .accounts
                .sorted()
                .into_iter()
                .filter(|a| a.provider == p)
                .filter(|a| {
                    q.as_ref().is_none_or(|q| {
                        a.display_name.to_lowercase().contains(q)
                            || a.provider.label().to_lowercase().contains(q)
                            || a.status_word().contains(q.as_str())
                    })
                })
                .map(|a| a.id.clone())
                .collect();
            if ids.is_empty() {
                continue;
            }
            rows.push(AcctRow::Provider(p));
            rows.extend(ids.into_iter().map(AcctRow::Account));
        }
        self.acct.rows = rows;
        if self.acct.cursor >= self.acct.rows.len() {
            self.acct.cursor = self.acct.rows.len().saturating_sub(1);
        }
        while matches!(
            self.acct.rows.get(self.acct.cursor),
            Some(AcctRow::Provider(_))
        ) && self.acct.cursor + 1 < self.acct.rows.len()
        {
            self.acct.cursor += 1;
        }
        self.acct.scroll.set_content(self.acct.rows.len());
    }

    fn acct_move(&mut self, delta: isize) {
        let n = self.acct.rows.len();
        if n == 0 {
            return;
        }
        let step = if delta < 0 { -1 } else { 1 };
        let mut remaining = delta.unsigned_abs();
        let mut i = self.acct.cursor as isize;
        let mut last_account = i;
        while remaining > 0 {
            let next = i + step;
            if next < 0 || next >= n as isize {
                break;
            }
            i = next;
            if matches!(self.acct.rows[i as usize], AcctRow::Account(_)) {
                last_account = i;
                remaining -= 1;
            }
        }
        self.acct.cursor = last_account as usize;
        self.acct.scroll.ensure_visible(self.acct.cursor);
    }

    fn render_accounts(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        self.build_accounts(w);
        let t = ctx.theme;
        let bg = t.canvas;
        let focused = ctx.interaction.focused(ACCOUNTS);
        let effective = self.pending.effective_accounts(&w.accounts);
        let inherited = effective
            .iter()
            .filter(|e| e.origin == Effective::InheritedDefault)
            .count();
        let enabled = effective.len() - inherited;
        let head = format!(
            "Active accounts  {} effective · {inherited} inherited · {enabled} enabled here{}",
            effective.len(),
            self.acct
                .filter
                .as_deref()
                .map(|f| format!("  · filter {f}"))
                .unwrap_or_default()
        );
        buf.set_string(
            area.x + 2,
            area.y,
            junie_tui::ui::reveal::cut(
                Rect::new(area.x + 2, area.y, area.width.saturating_sub(4), 1),
                &head,
            ),
            t.secondary().bg(bg).add_modifier(Modifier::BOLD),
        );
        let hint_txt = "registry in Accounts (c)";
        if area.width > width(&head) as u16 + width(hint_txt) as u16 + 8 {
            buf.set_string(
                area.right().saturating_sub(width(hint_txt) as u16 + 2),
                area.y,
                hint_txt,
                t.faint().bg(bg),
            );
        }
        let body = Rect::new(
            area.x,
            area.y + 1,
            area.width,
            area.height.saturating_sub(1),
        );
        ctx.control(ACCOUNTS, body, false);
        ctx.scrollable(ACCOUNTS, body);
        self.acct.scroll.set_viewport(body.height as usize);
        self.acct.scroll.follow(self.acct.cursor);
        let has_sb = self.acct.scroll.overflows();
        let row_w = body.width.saturating_sub(u16::from(has_sb));
        if self.acct.rows.is_empty() {
            buf.set_string(
                body.x + 2,
                body.y,
                "No accounts in the registry · c opens the Account & Usage Center",
                t.muted().bg(bg),
            );
        }
        let name_w = w
            .accounts
            .accounts
            .iter()
            .map(|a| width(&a.display_name))
            .max()
            .unwrap_or(8)
            .clamp(8, 28) as u16;
        let original_effective = self.original.effective_accounts(&w.accounts);
        for (k, i) in self.acct.scroll.visible_range().enumerate() {
            let y = body.y + k as u16;
            if y >= body.bottom() {
                break;
            }
            let rid = ACCOUNTS.child(i);
            match &self.acct.rows[i] {
                AcctRow::Provider(p) => {
                    let n = effective.iter().filter(|e| e.provider == *p).count();
                    buf.set_string(
                        body.x + 2,
                        y,
                        p.label(),
                        t.secondary().bg(bg).add_modifier(Modifier::BOLD),
                    );
                    let meta = match n {
                        0 => "none active".to_owned(),
                        1 => "1 active".to_owned(),
                        n => format!("{n} active · picker at session start"),
                    };
                    let mw = width(&meta) as u16;
                    if row_w > mw + 20 {
                        buf.set_string(
                            body.x + row_w.saturating_sub(mw + 1),
                            y,
                            &meta,
                            t.faint().bg(bg),
                        );
                    }
                }
                AcctRow::Account(id) => {
                    let Some(a) = w.accounts.get(id) else {
                        continue;
                    };
                    let eff = effective.iter().find(|e| &e.id == id);
                    let mut s = ctx.state(rid);
                    s.focused = focused && i == self.acct.cursor;
                    s.selected = i == self.acct.cursor;
                    let st = t.row(s, bg);
                    let rect = Rect::new(body.x, y, row_w, 1);
                    fill(buf, rect, st);
                    buf.set_string(
                        rect.x,
                        y,
                        t.gutter_symbol(s),
                        t.gutter(s, st.bg.unwrap_or(bg), false),
                    );
                    buf.set_string(
                        rect.x + 1,
                        y,
                        if s.selected { "›" } else { " " },
                        st.fg(if s.focused {
                            t.accent
                        } else {
                            t.text_secondary
                        }),
                    );
                    let was_active = original_effective.iter().any(|e| &e.id == id);
                    let changed = was_active != eff.is_some()
                        || self.original.accounts.preferred.get(&a.provider)
                            != self.pending.accounts.preferred.get(&a.provider);
                    buf.set_string(
                        rect.x + 3,
                        y,
                        if changed { "•" } else { " " },
                        st.fg(t.warning),
                    );
                    let active = eff.is_some();
                    buf.set_string(
                        rect.x + 5,
                        y,
                        if active { "[✓]" } else { "[ ]" },
                        st.fg(if active { t.text_primary } else { t.text_muted }),
                    );
                    let label_style = if !a.enabled {
                        st.fg(t.text_faint)
                    } else if s.selected {
                        st.add_modifier(Modifier::BOLD)
                    } else {
                        st
                    };

                    let label = if a.origin == crate::domain::account::AccountOrigin::Discovered
                        && a.display_name == "discovered"
                    {
                        "host login".to_owned()
                    } else {
                        a.display_name.clone()
                    };
                    buf.set_string(rect.x + 9, y, fit(&label, name_w as usize), label_style);
                    let sx = rect.x + 10 + name_w;
                    buf.set_string(
                        sx,
                        y,
                        if eff.is_some_and(|e| e.preferred) {
                            "★"
                        } else {
                            " "
                        },
                        st.fg(if s.focused {
                            t.text_primary
                        } else {
                            t.text_secondary
                        }),
                    );
                    let origin = match eff.map(|e| e.origin) {
                        Some(o) => o.label().to_owned(),
                        None if a.default_for_provider => "disabled here".to_owned(),
                        None if a.origin == crate::domain::account::AccountOrigin::Discovered => {
                            "discovered on host".to_owned()
                        }
                        None => "available".to_owned(),
                    };
                    let status = crate::domain::workspace::usability_of(a)
                        .label(w.surfaces.fixed.as_ref(), &w.surfaces.vocabulary);
                    let status_tone = if a.enabled && status == "ready" {
                        t.text_muted
                    } else {
                        t.warning
                    };
                    let ox = sx + 3;
                    buf.set_string(ox, y, fit(&origin, 22), st.fg(t.text_muted));
                    let stx = ox + 24;
                    if stx < rect.right() {
                        buf.set_string(
                            stx,
                            y,
                            junie_tui::ui::reveal::cut(
                                Rect::new(stx, y, rect.right().saturating_sub(stx + 1), 1),
                                &status,
                            ),
                            st.fg(status_tone),
                        );
                    }
                    legible_on_fill(buf, rect, t, bg);
                    ctx.clickable(rid, rect);
                }
            }
        }
        if has_sb {
            scrollbar::render_with_fade(
                Rect::new(body.x, body.y, body.width, body.height),
                buf,
                ctx,
                ACCOUNTS,
                &self.acct.scroll,
                focused,
            );
        }
    }

    fn accounts_key(&mut self, key: &Key, w: &mut World, cx: &mut Cx) -> Outcome {
        self.build_accounts(w);
        let n = self.acct.rows.len();
        let vp = self.acct.scroll.viewport_len.max(1) as isize;
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.acct_move(-1),
            KeyCode::Down | KeyCode::Char('j') => self.acct_move(1),
            KeyCode::PageUp => self.acct_move(-vp),
            KeyCode::PageDown => self.acct_move(vp),
            KeyCode::Home | KeyCode::Char('g') => self.acct_move(-(n as isize)),
            KeyCode::End | KeyCode::Char('G') => self.acct_move(n as isize),
            KeyCode::Char('/') => {
                let input = TextInput::new(WidgetId::of("editor.acct.filter.input"), "Filter")
                    .placeholder("name, provider, status")
                    .value(self.acct.filter.as_deref().unwrap_or(""))
                    .plain_label();
                let d = Dialog::prompt(
                    WidgetId::of("editor.acct.filter"),
                    "Filter accounts",
                    input,
                    "Apply",
                );
                cx.open(d, LayerTag::new("acct.filter"));
            }
            KeyCode::Char(' ') | KeyCode::Enter => {
                let Some(AcctRow::Account(id)) = self.acct.rows.get(self.acct.cursor).cloned()
                else {
                    return Outcome::Ignored;
                };
                if let Some(f) = &w.surfaces.fixed {
                    raise(&f.enable_workspace);
                    return Outcome::Consumed;
                }
                let Some(a) = w.accounts.get(&id).cloned() else {
                    return Outcome::Ignored;
                };
                let active = self
                    .pending
                    .effective_accounts(&w.accounts)
                    .iter()
                    .any(|e| e.id == id);
                let policy = &mut self.pending.accounts;
                if active {
                    if a.default_for_provider {
                        policy.disabled_defaults.insert(id.clone());
                    }
                    policy.enabled.remove(&id);
                    policy.preferred.retain(|_, v| *v != id);
                    policy.role_preferred.retain(|_, v| *v != id);
                    cx.status(format!(
                        "{} · off for this Workspace{}",
                        a.title(),
                        if a.default_for_provider {
                            " · the global default stays registered"
                        } else {
                            ""
                        }
                    ));
                } else {
                    if a.default_for_provider {
                        policy.disabled_defaults.remove(&id);
                    } else {
                        policy.enabled.insert(id.clone());
                    }
                    cx.status(format!(
                        "{} · active for this Workspace{}",
                        a.title(),
                        if !a.enabled {
                            " · disabled globally, usable once re-enabled in Accounts"
                        } else {
                            ""
                        }
                    ));
                }
            }
            KeyCode::Char('p') => {
                let Some(AcctRow::Account(id)) = self.acct.rows.get(self.acct.cursor).cloned()
                else {
                    return Outcome::Ignored;
                };
                let Some(a) = w.accounts.get(&id).cloned() else {
                    return Outcome::Ignored;
                };
                let active = self
                    .pending
                    .effective_accounts(&w.accounts)
                    .iter()
                    .any(|e| e.id == id);
                if !active {
                    cx.error(format!(
                        "{} must be active before it can be preferred",
                        a.title()
                    ));
                } else if self.pending.accounts.preferred.get(&a.provider) == Some(&id) {
                    self.pending.accounts.preferred.remove(&a.provider);
                    cx.status(format!(
                        "{} · no longer preferred · the default order applies",
                        a.title()
                    ));
                } else {
                    self.pending
                        .accounts
                        .preferred
                        .insert(a.provider, id.clone());
                    cx.status(format!(
                        "Preferred for {} ★ {}",
                        a.provider.short(),
                        a.title()
                    ));
                }
            }
            _ => return Outcome::Ignored,
        }
        Outcome::Changed
    }

    fn roles_key(&mut self, key: &Key, w: &mut World, cx: &mut Cx) -> Outcome {
        let n = self.role_rows.len();
        let Some(row) = self.role_rows.get(self.role_cursor).cloned() else {
            return Outcome::Ignored;
        };
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.role_cursor = self.role_cursor.saturating_sub(1)
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.role_cursor = (self.role_cursor + 1).min(n - 1)
            }
            KeyCode::Home | KeyCode::Char('g') => self.role_cursor = 0,
            KeyCode::End | KeyCode::Char('G') => self.role_cursor = n - 1,
            KeyCode::PageUp => {
                self.role_cursor = self
                    .role_cursor
                    .saturating_sub(self.role_scroll.viewport_len.max(1))
            }
            KeyCode::PageDown => {
                self.role_cursor =
                    (self.role_cursor + self.role_scroll.viewport_len.max(1)).min(n - 1)
            }
            KeyCode::Char('/') => {
                let input = TextInput::new(WidgetId::of("editor.roles.filter.input"), "Filter")
                    .placeholder("name or description")
                    .value(self.role_filter.as_deref().unwrap_or(""))
                    .plain_label();
                let d = Dialog::prompt(
                    WidgetId::of("editor.roles.filter"),
                    "Filter roles",
                    input,
                    "Apply",
                );
                cx.open(d, LayerTag::new("roles.filter"));
            }
            KeyCode::Char(' ') => {
                if let RoleRow::Role(name) = &row {
                    let all: Vec<RoleName> = w.roles.iter().map(|r| r.name.clone()).collect();
                    let mut list = match &self.pending.roles.allowed {
                        AllowedRoles::All => all.clone(),
                        AllowedRoles::Custom(l) => l.clone(),
                    };
                    if list.iter().any(|r| r == name) {
                        list.retain(|r| r != name);
                        if let Some(f) = &w.surfaces.fixed
                            && !list.iter().any(|r| all.contains(r))
                        {
                            cx.status(f.last_role.clone());
                            return Outcome::Consumed;
                        }
                        if self.pending.roles.default.as_deref() == Some(name.as_str()) {
                            self.pending.roles.default = None;
                        }
                    } else {
                        list.push(name.clone());
                    }
                    self.pending.roles.allowed =
                        if list.len() == all.len() && all.iter().all(|r| list.contains(r)) {
                            AllowedRoles::All
                        } else {
                            AllowedRoles::Custom(list)
                        };
                    cx.status(format!(
                        "{name} · {}",
                        if self.pending.roles.allows(name) {
                            "allowed"
                        } else {
                            "not allowed"
                        }
                    ));
                }
            }
            KeyCode::Enter | KeyCode::Char('*') => match &row {
                RoleRow::Load => self.open_role_picker(w, cx),
                RoleRow::Role(name) => {
                    let failed = w
                        .roles
                        .iter()
                        .any(|r| &r.name == name && r.load_error.is_some());
                    if failed {
                        self.start_role_load(name, w, cx);
                    } else if !self.pending.roles.allows(name) {
                        cx.error(format!(
                            "{name} must be allowed before it can be the default"
                        ));
                    } else if self.pending.roles.default.as_deref() == Some(name.as_str()) {
                        self.pending.roles.default = None;
                        cx.status(format!("{name} · no longer the default"));
                    } else {
                        self.pending.roles.default = Some(name.clone());
                        cx.status(format!("Default role ★ {name}"));
                    }
                }
            },
            KeyCode::Char('a') => self.open_role_picker(w, cx),
            _ => return Outcome::Ignored,
        }
        Outcome::Changed
    }

    fn open_role_picker(&mut self, w: &World, cx: &mut Cx) {
        let mut p = Picker::new(WidgetId::of("editor.roles.picker"), "Load role");
        p.set_width(84);
        p.scope = Some("trusted role registry".into());
        let (items, targets) = self.role_items(w, "");
        p.set_items(items)
            .unwrap_or_else(|error| p.report_item_error(error));
        self.picker_targets = targets;
        cx.open(p, LayerTag::new("role.load"));
    }

    fn role_items(&self, w: &World, query: &str) -> (Vec<Item>, Vec<String>) {
        let q = query.to_lowercase();
        let mut items = vec![];
        let mut targets = vec![];
        for r in &w.roles {
            let label = r.full_name();
            if !q.is_empty()
                && !label.to_lowercase().contains(&q)
                && !r.description.to_lowercase().contains(&q)
            {
                continue;
            }
            let already = self.pending.roles.allows(&r.name)
                && matches!(self.pending.roles.allowed, AllowedRoles::Custom(_));
            items.push(Item {
                tag: (if !r.trusted {
                    Some("trust required")
                } else if already {
                    Some("allowed")
                } else {
                    None
                })
                .map(Into::into),
                ..Item::new(label)
                    .detail(format!("{} · {}", r.source.label(), r.description))
                    .glyph(if r.trusted { "◆" } else { "!" })
                    .group(match &r.source {
                        RoleSource::Git { .. } => "git",
                        RoleSource::Local { .. } => "local",
                    })
            });
            targets.push(r.name.clone());
        }
        (items, targets)
    }

    fn open_trust_dialog(&mut self, e: &RoleEntry, w: &World, cx: &mut Cx) {
        let d = modals::trust_dialog(WidgetId::of("editor.trust"), e, w);
        cx.open(d, LayerTag::new("role.trust").key(&e.name));
    }

    fn start_role_load(&mut self, name: &str, w: &mut World, cx: &mut Cx) {
        self.pending_load = Some(name.to_owned());
        match w.load_role(name) {
            RoleLoad::Requested => cx.progress(job::ROLE_LOAD, format!("Loading role {name}…")),
            RoleLoad::NeedsTrust(entry) => self.open_trust_dialog(&entry, w, cx),
            RoleLoad::Unregistered => {
                self.pending_load = None;
                cx.error(format!("Role {name} not found"));
            }
        }
    }

    fn finish_role_load(
        &mut self,
        name: &str,
        result: &Result<Box<Snapshot>, String>,
        w: &mut World,
        cx: &mut Cx,
    ) {
        self.pending_load = None;
        if let Ok(snapshot) = result {
            w.apply((**snapshot).clone());
        }
        match (w.roles.iter().find(|r| r.name == name), result) {
            (Some(_), Ok(_)) => {
                let all: Vec<RoleName> = w.roles.iter().map(|r| r.name.clone()).collect();
                let mut list = match &self.pending.roles.allowed {
                    AllowedRoles::All => all.clone(),
                    AllowedRoles::Custom(l) => l.clone(),
                };
                if !list.iter().any(|r| r == name) {
                    list.push(name.to_owned());
                }
                self.pending.roles.allowed = if list.len() == all.len() {
                    AllowedRoles::All
                } else {
                    AllowedRoles::Custom(list)
                };
                cx.status(format!("Role {name} loaded and allowed"));
            }
            (Some(r), Err(error)) => {
                let d = InfoDialog::new(
                    WidgetId::of("editor.roleerr"),
                    &format!("Role {name} failed to load"),
                    vec![
                        Prop::new("Error", error.as_str()),
                        Prop::new("Source", r.source.label()),
                        Prop::new("Next", "trust stays granted · retry from the Roles tab"),
                    ],
                )
                .error()
                .width(70);
                cx.open(d, LayerTag::new("roleerr"));
            }
            (None, _) => cx.error(format!("Role {name} is not in the registry")),
        }
        self.build_roles(w);
    }

    fn open_workdir_picker(&mut self, w: &World, cx: &mut Cx) {
        let mut p = Picker::new(WidgetId::of("editor.workdir.picker"), "Working directory");
        p.set_width(72);
        p.scope = Some(self.pending.name.clone());
        let mut items = vec![];
        let mut targets = vec![];
        for m in &self.cfg.pending.mounts {
            items.push(Item {
                tag: (if m.destination == self.pending.workdir {
                    Some("current")
                } else {
                    None
                })
                .map(Into::into),
                ..Item::new(m.destination.clone())
                    .detail(format!("mount · {}", w.tilde(m.source_label())))
                    .glyph("▪")
                    .group("mounts")
            });
            targets.push(m.destination.clone());
            if let crate::domain::workspace::MountSource::Host(src) = &m.source {
                let src = crate::world::expand(&w.home, src);
                let listed = w.backend.file_source(w).list(&src).unwrap_or_default();
                for f in listed.iter().filter(|f| f.dir) {
                    let dest = format!("{}/{}", m.destination, f.name());
                    items.push(Item {
                        tag: (if dest == self.pending.workdir {
                            Some("current")
                        } else {
                            None
                        })
                        .map(Into::into),
                        ..Item::new(dest.clone())
                            .detail(f.meta.clone())
                            .group("subdirectories")
                    });
                    targets.push(dest);
                }
            }
        }
        if items.is_empty() {
            items.push(
                Item::new(self.pending.workdir.clone())
                    .detail("no mounts yet · add one in the Mounts tab")
                    .glyph("▪")
                    .tag("current"),
            );
            targets.push(self.pending.workdir.clone());
        }
        p.set_items(items)
            .unwrap_or_else(|error| p.report_item_error(error));
        self.picker_targets = targets;
        cx.open(p, LayerTag::new("workdir"));
    }

    fn general_key(&mut self, key: &Key, w: &mut World, cx: &mut Cx) -> Outcome {
        let focus = cx.focus.current();
        if focus == Some(NAME) {
            let (o, ev) = self.name.on_key(key);
            match ev {
                Some(InputAction::Committed) => {
                    self.name.field.error = None;
                    self.name.validate();
                    self.sync_pending();
                    cx.status(format!("Name {} · save to apply", self.pending.name));
                    return Outcome::Changed;
                }
                Some(InputAction::Cancelled) => {
                    cx.status("Reverted");
                    return Outcome::Changed;
                }
                _ => {}
            }
            if o.consumed() {
                return o;
            }
        }
        if focus == Some(WORKDIR) {
            let (o, fired) = self.workdir_button.on_key(key);
            if fired {
                self.open_workdir_picker(w, cx);
                return Outcome::Changed;
            }
            if o.consumed() {
                return o;
            }
        }
        if focus == Some(KEEP_AWAKE) {
            let o = self.keep_awake.on_key(key);
            if o.consumed() {
                self.sync_pending();
                return o;
            }
        }
        if focus == Some(GIT_PULL) {
            let o = self.git_pull.on_key(key);
            if o.consumed() {
                self.sync_pending();
                return o;
            }
        }
        if focus == Some(DOCKER) && (key.is_char(' ') || key.is(KeyCode::Enter)) {
            return self.toggle_docker(w, cx);
        }
        if w.surfaces.docker.is_some() && !self.name.editing && key.is_char('i') && key.plain() {
            self.open_docker_risks(w, cx, false);
            return Outcome::Changed;
        }
        if focus == Some(DIRTY_POLICY) {
            let (o, _) = self.dirty_policy.on_key(key);
            if o.consumed() {
                self.sync_pending();
                return o;
            }
        }
        match key.code {
            KeyCode::Down | KeyCode::Char('j') if !self.name.editing => {
                cx.focus.next(cx.ring);
                Outcome::Changed
            }
            KeyCode::Up | KeyCode::Char('k') if !self.name.editing => {
                cx.focus.prev(cx.ring);
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }
}

impl EditorScreen {
    fn refresh_taken(&mut self, w: &World) {
        self.taken = w
            .workspaces
            .iter()
            .filter(|x| Some(x.id) != self.workspace)
            .map(|x| (x.name.clone(), x.id))
            .collect();
    }
}

impl Page<Jackin> for EditorScreen {
    fn enter(&mut self, w: &mut World, cx: &mut Cx) {
        self.refresh_taken(w);
        self.build_roles(w);
        cx.focus.focus(if self.is_create() { NAME } else { TABS });
    }

    fn primary_focus(&self) -> Option<WidgetId> {
        Some(TABS)
    }

    fn is_editing(&self) -> bool {
        self.name.editing
    }

    fn animating(&self, _w: &World) -> bool {
        self.saving || self.pending_load.is_some()
    }

    fn on_tick(&mut self, _w: &mut World, _cx: &mut Cx) -> Outcome {
        if self.saving || self.pending_load.is_some() {
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    fn on_msg(&mut self, msg: &Msg, w: &mut World, cx: &mut Cx) -> Outcome {
        match msg {
            Msg::WorkspaceSaved { id, result } if self.saving => {
                self.finish_save(*id, result, w, cx);
                Outcome::Changed
            }
            Msg::RoleLoaded { role, result }
                if self.pending_load.as_deref() == Some(role.as_str()) =>
            {
                self.finish_role_load(role, result, w, cx);
                Outcome::Changed
            }
            Msg::RoleTrusted { role, result }
                if self.pending_load.as_deref() == Some(role.as_str()) =>
            {
                match result {
                    Ok(snapshot) => {
                        w.apply((**snapshot).clone());
                        self.start_role_load(role, w, cx);
                    }
                    Err(error) => {
                        self.pending_load = None;
                        cx.error(error.clone());
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
        if self.cfg.menu_open() {
            let o = self.cfg.menu_key(key, w, cx);
            self.sync_pending();

            if o != Outcome::Ignored {
                return o;
            }
        }
        let focus = cx.focus.current();
        let editing = self.is_editing();

        if key.ctrl() && key.code == KeyCode::Char('s') {
            self.open_preview(w, cx);
            return Outcome::Changed;
        }
        if !editing {
            match key.code {
                KeyCode::Char('[') => {
                    let i = (self.tabs.active + 4) % 5;
                    self.set_tab(i, cx);
                    return Outcome::Changed;
                }
                KeyCode::Char(']') => {
                    let i = (self.tabs.active + 1) % 5;
                    self.set_tab(i, cx);
                    return Outcome::Changed;
                }
                _ => {}
            }
        }
        if focus == Some(TABS) {
            if matches!(
                key.code,
                KeyCode::Enter | KeyCode::Down | KeyCode::Char('j')
            ) {
                cx.focus.focus(self.body_focus());
                return Outcome::Changed;
            }
            let (o, ev) = self.tabs.on_key(key);
            if let Some(TabAction::Activated(i)) = ev {
                self.tab = EdTab::ALL[i.min(4)];
                return Outcome::Changed;
            }
            if o.consumed() {
                return o;
            }
            match key.code {
                KeyCode::Esc => {
                    self.leave(cx);
                    return Outcome::Changed;
                }
                _ => return Outcome::Ignored,
            }
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
                self.open_preview(w, cx);
                return Outcome::Changed;
            }
            if o.consumed() {
                return o;
            }
        }
        if matches!(focus, Some(CANCEL) | Some(SAVE)) {
            match key.code {
                KeyCode::Left | KeyCode::Char('h') => {
                    cx.focus.focus(CANCEL);
                    return Outcome::Changed;
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    cx.focus.focus(SAVE);
                    return Outcome::Changed;
                }
                KeyCode::Esc | KeyCode::Up | KeyCode::Char('k') => {
                    cx.focus.focus(self.body_focus());
                    return Outcome::Changed;
                }
                _ => {}
            }
        }

        let body = match self.tab {
            EdTab::General
                if matches!(
                    focus,
                    Some(NAME)
                        | Some(WORKDIR)
                        | Some(KEEP_AWAKE)
                        | Some(GIT_PULL)
                        | Some(DOCKER)
                        | Some(DIRTY_POLICY)
                ) =>
            {
                self.general_key(key, w, cx)
            }
            EdTab::Roles if focus == Some(ROLES) => self.roles_key(key, w, cx),
            EdTab::Accounts if focus == Some(ACCOUNTS) => self.accounts_key(key, w, cx),
            t if t.cfg().is_some() && self.cfg.owns(t.cfg().unwrap(), focus) => {
                let o = self.cfg.on_key(t.cfg().unwrap(), key, w, cx);
                if o.consumed() {
                    self.sync_pending();
                }
                o
            }
            _ => Outcome::Ignored,
        };
        if body.consumed() {
            return body;
        }
        if editing {
            return Outcome::Ignored;
        }
        match key.code {
            KeyCode::Esc => {
                if self.name.field.error.is_some() {
                    self.name.field.error = None;
                }
                cx.focus.focus(TABS);
                Outcome::Changed
            }
            KeyCode::Char('c') if key.plain() && self.tab == EdTab::Accounts => {
                let sel = match self.acct.rows.get(self.acct.cursor) {
                    Some(AcctRow::Account(id)) => Some(id.clone()),
                    _ => None,
                };
                cx.go(Go::Accounts { select: sel });
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    fn pointer(&mut self, ev: &PointerEvent, w: &mut World, cx: &mut Cx) -> Outcome {
        let click = ev.is(Phase::Click);
        if self.saving && (click || ev.is(Phase::Secondary)) {
            return Outcome::Consumed;
        }
        if click && self.cfg.menu_open() {
            let o = self.cfg.menu_click(ev.id, w, cx);
            self.sync_pending();
            return o;
        }
        if let Some(o) = self.general_stack.pointer(GENERAL, ev) {
            return o;
        }
        if let Some(r) = self.tabs.route(ev) {
            let (o, act) = r.into_parts();
            if let Some(TabAction::Activated(i)) = act {
                self.tab = EdTab::ALL[i.min(4)];
            }
            if !click {
                return o;
            }
            cx.focus.focus(TABS);
            return o.or(Outcome::Changed);
        }
        if self.tab == EdTab::General {
            if let Some(r) = self.name.route(ev) {
                if click {
                    cx.focus.focus(NAME);
                }
                return r.outcome();
            }
            if let Some(r) = self.dirty_policy.route(ev) {
                if click {
                    cx.focus.focus(DIRTY_POLICY);
                }
                let o = r.outcome();
                self.sync_pending();
                return o;
            }
        }
        if let Some(o) = match self.tab {
            EdTab::Accounts => {
                scrollbar::pointer(ev, ACCOUNTS, &mut self.acct.scroll, TrackClick::Jump)
            }
            EdTab::Roles => scrollbar::pointer(ev, ROLES, &mut self.role_scroll, TrackClick::Jump),
            _ => None,
        } {
            return o;
        }
        if click {
            return self.click(ev, w, cx);
        }
        if let Some((Axis::Vertical, delta)) = ev.wheel() {
            let id = ev.id;
            if id == ROLES || id == scrollbar::id_for(ROLES) {
                self.role_scroll.scroll_by(delta as isize);
                return Outcome::Changed;
            }
            if id == ACCOUNTS || id == scrollbar::id_for(ACCOUNTS) {
                self.acct.scroll.scroll_by(delta as isize);
                return Outcome::Changed;
            }
        }
        match self.tab.cfg() {
            Some(t) => self.cfg.pointer(t, ev, w, cx),
            None => Outcome::Ignored,
        }
    }

    fn on_blur(
        &mut self,
        ch: junie_tui::core::focus::FocusChange,
        _w: &mut World,
        cx: &mut Cx,
    ) -> Outcome {
        if ch.from == self.dirty_policy.id {
            return self.dirty_policy.on_blur();
        }
        if ch.from != self.name.id {
            return Outcome::Ignored;
        }
        match self.name.on_blur() {
            Some(InputAction::Committed) => {
                self.name.field.error = None;
                self.name.validate();
                self.sync_pending();
                cx.status(format!("Name {} · save to apply", self.pending.name));
                Outcome::Changed
            }
            Some(InputAction::Cancelled) => {
                cx.status("Reverted");
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    fn on_paste(&mut self, text: &str, _w: &mut World, _cx: &mut Cx) -> Outcome {
        if self.name.editing {
            let out = self.name.on_paste(text);
            if out.consumed() {
                self.sync_pending();
            }
            out
        } else {
            Outcome::Ignored
        }
    }

    fn on_layer_result(
        &mut self,
        tag: &LayerTag,
        result: ModalResult,
        w: &mut World,
        cx: &mut Cx,
    ) -> Outcome {
        if let Some(o) = self.cfg.on_modal(tag, result.clone(), w, cx) {
            self.sync_pending();
            return o;
        }
        match (tag.kind, result) {
            (
                "roles.filter",
                ModalResult::Dialog {
                    action: Some(1),
                    text,
                },
            ) => {
                self.role_filter = text.filter(|t| !t.trim().is_empty());
                self.role_cursor = 0;
                self.build_roles(w);
                Outcome::Changed
            }
            (
                "acct.filter",
                ModalResult::Dialog {
                    action: Some(1),
                    text,
                },
            ) => {
                self.acct.filter = text.filter(|t| !t.trim().is_empty());
                self.acct.cursor = 0;
                self.build_accounts(w);
                Outcome::Changed
            }
            ("roles.filter" | "acct.filter", _) => Outcome::Changed,
            (
                "preview",
                ModalResult::Dialog {
                    action: Some(1), ..
                },
            ) => {
                self.start_save(w, cx);
                Outcome::Changed
            }
            (
                "preview.cleanup",
                ModalResult::Dialog {
                    action: Some(1), ..
                },
            ) => {
                self.cfg.clean_up();
                cx.status("Cleaned up isolated state · saving");
                self.start_save(w, cx);
                Outcome::Changed
            }
            ("preview" | "preview.cleanup", _) => {
                cx.status("Not saved · keep editing");
                cx.focus.focus(SAVE);
                Outcome::Changed
            }
            (
                "exit",
                ModalResult::Dialog {
                    action: Some(2), ..
                },
            ) => {
                self.open_preview(w, cx);
                Outcome::Changed
            }
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
                cx.go(Go::Manager);
                Outcome::Changed
            }
            ("exit", _) => Outcome::Changed,
            ("savefail", _) => {
                cx.focus.focus(SAVE);
                Outcome::Changed
            }
            ("workdir", ModalResult::Picked(i)) => {
                if let Some(d) = self.picker_targets.get(i).cloned() {
                    self.pending.workdir = d.clone();
                    cx.status(format!("Working directory {d} · save to apply"));
                }
                cx.focus.focus(WORKDIR);
                Outcome::Changed
            }
            ("role.load", ModalResult::Picked(i)) => {
                let Some(name) = self.picker_targets.get(i).cloned() else {
                    return Outcome::Changed;
                };
                self.start_role_load(&name, w, cx);
                cx.focus.focus(ROLES);
                Outcome::Changed
            }
            (
                "role.trust",
                ModalResult::Dialog {
                    action: Some(1), ..
                },
            ) => {
                w.perform(Effect::TrustRole {
                    role: tag.key.clone(),
                });
                Outcome::Changed
            }
            ("role.trust", _) => {
                self.pending_load = None;
                cx.status("Not trusted · role not loaded");
                Outcome::Changed
            }
            (
                "docker.on",
                ModalResult::Dialog {
                    action: Some(1), ..
                },
            ) => {
                self.docker.checked = true;
                self.sync_pending();
                cx.focus.focus(DOCKER);
                Outcome::Changed
            }
            ("docker.on" | "docker.info", _) => {
                cx.focus.focus(DOCKER);
                Outcome::Changed
            }
            ("roleerr", _) => Outcome::Changed,
            _ => Outcome::Changed,
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        let t = ctx.theme;
        let bg = t.canvas;
        self.sync_pending();
        let items: Vec<TabItem> = EdTab::ALL
            .iter()
            .zip(tab_names(w))
            .map(|(tab, name)| {
                let mut it = TabItem::new(name);
                it.dirty = match tab {
                    EdTab::General => self.general_dirty(),
                    EdTab::Roles => self.roles_dirty(),
                    EdTab::Accounts => self.accounts_dirty(),
                    t => self.cfg.tab_dirty(t.cfg().unwrap()),
                };
                it.error = match tab {
                    EdTab::General => self.name.field.error.is_some(),
                    EdTab::Roles | EdTab::Accounts => false,
                    t => self.cfg.tab_error(t.cfg().unwrap()),
                };
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
        let body = Rect::new(
            area.x + 1,
            area.y + 4,
            area.width.saturating_sub(2),
            area.height.saturating_sub(6),
        );
        self.row_status = None;
        match self.tab {
            EdTab::General => self.render_general(body, buf, ctx, w),
            EdTab::Roles => self.render_roles(body, buf, ctx, w),
            EdTab::Accounts => self.render_accounts(body, buf, ctx, w),
            t => {
                self.row_status = self.cfg.render(t.cfg().unwrap(), body, buf, ctx, w);
            }
        }

        let n = self.change_count();
        self.save.disabled = n == 0 && !self.is_create() && !self.saving;
        self.save.label = if self.saving {
            format!("{} Saving…", spinner_frame(w.now_ms() as u64 / 80))
        } else {
            "Save…".into()
        };
        let widths = [self.cancel.width(), self.save.width()];
        let lines = junie_tui::widgets::button::row_lines(area.width.saturating_sub(4), &widths, 3);
        let rects = row_layout_right(
            Rect::new(
                area.x,
                area.bottom().saturating_sub(lines),
                area.width.saturating_sub(4),
                1,
            ),
            &widths,
            3,
        );
        self.cancel.render(rects[0], buf, ctx, bg);
        self.save.render(rects[1], buf, ctx, bg);
        self.cfg.render_menu(area, buf, ctx);
    }

    fn hints(&self, focus: Option<WidgetId>, w: &World) -> Vec<Hint> {
        if self.saving {
            return vec![hint("", "Saving…")];
        }
        if self.cfg.menu_open() {
            return junie_tui::widgets::row_menu::hints();
        }
        let mut v = vec![];
        if focus == Some(TABS) {
            v.push(hint("←→", "Tab"));
            v.push(hint("1–5", "Jump"));
            v.push(hint("Enter", "Edit fields"));
        } else if let Some(b) = [&self.cancel, &self.save]
            .into_iter()
            .find(|b| Some(b.id) == focus)
        {
            v.push(hint("←→", "Choose"));
            v.push(b.hint());
        } else {
            match self.tab {
                EdTab::General => {
                    if focus == Some(NAME) {
                        v.push(if self.name.editing {
                            hint("Enter", "Apply")
                        } else {
                            junie_tui::widgets::widget_hints::EDIT
                        });
                    } else if self.fixed_at(focus) {
                    } else if focus == Some(WORKDIR) {
                        v.push(hint("Enter", "Choose…"));
                    } else if focus == Some(DOCKER) && w.surfaces.docker.is_some() {
                        v.extend(super::modals::docker_row_hints());
                    } else {
                        v.push(hint("Space", "Toggle"));
                    }
                    v.push(hint("Tab", "Next"));
                }
                EdTab::Roles => match self.role_rows.get(self.role_cursor) {
                    Some(RoleRow::Load) => v.push(hint("Enter", "Load role…")),
                    _ => {
                        v.push(hint("Space", "Allow"));
                        v.push(hint("Enter", "Set default"));
                        v.push(hint("a", "Load role…"));
                        v.push(hint("/", "Filter"));
                    }
                },
                EdTab::Accounts => match self.acct.rows.get(self.acct.cursor) {
                    Some(AcctRow::Account(_)) => {
                        if w.surfaces.fixed.is_none() {
                            v.push(hint("Space", "Enable / disable"));
                        }
                        v.push(hint("p", "Prefer"));
                        v.push(hint("/", "Filter"));
                        v.push(hint("c", "Manage accounts"));
                    }
                    _ => {
                        v.push(hint("↑↓", "Move"));
                        v.push(hint("c", "Manage accounts"));
                    }
                },
                t => v.extend(self.cfg.hints(t.cfg().unwrap(), focus, w)),
            }
        }
        v.push(hint("[ ]", "Switch tab"));
        v.push(hint("Ctrl+S", "Save"));
        v.push(hint("Esc", "Back"));
        v
    }

    fn crumb(&self, _w: &World) -> String {
        if self.is_create() {
            "Workspaces › new workspace › edit".into()
        } else {
            format!("Workspaces › {} › edit", self.original.name)
        }
    }

    fn stakes(&mut self, _w: &World) -> junie_tui::ui::leave::Stakes {
        self.sync_pending();
        let n = self.change_count();
        if n > 0 || self.is_create() {
            junie_tui::ui::leave::Stakes::Unsaved { changes: n }
        } else {
            junie_tui::ui::leave::Stakes::Clean
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

impl Screen for EditorScreen {
    fn layer_open(&self) -> bool {
        self.cfg.menu_hides_focus()
    }

    fn picker_items(&mut self, tag: &LayerTag, query: &str, w: &World) -> Option<Vec<Item>> {
        if tag.kind == "role.load" {
            let (items, targets) = self.role_items(w, query);
            self.picker_targets = targets;
            return Some(items);
        }
        if tag.kind == "cfg.role" {
            return Some(self.cfg.role_picker_items(query));
        }
        None
    }

    fn form_changed(&mut self, tag: &LayerTag, form: &mut super::modals::FormDialog, w: &World) {
        self.cfg.form_changed(tag, form, w);
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
        if n > 0 || self.is_create() {
            v.push(
                Segment::new(
                    format!(
                        "• {}{}",
                        plural(n.max(1), "change", "changes"),
                        if self.is_create() { " · new" } else { "" }
                    ),
                    Tone::Warning,
                )
                .priority(8),
            );
        }
        if let Some(s) = &self.row_status {
            v.push(Segment::new(s.clone(), Tone::Muted).priority(3));
        }
        v
    }

    fn request_leave(&mut self, _w: &mut World, cx: &mut Cx) -> Outcome {
        self.leave(cx);
        Outcome::Changed
    }
}

impl EditorScreen {
    fn click(&mut self, ev: &PointerEvent, w: &mut World, cx: &mut Cx) -> Outcome {
        let id = ev.id;
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
                self.open_preview(w, cx);
            }
            return Outcome::Changed;
        }
        match self.tab {
            EdTab::General => {
                if id == WORKDIR {
                    cx.focus.focus(WORKDIR);
                    if self.workdir_button.on_click() {
                        self.open_workdir_picker(w, cx);
                    }
                    return Outcome::Changed;
                }
                if id == KEEP_AWAKE {
                    cx.focus.focus(KEEP_AWAKE);
                    let o = self.keep_awake.on_click();
                    self.sync_pending();
                    return o;
                }
                if id == GIT_PULL {
                    cx.focus.focus(GIT_PULL);
                    let o = self.git_pull.on_click();
                    self.sync_pending();
                    return o;
                }
                if id == DOCKER {
                    cx.focus.focus(DOCKER);
                    return self.toggle_docker(w, cx);
                }
                Outcome::Ignored
            }
            EdTab::Accounts => {
                for i in 0..self.acct.rows.len() {
                    if ACCOUNTS.child(i) == id {
                        let same = self.acct.cursor == i;
                        self.acct.cursor = i;
                        cx.focus.focus(ACCOUNTS);
                        if same {
                            let k = Key {
                                code: KeyCode::Char(' '),
                                mods: ratatui::crossterm::event::KeyModifiers::NONE,
                            };
                            return self.accounts_key(&k, w, cx);
                        }
                        return Outcome::Changed;
                    }
                }
                Outcome::Ignored
            }
            EdTab::Roles => {
                for i in 0..self.role_rows.len() {
                    if ROLES.child(i) == id {
                        let same = self.role_cursor == i;
                        self.role_cursor = i;
                        cx.focus.focus(ROLES);
                        if same {
                            let k = Key {
                                code: KeyCode::Char(' '),
                                mods: ratatui::crossterm::event::KeyModifiers::NONE,
                            };
                            return self.roles_key(&k, w, cx);
                        }
                        return Outcome::Changed;
                    }
                }
                Outcome::Ignored
            }
            t => {
                let o = self.cfg.pointer(t.cfg().unwrap(), ev, w, cx);
                if o.consumed() {
                    self.sync_pending();
                }
                o
            }
        }
    }
}
