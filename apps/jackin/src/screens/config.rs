use std::collections::{BTreeMap, HashSet};

use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::hit::Axis;
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::PointerEvent;
use junie_tui::core::refusal::raise;
use junie_tui::core::scroll::{ScrollState, TrackClick};
use junie_tui::theme::Tone;
use junie_tui::ui::ctx::{RenderCtx, fill, legible_on_fill};
use junie_tui::ui::master_detail::{MasterDetail, Mode};
use junie_tui::ui::text::{fit, width};
use junie_tui::widgets::choice::{Checkbox, RadioGroup};
use junie_tui::widgets::dialog::Dialog;
use junie_tui::widgets::input::TextInput;
use junie_tui::widgets::keyhint::{Hint, hint};
use junie_tui::widgets::notice::Notice;
use junie_tui::widgets::option_list::Item;
use junie_tui::widgets::panel::{Panel, PanelFocus};
use junie_tui::widgets::picker::Picker;
use junie_tui::widgets::props::Prop;
use junie_tui::widgets::scrollbar;
use junie_tui::widgets::select::Select;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};

use super::modals::{
    BrowserResult, ChoiceDialog, FieldKindW, FieldValue, FormDialog, FormField, FormValues, OpFlow,
};
use super::{Cx, LayerTag, ModalResult};
use crate::domain::agent::AuthMode;
use crate::domain::onepassword::OpReference;
use crate::domain::vocabulary::fill_in;
use crate::domain::workspace::{
    EnvValue, EnvVar, Isolation, Mount, MountKind, MountScope, MountSource, RoleName, Workspace,
    env_key_error, is_credential_reference, looks_secret, mask,
};
use crate::world::{GlobalConfig, World};
use junie_tui::ui::context::ContextAnchor;
use junie_tui::widgets::menu::MenuItem;
use junie_tui::widgets::row_menu::RowMenu;
use ratatui::crossterm::event::KeyModifiers;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Workspace,
    Global,
}

impl Scope {
    fn label(self) -> &'static str {
        match self {
            Scope::Workspace => "workspace",
            Scope::Global => "global",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Mounts,
    Environments,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Doc {
    pub mounts: Vec<Mount>,
    pub env: Vec<EnvVar>,
    pub role_env: BTreeMap<RoleName, Vec<EnvVar>>,
}

impl Doc {
    pub fn from_workspace(w: &Workspace) -> Self {
        Self {
            mounts: w.mounts.clone(),
            env: w.env.clone(),
            role_env: w.role_env.clone(),
        }
    }

    pub fn apply_to_workspace(&self, w: &mut Workspace) {
        w.mounts = self.mounts.clone();
        w.env = self.env.clone();
        w.role_env = self.role_env.clone();
    }

    pub fn from_global(g: &GlobalConfig) -> Self {
        Self {
            mounts: g.mounts.clone(),
            env: g.env.clone(),
            role_env: g.role_env.clone(),
        }
    }

    pub fn apply_to_global(&self, g: &mut GlobalConfig) {
        g.mounts = self.mounts.clone();
        g.env = self.env.clone();
        g.role_env = self.role_env.clone();
    }

    fn env_of(&self, role: Option<&str>) -> &[EnvVar] {
        match role {
            None => &self.env,
            Some(r) => self.role_env.get(r).map(Vec::as_slice).unwrap_or(&[]),
        }
    }

    fn env_of_mut(&mut self, role: Option<&str>) -> &mut Vec<EnvVar> {
        match role {
            None => &mut self.env,
            Some(r) => self.role_env.entry(r.to_owned()).or_default(),
        }
    }

    fn roles(&self) -> Vec<RoleName> {
        let mut v: Vec<RoleName> = self
            .role_env
            .iter()
            .filter(|(_, vars)| !vars.is_empty())
            .map(|(r, _)| r.clone())
            .collect();
        v.sort();
        v.dedup();
        v
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    None,
    Added,
    Modified,
    Removed,
}

impl Change {
    fn glyph(self) -> &'static str {
        match self {
            Change::None => " ",
            Change::Added => "+",
            Change::Modified => "•",
            Change::Removed => "−",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowKey {
    Mount(String),
    RemovedMount(String),
    AddMount,
    Section(Option<RoleName>),
    Env(Option<RoleName>, String),
    AddEnv(Option<RoleName>),

    RoleSummary,

    AddRoleOverride,
}

#[derive(Debug, Clone)]
struct Row {
    key: RowKey,
    change: Change,
    cells: Vec<(String, Tone)>,

    problem: Option<String>,
    faint: bool,
    header: bool,
    folded: Option<bool>,
    meta: String,
}

#[derive(Default)]
struct ListState {
    rows: Vec<Row>,
    cursor: usize,
    scroll: ScrollState,
    area: Rect,
}

#[derive(Debug, Clone)]
enum Editing {
    Mount {
        original: Option<String>,
    },
    Env {
        role: Option<RoleName>,
        original: Option<String>,
    },
}

pub struct ConfigTabs {
    pub scope: Scope,
    pub original: Doc,
    pub pending: Doc,
    removed_mounts: Vec<Mount>,
    mounts: ListState,
    envs: ListState,
    folded: HashSet<Option<RoleName>>,
    unmasked: HashSet<(Option<RoleName>, String)>,
    editing: Option<Editing>,
    op_ref: Option<OpReference>,

    roles: Vec<RoleName>,

    registry: Vec<RoleName>,

    role_targets: Vec<RoleName>,
    ids: Ids,

    pub menu: RowMenu<(Tab, RowKey), KeyCode>,

    md: MasterDetail,
}

#[derive(Clone, Copy)]
struct Ids {
    mounts: WidgetId,
    envs: WidgetId,
    form: WidgetId,
    card: WidgetId,
}

impl ConfigTabs {
    pub fn new(
        scope: Scope,
        original: Doc,
        pending: Doc,
        base: WidgetId,
        registry: Vec<RoleName>,
    ) -> Self {
        let mut roles = original.roles();
        for r in pending.roles() {
            if !roles.contains(&r) {
                roles.push(r);
            }
        }
        let mut registry = registry;
        for r in &roles {
            if !registry.contains(r) {
                registry.push(r.clone());
            }
        }
        Self {
            scope,
            original,
            pending,
            removed_mounts: vec![],
            mounts: ListState::default(),
            envs: ListState::default(),
            folded: HashSet::new(),
            unmasked: HashSet::new(),
            editing: None,
            op_ref: None,
            roles,
            registry,
            role_targets: vec![],
            ids: Ids {
                mounts: base.sub("mounts"),
                envs: base.sub("envs"),
                form: base.sub("form"),
                card: base.sub("card"),
            },
            menu: RowMenu::new(base.sub("menu")),

            md: MasterDetail::new(base.sub("envs"), base.sub("card"))
                .detail_share(0, 56, 56)
                .collapse_below(150),
        }
    }

    pub fn owns(&self, tab: Tab, focus: Option<WidgetId>) -> bool {
        focus == Some(self.list_id(tab))
            || (tab == Tab::Environments && focus == Some(self.ids.card))
    }

    fn has_card(&self, tab: Tab) -> bool {
        let st = self.state_ref(tab);
        tab == Tab::Environments
            && matches!(
                st.rows.get(st.cursor).map(|r| &r.key),
                Some(RowKey::Env(..))
            )
    }

    pub fn list_id(&self, tab: Tab) -> WidgetId {
        match tab {
            Tab::Mounts => self.ids.mounts,
            Tab::Environments => self.ids.envs,
        }
    }

    fn state(&mut self, tab: Tab) -> &mut ListState {
        match tab {
            Tab::Mounts => &mut self.mounts,
            Tab::Environments => &mut self.envs,
        }
    }

    fn state_ref(&self, tab: Tab) -> &ListState {
        match tab {
            Tab::Mounts => &self.mounts,
            Tab::Environments => &self.envs,
        }
    }

    pub fn tab_dirty(&self, tab: Tab) -> bool {
        match tab {
            Tab::Mounts => self.pending.mounts != self.original.mounts,
            Tab::Environments => {
                self.pending.env != self.original.env
                    || self.pending.role_env != self.original.role_env
            }
        }
    }

    pub fn tab_error(&self, tab: Tab) -> bool {
        match tab {
            Tab::Mounts => self.pending.mounts.iter().any(|m| {
                m.drift.is_some()
                    || m.running_isolated && self.original.mounts.iter().all(|o| o != m)
            }),
            Tab::Environments => self
                .pending
                .env
                .iter()
                .chain(self.pending.role_env.values().flatten())
                .any(|e| env_key_error(&e.key).is_some()),
        }
    }

    pub fn blockers(&self) -> Vec<String> {
        let mut v = vec![];
        for m in &self.pending.mounts {
            if let Some(d) = &m.drift {
                v.push(format!("{} · source drift: {d}", m.destination));
            }
            if m.running_isolated && self.original.mounts.iter().all(|o| o != m) {
                v.push(format!(
                    "{} · a running instance holds isolated state for this mount; eject it first",
                    m.destination
                ));
            }
        }
        v
    }

    pub fn clean_up(&mut self) {
        for m in &mut self.pending.mounts {
            m.drift = None;
            m.running_isolated = false;
        }
    }

    pub fn diff_lines(&self) -> Vec<String> {
        let mut v = vec![];
        for m in &self.pending.mounts {
            match self
                .original
                .mounts
                .iter()
                .find(|o| o.destination == m.destination)
            {
                None => v.push(format!(
                    "+ mount {}  {} · {} · {}",
                    m.destination,
                    m.mode_label(),
                    m.isolation.label().to_lowercase(),
                    m.source_label()
                )),
                Some(o) if o != m => v.push(format!(
                    "~ mount {}  {} · {} → {} · {}",
                    m.destination,
                    o.mode_label(),
                    o.isolation.label().to_lowercase(),
                    m.mode_label(),
                    m.isolation.label().to_lowercase()
                )),
                _ => {}
            }
        }
        for o in &self.original.mounts {
            if !self
                .pending
                .mounts
                .iter()
                .any(|m| m.destination == o.destination)
            {
                v.push(format!("− mount {}", o.destination));
            }
        }
        let scopes: Vec<Option<RoleName>> = std::iter::once(None)
            .chain(self.roles.iter().cloned().map(Some))
            .collect();
        for s in &scopes {
            let a = self.pending.env_of(s.as_deref());
            let b = self.original.env_of(s.as_deref());
            let sc = s
                .as_deref()
                .map(|r| format!(" (role {r})"))
                .unwrap_or_default();
            for e in a {
                match b.iter().find(|o| o.key == e.key) {
                    None => v.push(format!("+ env {}{sc}  {}", e.key, e.value.source_label())),
                    Some(o) if o != e => v.push(format!(
                        "~ env {}{sc}  {} → {}",
                        e.key,
                        o.value.source_label(),
                        e.value.source_label()
                    )),
                    _ => {}
                }
            }
            for o in b {
                if !a.iter().any(|e| e.key == o.key) {
                    v.push(format!("− env {}{sc}", o.key));
                }
            }
        }
        v
    }

    pub fn summary_facts(&self) -> Vec<Prop> {
        let mut v = vec![];
        let (ma, mm, mr) = counts(&self.pending.mounts, &self.original.mounts, |m| {
            m.destination.clone()
        });
        if ma + mm + mr > 0 {
            v.push(Prop::new(
                "Mounts",
                format!("{ma} added · {mm} modified · {mr} removed"),
            ));
        }
        let mut ea = 0;
        let mut em = 0;
        let mut er = 0;
        let scopes: Vec<Option<RoleName>> = std::iter::once(None)
            .chain(self.roles.iter().cloned().map(Some))
            .collect();
        for s in &scopes {
            let (a, m, r) = counts(
                self.pending.env_of(s.as_deref()),
                self.original.env_of(s.as_deref()),
                |e| e.key.clone(),
            );
            ea += a;
            em += m;
            er += r;
        }
        if ea + em + er > 0 {
            v.push(Prop::new(
                "Environments",
                format!("{ea} added · {em} modified · {er} removed"),
            ));
        }
        v
    }

    fn build_rows(&mut self, tab: Tab, w: &World) {
        let mut roles = self.original.roles();
        for r in self.pending.roles() {
            if !roles.contains(&r) {
                roles.push(r);
            }
        }
        self.roles = roles;
        let rows = match tab {
            Tab::Mounts => self.mount_rows(w),
            Tab::Environments => self.env_rows(),
        };
        let st = self.state(tab);
        st.rows = rows;
        if st.cursor >= st.rows.len() {
            st.cursor = st.rows.len().saturating_sub(1);
        }

        while st
            .rows
            .get(st.cursor)
            .is_some_and(|r| r.header && r.folded.is_none())
            && st.cursor + 1 < st.rows.len()
        {
            st.cursor += 1;
        }
        st.scroll.set_content(st.rows.len());
    }

    fn mount_rows(&self, w: &World) -> Vec<Row> {
        let mut rows = vec![];
        for m in &self.pending.mounts {
            let change = match self
                .original
                .mounts
                .iter()
                .find(|o| o.destination == m.destination)
            {
                None => Change::Added,
                Some(o) if o != m => Change::Modified,
                _ => Change::None,
            };
            let problem = if let Some(d) = &m.drift {
                Some(format!(
                    "{} · source drift: {d}. Re-choose the source or remove the mount before saving.",
                    m.destination
                ))
            } else if m.running_isolated && change != Change::None {
                Some(format!(
                    "{} · a running instance holds isolated state for this mount; eject it first.",
                    m.destination
                ))
            } else {
                None
            };
            let mut cells = vec![
                (m.destination.clone(), Tone::Normal),
                (m.mode_label().into(), Tone::Secondary),
                (m.isolation.label().to_lowercase(), Tone::Secondary),
                (
                    if matches!(m.source, MountSource::Git(_)) {
                        "git".into()
                    } else {
                        "host".into()
                    },
                    Tone::Muted,
                ),
                (w.tilde(m.source_label()), Tone::Muted),
            ];
            if self.scope == Scope::Global {
                cells.insert(1, (m.scope.label(), Tone::Muted));
            }
            rows.push(Row {
                key: RowKey::Mount(m.destination.clone()),
                change,
                cells,
                problem,
                faint: false,
                header: false,
                folded: None,
                meta: String::new(),
            });
        }
        for m in &self.removed_mounts {
            let mut cells = vec![
                (m.destination.clone(), Tone::Faint),
                (m.mode_label().into(), Tone::Faint),
                (m.isolation.label().to_lowercase(), Tone::Faint),
                (
                    if matches!(m.source, MountSource::Git(_)) {
                        "git".into()
                    } else {
                        "host".into()
                    },
                    Tone::Faint,
                ),
                (w.tilde(m.source_label()), Tone::Faint),
            ];
            if self.scope == Scope::Global {
                cells.insert(1, (m.scope.label(), Tone::Faint));
            }
            rows.push(Row {
                key: RowKey::RemovedMount(m.destination.clone()),
                change: Change::Removed,
                cells,
                problem: None,
                faint: true,
                header: false,
                folded: None,
                meta: "u restores".into(),
            });
        }
        rows.push(Row {
            key: RowKey::AddMount,
            change: Change::None,
            cells: vec![("+ Add mount".into(), Tone::Secondary)],
            problem: None,
            faint: false,
            header: false,
            folded: None,
            meta: String::new(),
        });
        rows
    }

    fn env_rows(&self) -> Vec<Row> {
        let mut rows = vec![];
        let scopes: Vec<Option<RoleName>> = std::iter::once(None)
            .chain(self.roles.iter().cloned().map(Some))
            .collect();
        let configured = self.roles.len();
        for s in scopes {
            let vars = self.pending.env_of(s.as_deref());
            let orig = self.original.env_of(s.as_deref());
            let folded = self.folded.contains(&s);
            let in_registry = s
                .as_deref()
                .is_none_or(|r| self.registry.iter().any(|x| x == r));
            let title = match &s {
                None => self
                    .scope
                    .label()
                    .to_owned()
                    .replace("workspace", "Workspace")
                    .replace("global", "Global"),
                Some(r) => format!("Role: {r}"),
            };
            let mut meta = junie_tui::ui::text::plural(vars.len(), "var", "vars");
            if !in_registry {
                meta = format!("not in registry · {meta}");
            }
            rows.push(Row {
                key: RowKey::Section(s.clone()),
                change: Change::None,
                cells: vec![(title, Tone::Normal)],
                problem: None,
                faint: false,
                header: true,
                folded: if s.is_some() { Some(folded) } else { None },
                meta,
            });
            if s.is_some() && folded {
                continue;
            }
            for e in vars {
                let change = match orig.iter().find(|o| o.key == e.key) {
                    None => Change::Added,
                    Some(o) if o != e => Change::Modified,
                    _ => Change::None,
                };
                let shown = self.unmasked.contains(&(s.clone(), e.key.clone()));
                let (value, source) = match &e.value {
                    EnvValue::Plain(v) => (
                        if shown { v.clone() } else { mask(v) },
                        if shown {
                            "plain · shown".to_owned()
                        } else {
                            "plain".to_owned()
                        },
                    ),
                    EnvValue::OnePassword(r) => {
                        (mask_len(16), format!("[op] {}", r.display_path()))
                    }
                    EnvValue::HostEnv(name) => (format!("${name}"), "host env".into()),
                };
                let mut cells = vec![
                    (e.key.clone(), Tone::Normal),
                    (value, Tone::Secondary),
                    (source, Tone::Muted),
                ];
                if self.scope == Scope::Global {
                    cells.insert(
                        1,
                        (
                            s.as_deref()
                                .map(|r| format!("role {r}"))
                                .unwrap_or("global".into()),
                            Tone::Muted,
                        ),
                    );
                }
                rows.push(Row {
                    key: RowKey::Env(s.clone(), e.key.clone()),
                    change,
                    cells,
                    problem: env_key_error(&e.key),
                    faint: false,
                    header: false,
                    folded: None,
                    meta: String::new(),
                });
            }
            for o in orig {
                if !vars.iter().any(|e| e.key == o.key) {
                    let mut cells = vec![
                        (o.key.clone(), Tone::Faint),
                        (mask_len(12), Tone::Faint),
                        (o.value.source_label().into(), Tone::Faint),
                    ];
                    if self.scope == Scope::Global {
                        cells.insert(
                            1,
                            (
                                s.as_deref()
                                    .map(|r| format!("role {r}"))
                                    .unwrap_or("global".into()),
                                Tone::Faint,
                            ),
                        );
                    }
                    rows.push(Row {
                        key: RowKey::Env(s.clone(), o.key.clone()),
                        change: Change::Removed,
                        cells,
                        problem: None,
                        faint: true,
                        header: false,
                        folded: None,
                        meta: "u restores".into(),
                    });
                }
            }
            rows.push(Row {
                key: RowKey::AddEnv(s.clone()),
                change: Change::None,
                cells: vec![(
                    match &s {
                        None => "+ Add environment variable".into(),
                        Some(r) => format!("+ Add {r} environment variable"),
                    },
                    Tone::Secondary,
                )],
                problem: None,
                faint: false,
                header: false,
                folded: None,
                meta: String::new(),
            });
            if s.is_none() {
                rows.push(Row {
                    key: RowKey::RoleSummary,
                    change: Change::None,
                    cells: vec![("Role overrides".into(), Tone::Normal)],
                    problem: None,
                    faint: false,
                    header: true,
                    folded: None,
                    meta: if configured == 0 {
                        format!("none · {} in the registry", self.registry.len())
                    } else {
                        format!(
                            "{} configured · {} in the registry",
                            configured,
                            self.registry.len()
                        )
                    },
                });
            }
        }
        rows.push(Row {
            key: RowKey::AddRoleOverride,
            change: Change::None,
            cells: vec![("+ Add role override…".into(), Tone::Secondary)],
            problem: None,
            faint: false,
            header: false,
            folded: None,
            meta: String::new(),
        });
        rows
    }

    pub fn render(
        &mut self,
        tab: Tab,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        w: &World,
    ) -> Option<String> {
        self.build_rows(tab, w);
        let t = ctx.theme;
        let bg = t.canvas;
        let id = self.list_id(tab);
        let focused = ctx.interaction.focused(id);
        let scope = self.scope;
        let has_card = self.has_card(tab);
        let (list_area, card_area) = if tab == Tab::Mounts {
            (area, None)
        } else {
            let l = self.md.layout(area, buf, ctx, bg);
            match l.mode {
                Mode::Drawer => {
                    if has_card {
                        self.render_inspector(tab, l.detail, buf, ctx, w);
                    }
                    return None;
                }
                Mode::Split => (
                    l.master,
                    Some(Rect {
                        height: l.detail.height.min(24),
                        ..l.detail
                    }),
                ),
                Mode::Master => (l.master, None),
            }
        };

        let header: Vec<&str> = match (tab, scope) {
            (Tab::Mounts, Scope::Workspace) => {
                vec!["Destination", "Mode", "Isolation", "Kind", "Source"]
            }
            (Tab::Mounts, Scope::Global) => vec![
                "Destination",
                "Scope",
                "Mode",
                "Isolation",
                "Kind",
                "Source",
            ],
            (Tab::Environments, Scope::Workspace) => vec!["Key", "Value", "Source"],
            (Tab::Environments, Scope::Global) => vec!["Key", "Scope", "Value", "Source"],
        };
        let widths = column_widths(tab, scope, list_area.width.saturating_sub(7));
        let hidden = widths.iter().filter(|w| **w == 0).count();
        let mut y = list_area.y;
        if tab == Tab::Mounts {
            let mut x = list_area.x + 6;
            for (i, h) in header.iter().enumerate() {
                if widths[i] == 0 {
                    continue;
                }
                buf.set_string(
                    x,
                    y,
                    junie_tui::ui::reveal::cut(Rect::new(x, y, widths[i], 1), h),
                    t.muted().bg(bg),
                );
                x += widths[i] + 2;
            }
            if hidden > 0 {
                let tag = format!("{hidden}›");
                buf.set_string(
                    list_area.right().saturating_sub(width(&tag) as u16),
                    y,
                    &tag,
                    t.faint().bg(bg),
                );
            }
            y += 1;
        }
        let blocker_rows = {
            let st = self.state_ref(tab);
            st.rows
                .iter()
                .filter_map(|r| r.problem.clone())
                .collect::<Vec<_>>()
        };
        let reserve = if blocker_rows.is_empty() {
            0
        } else {
            1 + blocker_rows.len().min(2) as u16 * 2
        };
        let body = Rect::new(
            list_area.x,
            y,
            list_area.width,
            list_area.bottom().saturating_sub(y + reserve),
        );
        let cursor = self.state_ref(tab).cursor;
        let st = self.state(tab);
        st.area = body;
        st.scroll.set_viewport(body.height as usize);
        st.scroll.follow(cursor);
        ctx.control(id, body, false);
        ctx.scrollable(id, body);
        let has_sb = st.scroll.overflows();
        let row_w = body.width.saturating_sub(u16::from(has_sb));
        let range = st.scroll.visible_range();
        let rows = st.rows.clone();
        let mut status = None;
        let mounts = self.mounts_fixed(tab, w);
        let role_env = self.role_env_fixed(tab, w);
        for (k, i) in range.enumerate() {
            let y = body.y + k as u16;
            let row = &rows[i];
            let rid = id.child(i);
            let mut s = ctx.state(rid);
            s.focused = focused && i == cursor;
            s.selected = i == cursor;
            let fixed_add = mounts && row.key == RowKey::AddMount
                || role_env && row.key == RowKey::AddRoleOverride;
            if fixed_add {
                s.fixed = true;
                s.disabled = true;
            }
            let style = if row.header {
                Style::new().bg(bg)
            } else {
                t.row(s, bg)
            };
            let rect = Rect::new(body.x, y, row_w, 1);
            fill(buf, rect, style);
            if row.header {
                let mut x = rect.x + 2;
                if let Some(f) = row.folded {
                    buf.set_string(x, y, if f { "▸" } else { "▾" }, t.secondary().bg(bg));
                    ctx.clickable(rid.sub("fold"), Rect::new(x, y, 2, 1));
                }
                x += 2;
                let title = &row.cells[0].0;
                buf.set_string(
                    x,
                    y,
                    title,
                    if s.selected {
                        t.primary().bg(bg).add_modifier(Modifier::BOLD)
                    } else {
                        t.secondary().bg(bg).add_modifier(Modifier::BOLD)
                    },
                );
                if s.selected {
                    buf.set_string(rect.x, y, t.gutter_symbol(s), t.gutter(s, bg, false));
                }
                let mw = width(&row.meta) as u16;
                if mw > 0 && rect.width > mw + width(title) as u16 + 8 {
                    let tone = if row.meta.starts_with("not in registry") {
                        t.warning
                    } else {
                        t.text_faint
                    };
                    buf.set_string(
                        rect.right().saturating_sub(mw + 1),
                        y,
                        &row.meta,
                        Style::new().fg(tone).bg(bg),
                    );
                }
                ctx.clickable(rid, rect);
                continue;
            }
            buf.set_string(
                rect.x,
                y,
                t.gutter_symbol(s),
                t.gutter(s, style.bg.unwrap_or(bg), false),
            );
            let sel_glyph = if s.selected { "›" } else { " " };
            buf.set_string(
                rect.x + 1,
                y,
                sel_glyph,
                style.fg(if s.focused {
                    t.accent
                } else {
                    t.text_secondary
                }),
            );
            let change_style = match row.change {
                Change::Added => style.fg(t.text_primary),
                Change::Modified => style.fg(t.warning),
                Change::Removed => style.fg(t.text_faint),
                Change::None => style,
            };
            buf.set_string(rect.x + 3, y, row.change.glyph(), change_style);
            let mut x = rect.x + 6;
            let is_add = matches!(
                row.key,
                RowKey::AddMount | RowKey::AddEnv(_) | RowKey::AddRoleOverride
            );
            if is_add {
                buf.set_string(
                    x,
                    y,
                    &row.cells[0].0,
                    if fixed_add {
                        style
                    } else {
                        style.fg(if s.focused {
                            t.text_primary
                        } else {
                            t.text_secondary
                        })
                    },
                );
            } else {
                for (ci, (text, tone)) in row.cells.iter().enumerate() {
                    let cw = widths.get(ci).copied().unwrap_or(0);
                    if cw == 0 {
                        continue;
                    }
                    let mut cs = style.fg(if row.faint {
                        t.text_faint
                    } else {
                        t.tone(*tone)
                    });
                    if s.selected && ci == 0 && !row.faint {
                        cs = cs.fg(t.text_primary).add_modifier(Modifier::BOLD);
                    }
                    if row.change == Change::Removed {
                        cs = cs.add_modifier(Modifier::CROSSED_OUT);
                    }

                    let cell = Rect::new(x, y, cw, 1);
                    let shown = junie_tui::ui::reveal::fit(ctx, cell, text, s.focused);
                    buf.set_string(x, y, fit(&shown, cw as usize), cs);
                    x += cw + 2;
                }
            }
            if row.problem.is_some() {
                buf.set_string(rect.right().saturating_sub(2), y, "!", style.fg(t.error));
            } else if !row.meta.is_empty() && s.selected {
                let mw = width(&row.meta) as u16;
                buf.set_string(
                    rect.right().saturating_sub(mw + 1),
                    y,
                    &row.meta,
                    style.fg(t.text_faint),
                );
            }

            legible_on_fill(buf, rect, t, bg);
            ctx.clickable(rid, rect);
            if s.selected {
                status = self.row_status(&row.key, hidden > 0, w);
            }
        }
        if let Some(note) = self.fixed_note(tab, w) {
            let (x, ny) = (body.x + 6, body.y + rows.len() as u16 + 1);
            if ny < body.bottom() {
                buf.set_string(
                    x,
                    ny,
                    junie_tui::ui::reveal::cut(Rect::new(x, ny, row_w.saturating_sub(6), 1), note),
                    t.faint().bg(bg),
                );
            }
        }
        let st = self.state(tab);
        if has_sb {
            scrollbar::render_with_fade(
                Rect::new(body.x, body.y, body.width, body.height),
                buf,
                ctx,
                id,
                &st.scroll,
                focused,
            );
        }
        if reserve > 0 {
            let mut y = body.bottom() + 1;
            for p in blocker_rows.iter().take(2) {
                if y >= list_area.bottom() {
                    break;
                }

                y += Notice::error(p.as_str()).render(
                    Rect::new(
                        list_area.x + 3,
                        y,
                        list_area.width.saturating_sub(3),
                        list_area.bottom() - y,
                    ),
                    buf,
                    ctx,
                    bg,
                );
            }
        }
        if let Some(card) = card_area {
            self.render_inspector(tab, card, buf, ctx, w);
        }
        if has_card {
            self.md.finish(ctx);
        }
        status
    }

    fn row_status(&self, key: &RowKey, hidden: bool, w: &World) -> Option<String> {
        match key {
            RowKey::Mount(d) => {
                let m = self.pending.mounts.iter().find(|m| &m.destination == d)?;
                let o = self.original.mounts.iter().find(|m| &m.destination == d);
                match o {
                    Some(o) if o != m => Some(format!(
                        "was {} · {}{}",
                        o.mode_label(),
                        o.isolation.label().to_lowercase(),
                        if o.source != m.source {
                            format!(" · {}", w.tilde(o.source_label()))
                        } else {
                            String::new()
                        }
                    )),
                    None => Some("new mount".into()),
                    _ if hidden => Some(format!("source {}", w.tilde(m.source_label()))),
                    _ => None,
                }
            }
            RowKey::Env(role, k) => {
                let o = self
                    .original
                    .env_of(role.as_deref())
                    .iter()
                    .find(|e| &e.key == k);
                let p = self
                    .pending
                    .env_of(role.as_deref())
                    .iter()
                    .find(|e| &e.key == k);
                match (o, p) {
                    (Some(o), Some(p)) if o != p => Some(format!("was {}", o.value.source_label())),
                    (None, Some(_)) => Some("new variable".into()),
                    (Some(_), None) => Some("removed · u restores".into()),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn render_inspector(
        &mut self,
        tab: Tab,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        w: &World,
    ) {
        let t = ctx.theme;
        let st = self.state_ref(tab);
        let Some(row) = st.rows.get(st.cursor) else {
            return;
        };
        let (title, meta, lines): (String, String, Vec<(String, String, Tone)>) = match &row.key {
            RowKey::Env(role, key) => {
                let p = self
                    .pending
                    .env_of(role.as_deref())
                    .iter()
                    .find(|e| &e.key == key);
                let o = self
                    .original
                    .env_of(role.as_deref())
                    .iter()
                    .find(|e| &e.key == key);
                let mut lines = vec![];
                lines.push((
                    "Pending".into(),
                    p.map(|e| e.value.source_label().to_owned())
                        .unwrap_or("removed".into()),
                    Tone::Normal,
                ));
                lines.push((
                    "Original".into(),
                    o.map(|e| e.value.source_label().to_owned())
                        .unwrap_or("new".into()),
                    Tone::Muted,
                ));
                if let Some(EnvValue::OnePassword(r)) = p.map(|e| &e.value) {
                    lines.push(("Reference".into(), r.canonical(), Tone::Muted));
                    lines.push((
                        "Vault".into(),
                        format!("{} · {}", r.account, r.vault_name),
                        Tone::Muted,
                    ));
                }
                lines.push((
                    "Resolution".into(),
                    format!(
                        "resolved at launch · never stored in {} image",
                        w.surfaces.vocabulary.construct
                    ),
                    Tone::Faint,
                ));
                (
                    format!("Variable · {key}"),
                    role.as_deref()
                        .map(|r| format!("role {r}"))
                        .unwrap_or(self.scope.label().into()),
                    lines,
                )
            }
            _ => return,
        };

        let focused = ctx.interaction.focused(self.ids.card);
        ctx.control(self.ids.card, area, false);
        let inner = Panel::card(Some(&title))
            .meta(&meta)
            .focused(focused)
            .focus_mark(PanelFocus::Bar)
            .render(area, buf, t);
        let bg = t.surface;
        let lw = 14u16;
        for (i, (k, v, tone)) in lines.into_iter().enumerate() {
            let y = inner.y + i as u16;
            if y >= inner.bottom() {
                break;
            }
            buf.set_string(
                inner.x,
                y,
                junie_tui::ui::reveal::cut(Rect::new(inner.x, y, lw, 1), &k),
                t.muted().bg(bg),
            );
            buf.set_string(
                inner.x + lw,
                y,
                junie_tui::ui::reveal::cut(
                    Rect::new(inner.x + lw, y, inner.width.saturating_sub(lw), 1),
                    &v,
                ),
                Style::new().fg(t.tone(tone)).bg(bg),
            );
        }
    }

    pub fn hints(&self, tab: Tab, focus: Option<WidgetId>, w: &World) -> Vec<Hint> {
        if tab == Tab::Environments && focus == Some(self.ids.card) {
            return self.md.hints(focus);
        }
        let st = self.state_ref(tab);
        let key = st.rows.get(st.cursor).map(|r| r.key.clone());
        let mut v = vec![];
        let mounts = self.mounts_fixed(tab, w);
        let fixed = w.surfaces.fixed.is_some();
        match (tab, key) {
            (Tab::Mounts, _) if mounts => {}
            (Tab::Mounts, Some(RowKey::Mount(_))) => {
                v.push(hint("Enter", "Edit…"));
                v.push(hint("r", "Read-only"));
                v.push(hint("i", "Isolation"));
                v.push(hint("o", "Open source"));
                v.push(hint("d", "Remove"));
                if self.scope == Scope::Global {
                    v.push(hint("s", "Scope…"));
                }
                v.push(hint("a", "Add mount…"));
            }
            (Tab::Mounts, Some(RowKey::RemovedMount(_))) => {
                v.push(hint("u", "Undo remove"));
                v.push(hint("a", "Add mount…"));
            }
            (Tab::Mounts, _) => v.push(hint("Enter", "Add mount…")),
            (Tab::Environments, Some(RowKey::Env(..))) => {
                v.push(junie_tui::widgets::widget_hints::EDIT);
                v.push(hint("m", "Show"));
                if !fixed {
                    v.push(hint("p", "1Password…"));
                }
                if !(fixed && self.scope == Scope::Global) {
                    v.push(hint("s", "Scope…"));
                }
                v.push(hint("d", "Remove…"));
                v.push(hint("a", "Add…"));
            }
            (Tab::Environments, Some(RowKey::Section(Some(_)))) => {
                v.push(hint("←→", "Fold role"));
                v.push(hint("a", "Add…"));
            }
            (Tab::Environments, Some(RowKey::AddRoleOverride | RowKey::RoleSummary))
                if self.role_env_fixed(tab, w) => {}
            (Tab::Environments, Some(RowKey::AddRoleOverride | RowKey::RoleSummary)) => {
                v.push(hint("Enter", "Add role override…"));
            }
            (Tab::Environments, _) => {
                v.push(hint("Enter", "Add…"));
            }
        }
        v
    }

    fn move_cursor(&mut self, tab: Tab, delta: isize) {
        let st = self.state(tab);
        let n = st.rows.len();
        if n == 0 {
            return;
        }
        let mut i = st.cursor as isize;
        loop {
            i = (i + delta).clamp(0, n as isize - 1);
            let r = &st.rows[i as usize];
            if r.header && r.folded.is_none() {
                if i == 0 && delta < 0 || i == n as isize - 1 && delta > 0 {
                    break;
                }
                continue;
            }
            break;
        }
        st.cursor = i as usize;
        st.scroll.ensure_visible(st.cursor);
    }

    pub fn on_key(&mut self, tab: Tab, key: &Key, w: &mut World, cx: &mut Cx) -> Outcome {
        if tab == Tab::Environments {
            if let Some(o) = self.md.on_key(key, cx.focus) {
                return o;
            }
            if cx.focus.current() == Some(self.ids.card) {
                return Outcome::Ignored;
            }
        }
        self.build_rows(tab, w);
        if self.menu.is_context_key(key) {
            let st = self.state_ref(tab);
            let i = st.cursor;
            if i < st.rows.len() {
                let dy = i.saturating_sub(st.scroll.offset) as u16;
                let fallback = Rect::new(
                    st.area.x,
                    st.area.y + dy.min(st.area.height.saturating_sub(1)),
                    st.area.width.max(1),
                    1,
                );
                let anchor = ContextAnchor::Control {
                    id: self.list_id(tab).child(i),
                    fallback,
                };
                self.open_menu(tab, i, anchor, w);
            }
            return Outcome::Changed;
        }
        let st = self.state_ref(tab);
        let Some(row) = st.rows.get(st.cursor).cloned() else {
            return Outcome::Ignored;
        };
        let vp = st.scroll.viewport_len.max(1) as isize;
        let n = st.rows.len() as isize;
        if let Some(f) = &w.surfaces.fixed {
            if self.mounts_fixed(tab, w) && Self::edits_project(&row.key, key) {
                raise(&f.mounts);
                return Outcome::Consumed;
            }
            if self.role_env_fixed(tab, w) && Self::adds_role_override(&row.key, key) {
                raise(&f.role_env);
                return Outcome::Consumed;
            }
            if tab == Tab::Environments
                && matches!(row.key, RowKey::Env(..))
                && key.code == KeyCode::Char('p')
                && key.plain()
            {
                raise(&f.env_sources);
                return Outcome::Consumed;
            }
        }
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_cursor(tab, -1);
                return Outcome::Changed;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_cursor(tab, 1);
                return Outcome::Changed;
            }
            KeyCode::PageUp => {
                self.move_cursor(tab, -vp);
                return Outcome::Changed;
            }
            KeyCode::PageDown => {
                self.move_cursor(tab, vp);
                return Outcome::Changed;
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.move_cursor(tab, -n);
                return Outcome::Changed;
            }
            KeyCode::End | KeyCode::Char('G') => {
                self.move_cursor(tab, n);
                return Outcome::Changed;
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Char('h') | KeyCode::Char('l') => {
                if let RowKey::Section(Some(r)) = &row.key {
                    let s = Some(r.clone());
                    if matches!(key.code, KeyCode::Left | KeyCode::Char('h')) {
                        self.folded.insert(s);
                    } else {
                        self.folded.remove(&s);
                    }
                    return Outcome::Changed;
                }
                return Outcome::Ignored;
            }
            KeyCode::Char('a') if key.plain() => {
                let role = match &row.key {
                    RowKey::Section(r) | RowKey::Env(r, _) | RowKey::AddEnv(r) => r.clone(),
                    RowKey::RoleSummary | RowKey::AddRoleOverride => {
                        self.open_role_picker(cx);
                        return Outcome::Changed;
                    }
                    _ => None,
                };
                match tab {
                    Tab::Mounts => self.open_mount_form(None, w, cx),
                    Tab::Environments => self.open_env_form(role, None, w, cx),
                }
                return Outcome::Changed;
            }
            KeyCode::Char('u') if key.plain() => {
                return self.undo(tab, &row.key, w, cx);
            }
            _ => {}
        }
        match (&row.key, key.code) {
            (RowKey::AddMount, KeyCode::Enter) => self.open_mount_form(None, w, cx),
            (RowKey::AddEnv(r), KeyCode::Enter) => self.open_env_form(r.clone(), None, w, cx),
            (RowKey::AddRoleOverride | RowKey::RoleSummary, KeyCode::Enter) => {
                self.open_role_picker(cx)
            }
            (RowKey::Section(Some(r)), KeyCode::Enter | KeyCode::Char(' ')) => {
                let s = Some(r.clone());
                if !self.folded.remove(&s) {
                    self.folded.insert(s);
                }
            }
            (RowKey::Mount(d), KeyCode::Enter | KeyCode::Char('e') | KeyCode::Char('n')) => {
                self.open_mount_form(Some(d.clone()), w, cx)
            }
            (RowKey::Mount(d), KeyCode::Char('r')) => {
                if let Some(m) = self.pending.mounts.iter_mut().find(|m| &m.destination == d) {
                    m.readonly = !m.readonly;
                    cx.status(format!(
                        "{} · {}",
                        m.destination,
                        if m.readonly {
                            "read-only"
                        } else {
                            "read-write"
                        }
                    ));
                }
            }
            (RowKey::Mount(d), KeyCode::Char('i')) => {
                if let Some(m) = self.pending.mounts.iter_mut().find(|m| &m.destination == d) {
                    if m.running_isolated {
                        cx.error(format!("Cannot change isolation: a running instance holds isolated state for {}", m.destination));
                    } else {
                        m.isolation = m.isolation.next();
                        cx.status(format!(
                            "{} · isolation {}",
                            m.destination,
                            m.isolation.label().to_lowercase()
                        ));
                    }
                }
            }
            (RowKey::Mount(d), KeyCode::Char(c @ ('1' | '2' | '3'))) => {
                if let Some(m) = self.pending.mounts.iter_mut().find(|m| &m.destination == d) {
                    m.isolation = match c {
                        '1' => Isolation::Shared,
                        '2' => Isolation::Worktree,
                        _ => Isolation::Clone,
                    };
                    cx.status(format!(
                        "{} · isolation {}",
                        m.destination,
                        m.isolation.label().to_lowercase()
                    ));
                }
            }
            (RowKey::Mount(d), KeyCode::Char('o')) => {
                if let Some(m) = self.pending.mounts.iter().find(|m| &m.destination == d) {
                    match &m.source {
                        MountSource::Git(u) => cx.status(fill_in(
                            &w.surfaces.vocabulary.github_git_mount,
                            &[("url", u.as_str())],
                        )),
                        MountSource::Host(p) if m.kind == MountKind::Repository => {
                            match &w.surfaces.vocabulary.github_repo_mount {
                                Some(text) => {
                                    cx.status(fill_in(text, &[("url", w.tilde(p).as_str())]));
                                }
                                None => cx.error("no GitHub URL for this mount"),
                            }
                        }
                        _ => cx.error("no GitHub URL for this mount"),
                    }
                }
            }
            (RowKey::Mount(d), KeyCode::Char('d') | KeyCode::Delete) => {
                if let Some(i) = self.pending.mounts.iter().position(|m| &m.destination == d) {
                    let m = self.pending.mounts.remove(i);
                    if self
                        .original
                        .mounts
                        .iter()
                        .any(|o| o.destination == m.destination)
                    {
                        self.removed_mounts.push(m.clone());
                    }
                    cx.status(format!("Removed {} · u restores until save", m.destination));
                }
            }
            (RowKey::Mount(d), KeyCode::Char('s')) if self.scope == Scope::Global => {
                self.open_scope_picker(RowKey::Mount(d.clone()), w, cx);
            }
            (RowKey::RemovedMount(_), KeyCode::Enter) => return self.undo(tab, &row.key, w, cx),
            (RowKey::Env(r, k), KeyCode::Enter | KeyCode::Char('e')) => {
                if row.change == Change::Removed {
                    return self.undo(tab, &row.key, w, cx);
                }
                self.open_env_form(r.clone(), Some(k.clone()), w, cx);
            }
            (RowKey::Env(r, k), KeyCode::Char('m')) => {
                let id = (r.clone(), k.clone());
                let plain = self
                    .pending
                    .env_of(r.as_deref())
                    .iter()
                    .any(|e| &e.key == k && matches!(e.value, EnvValue::Plain(_)));
                if !plain {
                    cx.status("Only plain values can be shown · references resolve at launch");
                } else if !self.unmasked.remove(&id) {
                    self.unmasked.insert(id);
                    cx.status(format!("{k} shown · m masks it again"));
                }
            }
            (RowKey::Env(r, k), KeyCode::Char('p')) => {
                self.editing = Some(Editing::Env {
                    role: r.clone(),
                    original: Some(k.clone()),
                });
                let flow = OpFlow::new(self.ids.form.sub("op"), w.op.as_ref());
                cx.open(flow, LayerTag::new("cfg.env.op"));
            }
            (RowKey::Env(r, k), KeyCode::Char('s')) => {
                self.open_scope_picker(RowKey::Env(r.clone(), k.clone()), w, cx);
            }
            (RowKey::Env(r, k), KeyCode::Char('d') | KeyCode::Delete) => {
                if row.change == Change::Removed {
                    return Outcome::Consumed;
                }
                let d = Dialog::destructive(
                    self.ids.form.sub("envrm"),
                    &format!("Delete environment variable {k}?"),
                    &format!(
                        "The variable leaves the {} scope on save. Any 1Password reference stays in the vault.",
                        r.as_deref()
                            .map(|x| format!("role {x}"))
                            .unwrap_or(self.scope.label().into())
                    ),
                    "Delete",
                );
                cx.open(
                    d,
                    LayerTag::new("cfg.env.remove").key(format!(
                        "{}\u{1}{}",
                        r.clone().unwrap_or_default(),
                        k
                    )),
                );
            }
            _ => return Outcome::Ignored,
        }
        Outcome::Changed
    }

    fn mounts_fixed(&self, tab: Tab, w: &World) -> bool {
        tab == Tab::Mounts && w.surfaces.fixed.is_some()
    }

    fn role_env_fixed(&self, tab: Tab, w: &World) -> bool {
        tab == Tab::Environments && self.scope == Scope::Global && w.surfaces.fixed.is_some()
    }

    fn adds_role_override(row: &RowKey, key: &Key) -> bool {
        match (row, key.code) {
            (RowKey::RoleSummary | RowKey::AddRoleOverride, KeyCode::Char('a')) => key.plain(),
            (RowKey::RoleSummary | RowKey::AddRoleOverride, KeyCode::Enter) => true,
            (RowKey::Env(None, _), KeyCode::Char('s')) => key.plain(),
            _ => false,
        }
    }

    fn fixed_note<'a>(&self, tab: Tab, w: &'a World) -> Option<&'a str> {
        let f = w.surfaces.fixed.as_ref()?;
        if self.mounts_fixed(tab, w) {
            Some(&f.mounts.text)
        } else if self.role_env_fixed(tab, w) {
            Some(&f.role_env.text)
        } else {
            None
        }
    }

    fn edits_project(row: &RowKey, key: &Key) -> bool {
        match (row, key.code) {
            (RowKey::Mount(_), KeyCode::Char('a' | 'u'))
            | (RowKey::AddMount, KeyCode::Char('a')) => key.plain(),
            (RowKey::Mount(_), KeyCode::Enter | KeyCode::Delete)
            | (RowKey::AddMount, KeyCode::Enter) => true,
            (RowKey::Mount(_), KeyCode::Char(c)) => {
                matches!(c, 'e' | 'n' | 'r' | 'i' | '1' | '2' | '3' | 'd')
            }
            _ => false,
        }
    }

    fn row_title(row: &Row) -> String {
        match &row.key {
            RowKey::Mount(d) | RowKey::RemovedMount(d) => d.clone(),
            RowKey::Env(Some(r), k) => format!("{k} · role {r}"),
            RowKey::Env(None, k) => k.clone(),
            RowKey::Section(Some(r)) => format!("role {r}"),
            _ => row
                .cells
                .first()
                .map(|c| c.0.trim_start_matches("+ ").to_owned())
                .unwrap_or_default(),
        }
    }

    fn menu_items(&self, row: &Row) -> Vec<(MenuItem, KeyCode)> {
        let item = |label: &str, key: &'static str| MenuItem::new(label).shortcut(key);
        let add_env = (item("Add variable…", "a"), KeyCode::Char('a'));
        match &row.key {
            RowKey::Mount(d) => {
                let Some(m) = self.pending.mounts.iter().find(|m| &m.destination == d) else {
                    return vec![];
                };
                let o = self.original.mounts.iter().find(|o| &o.destination == d);
                let isolation = item("Next isolation", "i");
                let source = item("Open source", "o");
                let scope = item("Scope…", "s");
                let revert = item("Revert changes", "u");
                let has_url = matches!(m.source, MountSource::Git(_))
                    || matches!(m.source, MountSource::Host(_)) && m.kind == MountKind::Repository;
                vec![
                    (item("Edit…", "Enter"), KeyCode::Enter),
                    (
                        item(
                            if m.readonly {
                                "Make read-write"
                            } else {
                                "Make read-only"
                            },
                            "r",
                        ),
                        KeyCode::Char('r'),
                    ),
                    (
                        if m.running_isolated {
                            isolation.reason("a running instance holds isolated state")
                        } else {
                            isolation
                        },
                        KeyCode::Char('i'),
                    ),
                    (
                        if has_url {
                            source
                        } else {
                            source.reason("no GitHub URL for this mount")
                        },
                        KeyCode::Char('o'),
                    ),
                    (
                        if self.scope == Scope::Global {
                            scope
                        } else {
                            scope.reason("workspace mounts have no scope")
                        },
                        KeyCode::Char('s'),
                    ),
                    (
                        match o {
                            Some(o) if o != m => revert,
                            Some(_) => revert.reason("no changes"),
                            None => revert.reason("a new mount · remove it instead"),
                        }
                        .separator(),
                        KeyCode::Char('u'),
                    ),
                    (item("Add mount…", "a"), KeyCode::Char('a')),
                    (item("Remove", "d").danger(), KeyCode::Char('d')),
                ]
            }
            RowKey::RemovedMount(_) => vec![
                (item("Undo remove", "u"), KeyCode::Char('u')),
                (item("Add mount…", "a"), KeyCode::Char('a')),
            ],
            RowKey::AddMount => vec![(item("Add mount…", "Enter"), KeyCode::Enter)],
            RowKey::Env(..) if row.change == Change::Removed => {
                vec![(item("Restore", "u"), KeyCode::Char('u')), add_env]
            }
            RowKey::Env(r, k) => {
                let p = self
                    .pending
                    .env_of(r.as_deref())
                    .iter()
                    .find(|e| &e.key == k);
                let o = self
                    .original
                    .env_of(r.as_deref())
                    .iter()
                    .find(|e| &e.key == k);
                let plain = p.is_some_and(|e| matches!(e.value, EnvValue::Plain(_)));
                let shown = self.unmasked.contains(&(r.clone(), k.clone()));
                let show = item(if shown { "Mask value" } else { "Show value" }, "m");
                let revert = item("Revert changes", "u");
                vec![
                    (item("Edit…", "Enter"), KeyCode::Enter),
                    (
                        if plain {
                            show
                        } else {
                            show.reason("only plain values can be shown")
                        },
                        KeyCode::Char('m'),
                    ),
                    (item("1Password reference…", "p"), KeyCode::Char('p')),
                    (item("Scope…", "s"), KeyCode::Char('s')),
                    (
                        match (o, p) {
                            (Some(o), Some(p)) if o != p => revert,
                            (Some(_), _) => revert.reason("no changes"),
                            (None, _) => revert.reason("a new variable · remove it instead"),
                        }
                        .separator(),
                        KeyCode::Char('u'),
                    ),
                    add_env,
                    (item("Remove…", "d").danger(), KeyCode::Char('d')),
                ]
            }
            RowKey::Section(Some(r)) => vec![
                (
                    item(
                        if self.folded.contains(&Some(r.clone())) {
                            "Expand"
                        } else {
                            "Collapse"
                        },
                        "Enter",
                    ),
                    KeyCode::Enter,
                ),
                add_env,
            ],
            RowKey::Section(None) => vec![add_env],
            RowKey::AddEnv(_) => vec![(item("Add variable…", "Enter"), KeyCode::Enter)],
            RowKey::RoleSummary | RowKey::AddRoleOverride => {
                vec![(item("Add role override…", "Enter"), KeyCode::Enter)]
            }
        }
    }

    fn open_menu(&mut self, tab: Tab, i: usize, anchor: ContextAnchor, _w: &World) {
        let Some(row) = self.state_ref(tab).rows.get(i).cloned() else {
            return;
        };
        let items = self.menu_items(&row);
        if !items.is_empty() {
            self.menu
                .open((tab, row.key.clone()), anchor, Self::row_title(&row), items);
        }
    }

    pub fn menu_open(&self) -> bool {
        self.menu.is_open()
    }

    pub fn menu_hides_focus(&self) -> bool {
        self.menu.hides_page_focus()
    }

    pub fn menu_key(&mut self, key: &Key, w: &mut World, cx: &mut Cx) -> Outcome {
        let (o, chosen) = self.menu.on_key(key);
        match chosen {
            Some(((tab, target), code)) => {
                self.run_menu(tab, target, code, w, cx).or(Outcome::Changed)
            }
            None => o,
        }
    }

    pub fn menu_click(&mut self, id: WidgetId, w: &mut World, cx: &mut Cx) -> Outcome {
        let (o, chosen) = self.menu.on_click(id);
        match chosen {
            Some(((tab, target), code)) => self.run_menu(tab, target, code, w, cx),
            None => o,
        }
    }

    fn run_menu(
        &mut self,
        tab: Tab,
        target: RowKey,
        code: KeyCode,
        w: &mut World,
        cx: &mut Cx,
    ) -> Outcome {
        self.build_rows(tab, w);
        let Some(i) = self
            .state_ref(tab)
            .rows
            .iter()
            .position(|r| r.key == target)
        else {
            return Outcome::Changed;
        };
        let prev = self.state_ref(tab).cursor;
        self.state(tab).cursor = i;
        let key = Key {
            code,
            mods: KeyModifiers::NONE,
        };
        let out = self.on_key(tab, &key, w, cx);

        if !self.menu.config.moves_cursor {
            self.state(tab).cursor = prev;
        }
        out.or(Outcome::Changed)
    }

    pub fn pointer(&mut self, tab: Tab, ev: &PointerEvent, w: &mut World, cx: &mut Cx) -> Outcome {
        let (id, pos) = (ev.id, ev.pos());
        if let Some((axis, delta)) = ev.wheel() {
            return match axis {
                Axis::Vertical => self.wheel(tab, id, delta),
                Axis::Horizontal => Outcome::Ignored,
            };
        }
        let list = self.list_id(tab);
        if let Some(o) = scrollbar::pointer(ev, list, &mut self.state(tab).scroll, TrackClick::Jump)
        {
            return o;
        }
        match ev.phase() {
            Some(Phase::Click) => self.click(tab, id, w, cx),
            Some(Phase::Secondary) => self.secondary(tab, id, pos, w, cx),
            _ => Outcome::Ignored,
        }
    }

    fn secondary(
        &mut self,
        tab: Tab,
        id: WidgetId,
        pos: Position,
        w: &mut World,
        cx: &mut Cx,
    ) -> Outcome {
        self.build_rows(tab, w);
        let list = self.list_id(tab);
        let Some(i) = self
            .state_ref(tab)
            .scroll
            .visible_range()
            .find(|&i| list.child(i) == id || list.child(i).sub("fold") == id)
        else {
            return Outcome::Ignored;
        };
        if self.menu.config.moves_cursor {
            self.state(tab).cursor = i;
        }
        cx.focus.focus(list);
        self.open_menu(tab, i, ContextAnchor::Pointer(pos), w);
        Outcome::Changed
    }

    pub fn render_menu(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        self.menu.render(screen, buf, ctx);
    }

    fn undo(&mut self, tab: Tab, key: &RowKey, _w: &World, cx: &mut Cx) -> Outcome {
        match (tab, key) {
            (Tab::Mounts, RowKey::RemovedMount(d)) => {
                if let Some(i) = self.removed_mounts.iter().position(|m| &m.destination == d) {
                    let m = self.removed_mounts.remove(i);
                    cx.status(format!("Restored {}", m.destination));
                    self.pending.mounts.push(m);
                }
                Outcome::Changed
            }
            (Tab::Mounts, RowKey::Mount(d)) => {
                if let Some(o) = self
                    .original
                    .mounts
                    .iter()
                    .find(|m| &m.destination == d)
                    .cloned()
                {
                    if let Some(m) = self.pending.mounts.iter_mut().find(|m| &m.destination == d) {
                        *m = o;
                    }
                    cx.status(format!("Reverted {d}"));
                } else {
                    cx.status("Nothing to undo");
                }
                Outcome::Changed
            }
            (Tab::Environments, RowKey::Env(r, k)) => {
                let o = self
                    .original
                    .env_of(r.as_deref())
                    .iter()
                    .find(|e| &e.key == k)
                    .cloned();
                let list = self.pending.env_of_mut(r.as_deref());
                match o {
                    Some(o) => {
                        if let Some(e) = list.iter_mut().find(|e| &e.key == k) {
                            *e = o;
                        } else {
                            list.push(o);
                        }
                        cx.status(format!("Restored {k}"));
                    }
                    None => cx.status("Nothing to undo"),
                }
                Outcome::Changed
            }
            _ => {
                cx.status("Nothing to undo");
                Outcome::Consumed
            }
        }
    }

    fn click(&mut self, tab: Tab, id: WidgetId, w: &mut World, cx: &mut Cx) -> Outcome {
        if id == self.ids.card {
            cx.focus.focus(id);
            return Outcome::Changed;
        }
        self.build_rows(tab, w);
        let list = self.list_id(tab);
        let st = self.state_ref(tab);
        for i in st.scroll.visible_range() {
            if list.child(i).sub("fold") == id {
                if let RowKey::Section(Some(r)) = &st.rows[i].key {
                    let s = Some(r.clone());
                    if !self.folded.remove(&s) {
                        self.folded.insert(s);
                    }
                }
                self.state(tab).cursor = i;
                cx.focus.focus(list);
                return Outcome::Changed;
            }
            if list.child(i) == id {
                let same = st.cursor == i;
                self.state(tab).cursor = i;
                cx.focus.focus(list);
                if same {
                    let key = Key {
                        code: KeyCode::Enter,
                        mods: ratatui::crossterm::event::KeyModifiers::NONE,
                    };
                    return self.on_key(tab, &key, w, cx);
                }
                return Outcome::Changed;
            }
        }
        Outcome::Ignored
    }

    fn wheel(&mut self, tab: Tab, id: WidgetId, delta: i32) -> Outcome {
        let list = self.list_id(tab);
        if id == list || id == scrollbar::id_for(list) {
            self.state(tab).scroll.scroll_by(delta as isize);
            return Outcome::Changed;
        }
        Outcome::Ignored
    }

    fn scope_options(&self, w: &World) -> Vec<String> {
        let mut v = vec![self.scope.label().to_owned()];
        if !(self.scope == Scope::Global && w.surfaces.fixed.is_some()) {
            v.extend(self.registry.iter().map(|r| format!("role {r}")));
        }
        v
    }

    fn scope_at(&self, i: usize) -> Option<RoleName> {
        if i == 0 {
            None
        } else {
            self.registry.get(i - 1).cloned()
        }
    }

    fn scope_index(&self, role: Option<&str>) -> usize {
        role.and_then(|r| self.registry.iter().position(|x| x == r))
            .map(|i| i + 1)
            .unwrap_or(0)
    }

    fn open_scope_picker(&mut self, key: RowKey, w: &World, cx: &mut Cx) {
        let opts = self.scope_options(w);
        let refs: Vec<&str> = opts.iter().map(String::as_str).collect();
        let (title, current) = match &key {
            RowKey::Mount(d) => (
                format!("Scope for {d}"),
                self.pending
                    .mounts
                    .iter()
                    .find(|m| &m.destination == d)
                    .map(|m| match &m.scope {
                        MountScope::Role(r) => self.scope_index(Some(r)),
                        _ => 0,
                    }),
            ),
            RowKey::Env(r, k) => (
                format!("Scope for {k}"),
                Some(self.scope_index(r.as_deref())),
            ),
            _ => return,
        };
        let d = ChoiceDialog::new(
            self.ids.form.sub("scope"),
            &title,
            "Scope",
            &refs,
            current.unwrap_or(0),
        )
        .width(54);
        let k = match &key {
            RowKey::Mount(d) => format!("m\u{1}{d}"),
            RowKey::Env(r, k) => format!("e\u{1}{}\u{1}{k}", r.clone().unwrap_or_default()),
            _ => String::new(),
        };
        cx.open(d, LayerTag::new("cfg.scope").key(&k));
    }

    fn open_role_picker(&mut self, cx: &mut Cx) {
        let mut p = Picker::new(self.ids.form.sub("rolepick"), "Add role override");
        p.set_width(72);
        p.scope = Some(format!("{} roles", self.registry.len()));
        let items = self.role_picker_items("");
        p.set_items(items)
            .unwrap_or_else(|error| p.report_item_error(error));
        cx.open(p, LayerTag::new("cfg.role"));
    }

    pub fn role_picker_items(&mut self, query: &str) -> Vec<Item> {
        let q = query.to_lowercase();
        let mut items = vec![];
        let mut targets = vec![];
        for r in &self.registry {
            if !q.is_empty() && !r.to_lowercase().contains(&q) {
                continue;
            }
            let n = self.pending.env_of(Some(r)).len();
            items.push(Item {
                tag: (if n > 0 { Some("configured") } else { None }).map(Into::into),
                ..Item::new(r.clone())
                    .detail(if n > 0 {
                        format!(
                            "{} · Enter adds another",
                            junie_tui::ui::text::plural(n, "override", "overrides")
                        )
                    } else {
                        "no overrides yet".into()
                    })
                    .glyph(if n > 0 { "▪" } else { "·" })
                    .group("roles")
            });
            targets.push(r.clone());
        }
        self.role_targets = targets;
        items
    }

    fn open_mount_form(&mut self, original: Option<String>, w: &World, cx: &mut Cx) {
        let m = original
            .as_ref()
            .and_then(|d| self.pending.mounts.iter().find(|m| &m.destination == d))
            .cloned();
        let title = match &m {
            Some(m) => format!("Edit mount · {}", m.destination),
            None => "New mount".into(),
        };
        let kind_idx = match m.as_ref().map(|m| &m.source) {
            Some(MountSource::Git(_)) => 1,
            _ => 0,
        };
        let iso_idx = match m.as_ref().map(|m| m.isolation) {
            Some(Isolation::Worktree) => 1,
            Some(Isolation::Clone) => 2,
            _ => 0,
        };
        let f = self.ids.form;
        let mut fields = vec![
            FormField::radio(
                "kind",
                RadioGroup::new(
                    f.sub("kind"),
                    "Source",
                    &["Host directory", "Git repository"],
                    kind_idx,
                ),
            ),
            FormField::chooser(
                "host",
                f.sub("browse"),
                "Host directory",
                &m.as_ref()
                    .map(|m| match &m.source {
                        MountSource::Host(p) => w.tilde(p),
                        _ => String::new(),
                    })
                    .filter(|s| !s.is_empty())
                    .unwrap_or("not chosen".into()),
                "Browse…",
            ),
            FormField::input(
                "git",
                TextInput::new(f.sub("git"), "Git URL")
                    .placeholder("github.com/org/repo")
                    .value(match m.as_ref().map(|m| &m.source) {
                        Some(MountSource::Git(u)) => u.as_str(),
                        _ => "",
                    }),
            )
            .hidden(),
            FormField::input(
                "dest",
                TextInput::new(f.sub("dest"), "Destination")
                    .required(true)
                    .placeholder("/workspace/name")
                    .help(&format!(
                        "Absolute path inside {}",
                        w.surfaces.vocabulary.construct
                    ))
                    .value(m.as_ref().map(|m| m.destination.as_str()).unwrap_or("")),
            ),
            FormField::check(
                "ro",
                Checkbox::new(
                    f.sub("ro"),
                    &w.surfaces.vocabulary.read_only_label,
                    m.as_ref().is_some_and(|m| m.readonly),
                ),
            ),
            FormField::select(
                "iso",
                Select::new(
                    f.sub("iso"),
                    "Isolation",
                    &[
                        "Shared · edits land on the host",
                        "Worktree · git worktree per instance",
                        "Clone · full copy per instance",
                    ],
                    iso_idx,
                ),
            ),
        ];
        if self.scope == Scope::Global {
            let opts = self.scope_options(w);
            let refs: Vec<&str> = opts.iter().map(String::as_str).collect();
            let idx = match m.as_ref().map(|m| &m.scope) {
                Some(MountScope::Role(r)) => self.scope_index(Some(r)),
                _ => 0,
            };
            fields.push(FormField::select(
                "scope",
                Select::new(f.sub("scope"), "Scope", &refs, idx),
            ));
        }
        let mut form = FormDialog::new(f, &title, fields)
            .width(66)
            .meta(if m.is_some() {
                "mount · edits"
            } else {
                "mount · new"
            })
            .keep_open_on_save();
        form.set_visible("git", kind_idx == 1);
        form.set_visible("host", kind_idx == 0);
        self.editing = Some(Editing::Mount { original });
        cx.open(form, LayerTag::new("cfg.mount"));
    }

    fn open_env_form(
        &mut self,
        role: Option<RoleName>,
        original: Option<String>,
        w: &World,
        cx: &mut Cx,
    ) {
        let e = original
            .as_ref()
            .and_then(|k| {
                self.pending
                    .env_of(role.as_deref())
                    .iter()
                    .find(|e| &e.key == k)
            })
            .cloned();
        let scope_word = role
            .as_deref()
            .map(|r| format!("role {r}"))
            .unwrap_or(self.scope.label().into());
        let title = match (&e, &role) {
            (Some(e), _) => format!("Edit {}", e.key),
            (None, None) => format!("New {} environment key", self.scope.label()),
            (None, Some(r)) => format!("New {r} environment key"),
        };
        let src_idx = match e.as_ref().map(|e| &e.value) {
            Some(EnvValue::OnePassword(_)) => 1,
            Some(EnvValue::HostEnv(_)) => 2,
            _ => 0,
        };
        let f = self.ids.form;
        self.op_ref = match e.as_ref().map(|e| &e.value) {
            Some(EnvValue::OnePassword(r)) => Some(r.clone()),
            _ => None,
        };
        let scope_opts = self.scope_options(w);
        let scope_refs: Vec<&str> = scope_opts.iter().map(String::as_str).collect();
        let mut sources = RadioGroup::new(
            f.sub("src"),
            "Value source",
            &[
                w.surfaces.vocabulary.env_plain_label.as_str(),
                w.surfaces.vocabulary.env_op_label.as_str(),
                "Host environment variable",
            ],
            src_idx,
        );
        if let Some(fx) = &w.surfaces.fixed {
            sources = sources
                .fixed_option(1, fx.env_sources.clone())
                .fixed_option(2, fx.env_sources.clone());
        }
        let mut fields = vec![
            FormField::input(
                "key",
                TextInput::new(f.sub("key"), "Key")
                    .required(true)
                    .required_message("Key is required")
                    .placeholder("DATABASE_URL")
                    .validator(env_key_error)
                    .value(e.as_ref().map(|e| e.key.as_str()).unwrap_or("")),
            ),
            FormField::radio("src", sources),
            FormField::input(
                "value",
                TextInput::new(
                    f.sub("value"),
                    &format!(
                        "Value for {}",
                        e.as_ref().map(|e| e.key.as_str()).unwrap_or("the key")
                    ),
                )
                .required(true)
                .masked()
                .reveal_tail(4)
                .placeholder(&w.surfaces.vocabulary.env_plain_placeholder)
                .help(
                    w.surfaces
                        .vocabulary
                        .env_plain_help
                        .as_deref()
                        .unwrap_or(""),
                )
                .value(match e.as_ref().map(|e| &e.value) {
                    Some(EnvValue::Plain(v)) => v.as_str(),
                    _ => "",
                }),
            ),
            FormField::chooser(
                "op",
                f.sub("op"),
                "1Password reference",
                &self
                    .op_ref
                    .as_ref()
                    .map(|r| r.display_path())
                    .unwrap_or("not chosen".into()),
                "Choose…",
            )
            .hidden(),
            FormField::input(
                "host",
                TextInput::new(f.sub("host"), "Host variable")
                    .placeholder("GH_TOKEN")
                    .help("Forwarded from the host shell at launch")
                    .value(match e.as_ref().map(|e| &e.value) {
                        Some(EnvValue::HostEnv(h)) => h.as_str(),
                        _ => "",
                    }),
            )
            .hidden(),
            FormField::select(
                "scope",
                Select::new(
                    f.sub("scope"),
                    "Scope",
                    &scope_refs,
                    self.scope_index(role.as_deref()),
                ),
            ),
        ];
        let _ = scope_word;
        if let Some(fx) = &w.surfaces.fixed {
            let key_at = fields
                .iter()
                .position(|x| x.name == "key")
                .map_or(0, |i| i + 1);
            let src_at = fields
                .iter()
                .position(|x| x.name == "src")
                .map_or(0, |i| i + 1);
            let secret = e.as_ref().is_some_and(|e| looks_secret(&e.key));
            let lines = fx
                .env_secret_note
                .iter()
                .map(|line| (line.clone(), Tone::Muted))
                .collect();
            let mut note = FormField::note("key_note", lines);
            note.visible = secret;
            fields.insert(
                src_at,
                FormField::note("src_note", vec![(fx.env_sources.text.clone(), Tone::Muted)]),
            );
            fields.insert(key_at, note);
        }
        if e.is_some()
            && let Some(fld) = fields.iter_mut().find(|x| x.name == "key")
            && let FieldKindW::Input(i) = &mut fld.kind
        {
            i.help = "Renaming replaces the row".into();
        }
        let mut form = FormDialog::new(f, &title, fields)
            .width(70)
            .meta("environment")
            .keep_open_on_save();
        form.set_visible("value", src_idx == 0);
        form.set_visible("op", src_idx == 1);
        form.set_visible("host", src_idx == 2);
        self.editing = Some(Editing::Env { role, original });
        let _ = w;
        cx.open(form, LayerTag::new("cfg.env"));
    }

    pub fn form_changed(&mut self, tag: &LayerTag, form: &mut FormDialog, w: &World) {
        match tag.kind {
            "cfg.mount" => {
                let kind = form.choice("kind");
                form.set_visible("git", kind == 1);
                form.set_visible("host", kind == 0);
                form.error = None;
            }
            "cfg.env" => {
                let src = form.choice("src");
                form.set_visible("value", src == 0);
                form.set_visible("op", src == 1);
                form.set_visible("host", src == 2);
                if w.surfaces.fixed.is_some() {
                    form.set_visible("key_note", looks_secret(&form.text("key")));
                }
                form.error = None;
            }
            _ => {}
        }
    }

    pub fn on_modal(
        &mut self,
        tag: &LayerTag,
        result: ModalResult,
        w: &mut World,
        cx: &mut Cx,
    ) -> Option<Outcome> {
        if !tag.kind.starts_with("cfg.") {
            return None;
        }
        let out = match (tag.kind, result) {
            ("cfg.mount", ModalResult::FormAction(name, values)) => {
                if name == "save" {
                    return Some(self.save_mount(values, w, cx));
                }
                if name == "choose:host" {
                    let b = super::modals::file_browser(
                        self.ids.form.sub("browser"),
                        "Choose the host directory",
                        &w.cwd,
                        false,
                        true,
                        w,
                    );
                    cx.open(b, LayerTag::new("cfg.mount.browse"));
                }
                Outcome::Changed
            }
            ("cfg.mount.browse", ModalResult::Browser(BrowserResult::Chosen { path, .. })) => {
                let tilde = w.tilde(&path);
                let base = path.rsplit('/').next().unwrap_or("mount").to_owned();
                cx.with_top::<FormDialog>(move |f| {
                    f.set_chooser("host", &tilde, None);
                    if f.text("dest").trim().is_empty() {
                        f.set_text("dest", &format!("/workspace/{base}"));
                    }
                });
                Outcome::Changed
            }
            ("cfg.mount.browse", ModalResult::Browser(BrowserResult::Url(url))) => {
                cx.with_top::<FormDialog>(move |f| {
                    if let Some(fld) = f.field_mut("kind")
                        && let FieldKindW::Radio(r) = &mut fld.kind
                    {
                        r.selected = 1;
                    }
                    f.set_visible("git", true);
                    f.set_visible("host", false);
                    f.set_text("git", &url);
                });
                Outcome::Changed
            }
            ("cfg.env", ModalResult::FormAction(name, values)) => match name.as_str() {
                "choose:op" => {
                    let flow = OpFlow::new(self.ids.form.sub("op"), w.op.as_ref());
                    cx.open(flow, LayerTag::new("cfg.env.opform"));
                    Outcome::Changed
                }
                "save" => self.save_env(values, w, cx),
                _ => Outcome::Changed,
            },
            ("cfg.env.opform", ModalResult::Op(Some(r))) => {
                let path = r.display_path();
                let masked =
                    w.op.describe(&r)
                        .map(|d| d.masked)
                        .unwrap_or("••••••••".into());
                self.op_ref = Some(r);
                cx.with_top::<FormDialog>(move |f| {
                    f.set_chooser(
                        "op",
                        &path,
                        Some(&format!("value {masked} · resolved at launch")),
                    )
                });
                Outcome::Changed
            }
            ("cfg.env.op", ModalResult::Op(Some(r))) => {
                if let Some(Editing::Env {
                    role,
                    original: Some(k),
                }) = self.editing.clone()
                    && let Some(e) = self
                        .pending
                        .env_of_mut(role.as_deref())
                        .iter_mut()
                        .find(|e| e.key == k)
                {
                    e.value = EnvValue::OnePassword(r.clone());
                    cx.status(format!("{k} now resolves from {}", r.display_path()));
                }
                self.editing = None;
                Outcome::Changed
            }
            (
                "cfg.env.remove",
                ModalResult::Dialog {
                    action: Some(1), ..
                },
            ) => {
                let (role, key) = tag
                    .key
                    .split_once('\u{1}')
                    .map(|(a, b)| {
                        (
                            if a.is_empty() {
                                None
                            } else {
                                Some(a.to_owned())
                            },
                            b.to_owned(),
                        )
                    })
                    .unwrap_or((None, tag.key.clone()));
                self.pending
                    .env_of_mut(role.as_deref())
                    .retain(|e| e.key != key);
                cx.status(format!("Removed {key} · u restores until save"));
                Outcome::Changed
            }
            ("cfg.role", ModalResult::Picked(i)) => {
                if let Some(role) = self.role_targets.get(i).cloned() {
                    self.open_env_form(Some(role), None, w, cx);
                }
                Outcome::Changed
            }
            ("cfg.scope", ModalResult::Choice(Some(i))) => {
                let scope = self.scope_at(i);
                let parts: Vec<&str> = tag.key.split('\u{1}').collect();
                match parts.as_slice() {
                    ["m", d] => {
                        if let Some(m) =
                            self.pending.mounts.iter_mut().find(|m| &m.destination == d)
                        {
                            m.scope = scope
                                .clone()
                                .map(MountScope::Role)
                                .unwrap_or(MountScope::Global);
                            cx.status(format!("{d} · scope {}", m.scope.label()));
                        }
                    }
                    ["e", r, k] => {
                        let from = if r.is_empty() {
                            None
                        } else {
                            Some((*r).to_owned())
                        };
                        if from != scope {
                            let list = self.pending.env_of_mut(from.as_deref());
                            if let Some(i) = list.iter().position(|e| &e.key == k) {
                                let e = list.remove(i);
                                let target = self.pending.env_of_mut(scope.as_deref());
                                target.retain(|x| x.key != e.key);
                                target.push(e);
                                cx.status(format!(
                                    "{k} moved to {}",
                                    scope
                                        .as_deref()
                                        .map(|r| format!("role {r}"))
                                        .unwrap_or(self.scope.label().into())
                                ));
                            }
                        }
                    }
                    _ => {}
                }
                Outcome::Changed
            }
            (_, ModalResult::Form(None)) => {
                self.editing = None;
                cx.status("Cancelled · nothing changed");
                Outcome::Changed
            }
            _ => Outcome::Changed,
        };
        Some(out)
    }

    fn save_mount(&mut self, values: FormValues, w: &World, cx: &mut Cx) -> Outcome {
        let dest = text(&values, "dest").trim().to_owned();
        if !dest.starts_with('/') {
            let construct = w.surfaces.vocabulary.construct.clone();
            cx.with_top::<FormDialog>(move |f| {
                f.error = Some(format!(
                    "Destination must be an absolute path inside {construct}"
                ))
            });
            return Outcome::Changed;
        }
        let kind = choice(&values, "kind");
        let source = if kind == 1 {
            let url = text(&values, "git")
                .trim()
                .trim_start_matches("https://")
                .to_owned();
            if url.is_empty() || !url.contains('/') {
                cx.with_top::<FormDialog>(|f| {
                    f.error = Some("Git URL is required (host/org/repo)".into())
                });
                return Outcome::Changed;
            }
            MountSource::Git(url)
        } else {
            let host = chooser_value(&values, "host");
            if host.is_empty() || host == "not chosen" {
                cx.with_top::<FormDialog>(|f| {
                    f.error = Some("Choose the host directory first".into())
                });
                return Outcome::Changed;
            }
            MountSource::Host(crate::world::expand(&w.home, &host))
        };
        let original = match &self.editing {
            Some(Editing::Mount { original }) => original.clone(),
            _ => None,
        };
        if self
            .pending
            .mounts
            .iter()
            .any(|m| m.destination == dest && Some(&m.destination) != original.as_ref())
        {
            cx.with_top::<FormDialog>(move |f| {
                f.error = Some(format!("{dest} is already a mount destination"))
            });
            return Outcome::Changed;
        }
        let iso = match choice(&values, "iso") {
            1 => Isolation::Worktree,
            2 => Isolation::Clone,
            _ => Isolation::Shared,
        };
        let scope = if self.scope == Scope::Global {
            self.scope_at(choice(&values, "scope"))
                .map(MountScope::Role)
                .unwrap_or(MountScope::Global)
        } else {
            MountScope::Workspace
        };
        let mut m = match original
            .as_ref()
            .and_then(|d| self.pending.mounts.iter().find(|m| &m.destination == d))
            .cloned()
        {
            Some(m) => m,
            None => Mount::host("", &dest),
        };
        let is_git = matches!(source, MountSource::Git(_));
        m.source = source;
        m.destination = dest.clone();
        m.readonly = checked(&values, "ro");
        m.isolation = iso;
        m.kind = if is_git || m.kind == MountKind::Repository {
            MountKind::Repository
        } else {
            MountKind::Directory
        };
        m.scope = scope;
        m.drift = None;
        match original {
            Some(d) => {
                if let Some(slot) = self.pending.mounts.iter_mut().find(|x| x.destination == d) {
                    *slot = m;
                }
                cx.status(format!("Mount {dest} updated · save to apply"));
            }
            None => {
                self.pending.mounts.push(m);
                cx.status(format!("Mount {dest} added · save to apply"));
            }
        }
        self.editing = None;
        let st = &mut self.mounts;
        st.cursor = self
            .pending
            .mounts
            .iter()
            .position(|m| m.destination == dest)
            .unwrap_or(0);
        cx.close();
        Outcome::Changed
    }

    fn save_env(&mut self, values: FormValues, w: &World, cx: &mut Cx) -> Outcome {
        let key = text(&values, "key").trim().to_owned();
        if let Some(e) = env_key_error(&key) {
            cx.with_top::<FormDialog>(move |f| f.error = Some(e));
            return Outcome::Changed;
        }
        let value = match choice(&values, "src") {
            0 => {
                let v = text(&values, "value");
                if v.is_empty() {
                    cx.with_top::<FormDialog>(|f| f.error = Some("Value cannot be empty".into()));
                    return Outcome::Changed;
                }
                if let Some(fx) = &w.surfaces.fixed
                    && is_credential_reference(&v)
                {
                    let e = fx.env_reference.clone();
                    cx.with_top::<FormDialog>(move |f| f.error = Some(e));
                    return Outcome::Changed;
                }
                EnvValue::Plain(v)
            }
            1 => match self.op_ref.clone() {
                Some(r) => EnvValue::OnePassword(r),
                None => {
                    cx.with_top::<FormDialog>(|f| {
                        f.error = Some("Choose a 1Password item and field first".into())
                    });
                    return Outcome::Changed;
                }
            },
            _ => {
                let h = text(&values, "host").trim().to_owned();
                if let Some(e) = env_key_error(&h) {
                    cx.with_top::<FormDialog>(move |f| {
                        f.error = Some(format!("Host variable: {e}"))
                    });
                    return Outcome::Changed;
                }
                EnvValue::HostEnv(h)
            }
        };
        let Some(Editing::Env {
            role: from,
            original,
        }) = self.editing.clone()
        else {
            return Outcome::Changed;
        };
        let role = self.scope_at(choice(&values, "scope"));
        let renamed_in_place = role == from;
        let taken = self
            .pending
            .env_of_mut(role.as_deref())
            .iter()
            .any(|e| e.key == key && !(renamed_in_place && original.as_ref() == Some(&e.key)));
        if taken {
            cx.with_top::<FormDialog>(move |f| {
                f.error = Some(format!("{key} already exists in this scope"))
            });
            return Outcome::Changed;
        }
        if let Some(o) = &original {
            self.pending
                .env_of_mut(from.as_deref())
                .retain(|e| &e.key != o);
        }
        self.pending.env_of_mut(role.as_deref()).push(EnvVar {
            key: key.clone(),
            value,
        });
        self.unmasked.remove(&(role.clone(), key.clone()));
        self.editing = None;
        self.op_ref = None;
        cx.status(format!(
            "{key} · {} · save to apply",
            if original.is_some() {
                "updated"
            } else {
                "added"
            }
        ));
        cx.close();
        Outcome::Changed
    }
}

fn counts<T: PartialEq>(
    pending: &[T],
    original: &[T],
    key: impl Fn(&T) -> String,
) -> (usize, usize, usize) {
    let mut added = 0;
    let mut modified = 0;
    for p in pending {
        match original.iter().find(|o| key(o) == key(p)) {
            None => added += 1,
            Some(o) if o != p => modified += 1,
            _ => {}
        }
    }
    let removed = original
        .iter()
        .filter(|o| !pending.iter().any(|p| key(p) == key(o)))
        .count();
    (added, modified, removed)
}

fn column_widths(tab: Tab, scope: Scope, avail: u16) -> Vec<u16> {
    match (tab, scope) {
        (Tab::Mounts, Scope::Workspace) => {
            let fixed = 4 + 9 + 6;
            if avail >= 90 {
                let rest = avail.saturating_sub(fixed + 8);
                let dest = (rest * 45 / 100).max(20);
                vec![dest, 4, 9, 6, rest.saturating_sub(dest)]
            } else if avail >= 60 {
                vec![avail.saturating_sub(fixed + 6), 4, 9, 6, 0]
            } else {
                vec![avail.saturating_sub(4 + 9 + 4), 4, 9, 0, 0]
            }
        }
        (Tab::Mounts, Scope::Global) => {
            let fixed = 14 + 4 + 9 + 6;
            if avail >= 100 {
                let rest = avail.saturating_sub(fixed + 10);
                let dest = (rest * 45 / 100).max(20);
                vec![dest, 14, 4, 9, 6, rest.saturating_sub(dest)]
            } else if avail >= 70 {
                vec![avail.saturating_sub(fixed + 8), 14, 4, 9, 6, 0]
            } else {
                vec![avail.saturating_sub(14 + 4 + 9 + 6), 14, 4, 9, 0, 0]
            }
        }
        (Tab::Environments, Scope::Workspace) => {
            let key = 18.min(avail / 3);
            let value = if avail >= 90 { 24 } else { 18 };
            vec![key, value, avail.saturating_sub(key + value + 4)]
        }
        (Tab::Environments, Scope::Global) => {
            let key = 18.min(avail / 3);
            let value = if avail >= 100 { 22 } else { 16 };
            vec![key, 14, value, avail.saturating_sub(key + 14 + value + 6)]
        }
    }
}

fn text(values: &FormValues, name: &str) -> String {
    values
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| match v {
            FieldValue::Text(s) => s.clone(),
            _ => String::new(),
        })
        .unwrap_or_default()
}

fn chooser_value(values: &FormValues, name: &str) -> String {
    values
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| match v {
            FieldValue::Text(s) => s.clone(),
            _ => String::new(),
        })
        .unwrap_or_default()
}

fn choice(values: &FormValues, name: &str) -> usize {
    values
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| match v {
            FieldValue::Choice(i) => *i,
            _ => 0,
        })
        .unwrap_or(0)
}

fn checked(values: &FormValues, name: &str) -> bool {
    values
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| matches!(v, FieldValue::Bool(true)))
        .unwrap_or(false)
}

pub fn mode_label(m: AuthMode) -> &'static str {
    match m {
        AuthMode::Sync => "sync",
        AuthMode::ApiKey => "api key",
        AuthMode::OAuthToken => "oauth token",
        AuthMode::Ignore => "ignore",
    }
}

fn mask_len(n: usize) -> String {
    "*".repeat(n)
}
