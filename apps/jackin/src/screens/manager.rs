use crate::screens::Jackin;
use std::collections::HashSet;
use termrock::shell::Page;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use termrock::core::event::{Key, Outcome};
use termrock::core::hit::Axis;
use termrock::core::id::{ItemKey, Part, PartRef, WidgetId};
use termrock::core::intent::Phase;
use termrock::core::pointer::PointerEvent;
use termrock::core::scroll::{ScrollState, TrackClick};
use termrock::core::selection::ClickPolicy;
use termrock::theme::{ButtonKind, Tone};
use termrock::ui::ctx::{RenderCtx, fill, legible_on_fill};
use termrock::ui::layout::{Split, SplitDir};
use termrock::ui::master_detail::{MasterDetail, Mode};
use termrock::ui::text::{fit, truncate_middle, width};
use termrock::widgets::button::{Button, row_layout, row_lines};
use termrock::widgets::dialog::Dialog;
use termrock::widgets::empty::{self, EmptyState};
use termrock::widgets::keyhint::{Hint, hint};
use termrock::widgets::option_list::Item;
use termrock::widgets::panel::{Panel, PanelFocus};
use termrock::widgets::picker::{Picker, PickerAction};
use termrock::widgets::progress::spinner_frame;
use termrock::widgets::props::{self, Prop};
use termrock::widgets::scrollbar;
use termrock::widgets::segments::Segment;
use termrock::widgets::splitter::Splitter;
use termrock::widgets::viewport::Span;

use super::modals::{InfoDialog, InfoResult};
use super::row_meta::{ClockStamp, Moment, RowMeta, clocks};
use super::{Cx, Go, LayerTag, ModalResult, Screen};
use crate::backend::Effect;
use crate::domain::account::AccountId;
use crate::domain::agent::Agent;
use crate::domain::instance::{
    DaemonSnapshot, Instance, InstanceEntry, InstanceId, InstanceStatus,
};
use crate::domain::precedence::PrecedenceLevel;
use crate::domain::role::LaunchRole;
use crate::domain::tuple::{AccountRequirement, InstanceChoice, TupleIdentity};
use crate::domain::vocabulary::fill_in;
use crate::domain::workspace::WorkspaceId;
use crate::world::{Health, LaunchSource, Msg, World, job};
use termrock::ui::context::ContextAnchor;
use termrock::ui::text::plural;
use termrock::widgets::menu::MenuItem;
use termrock::widgets::row_menu::RowMenu;

pub const TREE: WidgetId = WidgetId::of("manager.tree");
pub const DETAIL: WidgetId = WidgetId::of("manager.detail");
pub const ROSTER: WidgetId = WidgetId::of("manager.roster");

fn roster_row(instance: &InstanceId) -> PartRef {
    let key = instance.row_key();
    PartRef::item(Part::ROW, ItemKey::text(&key))
}

fn id_for_row_key(key: &str) -> Option<InstanceId> {
    InstanceId::from_row_key(key)
}
pub const SEAM: WidgetId = WidgetId::of("manager.seam");
pub const MENU: WidgetId = WidgetId::of("manager.menu");

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowKey {
    CurrentDir,
    Workspace(WorkspaceId),
    Instance(InstanceId),
    UnattributedSandboxes,
    NewWorkspace,
}

fn instance_mark(s: InstanceStatus) -> (&'static str, Tone) {
    match s {
        InstanceStatus::Crashed | InstanceStatus::FailedSetup => ("!", Tone::Error),
        s if s.is_live() => ("◉", Tone::Active),
        _ => ("◌", Tone::Muted),
    }
}

fn status_word_tone(s: InstanceStatus, otherwise: Tone) -> Tone {
    if s.is_live() { Tone::Active } else { otherwise }
}

fn instance_line(
    i: &crate::domain::instance::Instance,
    tone: Tone,
    words: &crate::domain::vocabulary::Vocabulary,
) -> Vec<Span> {
    let (g, gt) = instance_mark(i.status);
    vec![
        Span::new(format!("{g} "), gt),
        Span::new(
            format!(
                "{}  {} · {} · ",
                i.id.short_suffix(),
                i.role_label(),
                i.agent.label()
            ),
            tone,
        ),
        Span::new(
            words.status_label(i.status),
            status_word_tone(i.status, tone),
        ),
    ]
}

fn entry_line(
    entry: &InstanceEntry,
    tone: Tone,
    words: &crate::domain::vocabulary::Vocabulary,
) -> Vec<Span> {
    match entry {
        InstanceEntry::Available(instance) => instance_line(instance, tone, words),
        InstanceEntry::Unavailable(instance) => {
            let display = instance.display();
            vec![
                Span::new("? ", Tone::Warning),
                Span::new(
                    format!(
                        "{} · {} · {} · {}",
                        display.sandbox_name,
                        display.gateway,
                        instance.phase.label(),
                        display.reason
                    ),
                    tone,
                ),
            ]
        }
    }
}

#[derive(Debug, Clone)]
struct Row {
    key: RowKey,
    depth: u16,
    glyph: &'static str,
    glyph_tone: Tone,
    label: String,
    meta: RowMeta,
    meta_tone: Tone,
    trailing: Option<(&'static str, Tone)>,
    expandable: bool,
}

/// What the cached rows were built from.
struct RowsStamp {
    roster: u64,
    expanded: HashSet<WorkspaceId>,
    busy: Vec<(RowKey, &'static str)>,
    selected: RowKey,
}

/// The row of a sandbox the backend could not describe. `named`: the row sits
/// under the unattributed group, so its text says which workspace it claims.
fn unavailable_row(unavailable: &crate::domain::instance::UnavailableInstance, named: bool) -> Row {
    let display = unavailable.display();
    let owner = if named {
        format!("{} · ", display.workspace_name)
    } else {
        String::new()
    };
    let head = format!(
        "{} · {owner}{} · {} · ",
        display.gateway,
        display.role,
        unavailable.phase.label()
    );
    let tail = format!("metadata unavailable: {}", display.reason);
    let meta = match unavailable.created_secs {
        Some(secs) => RowMeta::timed(head, Moment::Ago(secs), format!(" · {tail}")),
        None => RowMeta::plain(format!("{head}creation time unavailable · {tail}")),
    };
    Row {
        key: RowKey::Instance(unavailable.id.clone()),
        depth: 1,
        glyph: "?",
        glyph_tone: Tone::Warning,
        label: display.sandbox_name,
        meta,
        meta_tone: Tone::Warning,
        trailing: None,
        expandable: false,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DetailRow {
    Pane { tab: usize, pane: u64, text: String },
    Text(String, Tone),
    Session(String),
    Blank,
}

/// What starts once the user has picked among the accounts an agent offers.
enum Start {
    Launch {
        workspace: Option<WorkspaceId>,
        role: String,
    },
    Session {
        instance: InstanceId,
    },
}

/// The account picker that is open, and what it starts.
struct Choosing {
    start: Start,
    tuple: TupleIdentity,
    accounts: Vec<AccountId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AccountSelection {
    Automatic {
        account: AccountId,
        level: PrecedenceLevel,
    },
    Explicit(AccountId),
    Accountless,
    Unselected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LaunchChoice {
    tuple: TupleIdentity,
    model: String,
    account: AccountSelection,
}

impl LaunchChoice {
    fn instance(&self) -> Option<InstanceChoice> {
        let account = match &self.account {
            AccountSelection::Automatic { account, .. } | AccountSelection::Explicit(account) => {
                Some(account.clone())
            }
            AccountSelection::Accountless => None,
            AccountSelection::Unselected => return None,
        };
        Some(InstanceChoice {
            agent: self.tuple.agent,
            provider: self.tuple.provider,
            account,
            model: self.model.clone(),
        })
    }
}

#[derive(Debug, Clone)]
struct LaunchPickerTarget {
    key: ItemKey,
    choice: LaunchChoice,
}

pub struct ManagerScreen {
    pub selected: RowKey,
    pub expanded: HashSet<WorkspaceId>,
    rows: Vec<Row>,
    stamp: Option<RowsStamp>,
    clock: ClockStamp,
    pub scroll: ScrollState,
    pub detail_scroll: ScrollState,
    pub detail_cursor: usize,
    detail_rows: Vec<DetailRow>,

    md: MasterDetail,
    tree_area: Rect,
    detail_area: Rect,
    actions: Vec<Button>,
    pub still_inside: Option<(String, Vec<String>, i64)>,
    pub busy_rows: Vec<(RowKey, &'static str)>,
    refresh_asked: bool,
    pending_launch: Option<(Option<WorkspaceId>, String)>,
    pending_session: Option<InstanceId>,
    choosing: Option<Choosing>,
    launch_picker_targets: Vec<LaunchPickerTarget>,
    session_picker_targets: Vec<(Agent, Option<String>)>,
    launch_queue: Vec<LaunchChoice>,
    launch_chaining: bool,
    launch_collected: Vec<LaunchChoice>,
    wide: bool,

    pub menu: RowMenu<RowKey, &'static str>,
}

impl Default for ManagerScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl ManagerScreen {
    pub fn new() -> Self {
        Self {
            selected: RowKey::CurrentDir,
            expanded: HashSet::new(),
            rows: vec![],
            stamp: None,
            clock: ClockStamp::default(),
            scroll: ScrollState::default(),
            detail_scroll: ScrollState::default(),
            detail_cursor: 0,
            detail_rows: vec![],
            md: MasterDetail::new(TREE, DETAIL)
                .resizable(Split::new(32, 28, 40))
                .seam(Splitter::new(SEAM, SplitDir::Horizontal))
                .collapse_below(100)
                .summary_rows(6),
            tree_area: Rect::ZERO,
            detail_area: Rect::ZERO,
            actions: vec![],
            still_inside: None,
            busy_rows: vec![],
            refresh_asked: false,
            pending_launch: None,
            pending_session: None,
            choosing: None,
            launch_picker_targets: vec![],
            session_picker_targets: vec![],
            launch_queue: vec![],
            launch_chaining: false,
            launch_collected: vec![],
            wide: false,
            menu: RowMenu::new(MENU),
        }
    }

    pub fn select_instance(&mut self, id: &InstanceId, w: &World) {
        if let Some(entry) = w.entry(id)
            && let Some(ws) = entry.workspace_id()
        {
            self.expanded.insert(ws);
        }
        self.selected = RowKey::Instance(id.clone());
    }

    /// Builds the rows again only when something they are made of moved: the
    /// world's roster epoch, the folded workspaces or the busy rows. Anything
    /// else (cursor, scroll, hover, the clock) reuses the rows as they are.
    fn ensure_rows(&mut self, w: &World) {
        let epoch = w.epochs().roster;
        let fresh = self.stamp.as_ref().is_some_and(|stamp| {
            stamp.roster == epoch && stamp.expanded == self.expanded && stamp.busy == self.busy_rows
        });
        if fresh {
            if self
                .stamp
                .as_ref()
                .is_some_and(|s| s.selected != self.selected)
            {
                self.settle_selection(w);
                if let Some(stamp) = self.stamp.as_mut() {
                    stamp.selected = self.selected.clone();
                }
            }
            return;
        }
        self.build_rows(w);
        self.stamp = Some(RowsStamp {
            roster: epoch,
            expanded: self.expanded.clone(),
            busy: self.busy_rows.clone(),
            selected: self.selected.clone(),
        });
    }

    fn build_rows(&mut self, w: &World) {
        w.projection.work.count_manager_build();
        let roster = w.roster();
        let entry_at = |index: &usize| w.instances.get(*index);
        let mut rows = vec![];
        let cwd_ws = w.cwd_workspace().map(|x| x.id);
        rows.push(Row {
            key: RowKey::CurrentDir,
            depth: 0,
            glyph: " ",
            glyph_tone: Tone::Normal,
            label: "Current directory".into(),
            meta: RowMeta::plain(match cwd_ws.and_then(|id| w.workspace(id)) {
                Some(ws) => format!("saved as {}", ws.name),
                None => w.tilde(&w.cwd),
            }),
            meta_tone: Tone::Muted,
            trailing: None,
            expandable: false,
        });
        let mut known = HashSet::with_capacity(w.workspaces.len());
        for ws in &w.workspaces {
            known.insert(ws.id);
            let group = roster.group(Some(ws.id));
            let count = group.map_or(0, |g| g.indices.len());
            let running = group.map_or(0, |g| g.running);
            let failed = group.is_some_and(|g| g.failed);
            let restore = group.is_some_and(|g| g.restore);
            let (meta, tone) = if let Some((_, what)) = self
                .busy_rows
                .iter()
                .find(|(k, _)| *k == RowKey::Workspace(ws.id))
            {
                (RowMeta::spinner("", what), Tone::Secondary)
            } else if running > 0 {
                (
                    RowMeta::plain(format!("{running} running")),
                    Tone::Secondary,
                )
            } else if failed {
                (RowMeta::plain("! failed"), Tone::Error)
            } else if restore {
                (RowMeta::plain("restore available"), Tone::Secondary)
            } else if count == 0 {
                (RowMeta::plain("idle"), Tone::Faint)
            } else {
                (
                    RowMeta::plain(plural(count, "record", "records")),
                    Tone::Muted,
                )
            };
            let expanded = self.expanded.contains(&ws.id);
            rows.push(Row {
                key: RowKey::Workspace(ws.id),
                depth: 0,
                glyph: if count == 0 {
                    " "
                } else if expanded {
                    "▾"
                } else {
                    "▸"
                },
                glyph_tone: Tone::Secondary,
                label: ws.name.clone(),
                meta,
                meta_tone: tone,
                trailing: None,
                expandable: count > 0,
            });
            if expanded && let Some(group) = group {
                for entry in group.indices.iter().filter_map(entry_at) {
                    let i = match entry {
                        InstanceEntry::Available(instance) => instance,
                        InstanceEntry::Unavailable(unavailable) => {
                            rows.push(unavailable_row(unavailable, false));
                            continue;
                        }
                    };
                    let (glyph, gt) = instance_mark(i.status);
                    let gateway = &i.id.gateway;
                    let (meta, mt) = if let Some((_, what)) = self
                        .busy_rows
                        .iter()
                        .find(|(k, _)| *k == RowKey::Instance(i.id.clone()))
                    {
                        (
                            RowMeta::spinner(format!("{gateway} · "), what),
                            Tone::Secondary,
                        )
                    } else {
                        let plain = |text: String, tone| {
                            (RowMeta::plain(format!("{gateway} · {text}")), tone)
                        };
                        match i.status {
                            InstanceStatus::Running => (
                                RowMeta::timed(
                                    format!("{gateway} · running "),
                                    Moment::Running(i.created_secs),
                                    "",
                                ),
                                Tone::Active,
                            ),
                            InstanceStatus::CleanExited => (
                                RowMeta::timed(
                                    format!("{gateway} · exited clean · "),
                                    Moment::Ago(i.last_seen_secs),
                                    "",
                                ),
                                Tone::Muted,
                            ),
                            InstanceStatus::Crashed => {
                                plain("crashed · exit 137".into(), Tone::Error)
                            }
                            InstanceStatus::PreservedDirty => {
                                plain("preserved · dirty".into(), Tone::Secondary)
                            }
                            InstanceStatus::PreservedUnpushed => plain(
                                w.surfaces.vocabulary.status_label(i.status),
                                Tone::Secondary,
                            ),
                            InstanceStatus::RestoreAvailable => {
                                plain("restore available".into(), Tone::Secondary)
                            }
                            InstanceStatus::FailedSetup => {
                                plain("failed setup".into(), Tone::Error)
                            }
                            _ => plain(i.status.label().into(), Tone::Muted),
                        }
                    };
                    rows.push(Row {
                        key: RowKey::Instance(i.id.clone()),
                        depth: 1,
                        glyph,
                        glyph_tone: gt,
                        label: format!(
                            "{}  {} · {}",
                            i.id.short_suffix(),
                            i.role_label(),
                            i.agent.label()
                        ),
                        meta,
                        meta_tone: mt,
                        trailing: if i.status.dirty() {
                            Some(("•", Tone::Warning))
                        } else {
                            None
                        },
                        expandable: false,
                    });
                }
            }
        }
        let mut unattributed: Vec<usize> = roster
            .groups()
            .filter(|(key, _)| key.is_none_or(|id| !known.contains(&id)))
            .flat_map(|(_, members)| members.indices.iter().copied())
            .filter(|index| matches!(w.instances.get(*index), Some(InstanceEntry::Unavailable(_))))
            .collect();
        unattributed.sort_unstable();
        if !unattributed.is_empty() {
            rows.push(Row {
                key: RowKey::UnattributedSandboxes,
                depth: 0,
                glyph: "▾",
                glyph_tone: Tone::Warning,
                label: "Unattributed sandboxes".into(),
                meta: RowMeta::plain(plural(unattributed.len(), "record", "records")),
                meta_tone: Tone::Warning,
                trailing: None,
                expandable: false,
            });
            for index in &unattributed {
                if let Some(InstanceEntry::Unavailable(unavailable)) = w.instances.get(*index) {
                    rows.push(unavailable_row(unavailable, true));
                }
            }
        }
        rows.push(Row {
            key: RowKey::NewWorkspace,
            depth: 0,
            glyph: " ",
            glyph_tone: Tone::Normal,
            label: "+ New workspace".into(),
            meta: RowMeta::plain(""),
            meta_tone: Tone::Muted,
            trailing: None,
            expandable: false,
        });
        self.rows = rows;
        self.settle_selection(w);
        self.scroll.set_content(self.rows.len());
    }

    /// Keeps the selection on a row that exists: the nearest owner of a row
    /// that went away, or the first selectable row.
    fn settle_selection(&mut self, w: &World) {
        if !self.rows.iter().any(|r| r.key == self.selected) {
            self.selected = match &self.selected {
                RowKey::Instance(id) => {
                    w.entry(id).map_or(RowKey::CurrentDir, |entry| match entry {
                        InstanceEntry::Available(instance) => instance
                            .workspace
                            .map(RowKey::Workspace)
                            .unwrap_or(RowKey::CurrentDir),
                        InstanceEntry::Unavailable(instance) => instance
                            .workspace
                            .filter(|workspace| w.workspace(*workspace).is_some())
                            .map(RowKey::Workspace)
                            .unwrap_or(RowKey::UnattributedSandboxes),
                    })
                }
                _ => RowKey::CurrentDir,
            };
        }
    }

    /// Whether the clock moved since the last paint in a way the page shows:
    /// an age or duration crossing a whole second, or a spinner frame.
    /// Nothing structural is rebuilt for it.
    fn clock_due(&self, w: &World) -> bool {
        let (rows_age, spinning) = clocks(
            self.scroll
                .visible_range()
                .filter_map(|i| self.rows.get(i))
                .map(|r| &r.meta),
        );
        let card_age =
            matches!(self.selected, RowKey::Instance(_) | RowKey::Workspace(_)) || self.wide;
        self.clock.due(w, rows_age || card_age, spinning)
    }

    fn cursor(&self) -> Option<usize> {
        self.rows.iter().position(|r| r.key == self.selected)
    }

    fn move_cursor(&mut self, delta: isize) {
        let n = self.rows.len();
        if n == 0 {
            return;
        }
        let cur = self.cursor().unwrap_or(0) as isize;
        let next = (cur + delta).clamp(0, n as isize - 1) as usize;
        self.selected = self.rows[next].key.clone();
        self.scroll.ensure_visible(next);
        self.detail_cursor = 0;
        self.detail_scroll.jump_start();
    }

    fn selected_instance<'a>(&self, w: &'a World) -> Option<&'a crate::domain::instance::Instance> {
        match &self.selected {
            RowKey::Instance(id) => w.instance(id),
            _ => None,
        }
    }

    fn selected_entry<'a>(&self, w: &'a World) -> Option<&'a InstanceEntry> {
        match &self.selected {
            RowKey::Instance(id) => w.entry(id),
            _ => None,
        }
    }

    fn selected_workspace<'a>(
        &self,
        w: &'a World,
    ) -> Option<&'a crate::domain::workspace::Workspace> {
        match &self.selected {
            RowKey::Workspace(id) => w.workspace(*id),
            RowKey::CurrentDir => w.cwd_workspace(),
            _ => None,
        }
    }

    fn row_id(i: usize) -> WidgetId {
        TREE.child(i)
    }

    fn toggle_id(i: usize) -> WidgetId {
        TREE.child(i).sub("toggle")
    }

    fn detail_row_id(i: usize) -> WidgetId {
        DETAIL.child(i)
    }

    fn toggle_expand(&mut self, ws: WorkspaceId) {
        if !self.expanded.remove(&ws) {
            self.expanded.insert(ws);
        }
    }

    fn launch_choice(&self, w: &World) -> Option<(Option<WorkspaceId>, LaunchRole)> {
        match &self.selected {
            RowKey::Workspace(id) => Some((
                Some(*id),
                w.launch_role(LaunchSource::Row(w.workspace(*id))),
            )),
            RowKey::CurrentDir => {
                let saved = w.cwd_workspace();
                Some((
                    saved.map(|x| x.id),
                    w.launch_role(LaunchSource::CurrentDir(saved)),
                ))
            }
            _ => None,
        }
    }

    fn launch_still_offered(
        &self,
        w: &World,
        (ws, role): &(Option<WorkspaceId>, String),
        choice: &LaunchChoice,
    ) -> bool {
        if self.launch_choice(w) != Some((*ws, LaunchRole::Role(role.clone()))) {
            return false;
        }
        if ws.is_some_and(|id| w.workspace(id).is_none()) {
            return false;
        }
        let offer = effective_launch_offer(w, choice.tuple, *ws, Some(role));
        if !offer.launchable() || choice.model != offer.model {
            return false;
        }
        match &choice.account {
            AccountSelection::Automatic { account, level } => {
                let workspace = ws.and_then(|id| w.workspace(id));
                let resolved = w.account_for(choice.tuple, workspace, Some(role), None);
                offer.account_offer.preselected.as_ref() == Some(account)
                    && resolved.account.as_ref() == Some(account)
                    && resolved.level == *level
                    && w.accounts
                        .get(account)
                        .is_some_and(|registered| registered.provider == choice.tuple.provider)
            }
            AccountSelection::Explicit(account) => {
                offer.account_offer.accounts.contains(account)
                    && w.accounts
                        .get(account)
                        .is_some_and(|registered| registered.provider == choice.tuple.provider)
            }
            AccountSelection::Accountless => offer.accountless_allowed(),
            AccountSelection::Unselected => {
                offer.account_offer.preselected.is_none()
                    && !offer.account_offer.accounts.is_empty()
            }
        }
    }

    fn open_launch_picker(&mut self, w: &World, cx: &mut Cx) {
        let Some((ws, launch_role)) = self.launch_choice(w) else {
            return;
        };
        let (role, unavailable) = match launch_role {
            LaunchRole::Role(role) => (role, None),
            LaunchRole::Unavailable(why) => (String::new(), Some(why)),
        };
        self.pending_launch = unavailable.is_none().then(|| (ws, role.clone()));
        self.launch_queue.clear();
        self.launch_collected.clear();
        self.launch_chaining = false;
        let mut p = Picker::new(WidgetId::of("manager.launch"), "Launch · choose Agent")
            .multi_select(true)
            .expect("empty picker accepts multi-selection");
        p.set_searchable(false)
            .expect("disabling picker search is valid");
        p.set_width(84);
        let shown_role = if unavailable.is_some() {
            "no default role"
        } else {
            role.as_str()
        };
        let mut scope = match ws.and_then(|id| w.workspace(id)) {
            Some(x) => format!("{} › {shown_role}", x.name),
            None => format!("{} › {shown_role}", w.tilde(&w.cwd)),
        };
        if w.docker_on(ws, &role) {
            scope.push_str(" · Docker is on for this launch");
        }
        p.scope = Some(scope);
        let (items, targets) = if w.surfaces.tuples.is_empty() {
            let (items, agents) = agent_rows(w, ws, unavailable.is_none().then_some(role.as_str()));
            let targets = agents
                .into_iter()
                .map(|(agent, _)| {
                    let tuple = TupleIdentity::new(agent, agent.provider());
                    LaunchPickerTarget {
                        key: ItemKey::text(agent.label()),
                        choice: launch_choice_snapshot(w, tuple, ws, Some(&role)),
                    }
                })
                .collect();
            (items, targets)
        } else {
            tuple_rows(w, ws, unavailable.is_none().then_some(role.as_str()))
        };
        let items = match &unavailable {
            Some(why) => items
                .into_iter()
                .map(|i| Item {
                    tag: Some("unavailable".into()),
                    ..i.detail(why.clone()).disabled(true)
                })
                .collect(),
            None => items,
        };
        p.set_items(items)
            .unwrap_or_else(|error| p.report_item_error(error));
        self.launch_picker_targets = targets;
        cx.open(p, LayerTag::new("launch"));
    }

    fn advance_launch_queue(&mut self, w: &World, cx: &mut Cx) -> Outcome {
        loop {
            let Some(choice) = self.launch_queue.first().cloned() else {
                self.launch_chaining = false;
                let Some((ws, role)) = self.pending_launch.clone() else {
                    return Outcome::Changed;
                };
                let mut seen = HashSet::new();
                if self.launch_collected.is_empty()
                    || self.launch_collected.iter().any(|choice| {
                        !seen.insert(choice.tuple.agent)
                            || !self.launch_still_offered(w, &(ws, role.clone()), choice)
                    })
                {
                    self.launch_options_changed(cx);
                    return Outcome::Changed;
                }
                let Some(instances) = self
                    .launch_collected
                    .iter()
                    .map(LaunchChoice::instance)
                    .collect::<Option<Vec<_>>>()
                else {
                    self.launch_options_changed(cx);
                    return Outcome::Changed;
                };
                self.pending_launch = None;
                self.launch_queue.clear();
                self.launch_collected.clear();
                cx.go(Go::LaunchMany {
                    workspace: ws,
                    role,
                    instances,
                    task: None,
                });
                return Outcome::Changed;
            };
            let (ws, role) = match &self.pending_launch {
                Some((ws, role)) => (*ws, role.clone()),
                None => return Outcome::Changed,
            };
            if !self.launch_still_offered(w, &(ws, role.clone()), &choice) {
                self.launch_options_changed(cx);
                return Outcome::Changed;
            }
            if !matches!(choice.account, AccountSelection::Unselected) {
                self.launch_queue.remove(0);
                self.launch_collected.push(choice);
                continue;
            }
            self.choose_account(
                Start::Launch {
                    workspace: ws,
                    role,
                },
                choice.tuple,
                w,
                cx,
            );
            return Outcome::Changed;
        }
    }

    fn launch_options_changed(&mut self, cx: &mut Cx) {
        self.pending_launch = None;
        self.launch_queue.clear();
        self.launch_collected.clear();
        self.launch_chaining = false;
        self.choosing = None;
        cx.status("The launch options changed · open Launch again");
    }

    /// Asks which of the accounts an agent offers it starts with, when the
    /// offer names none.
    fn choose_account(&mut self, start: Start, tuple: TupleIdentity, w: &World, cx: &mut Cx) {
        let (workspace, role) = match &start {
            Start::Launch { workspace, role } => (*workspace, Some(role.clone())),
            Start::Session { instance } => match w.instance(instance) {
                Some(i) => (i.workspace, i.role.clone()),
                None => return,
            },
        };
        let ws = workspace.and_then(|id| w.workspace(id));
        let offer = w.offer_for_provider(tuple.agent, tuple.provider, ws, role.as_deref());
        let (picker, accounts) = super::modals::account_picker(
            WidgetId::of("manager.account"),
            w,
            tuple.agent,
            tuple.provider,
            ws,
            &offer,
        );
        self.choosing = Some(Choosing {
            start,
            tuple,
            accounts,
        });
        cx.open(picker, LayerTag::new("account"));
    }

    fn open_session_picker(&mut self, instance: &InstanceId, w: &World, cx: &mut Cx) {
        let Some(i) = w.instance(instance) else {
            if let Some(InstanceEntry::Unavailable(unavailable)) = w.entry(instance) {
                let display = unavailable.display();
                cx.status(format!(
                    "Cannot open an agent session for {}: {}",
                    display.sandbox_name, display.reason
                ));
            }
            return;
        };
        self.pending_session = Some(instance.clone());
        let mut p = Picker::new(
            WidgetId::of("manager.session"),
            "New session · choose Agent",
        );
        p.set_searchable(false)
            .expect("disabling picker search is valid");
        p.set_width(84);
        p.scope = Some(format!(
            "instance {} › {}",
            i.id.short_suffix(),
            i.role_label()
        ));
        let (mut items, mut targets) = agent_rows(w, i.workspace, i.role.as_deref());
        items.push(
            Item::new("Shell")
                .detail("zsh · no provider account")
                .glyph("$")
                .group("shells"),
        );
        targets.push((Agent::ClaudeCode, Some("__shell__".into())));
        p.set_items(items)
            .unwrap_or_else(|error| p.report_item_error(error));
        self.session_picker_targets = targets;
        cx.open(p, LayerTag::new("session"));
    }

    fn activate(&mut self, w: &mut World, cx: &mut Cx) -> Outcome {
        match self.selected.clone() {
            RowKey::CurrentDir | RowKey::Workspace(_) => {
                self.open_launch_picker(w, cx);
                Outcome::Changed
            }
            RowKey::NewWorkspace => {
                cx.go(Go::Prelude);
                Outcome::Changed
            }
            RowKey::Instance(id) => self.reconnect(&id, w, cx),
            RowKey::UnattributedSandboxes => Outcome::Consumed,
        }
    }

    fn attach_restored(&mut self, id: &InstanceId, w: &mut World, cx: &mut Cx) {
        if w.instance(id).is_none() {
            return;
        }
        w.open_first_session(id);
        let short = id.short_suffix();
        cx.status(fill_in(
            &w.surfaces.vocabulary.lifecycle.restored,
            &[("id", &short)],
        ));
        cx.go(Go::Attach {
            instance: id.clone(),
            pane: None,
        });
    }

    fn reconnect(&mut self, id: &InstanceId, w: &mut World, cx: &mut Cx) -> Outcome {
        if let Some(InstanceEntry::Unavailable(unavailable)) = w.entry(id) {
            let display = unavailable.display();
            cx.status(format!(
                "Cannot reconnect {}: {}",
                display.sandbox_name, display.reason
            ));
            return Outcome::Consumed;
        }
        let Some(i) = w.instance(id) else {
            return Outcome::Consumed;
        };
        match i.status {
            InstanceStatus::Running => {
                cx.go(Go::Attach {
                    instance: id.clone(),
                    pane: None,
                });
            }
            InstanceStatus::RestoreAvailable
            | InstanceStatus::PreservedDirty
            | InstanceStatus::PreservedUnpushed => {
                let row = RowKey::Instance(id.clone());
                if self.busy_rows.iter().any(|(k, _)| *k == row) {
                    cx.status("Restore already running");
                    return Outcome::Consumed;
                }
                self.busy_rows.push((row, "restoring…"));
                w.perform(Effect::Restore {
                    instance: id.clone(),
                });
            }
            InstanceStatus::Crashed => {
                cx.error(format!(
                    "Cannot reconnect: {} crashed (exit 137) · inspect or purge it",
                    id.short_suffix()
                ));
            }
            InstanceStatus::FailedSetup => {
                cx.error(format!(
                    "Cannot reconnect: {} {} · launch again",
                    id.short_suffix(),
                    w.surfaces.vocabulary.never_reached
                ));
            }
            InstanceStatus::CleanExited => {
                cx.status(format!(
                    "{} exited cleanly · launch the Workspace to start a new instance",
                    id.short_suffix()
                ));
            }
            _ => {}
        }
        Outcome::Changed
    }

    fn inspect(&mut self, id: &InstanceId, w: &World, cx: &mut Cx) {
        if let Some(InstanceEntry::Unavailable(unavailable)) = w.entry(id) {
            let display = unavailable.display();
            let created = unavailable.created_secs.map_or_else(
                || "creation time unavailable".to_owned(),
                |secs| w.clock.ago(secs),
            );
            let props = vec![
                Prop::new("Sandbox", display.sandbox_name.clone()).copyable(),
                Prop::new("Gateway", display.gateway.clone()).copyable(),
                Prop::new("Workspace", display.workspace_name),
                Prop::new("Role", display.role),
                Prop::new("Phase", unavailable.phase.label()),
                Prop::new("Created", created),
                Prop::new("Agent metadata", display.reason).tone(Tone::Warning),
                Prop::new("Agent actions", "unavailable until metadata verifies").tone(Tone::Muted),
            ];
            cx.open(
                InfoDialog::new(WidgetId::of("manager.info"), &display.sandbox_name, props)
                    .meta("read-only"),
                LayerTag::new("info").key(id.row_key()),
            );
            return;
        }
        let Some(i) = w.instance(id) else { return };
        let ws = i.workspace.and_then(|x| w.workspace(x));
        let account = match &i.account {
            Some(id) => w.accounts.get(id).map_or_else(|| id.clone(), |a| a.title()),
            None => "no registered account used".to_owned(),
        };
        let mut props = vec![
            Prop::new(&w.surfaces.vocabulary.container_noun, i.container_id()).copyable(),
            Prop::new(
                "Image",
                format!(
                    "jackin/derived:{}-{}",
                    w.workspace_label(i),
                    termrock::ui::text::clip(i.run_id.trim_start_matches("run-"), 4)
                ),
            ),
            Prop::new(
                "Workspace",
                format!(
                    "{} › role {}",
                    ws.map(|x| x.name.as_str()).unwrap_or("current directory"),
                    i.role_label()
                ),
            ),
            Prop::new(
                "Agent",
                format!("{} · account {}", i.agent.label(), account),
            ),
            Prop::new(
                "Target",
                format!(
                    "{} · {}",
                    i.workdir_label(),
                    w.surfaces
                        .vocabulary
                        .count(ws.map(|x| x.mounts.len()).unwrap_or(1))
                ),
            ),
            Prop::new("Run id", i.run_id.clone()).copyable(),
            Prop::new("Lifecycle", w.surfaces.vocabulary.status_label(i.status)).tone(
                if i.status.is_live() {
                    Tone::Normal
                } else {
                    Tone::Secondary
                },
            ),
        ];
        let words = &w.surfaces.vocabulary.daemon;
        match &i.daemon {
            DaemonSnapshot::Unavailable => props.push(
                Prop::new(&words.inspect_label, words.inspect_unavailable.as_str())
                    .tone(Tone::Warning),
            ),
            DaemonSnapshot::NoTabs => {
                props.push(Prop::new(&words.inspect_label, "attached · no tabs"))
            }
            DaemonSnapshot::Tabs(t) => props.push(Prop::new(
                &words.inspect_label,
                format!(
                    "attached · {} · {}",
                    plural(t.len(), "tab", "tabs"),
                    w.clock.ago(i.last_seen_secs)
                ),
            )),
        }
        let d = InfoDialog::new(
            WidgetId::of("manager.info"),
            &format!(
                "{} {}",
                w.surfaces.vocabulary.container_noun,
                id.short_suffix()
            ),
            props,
        )
        .meta("read-only");
        cx.open(d, LayerTag::new("info").key(id.row_key()));
    }

    fn stop(&mut self, id: &InstanceId, w: &World, cx: &mut Cx) {
        if let Some(InstanceEntry::Unavailable(unavailable)) = w.entry(id) {
            if !unavailable.phase.ready() {
                let display = unavailable.display();
                cx.status(format!(
                    "{} is {} · nothing to stop",
                    display.sandbox_name,
                    unavailable.phase.label()
                ));
                return;
            }
            cx.go(Go::PowerStop {
                instance: id.clone(),
            });
            return;
        }
        let Some(i) = w.instance(id) else { return };
        if !i.status.stoppable() {
            cx.status(format!(
                "{} is not running · nothing to stop",
                id.short_suffix()
            ));
            return;
        }
        let short = id.short_suffix().to_owned();
        let d = Dialog::destructive(
            WidgetId::of("manager.stop"),
            &format!("Stop instance {short}?"),
            &fill_in(
                &w.surfaces.vocabulary.lifecycle.stop_body,
                &[(
                    "state",
                    if i.is_dirty() {
                        "preserved with uncommitted work"
                    } else {
                        "restore available"
                    },
                )],
            ),
            "Stop",
        )
        .cancel_label("Keep running");
        cx.open(d, LayerTag::new("stop").key(id.row_key()));
    }

    fn start_unavailable(&self, id: &InstanceId, w: &World, cx: &mut Cx) -> Outcome {
        let Some(InstanceEntry::Unavailable(unavailable)) = w.entry(id) else {
            cx.status("Cannot start: the unavailable sandbox row is no longer present");
            return Outcome::Consumed;
        };
        let display = unavailable.display();
        if unavailable.phase.startable() {
            cx.go(Go::PowerStart {
                instance: id.clone(),
            });
            return Outcome::Changed;
        }
        let reason = if unavailable.phase.ready() {
            format!(
                "Sandbox {} is already ready · use t to stop",
                display.sandbox_name
            )
        } else {
            format!(
                "Cannot start sandbox {} while {} · only stopped sandboxes can start",
                display.sandbox_name,
                unavailable.phase.label()
            )
        };
        cx.status(reason);
        Outcome::Consumed
    }

    fn purge(&mut self, id: &InstanceId, w: &World, cx: &mut Cx) {
        if let Some(InstanceEntry::Unavailable(unavailable)) = w.entry(id) {
            let display = unavailable.display();
            cx.error(format!(
                "Cannot purge {} while its work state is unknown: {}",
                display.sandbox_name, display.reason
            ));
            return;
        }
        let Some(i) = w.instance(id) else { return };
        let short = id.short_suffix().to_owned();
        let ws = i
            .workspace
            .and_then(|x| w.workspace(x))
            .map(|x| x.name.clone())
            .unwrap_or("current directory".into());
        let sessions = i.sessions.as_ref().map(|s| s.len()).unwrap_or(0);
        let words = &w.surfaces.vocabulary.lifecycle;
        let facts = vec![
            Prop::new("Action", words.purge_action.as_str()).tone(Tone::Error),
            Prop::new(
                "Target",
                format!("{short} · {ws} › {} · {}", i.role_label(), i.agent.label()),
            ),
            Prop::new(
                "Scope",
                fill_in(
                    &words.purge_scope,
                    &[
                        (
                            "sessions",
                            &plural(sessions, "preserved session", "preserved sessions"),
                        ),
                        (
                            "copy",
                            if i.is_dirty() {
                                &words.copy_dirty
                            } else {
                                &words.copy_clean
                            },
                        ),
                    ],
                ),
            )
            .tone(Tone::Secondary),
            Prop::new(
                "Risk",
                if i.is_dirty() {
                    words.risk_dirty.as_str()
                } else {
                    words.risk_clean.as_str()
                },
            )
            .tone(Tone::Warning),
            Prop::new("Reversible", "no").tone(Tone::Secondary),
        ];
        let d = Dialog::facts(
            WidgetId::of("manager.purge"),
            &format!("Purge instance {short}?"),
            facts,
            vec![],
            Some(&short),
            Button::danger(WidgetId::of("manager.purge").sub("ok"), "Purge"),
        );
        cx.open(d, LayerTag::new("purge").key(id.row_key()));
    }

    fn delete(&mut self, ws: WorkspaceId, w: &World, cx: &mut Cx) {
        let Some(x) = w.workspace(ws) else { return };
        let kids = w.entries_of(Some(ws)).len();
        let d = Dialog::destructive(
            WidgetId::of("manager.delete"),
            &format!("Delete workspace {}?", x.name),
            &format!(
                "The saved configuration is removed. {} and files on disk are kept.",
                if kids == 0 {
                    "Instances".to_owned()
                } else {
                    plural(kids, "instance record", "instance records")
                }
            ),
            "Delete",
        );
        cx.open(d, LayerTag::new("delete").n(ws as usize));
    }

    fn prewarm(&mut self, ws: WorkspaceId, w: &mut World, cx: &mut Cx) {
        if self
            .busy_rows
            .iter()
            .any(|(k, _)| *k == RowKey::Workspace(ws))
        {
            cx.status("Prewarm already running");
            return;
        }
        self.busy_rows.push((RowKey::Workspace(ws), "prewarming…"));
        w.perform(Effect::Prewarm { workspace: ws });
        cx.progress(job::PREWARM, "Prewarming the derived image…");
    }

    fn open_github(&mut self, w: &World, cx: &mut Cx) {
        let Some(ws) = self.selected_workspace(w) else {
            cx.status("Select a saved workspace to open its repository");
            return;
        };
        match w.workspace_repo(ws.id) {
            Some(r) => cx.status(fill_in(
                &w.surfaces.vocabulary.github_workspace,
                &[("url", r.url.as_str())],
            )),
            None => cx.status(format!("No GitHub source for {}", ws.name)),
        }
    }

    fn menu_items(&self, key: &RowKey, w: &World) -> (String, Vec<(MenuItem, &'static str)>) {
        let item = |label: &str, key: &'static str| MenuItem::new(label).shortcut(key);
        match key {
            RowKey::Instance(id) => {
                if let Some(InstanceEntry::Unavailable(unavailable)) = w.entry(id) {
                    let display = unavailable.display();
                    let mut lifecycle = vec![];
                    if unavailable.phase.ready() {
                        lifecycle.push((item("Stop sandbox", "t").danger(), "power-stop"));
                    } else if unavailable.phase.startable() {
                        lifecycle.push((item("Start sandbox", "r"), "power-start"));
                    }
                    lifecycle.push((item("Inspect", "i").separator(), "inspect"));
                    return (display.sandbox_name, lifecycle);
                }
                let Some(i) = w.instance(id) else {
                    return (String::new(), vec![]);
                };
                let short = id.short_suffix();
                let live = i.status.is_live();
                let off = "needs a running instance";
                let reconnect = if live {
                    item("Reconnect", "Enter")
                } else if i.status.reconnectable() && i.status != InstanceStatus::Crashed {
                    item("Restore", "Enter")
                } else {
                    let why = match i.status {
                        InstanceStatus::Crashed => "crashed (exit 137) · inspect or purge it",
                        InstanceStatus::FailedSetup => w.surfaces.vocabulary.never_reached.as_str(),
                        _ => "exited · launch the workspace again",
                    };
                    item("Reconnect", "Enter").reason(why)
                };
                let session = item("New session…", "a");
                let shell = item("Shell", "!");
                let stop = item("Stop…", "t");
                (
                    format!("Instance {short}"),
                    vec![
                        (reconnect, "reconnect"),
                        (if live { session } else { session.reason(off) }, "session"),
                        (
                            if live {
                                shell.separator()
                            } else {
                                shell.reason(off).separator()
                            },
                            "shell",
                        ),
                        (item("Inspect", "i"), "inspect"),
                        (
                            if i.status.stoppable() {
                                stop
                            } else {
                                stop.reason("not running")
                            }
                            .separator(),
                            "stop",
                        ),
                        (item("Purge…", "p").danger(), "purge"),
                    ],
                )
            }
            RowKey::Workspace(id) => (
                w.workspace(*id).map(|x| x.name.clone()).unwrap_or_default(),
                vec![
                    (item("Launch…", "Enter"), "launch"),
                    (item("Edit", "e"), "edit"),
                    (item("Prewarm", "w"), "prewarm"),
                    (
                        item(&w.surfaces.vocabulary.github_menu, "o").separator(),
                        "github",
                    ),
                    (item("Delete…", "d").danger(), "delete"),
                ],
            ),
            RowKey::CurrentDir => {
                let saved = w.cwd_workspace().is_some();
                let not_saved = "not a saved workspace";
                let edit = item("Edit", "e");
                let github = item(&w.surfaces.vocabulary.github_menu, "o");
                (
                    "Current directory".into(),
                    vec![
                        (item("Launch…", "Enter"), "launch"),
                        (if saved { edit } else { edit.reason(not_saved) }, "edit"),
                        (
                            if saved {
                                github
                            } else {
                                github.reason(not_saved)
                            },
                            "github",
                        ),
                        (item("Create workspace", "n"), "new"),
                    ],
                )
            }
            RowKey::NewWorkspace => (
                "New workspace".into(),
                vec![(item("Create workspace", "Enter"), "new")],
            ),
            RowKey::UnattributedSandboxes => ("Unattributed sandboxes".into(), vec![]),
        }
    }

    fn open_menu(&mut self, i: usize, anchor: ContextAnchor, w: &World) {
        let Some(key) = self.rows.get(i).map(|r| r.key.clone()) else {
            return;
        };
        let (title, items) = self.menu_items(&key, w);
        if !items.is_empty() {
            self.menu.open(key, anchor, title, items);
        }
    }

    fn run_menu(
        &mut self,
        target: RowKey,
        cmd: &'static str,
        w: &mut World,
        cx: &mut Cx,
    ) -> Outcome {
        let prev = std::mem::replace(&mut self.selected, target.clone());
        let out = if cmd == "github" {
            self.open_github(w, cx);
            Outcome::Changed
        } else {
            self.fire_action(cmd, w, cx)
        };

        if !self.menu.config.moves_cursor && self.selected == target {
            self.selected = prev;
        }
        out.or(Outcome::Changed)
    }

    fn build_detail(&mut self, w: &World) {
        let mut rows = vec![];
        if let Some(i) = self.selected_instance(w) {
            match &i.daemon {
                DaemonSnapshot::Tabs(tabs) => {
                    for (ti, t) in tabs.iter().enumerate() {
                        rows.push(DetailRow::Text(
                            format!(
                                "{} tab {}  {}",
                                if t.active { "▾" } else { "▸" },
                                ti + 1,
                                t.label
                            ),
                            Tone::Secondary,
                        ));
                        if t.active || tabs.len() <= 3 {
                            for (pi, p) in t.panes.iter().enumerate() {
                                let pane_id = w
                                    .daemons
                                    .get(&i.id)
                                    .and_then(|d| d.tabs.get(ti).map(|tt| tt.leaves()))
                                    .and_then(|l| l.get(pi).copied())
                                    .unwrap_or(0);
                                rows.push(DetailRow::Pane {
                                    tab: ti,
                                    pane: pane_id,
                                    text: format!(
                                        "{} pane {}  {:<10} {}",
                                        if p.focused { "›" } else { " " },
                                        pi + 1,
                                        p.agent.map(|a| a.short()).unwrap_or("shell"),
                                        p.state.label()
                                    ),
                                });
                            }
                        }
                    }
                }
                DaemonSnapshot::NoTabs => rows.push(DetailRow::Text(
                    w.surfaces.vocabulary.daemon.no_tabs.clone(),
                    Tone::Muted,
                )),
                DaemonSnapshot::Unavailable => rows.push(DetailRow::Text(
                    if i.status.is_live() {
                        w.surfaces.vocabulary.daemon.unavailable_running.clone()
                    } else {
                        w.surfaces.vocabulary.daemon.unavailable_stopped.clone()
                    },
                    Tone::Muted,
                )),
            }
            rows.push(DetailRow::Blank);
            match &i.sessions {
                Ok(s) if s.is_empty() => {
                    rows.push(DetailRow::Text("No sessions recorded".into(), Tone::Muted))
                }
                Ok(s) => {
                    for r in s {
                        rows.push(DetailRow::Session(format!(
                            "{:<5} {:<11} {:<8} {}",
                            r.id,
                            w.clock.ago(r.started_secs),
                            r.agent.map(|a| a.short()).unwrap_or("shell"),
                            r.status.label()
                        )));
                    }
                }
                Err(e) => rows.push(DetailRow::Text(format!("! {}", e.label()), Tone::Error)),
            }
        } else if let Some(InstanceEntry::Unavailable(unavailable)) = self.selected_entry(w) {
            let display = unavailable.display();
            let created = unavailable.created_secs.map_or_else(
                || "creation time unavailable".to_owned(),
                |secs| w.clock.ago(secs),
            );
            rows.push(DetailRow::Text(
                format!("Sandbox  {}", display.sandbox_name),
                Tone::Warning,
            ));
            rows.push(DetailRow::Text(
                format!("Gateway  {}", display.gateway),
                Tone::Secondary,
            ));
            rows.push(DetailRow::Text(
                format!("Workspace  {}", display.workspace_name),
                Tone::Secondary,
            ));
            rows.push(DetailRow::Text(
                format!("Role  {}", display.role),
                Tone::Secondary,
            ));
            rows.push(DetailRow::Text(
                format!("Phase  {}", unavailable.phase.label()),
                Tone::Secondary,
            ));
            let power = if unavailable.phase.ready() {
                "stop available"
            } else if unavailable.phase.startable() {
                "start available"
            } else {
                "start only available while stopped"
            };
            rows.push(DetailRow::Text(format!("Power  {power}"), Tone::Muted));
            rows.push(DetailRow::Text(
                format!("Created  {created}"),
                Tone::Secondary,
            ));
            rows.push(DetailRow::Text(
                format!("Agent metadata unavailable  {}", display.reason),
                Tone::Warning,
            ));
        }
        self.detail_rows = rows;
        self.detail_cursor = self
            .detail_cursor
            .min(self.detail_rows.len().saturating_sub(1));
    }

    fn rebuild_actions(&mut self, w: &World) {
        let mut v = vec![];
        let mk =
            |name: &str, label: &str, kind: ButtonKind| Button::new(DETAIL.sub(name), label, kind);
        match &self.selected {
            RowKey::Instance(id) => match w.entry(id) {
                Some(InstanceEntry::Available(i)) => {
                    if i.status.is_live() {
                        v.push(mk("reconnect", "Reconnect", ButtonKind::Secondary));
                        v.push(mk("session", "New session", ButtonKind::Secondary));
                        v.push(mk("shell", "Shell", ButtonKind::Secondary));
                    } else if i.status.reconnectable() && i.status != InstanceStatus::Crashed {
                        v.push(mk("reconnect", "Restore", ButtonKind::Secondary));
                    }
                    v.push(mk("inspect", "Inspect", ButtonKind::Secondary));
                    if i.status.stoppable() {
                        v.push(mk("stop", "Stop", ButtonKind::Secondary));
                    }
                    v.push(mk("purge", "Purge…", ButtonKind::Danger));
                }
                Some(InstanceEntry::Unavailable(unavailable)) => {
                    v.push(mk("inspect", "Inspect", ButtonKind::Secondary));
                    if unavailable.phase.ready() {
                        v.push(mk("power-stop", "Stop sandbox", ButtonKind::Secondary));
                    } else if unavailable.phase.startable() {
                        v.push(mk("power-start", "Start sandbox", ButtonKind::Secondary));
                    }
                }
                None => {}
            },
            RowKey::Workspace(_) => {
                v.push(mk("launch", "Launch", ButtonKind::Secondary));
                v.push(mk("edit", "Edit", ButtonKind::Secondary));
                v.push(mk("prewarm", "Prewarm", ButtonKind::Secondary));
                v.push(mk("delete", "Delete…", ButtonKind::Danger));
            }
            RowKey::CurrentDir => {
                v.push(mk("launch", "Launch", ButtonKind::Secondary));
                if w.cwd_workspace().is_some() {
                    v.push(mk("edit", "Edit", ButtonKind::Secondary));
                } else {
                    v.push(mk("new", "Create workspace", ButtonKind::Secondary));
                }
            }
            RowKey::NewWorkspace => v.push(mk("new", "Create workspace", ButtonKind::Secondary)),
            RowKey::UnattributedSandboxes => {}
        }
        self.actions = v;
    }

    fn fire_action(&mut self, name: &str, w: &mut World, cx: &mut Cx) -> Outcome {
        match (name, self.selected.clone()) {
            ("reconnect", RowKey::Instance(id)) => self.reconnect(&id, w, cx),
            ("session", RowKey::Instance(id)) => {
                self.open_session_picker(&id, w, cx);
                Outcome::Changed
            }
            ("shell", RowKey::Instance(id)) => {
                cx.go(Go::NewSession {
                    instance: id,
                    agent: None,
                    account: None,
                });
                Outcome::Changed
            }
            ("inspect", RowKey::Instance(id)) => {
                self.inspect(&id, w, cx);
                Outcome::Changed
            }
            ("power-start", RowKey::Instance(instance)) => self.start_unavailable(&instance, w, cx),
            ("power-stop", RowKey::Instance(instance)) => {
                self.stop(&instance, w, cx);
                Outcome::Changed
            }
            ("stop", RowKey::Instance(id)) => {
                self.stop(&id, w, cx);
                Outcome::Changed
            }
            ("purge", RowKey::Instance(id)) => {
                self.purge(&id, w, cx);
                Outcome::Changed
            }
            ("launch", _) => {
                self.open_launch_picker(w, cx);
                Outcome::Changed
            }
            ("edit", RowKey::Workspace(id)) => {
                cx.go(Go::Editor {
                    workspace: Some(id),
                    pending: None,
                });
                Outcome::Changed
            }
            ("edit", RowKey::CurrentDir) => {
                if let Some(ws) = w.cwd_workspace() {
                    cx.go(Go::Editor {
                        workspace: Some(ws.id),
                        pending: None,
                    });
                }
                Outcome::Changed
            }
            ("prewarm", RowKey::Workspace(id)) => {
                self.prewarm(id, w, cx);
                Outcome::Changed
            }
            ("delete", RowKey::Workspace(id)) => {
                self.delete(id, w, cx);
                Outcome::Changed
            }
            ("new", _) => {
                cx.go(Go::Prelude);
                Outcome::Changed
            }
            _ => Outcome::Consumed,
        }
    }

    fn draw_tree(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        let t = ctx.theme;
        let focused = ctx.interaction.focused(TREE);
        let running = w.running_count();
        let pos = scrollbar::position_label(&self.scroll);
        let meta = if pos.is_empty() {
            if running > 0 {
                format!("{running} running")
            } else {
                "no instances".into()
            }
        } else if running > 0 {
            format!("{running} running · {pos}")
        } else {
            pos
        };
        let inner = Panel::framed(Some("Workspaces"))
            .focused(focused)
            .meta(&meta)
            .render(area, buf, t);
        self.tree_area = inner;
        let bg = t.canvas;
        self.scroll.set_viewport(inner.height as usize);
        if let Some(i) = self.cursor() {
            self.scroll.follow(i);
        }
        ctx.control(TREE, inner, false);
        ctx.scrollable(TREE, inner);
        if inner.is_empty() {
            return;
        }
        let has_sb = self.scroll.overflows();
        let row_w = inner.width.saturating_sub(u16::from(has_sb));
        let cursor = self.cursor();

        let show_meta = row_w >= 44;
        for (k, i) in self.scroll.visible_range().enumerate() {
            let y = inner.y + k as u16;
            let row = &self.rows[i];
            let rid = Self::row_id(i);

            let mut s = ctx.child_state(TREE, rid, cursor == Some(i));
            s.hovered |= ctx.interaction.hovered(Self::toggle_id(i));
            s.pressed |= ctx.interaction.pressed(Self::toggle_id(i));
            s.focused = focused && cursor == Some(i);
            s.selected = cursor == Some(i);
            let st = t.row(s, bg);
            let rect = Rect::new(inner.x - 1, y, row_w + 1, 1);
            fill(buf, rect, st);
            buf.set_string(
                rect.x,
                y,
                t.gutter_symbol(s),
                t.gutter(s, st.bg.unwrap_or(bg), false),
            );
            let mut x = rect.x + 2 + row.depth * 2;
            let gs = st
                .fg(t.tone(row.glyph_tone))
                .remove_modifier(Modifier::BOLD);
            let gs = if s.focused && row.glyph_tone == Tone::Normal {
                st
            } else {
                gs
            };
            buf.set_string(x, y, row.glyph, gs);
            if row.expandable {
                ctx.clickable(Self::toggle_id(i), Rect::new(x, y, 2, 1));
            }
            x += 2;
            let meta_text = if show_meta {
                row.meta.text(w)
            } else {
                String::new()
            };
            let meta_w = width(&meta_text) as u16;
            let trailing_w: u16 = if row.trailing.is_some() { 2 } else { 0 };
            let avail = rect.right().saturating_sub(x + 1);
            let lw = avail.saturating_sub(if meta_w > 0 { meta_w + 2 } else { 0 } + trailing_w);
            let label_style = if row.key == RowKey::NewWorkspace {
                st.fg(if s.focused {
                    t.text_primary
                } else {
                    t.text_secondary
                })
            } else if s.selected {
                st.fg(t.accent)
            } else {
                st
            };
            buf.set_string(
                x,
                y,
                fit(&truncate_middle(&row.label, lw as usize), lw as usize),
                label_style,
            );
            if meta_w > 0 && meta_w + 4 < avail {
                buf.set_string(
                    rect.right().saturating_sub(meta_w + 1 + trailing_w),
                    y,
                    &meta_text,
                    st.fg(t.tone(row.meta_tone)).remove_modifier(Modifier::BOLD),
                );
            }
            if let Some((g, tone)) = row.trailing {
                buf.set_string(rect.right().saturating_sub(2), y, g, st.fg(t.tone(tone)));
            }

            legible_on_fill(buf, rect, t, bg);
            ctx.clickable(rid, rect);
            if row.expandable {
                ctx.clickable(
                    Self::toggle_id(i),
                    Rect::new(rect.x + 2 + row.depth * 2, y, 2, 1),
                );
            }
        }
        if has_sb {
            scrollbar::render_with_fade(
                Rect::new(inner.x, inner.y, inner.width, inner.height),
                buf,
                ctx,
                TREE,
                &self.scroll,
                focused,
            );
        }
    }

    fn draw_detail(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        w: &World,
        as_drawer: bool,
    ) {
        ctx.visual_order(|ctx| self.draw_detail_card(area, buf, ctx, w, as_drawer));
    }

    fn draw_detail_card(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        w: &World,
        as_drawer: bool,
    ) {
        let t = ctx.theme;
        let focused = ctx.interaction.focused(DETAIL)
            || self.actions.iter().any(|b| ctx.interaction.focused(b.id));
        self.build_detail(w);
        self.rebuild_actions(w);
        let (title, scope_word): (String, String) = match &self.selected {
            RowKey::CurrentDir => (
                match w.cwd_workspace() {
                    Some(ws) => format!("Current directory · {}", ws.name),
                    None => "Current directory".into(),
                },
                match w.cwd_workspace() {
                    Some(_) => "saved workspace".into(),
                    None => "not saved".into(),
                },
            ),
            RowKey::Workspace(id) => {
                let ws = w.workspace(*id);
                let n = w.running_count_of(Some(*id));
                (
                    ws.map(|x| x.name.clone()).unwrap_or_default(),
                    if n > 0 {
                        format!("saved workspace · {n} running")
                    } else {
                        "saved workspace".into()
                    },
                )
            }
            RowKey::Instance(id) => {
                let entry = w.entry(id);
                (
                    match entry {
                        Some(InstanceEntry::Available(i)) => format!(
                            "{} · {}",
                            id.short_suffix(),
                            i.workspace
                                .and_then(|x| w.workspace(x))
                                .map(|x| x.name.as_str())
                                .unwrap_or("current directory")
                        ),
                        Some(InstanceEntry::Unavailable(i)) => i.display().sandbox_name,
                        None => id.short_suffix(),
                    },
                    match entry {
                        Some(InstanceEntry::Available(i)) => format!(
                            "instance · {}",
                            w.surfaces.vocabulary.status_label(i.status)
                        ),
                        Some(InstanceEntry::Unavailable(i)) => {
                            format!("metadata unavailable · {}", i.phase.label())
                        }
                        None => "instance unavailable".into(),
                    },
                )
            }
            RowKey::NewWorkspace => ("New workspace".into(), "create".into()),
            RowKey::UnattributedSandboxes => (
                "Unattributed sandboxes".into(),
                "unavailable metadata".into(),
            ),
        };
        let inner = if as_drawer {
            Panel::framed(Some(&title))
                .focused(focused)
                .meta(&scope_word)
                .render(area, buf, t)
        } else {
            let card_is_stop =
                ctx.interaction.focused(DETAIL) && !matches!(self.selected, RowKey::Instance(_));
            Panel::card(Some(&title))
                .focused(focused)
                .focus_mark(if card_is_stop {
                    PanelFocus::Bar
                } else {
                    PanelFocus::Title
                })
                .meta(&scope_word)
                .render(area, buf, t)
        };
        self.detail_area = inner;
        let bg = if as_drawer { t.canvas } else { t.surface };
        ctx.scrollable(DETAIL, inner);
        if inner.is_empty() {
            return;
        }
        let widths: Vec<u16> = self.actions.iter().map(|b| b.width()).collect();
        let action_lines = row_lines(inner.width + 1, &widths, 2);
        let reserve = action_lines.max(2);
        let mut y = inner.y;
        let put = |buf: &mut Buffer, y: u16, text: &str, style: Style| {
            if y < inner.bottom() {
                buf.set_string(
                    inner.x,
                    y,
                    termrock::ui::reveal::cut(Rect::new(inner.x, y, inner.width, 1), text),
                    style.bg(bg),
                );
            }
        };
        match self.selected.clone() {
            RowKey::CurrentDir | RowKey::Workspace(_) => {
                let ws = self.selected_workspace(w);
                match ws {
                    Some(ws) => {
                        let mut facts = vec![
                            Prop::new("Working dir", ws.workdir.clone()),
                            Prop::new(
                                w.surfaces.vocabulary.project_tab.as_str(),
                                if ws.mounts.is_empty() {
                                    "none".into()
                                } else {
                                    ws.mounts
                                        .iter()
                                        .map(|m| {
                                            let source = w.tilde(m.source_label());
                                            let isolation = m.isolation.label().to_lowercase();
                                            fill_in(
                                                &w.surfaces.vocabulary.project_card,
                                                &[
                                                    ("source", source.as_str()),
                                                    ("mode", m.mode_label()),
                                                    ("isolation", isolation.as_str()),
                                                ],
                                            )
                                        })
                                        .collect::<Vec<_>>()
                                        .join("\n")
                                },
                            )
                            .wrap(),
                            Prop::new(
                                "Roles",
                                format!(
                                    "{}{}",
                                    ws.roles
                                        .default
                                        .as_ref()
                                        .map(|d| format!("{d} ★ · "))
                                        .unwrap_or_default(),
                                    match &ws.roles.allowed {
                                        crate::domain::workspace::AllowedRoles::All =>
                                            format!("allowed all ({} in registry)", w.roles.len()),
                                        crate::domain::workspace::AllowedRoles::Custom(l) =>
                                            format!("allowed {} of {}", l.len(), w.roles.len()),
                                    }
                                ),
                            ),
                            Prop::new(
                                "Environments",
                                format!(
                                    "{} · {} [op]",
                                    plural(ws.env_count(), "var", "vars"),
                                    ws.env
                                        .iter()
                                        .chain(ws.role_env.values().flatten())
                                        .filter(|e| matches!(
                                            e.value,
                                            crate::domain::workspace::EnvValue::OnePassword(_)
                                        ))
                                        .count()
                                ),
                            ),
                        ];
                        let effective = ws.effective_accounts(&w.accounts);
                        let mut acct_lines: Vec<String> = effective
                            .iter()
                            .map(|e| {
                                let name = w
                                    .accounts
                                    .get(&e.id)
                                    .map(|a| a.title())
                                    .unwrap_or_else(|| e.id.clone());
                                let mut line = name;
                                if e.preferred {
                                    line.push_str(" ★");
                                }
                                if !e.usable.is_ready() {
                                    line.push_str(&format!(
                                        " · {}",
                                        e.usable.label(
                                            w.surfaces.fixed.as_ref(),
                                            &w.surfaces.vocabulary
                                        )
                                    ));
                                }
                                line
                            })
                            .collect();
                        if acct_lines.is_empty() {
                            acct_lines.push("none active · enable one in the editor".into());
                        }
                        facts.push(Prop::new("Accounts", acct_lines.join(" · ")).wrap());
                        facts.push(Prop::new(
                            "Policies",
                            format!(
                                "git pull {} · keep awake {} · dirty exit {}{}",
                                if ws.git_pull { "enabled" } else { "disabled" },
                                if ws.keep_awake { "on" } else { "off" },
                                ws.dirty_policy.label(),
                                if ws.docker && w.surfaces.docker.is_some() {
                                    " · docker on"
                                } else {
                                    ""
                                }
                            ),
                        ));
                        let used = props::render(
                            Rect::new(inner.x, y, inner.width, inner.height),
                            buf,
                            t,
                            &facts,
                            bg,
                        );
                        y += used + 1;
                        let kids = w.entries_of(Some(ws.id));
                        if !kids.is_empty() {
                            put(
                                buf,
                                y,
                                "Instances",
                                t.secondary().add_modifier(Modifier::BOLD),
                            );
                            let hv = w.health_view();
                            let ago = w.clock.ago(hv.at_secs);
                            let meta = match hv.health {
                                Health::Healthy => format!("{} · {ago}", hv.noun),
                                Health::Stale => format!("▲ {} stale · {ago}", hv.noun),
                                Health::Failing => format!("▲ {} failing · {ago}", hv.noun),
                            };
                            let mw = width(&meta) as u16;
                            if y < inner.bottom() && inner.width > mw + 12 {
                                buf.set_string(inner.right() - mw, y, &meta, t.faint().bg(bg));
                            }
                            y += 1;
                            for entry in kids {
                                let tone = entry.as_available().map_or(Tone::Warning, |i| {
                                    if i.status.is_live() {
                                        Tone::Normal
                                    } else {
                                        Tone::Secondary
                                    }
                                });
                                let line = entry_line(entry, tone, &w.surfaces.vocabulary);
                                let text = termrock::widgets::viewport::line_text(&line);
                                put(buf, y, &text, t.secondary());
                                if y < inner.bottom() {
                                    termrock::widgets::viewport::render_spans(
                                        buf,
                                        inner.x,
                                        y,
                                        inner.width,
                                        t,
                                        &line,
                                        bg,
                                    );
                                }
                                y += 1;
                            }
                        }
                    }
                    None => {
                        let e = EmptyState::new("Create a workspace from this directory.").hint(
                            &fill_in(
                                &w.surfaces.vocabulary.first_use,
                                &[("path", w.tilde(&w.cwd).as_str())],
                            ),
                        );
                        empty::render(
                            Rect::new(
                                inner.x,
                                inner.y,
                                inner.width,
                                inner.height.saturating_sub(reserve + 1),
                            ),
                            buf,
                            t,
                            &e,
                            bg,
                        );
                        y = inner.bottom().saturating_sub(reserve);
                    }
                }
            }
            RowKey::NewWorkspace => {
                let e = EmptyState::new("New workspace").hint("Enter starts the five-step create chain: source, destination, working directory, name, then the editor.");
                empty::render(
                    Rect::new(
                        inner.x,
                        inner.y,
                        inner.width,
                        inner.height.saturating_sub(reserve + 1),
                    ),
                    buf,
                    t,
                    &e,
                    bg,
                );
                y = inner.bottom().saturating_sub(reserve);
            }
            RowKey::Instance(id) => {
                if let Some(i) = w.instance(&id) {
                    let ws = i.workspace.and_then(|x| w.workspace(x));
                    let acc = w.account_for(instance_tuple(i), ws, i.role.as_deref(), None);
                    let facts = vec![
                        Prop::new(
                            "Workspace",
                            format!(
                                "{} › role {}",
                                ws.map(|x| x.name.as_str()).unwrap_or("current directory"),
                                i.role_label()
                            ),
                        ),
                        Prop::new(
                            "Agent",
                            format!(
                                "{} · account {} ({})",
                                i.agent.label(),
                                acc.label(&w.accounts),
                                acc.level.label()
                            ),
                        ),
                        Prop::new(&w.surfaces.vocabulary.container_noun, i.container_id()),
                        Prop::new(
                            "Started",
                            format!(
                                "{} · last seen {}",
                                w.clock.ago(i.created_secs),
                                w.clock.ago(i.last_seen_secs)
                            ),
                        ),
                        Prop::new(
                            "Lifecycle",
                            format!(
                                "{} · {}",
                                w.surfaces.vocabulary.status_label(i.status),
                                w.surfaces.vocabulary.status_description(i.status)
                            ),
                        )
                        .tone(match i.status {
                            InstanceStatus::Crashed | InstanceStatus::FailedSetup => Tone::Error,
                            InstanceStatus::Running => Tone::Normal,
                            _ => Tone::Secondary,
                        })
                        .wrap(),
                        Prop::new("Working tree", w.surfaces.vocabulary.dirty_summary(i)).tone(
                            if i.is_dirty() {
                                Tone::Warning
                            } else {
                                Tone::Secondary
                            },
                        ),
                    ];
                    let mut facts = facts;
                    if let Some(b) = &i.branch {
                        facts.push(Prop::new(
                            "Branch",
                            match &i.pr {
                                Some((n, title)) => format!("{b} · PR #{n} · {title}"),
                                None => b.clone(),
                            },
                        ));
                    }
                    let used = props::render(
                        Rect::new(inner.x, y, inner.width, inner.height),
                        buf,
                        t,
                        &facts,
                        bg,
                    );
                    y += used + 1;
                    let live_focus = ctx.interaction.focused(DETAIL);
                    put(
                        buf,
                        y,
                        "Live topology",
                        t.secondary().add_modifier(Modifier::BOLD),
                    );
                    let words = &w.surfaces.vocabulary.daemon;
                    let src = match &i.daemon {
                        DaemonSnapshot::Unavailable => words.topology_unavailable.clone(),
                        _ => fill_in(
                            &words.topology_source,
                            &[("ago", &w.clock.ago(i.last_seen_secs))],
                        ),
                    };
                    let sw = width(&src) as u16;
                    if y < inner.bottom() && inner.width > sw + 16 {
                        buf.set_string(inner.right() - sw, y, &src, t.faint().bg(bg));
                    }
                    y += 1;
                    let list_top = y;
                    let rows_avail = inner.bottom().saturating_sub(y + reserve) as usize;
                    self.detail_scroll.set_content(self.detail_rows.len());
                    self.detail_scroll.set_viewport(rows_avail);
                    if live_focus {
                        self.detail_scroll.ensure_visible(self.detail_cursor);
                    }
                    ctx.control(
                        DETAIL,
                        Rect::new(inner.x, list_top, inner.width, rows_avail as u16),
                        false,
                    );
                    let mut sessions_heading_done = false;
                    for (k, ri) in self.detail_scroll.visible_range().enumerate() {
                        let yy = list_top + k as u16;
                        let row = &self.detail_rows[ri];
                        let rid = Self::detail_row_id(ri);
                        match row {
                            DetailRow::Pane { text, .. } => {
                                let mut s = ctx.state(rid);
                                s.focused = live_focus && ri == self.detail_cursor;
                                let st = t.row(s, bg);
                                let r = Rect::new(inner.x - 1, yy, inner.width + 1, 1);
                                fill(buf, r, st);
                                buf.set_string(
                                    r.x,
                                    yy,
                                    t.gutter_symbol(s),
                                    t.gutter(s, st.bg.unwrap_or(bg), false),
                                );
                                buf.set_string(
                                    r.x + 2,
                                    yy,
                                    termrock::ui::reveal::cut(
                                        Rect::new(r.x + 2, yy, r.width.saturating_sub(3), 1),
                                        text,
                                    ),
                                    st,
                                );
                                legible_on_fill(buf, r, t, bg);
                                ctx.clickable(rid, r);
                            }
                            DetailRow::Text(text, tone) => {
                                put(buf, yy, text, Style::new().fg(t.tone(*tone)))
                            }
                            DetailRow::Session(text) => {
                                if !sessions_heading_done {
                                    sessions_heading_done = true;
                                }
                                put(buf, yy, text, t.secondary());
                            }
                            DetailRow::Blank => {
                                put(
                                    buf,
                                    yy,
                                    "Sessions",
                                    t.secondary().add_modifier(Modifier::BOLD),
                                );
                                let m = format!(
                                    "manifest · {}",
                                    plural(
                                        i.sessions.as_ref().map(|s| s.len()).unwrap_or(0),
                                        "recorded",
                                        "recorded"
                                    )
                                );
                                let mw = width(&m) as u16;
                                if yy < inner.bottom() && inner.width > mw + 12 {
                                    buf.set_string(inner.right() - mw, yy, &m, t.faint().bg(bg));
                                }
                            }
                        }
                    }
                    if self.detail_scroll.overflows() {
                        scrollbar::render_with_fade(
                            Rect::new(inner.x, list_top, inner.width, rows_avail as u16),
                            buf,
                            ctx,
                            DETAIL,
                            &self.detail_scroll,
                            live_focus,
                        );
                    }
                    y = inner.bottom().saturating_sub(reserve);
                } else if let Some(InstanceEntry::Unavailable(unavailable)) = w.entry(&id) {
                    let display = unavailable.display();
                    let power = if unavailable.phase.ready() {
                        "stop available · t"
                    } else if unavailable.phase.startable() {
                        "start available · r"
                    } else {
                        "start only available while stopped"
                    };
                    let created = unavailable.created_secs.map_or_else(
                        || "creation time unavailable".to_owned(),
                        |secs| w.clock.ago(secs),
                    );
                    let facts = vec![
                        Prop::new("Sandbox", display.sandbox_name).copyable(),
                        Prop::new("Gateway", display.gateway).copyable(),
                        Prop::new("Workspace", display.workspace_name),
                        Prop::new("Role", display.role),
                        Prop::new("Phase", unavailable.phase.label()),
                        Prop::new("Power", power),
                        Prop::new("Created", created),
                        Prop::new("Agent metadata", display.reason).tone(Tone::Warning),
                        Prop::new("Actions", "agent metadata actions are unavailable")
                            .tone(Tone::Muted),
                    ];
                    let used = props::render(
                        Rect::new(inner.x, y, inner.width, inner.height),
                        buf,
                        t,
                        &facts,
                        bg,
                    );
                    y += used + 1;
                }
            }
            RowKey::UnattributedSandboxes => {
                y = inner.bottom().saturating_sub(reserve);
            }
        }

        let ay = inner
            .bottom()
            .saturating_sub(action_lines)
            .max(y)
            .max(inner.y);
        if ay < inner.bottom() {
            let rects = row_layout(
                Rect::new(inner.x - 1, ay, inner.width + 1, inner.bottom() - ay),
                &widths,
                2,
            );
            for (b, r) in self.actions.iter_mut().zip(rects) {
                b.render(r, buf, ctx, bg);
            }
        }
        if !matches!(self.selected, RowKey::Instance(_)) {
            ctx.control(DETAIL, Rect::new(inner.x, inner.y, inner.width, 1), false);
        }
    }

    fn draw_roster(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        let t = ctx.theme;
        let focused = ctx.interaction.focused(ROSTER);
        let running = w.running_entries();
        let inner = Panel::card(Some("Running"))
            .focused(focused)
            .meta(&plural(running.len(), "instance", "instances"))
            .render(area, buf, t);
        let bg = t.surface;
        ctx.control(ROSTER, inner, false);
        if inner.is_empty() {
            return;
        }
        let mut y = inner.y;
        for entry in &running {
            if y + 2 >= inner.bottom() {
                break;
            }
            let Some(i) = entry.as_available() else {
                if let InstanceEntry::Unavailable(u) = entry {
                    let display = u.display();
                    let ws = u
                        .workspace
                        .and_then(|x| w.workspace(x))
                        .map(|x| x.name.as_str())
                        .unwrap_or_else(|| {
                            if u.workspace_name.is_some() {
                                display.workspace_name.as_str()
                            } else {
                                "current directory"
                            }
                        });
                    let role = if u.role.is_some() {
                        display.role.as_str()
                    } else {
                        "unknown role"
                    };
                    let rid = ROSTER.part(roster_row(&u.id));
                    let mut s = ctx.state(rid);
                    s.selected = self.selected == RowKey::Instance(u.id.clone());
                    let st = t.row(s, bg);
                    let r = Rect::new(inner.x - 1, y, inner.width + 1, 3);
                    fill(buf, r, st);
                    buf.set_string(
                        inner.x,
                        y,
                        termrock::ui::reveal::cut(
                            Rect::new(inner.x, y, inner.width, 1),
                            &format!(
                                "◉ {}  {ws} · {role} · {}",
                                display.short_suffix,
                                u.phase.label()
                            ),
                        ),
                        st,
                    );
                    buf.set_string(
                        inner.x + 3,
                        y + 1,
                        termrock::ui::reveal::cut(
                            Rect::new(inner.x + 3, y + 1, inner.width.saturating_sub(3), 1),
                            &display.reason,
                        ),
                        st.fg(t.text_muted).remove_modifier(Modifier::BOLD),
                    );
                    legible_on_fill(buf, r, t, bg);
                    ctx.register_part(ROSTER, roster_row(&u.id), r);
                    y += 4;
                }
                continue;
            };
            let ws = i
                .workspace
                .and_then(|x| w.workspace(x))
                .map(|x| x.name.as_str())
                .unwrap_or("current directory");
            let acc = w.account_for(
                instance_tuple(i),
                i.workspace.and_then(|x| w.workspace(x)),
                i.role.as_deref(),
                None,
            );
            let rid = ROSTER.part(roster_row(&i.id));
            let mut s = ctx.state(rid);
            s.selected = self.selected == RowKey::Instance(i.id.clone());
            let st = t.row(s, bg);
            let r = Rect::new(inner.x - 1, y, inner.width + 1, 3);
            fill(buf, r, st);
            buf.set_string(
                inner.x,
                y,
                termrock::ui::reveal::cut(
                    Rect::new(inner.x, y, inner.width, 1),
                    &format!(
                        "◉ {}  {ws} · {} · {}",
                        i.id.short_suffix(),
                        i.role_label(),
                        i.agent.label()
                    ),
                ),
                st,
            );
            let running_for = termrock::ui::text::duration(
                std::time::Duration::from_secs((w.now_secs() - i.created_secs).max(0) as u64),
                termrock::ui::text::DurationStyle::Spoken,
            );
            let words = &w.surfaces.vocabulary.daemon;
            let live = match &i.daemon {
                DaemonSnapshot::Tabs(tabs) => format!(
                    "running {running_for} · {} · {}",
                    plural(tabs.len(), "tab", "tabs"),
                    plural(tabs.iter().map(|t| t.panes.len()).sum(), "pane", "panes")
                ),
                DaemonSnapshot::NoTabs => {
                    fill_in(&words.card_no_tabs, &[("duration", &running_for)])
                }
                DaemonSnapshot::Unavailable => {
                    fill_in(&words.card_unavailable, &[("duration", &running_for)])
                }
            };
            buf.set_string(
                inner.x + 3,
                y + 1,
                termrock::ui::reveal::cut(
                    Rect::new(inner.x + 3, y + 1, inner.width.saturating_sub(3), 1),
                    &live,
                ),
                st.fg(t.text_muted).remove_modifier(Modifier::BOLD),
            );

            let active = t.tone_on(Tone::Active, st.bg.unwrap_or(bg));
            buf.set_style(Rect::new(inner.x, y, 1, 1), Style::new().fg(active));
            let word = width("running") as u16;
            if inner.width > word + 3 {
                buf.set_style(
                    Rect::new(inner.x + 3, y + 1, word, 1),
                    Style::new().fg(active),
                );
            }
            buf.set_string(
                inner.x + 3,
                y + 2,
                termrock::ui::reveal::cut(
                    Rect::new(inner.x + 3, y + 2, inner.width.saturating_sub(3), 1),
                    &format!("account {}", acc.label(&w.accounts)),
                ),
                st.fg(t.text_muted).remove_modifier(Modifier::BOLD),
            );
            legible_on_fill(buf, r, t, bg);
            ctx.register_part(ROSTER, roster_row(&i.id), r);
            y += 4;
        }
        let preserved: Vec<_> = w
            .instances
            .iter()
            .filter_map(InstanceEntry::as_available)
            .filter(|i| i.status.dirty() || i.status == InstanceStatus::RestoreAvailable)
            .collect();
        if !preserved.is_empty() && y + 2 < inner.bottom() {
            y += 1;
            buf.set_string(
                inner.x,
                y,
                "Preserved",
                t.secondary().bg(bg).add_modifier(Modifier::BOLD),
            );
            let m = plural(preserved.len(), "record", "records");
            buf.set_string(inner.right() - width(&m) as u16, y, &m, t.faint().bg(bg));
            y += 1;
            for i in preserved {
                if y >= inner.bottom() {
                    break;
                }
                let ws = i
                    .workspace
                    .and_then(|x| w.workspace(x))
                    .map(|x| x.name.as_str())
                    .unwrap_or("current directory");
                buf.set_string(
                    inner.x,
                    y,
                    termrock::ui::reveal::cut(
                        Rect::new(inner.x, y, inner.width, 1),
                        &format!(
                            "◌ {}  {ws} · {} · {}",
                            i.id.short_suffix(),
                            i.role_label(),
                            w.surfaces.vocabulary.status_label(i.status)
                        ),
                    ),
                    t.secondary().bg(bg),
                );
                y += 1;
            }
        }
        if y + 3 < inner.bottom() {
            y += 1;
            let hv = w.health_view();
            buf.set_string(
                inner.x,
                y,
                termrock::ui::text::capitalize(&hv.noun),
                t.secondary().bg(bg).add_modifier(Modifier::BOLD),
            );
            let ago = w.clock.ago(hv.at_secs);
            let (h, tone) = match hv.health {
                Health::Healthy => (format!("healthy · {ago}"), Tone::Secondary),
                Health::Stale => (format!("▲ stale · {ago}"), Tone::Warning),
                Health::Failing => (format!("▲ failing · {ago}"), Tone::Error),
            };
            buf.set_string(
                inner.right() - width(&h) as u16,
                y,
                &h,
                Style::new().fg(t.tone(tone)).bg(bg),
            );
            y += 1;
            buf.set_string(
                inner.x,
                y,
                "Refresh      throttled · every 5 s",
                t.muted().bg(bg),
            );
            y += 1;
            let summary = crate::domain::usage::OverallSummary::compute(&w.accounts.accounts);
            buf.set_string(
                inner.x,
                y,
                termrock::ui::reveal::cut(
                    Rect::new(inner.x, y, inner.width, 1),
                    &format!(
                        "Usage        {} · {}",
                        summary.health.label(),
                        summary.issues_line()
                    ),
                ),
                t.muted().bg(bg),
            );
        }
    }
}

fn tuple_rows(
    w: &World,
    ws: Option<WorkspaceId>,
    role: Option<&str>,
) -> (Vec<Item>, Vec<LaunchPickerTarget>) {
    let wsr = ws.and_then(|id| w.workspace(id));
    let mut tuples = Vec::new();
    for agent in Agent::ALL {
        let default = TupleIdentity::new(agent, agent.provider());
        if w.offer_for_provider(agent, default.provider, wsr, role)
            .configured
            || w.surfaces
                .tuples
                .iter()
                .any(|offer| offer.agent == agent && offer.provider == default.provider)
        {
            tuples.push(default);
        }
        for offer in w
            .surfaces
            .tuples
            .iter()
            .filter(|offer| offer.agent == agent)
        {
            let tuple = TupleIdentity::new(offer.agent, offer.provider);
            if !tuples.contains(&tuple) {
                tuples.push(tuple);
            }
        }
    }
    let mut counts = std::collections::HashMap::<Agent, usize>::new();
    for tuple in &tuples {
        *counts.entry(tuple.agent).or_default() += 1;
    }
    let mut items = Vec::new();
    let mut targets = Vec::new();
    for tuple in tuples {
        let tuple_offer = w
            .surfaces
            .tuples
            .iter()
            .find(|offer| offer.agent == tuple.agent && offer.provider == tuple.provider);
        let effective = effective_launch_offer(w, tuple, ws, role);
        let label = if counts.get(&tuple.agent).copied().unwrap_or_default() > 1 {
            tuple.label()
        } else {
            tuple.agent.label().to_owned()
        };
        let account_detail = if let Some(blocked) = &effective.account_offer.blocked {
            blocked.clone()
        } else if let Some(account) = &effective.account_offer.preselected {
            let resolved = w.account_for(tuple, ws.and_then(|id| w.workspace(id)), role, None);
            let additional = effective.account_offer.accounts.len().saturating_sub(1);
            format!(
                "account {} · {}{}",
                w.accounts
                    .get(account)
                    .map(|entry| entry.title())
                    .unwrap_or_else(|| account.clone()),
                resolved.level.label(),
                if additional > 0 {
                    format!(" · +{additional} more available")
                } else {
                    String::new()
                }
            )
        } else if effective.account_offer.accounts.len() > 1 {
            format!(
                "{} accounts · choose at start",
                effective.account_offer.accounts.len()
            )
        } else if effective.accountless_allowed() {
            "accountless choice offered by backend".to_owned()
        } else {
            "no usable account".to_owned()
        };
        let (detail, tag) = match tuple_offer {
            Some(offer) if offer.state == crate::domain::tuple::TupleState::Supported => (
                account_detail,
                if effective.launchable() {
                    if effective.account_offer.accounts.len() > 1 {
                        "choose at start"
                    } else {
                        "ready"
                    }
                } else {
                    "unavailable"
                },
            ),
            Some(offer) => (
                offer
                    .reason
                    .clone()
                    .unwrap_or_else(|| offer.state.label().to_owned()),
                "unavailable",
            ),
            None => (
                account_detail,
                if effective.account_offer.blocked.is_some()
                    || effective.account_offer.accounts.is_empty()
                {
                    "unavailable"
                } else if effective.account_offer.accounts.len() > 1 {
                    "choose at start"
                } else {
                    "ready"
                },
            ),
        };
        let key = ItemKey::text(&format!(
            "tuple:{}:{}",
            tuple.agent.short(),
            tuple.provider.short()
        ));
        items.push(
            Item::new(label)
                .key(key)
                .detail(detail)
                .glyph(if tuple.agent.registerable() {
                    "▪"
                } else {
                    "·"
                })
                .group("agents")
                .selection_group(ItemKey::text(tuple.agent.short()))
                .tag(tag)
                .disabled(
                    !effective.launchable()
                        || (effective.matrix_present
                            && !tuple_offer.is_some_and(|offer| {
                                offer.state == crate::domain::tuple::TupleState::Supported
                            })),
                ),
        );
        targets.push(LaunchPickerTarget {
            key,
            choice: launch_choice_snapshot(w, tuple, ws, role),
        });
    }
    (items, targets)
}

struct EffectiveLaunchOffer {
    model: String,
    account_offer: crate::world::AgentOffer,
    matrix_present: bool,
    tuple: Option<crate::domain::tuple::TupleOffer>,
    account_requirement: AccountRequirement,
}

impl EffectiveLaunchOffer {
    fn accountless_allowed(&self) -> bool {
        self.matrix_present
            && self.tuple.is_some()
            && self.account_requirement == AccountRequirement::Optional
            && self.account_offer.accounts.is_empty()
            && !self.account_offer.configured
            && self.account_offer.blocked.is_none()
    }
}

impl EffectiveLaunchOffer {
    fn launchable(&self) -> bool {
        if self.matrix_present {
            self.tuple.is_some()
                && self.account_offer.blocked.is_none()
                && (!self.account_offer.accounts.is_empty() || self.accountless_allowed())
        } else {
            self.account_offer.configured
                && self.account_offer.blocked.is_none()
                && !self.account_offer.accounts.is_empty()
        }
    }
}

fn effective_launch_offer(
    world: &World,
    tuple_identity: TupleIdentity,
    workspace: Option<WorkspaceId>,
    role: Option<&str>,
) -> EffectiveLaunchOffer {
    let ws = workspace.and_then(|id| world.workspace(id));
    let account_offer =
        world.offer_for_provider(tuple_identity.agent, tuple_identity.provider, ws, role);
    let matrix_present = !world.surfaces.tuples.is_empty();
    let tuple = supported_launch_tuple(world, tuple_identity).cloned();
    EffectiveLaunchOffer {
        model: tuple
            .as_ref()
            .map_or_else(String::new, |offer| offer.default_model.clone()),
        account_offer,
        matrix_present,
        tuple,
        account_requirement: world.backend.account_requirement(tuple_identity),
    }
}

fn launch_choice_snapshot(
    world: &World,
    tuple: TupleIdentity,
    workspace: Option<WorkspaceId>,
    role: Option<&str>,
) -> LaunchChoice {
    let effective = effective_launch_offer(world, tuple, workspace, role);
    let account = match effective.account_offer.preselected.clone() {
        Some(account) => {
            let resolved = world.account_for(
                tuple,
                workspace.and_then(|id| world.workspace(id)),
                role,
                None,
            );
            AccountSelection::Automatic {
                account,
                level: resolved.level,
            }
        }
        None if effective.accountless_allowed() => AccountSelection::Accountless,
        None => AccountSelection::Unselected,
    };
    LaunchChoice {
        tuple,
        model: effective.model,
        account,
    }
}

fn instance_tuple(instance: &Instance) -> TupleIdentity {
    instance.agents.first().map_or_else(
        || TupleIdentity::new(instance.agent, instance.agent.provider()),
        |choice| TupleIdentity::new(choice.agent, choice.provider),
    )
}

fn agent_rows(
    w: &World,
    ws: Option<WorkspaceId>,
    role: Option<&str>,
) -> (Vec<Item>, Vec<(Agent, Option<String>)>) {
    let wsr = ws.and_then(|id| w.workspace(id));
    let mut items = vec![];
    let mut targets = vec![];

    for (a, offer) in w.offered_agents(wsr, role) {
        let effective = effective_launch_offer(w, TupleIdentity::new(a, a.provider()), ws, role);
        let (detail, _disabled, tag) = match (&offer.preselected, &offer.blocked) {
            (Some(id), _) => {
                let acc = w.accounts.get(id);
                let r = w.account_for(TupleIdentity::new(a, a.provider()), wsr, role, None);
                let more = w.eligible_accounts(a, wsr, role).len().saturating_sub(1);
                (
                    format!(
                        "account {} · {}{}",
                        acc.map(|x| x.title()).unwrap_or(id.clone()),
                        r.level.label(),
                        if more > 0 {
                            format!(" · +{more} more available")
                        } else {
                            String::new()
                        }
                    ),
                    false,
                    Some(if more > 0 { "choose at start" } else { "ready" }),
                )
            }
            (None, Some(why)) => (why.clone(), true, Some("unavailable")),
            (None, None) if offer.accounts.len() > 1 => (
                format!("{} accounts · choose at start", offer.accounts.len()),
                false,
                Some("choose at start"),
            ),
            (None, None) => ("no usable account".to_owned(), true, Some("unavailable")),
        };
        let (detail, disabled, tag) = if effective.matrix_present {
            let account_detail = if let Some(reason) = &effective.account_offer.blocked {
                reason.clone()
            } else if let Some(id) = &effective.account_offer.preselected {
                let resolved = w.account_for(TupleIdentity::new(a, a.provider()), wsr, role, None);
                let more = effective.account_offer.accounts.len().saturating_sub(1);
                format!(
                    "account {} · {}{}",
                    w.accounts
                        .get(id)
                        .map(|account| account.title())
                        .unwrap_or_else(|| id.clone()),
                    resolved.level.label(),
                    if more > 0 {
                        format!(" · +{more} more available")
                    } else {
                        String::new()
                    }
                )
            } else if effective.account_offer.accounts.len() > 1 {
                format!(
                    "{} accounts · choose at start",
                    effective.account_offer.accounts.len()
                )
            } else if effective.account_offer.accounts.is_empty() {
                "no usable account".to_owned()
            } else {
                "choose at start".to_owned()
            };
            match &effective.tuple {
                Some(tuple) => {
                    let account_ready = effective.account_offer.has_usable_account();
                    let tag = if !account_ready {
                        "unavailable"
                    } else if effective.account_offer.accounts.len() > 1 {
                        "choose at start"
                    } else {
                        "ready"
                    };
                    (
                        format!("{} · {account_detail}", tuple.provider.label()),
                        !account_ready,
                        Some(tag),
                    )
                }
                None => {
                    let rejected = w
                        .surfaces
                        .tuples
                        .iter()
                        .find(|tuple| tuple.agent == a && tuple.provider == a.provider());
                    (
                        rejected
                            .and_then(|tuple| tuple.reason.clone())
                            .unwrap_or_else(|| {
                                rejected
                                    .map_or("unavailable", |tuple| tuple.state.label())
                                    .to_owned()
                            }),
                        true,
                        Some("unavailable"),
                    )
                }
            }
        } else {
            (detail, !effective.launchable(), tag)
        };
        items.push(Item {
            tag: (tag).map(Into::into),
            ..Item::new(a.label())
                .key(ItemKey::text(a.label()))
                .detail(detail)
                .glyph(if a.registerable() { "▪" } else { "·" })
                .group("agents")
                .disabled(disabled)
        });
        targets.push((a, offer.preselected.clone()));
    }
    (items, targets)
}

impl Page<Jackin> for ManagerScreen {
    fn enter(&mut self, w: &mut World, cx: &mut Cx) {
        self.ensure_rows(w);
        self.md.close_drawer(cx.focus);
    }

    fn primary_focus(&self) -> Option<WidgetId> {
        Some(TREE)
    }

    fn on_tick(&mut self, w: &mut World, _cx: &mut Cx) -> Outcome {
        let mut out = Outcome::Ignored;
        if let Some((_, _, until)) = &self.still_inside
            && w.now_ms() >= *until
        {
            self.still_inside = None;
            out = Outcome::Changed;
        }
        if !self.busy_rows.is_empty() {
            out = Outcome::Changed;
        }

        if w.now_secs() - w.last_refresh_secs >= 5 && w.refresh() {
            w.last_refresh_secs = w.now_secs();
            out = Outcome::Changed;
        }
        self.ensure_rows(w);
        if self.clock_due(w) {
            out = Outcome::Changed;
        }
        out
    }

    fn on_msg(&mut self, msg: &Msg, w: &mut World, cx: &mut Cx) -> Outcome {
        match msg {
            Msg::Prewarmed { workspace, result } => {
                self.busy_rows
                    .retain(|(k, _)| *k != RowKey::Workspace(*workspace));
                match result {
                    Ok(()) => {
                        let name = w
                            .workspace(*workspace)
                            .map(|x| x.name.clone())
                            .unwrap_or_default();
                        cx.status(format!("Prewarmed {name} · derived image cached"));
                    }
                    Err(error) => cx.error(error.clone()),
                }
            }
            Msg::Stopped { instance, result } => {
                self.busy_rows
                    .retain(|(k, _)| *k != RowKey::Instance(instance.clone()));
                match result {
                    Ok(snapshot) => {
                        w.apply((**snapshot).clone());
                        w.daemons.remove(instance);
                        crate::world::refresh_snapshots(w);
                        w.sync_arbiter();
                        cx.status(format!("Instance {} stopped", instance.short_suffix()));
                    }
                    Err(error) => cx.error(error.clone()),
                }
            }
            Msg::PowerStarted { instance, result } => {
                self.busy_rows
                    .retain(|(key, _)| *key != RowKey::Instance(instance.clone()));
                match result {
                    Ok(snapshot) => {
                        w.apply((**snapshot).clone());
                        w.sync_arbiter();
                        let name = match w.entry(instance) {
                            Some(InstanceEntry::Unavailable(row)) => row.display().sandbox_name,
                            Some(InstanceEntry::Available(row)) => row.container_id(),
                            None => instance.short_label(),
                        };
                        cx.status(format!("Sandbox {name} started"));
                    }
                    Err(error) => cx.error(error.clone()),
                }
            }
            Msg::PowerStopped { instance, result } => {
                self.busy_rows
                    .retain(|(key, _)| *key != RowKey::Instance(instance.clone()));
                match result {
                    Ok(snapshot) => {
                        w.apply((**snapshot).clone());
                        w.daemons.remove(instance);
                        crate::world::refresh_snapshots(w);
                        w.sync_arbiter();
                        let name = match w.entry(instance) {
                            Some(InstanceEntry::Unavailable(row)) => row.display().sandbox_name,
                            Some(InstanceEntry::Available(row)) => row.container_id(),
                            None => instance.short_label(),
                        };
                        cx.status(format!("Sandbox {name} stopped"));
                    }
                    Err(error) => cx.error(error.clone()),
                }
            }
            Msg::Purged { instance, result } => {
                self.busy_rows
                    .retain(|(k, _)| *k != RowKey::Instance(instance.clone()));
                match result {
                    Ok(snapshot) => {
                        w.apply((**snapshot).clone());
                        w.daemons.remove(instance);
                        w.sync_arbiter();
                        cx.status(fill_in(
                            &w.surfaces.vocabulary.lifecycle.purged,
                            &[("id", &instance.short_suffix())],
                        ));
                    }
                    Err(error) => {
                        let text = match &w.surfaces.vocabulary.lifecycle.purge_refused_next {
                            Some(next) => format!("{error} · {next}"),
                            None => error.clone(),
                        };
                        cx.error(text);
                    }
                }
            }
            Msg::WorkspaceDeleted { id, result } => {
                self.busy_rows.retain(|(k, _)| *k != RowKey::Workspace(*id));
                match result {
                    Ok(saved) => {
                        let name = w.workspace(*id).map(|x| x.name.clone()).unwrap_or_default();
                        w.apply((*saved.snapshot).clone());
                        self.selected = RowKey::CurrentDir;
                        let status = format!("Deleted workspace {name} · instances and files kept");
                        cx.status(match saved.warning {
                            Some(warning) => format!("{} · {status}", warning.status()),
                            None => status,
                        });
                    }
                    Err(error) if w.workspace(*id).is_some() => cx.error(error.clone()),
                    Err(_) => {}
                }
            }
            Msg::Restored { instance, result } => {
                self.busy_rows
                    .retain(|(k, _)| *k != RowKey::Instance(instance.clone()));
                match result {
                    Ok(snapshot) => {
                        w.apply((**snapshot).clone());
                        self.attach_restored(instance, w, cx);
                    }
                    Err(error) => cx.error(error.clone()),
                }
            }
            Msg::LaunchCancelled { .. } => {}
            Msg::LaunchLeft { left } => cx.status(left.words()),
            Msg::Refreshed { result, .. } => {
                let asked = std::mem::take(&mut self.refresh_asked);
                if let Err(error) = result {
                    let text = format!("Refresh failed: {error} · showing last-good rows");
                    if asked {
                        cx.error(text);
                    } else {
                        cx.notice(text, Tone::Error);
                    }
                }
            }
            _ => return Outcome::Ignored,
        }
        self.ensure_rows(w);
        Outcome::Changed
    }

    fn on_key(&mut self, key: &Key, w: &mut World, cx: &mut Cx) -> Outcome {
        self.ensure_rows(w);
        if self.menu.is_open() {
            let (o, chosen) = self.menu.on_key(key);
            match chosen {
                Some((target, cmd)) => return self.run_menu(target, cmd, w, cx),

                None if o == Outcome::Ignored => {}
                None => return o,
            }
        }
        let focus = cx.focus.current();
        let in_tree = focus == Some(TREE);
        if in_tree
            && self.menu.is_context_key(key)
            && let Some(i) = self.cursor()
        {
            let dy = i.saturating_sub(self.scroll.offset) as u16;
            let fallback = Rect::new(
                self.tree_area.x,
                self.tree_area.y + dy.min(self.tree_area.height.saturating_sub(1)),
                self.tree_area.width.max(1),
                1,
            );
            let anchor = ContextAnchor::Control {
                id: Self::row_id(i),
                fallback,
            };
            self.open_menu(i, anchor, w);
            return Outcome::Changed;
        }
        let in_detail = focus == Some(DETAIL);
        let action_focus = self.actions.iter().position(|b| Some(b.id) == focus);
        if (in_detail || action_focus.is_some())
            && self.menu.is_context_key(key)
            && let Some(i) = self.cursor()
        {
            let anchor = match action_focus {
                Some(b) => ContextAnchor::Control {
                    id: self.actions[b].id,
                    fallback: self.actions[b].area,
                },
                None => {
                    let dy = self.detail_cursor.saturating_sub(self.detail_scroll.offset) as u16;
                    ContextAnchor::Control {
                        id: Self::detail_row_id(self.detail_cursor),
                        fallback: Rect::new(
                            self.detail_area.x,
                            self.detail_area.y + dy.min(self.detail_area.height.saturating_sub(1)),
                            self.detail_area.width.max(1),
                            1,
                        ),
                    }
                }
            };
            self.open_menu(i, anchor, w);
            return Outcome::Changed;
        }

        match key.code {
            KeyCode::Char('n') if key.plain() && action_focus.is_none() => {
                if let RowKey::Instance(id) = self.selected.clone() {
                    self.open_session_picker(&id, w, cx);
                } else {
                    cx.go(Go::Prelude);
                }
                return Outcome::Changed;
            }
            KeyCode::Char('e') if key.plain() => {
                return match self.selected.clone() {
                    RowKey::Workspace(id) => {
                        cx.go(Go::Editor {
                            workspace: Some(id),
                            pending: None,
                        });
                        Outcome::Changed
                    }
                    RowKey::CurrentDir => {
                        match w.cwd_workspace() {
                            Some(ws) => {
                                let id = ws.id;
                                cx.go(Go::Editor {
                                    workspace: Some(id),
                                    pending: None,
                                });
                                Outcome::Changed
                            }
                            None => {
                                cx.status("The current directory is not a saved workspace · n creates one");
                                Outcome::Changed
                            }
                        }
                    }
                    _ => Outcome::Ignored,
                };
            }
            KeyCode::Char('d') if key.plain() => {
                if let RowKey::Workspace(id) = self.selected {
                    self.delete(id, w, cx);
                    return Outcome::Changed;
                }
            }
            KeyCode::Char('w') if key.plain() => {
                if let RowKey::Workspace(id) = self.selected {
                    self.prewarm(id, w, cx);
                    return Outcome::Changed;
                }
            }
            KeyCode::Char('o') if key.plain() => {
                self.open_github(w, cx);
                return Outcome::Changed;
            }
            KeyCode::Char('r') if key.plain() => {
                if let RowKey::Instance(id) = self.selected.clone() {
                    if matches!(w.entry(&id), Some(InstanceEntry::Unavailable(_))) {
                        return self.start_unavailable(&id, w, cx);
                    }
                    return self.reconnect(&id, w, cx);
                }
            }
            KeyCode::Char('a') if key.plain() => {
                if let RowKey::Instance(id) = self.selected.clone() {
                    self.open_session_picker(&id, w, cx);
                    return Outcome::Changed;
                }
            }
            KeyCode::Char('!') => {
                if let RowKey::Instance(id) = self.selected.clone() {
                    if let Some(InstanceEntry::Unavailable(unavailable)) = w.entry(&id) {
                        let display = unavailable.display();
                        cx.status(format!(
                            "Cannot open a shell for {}: {}",
                            display.sandbox_name, display.reason
                        ));
                    } else if w.instance(&id).is_some_and(|i| i.status.is_live()) {
                        cx.go(Go::NewSession {
                            instance: id,
                            agent: None,
                            account: None,
                        });
                    } else {
                        cx.status("Shell needs a running instance · reconnect or restore first");
                    }
                    return Outcome::Changed;
                }
            }
            KeyCode::Char('i') if key.plain() => {
                if let RowKey::Instance(id) = self.selected.clone() {
                    self.inspect(&id, w, cx);
                    return Outcome::Changed;
                }
            }
            KeyCode::Char('t') if key.plain() => {
                if let RowKey::Instance(id) = self.selected.clone() {
                    self.stop(&id, w, cx);
                    return Outcome::Changed;
                }
            }
            KeyCode::Char('p') if key.plain() => {
                if let RowKey::Instance(id) = self.selected.clone() {
                    self.purge(&id, w, cx);
                    return Outcome::Changed;
                }
            }
            KeyCode::F(5) => {
                w.last_refresh_secs = w.now_secs() - 6;
                self.refresh_asked = true;
                cx.status("Refreshing…");
                return Outcome::Changed;
            }
            _ => {}
        }
        if in_tree {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.move_cursor(-1);
                    return Outcome::Changed;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.move_cursor(1);
                    return Outcome::Changed;
                }
                KeyCode::PageUp => {
                    self.move_cursor(-(self.scroll.viewport_len.max(1) as isize));
                    return Outcome::Changed;
                }
                KeyCode::PageDown => {
                    self.move_cursor(self.scroll.viewport_len.max(1) as isize);
                    return Outcome::Changed;
                }
                KeyCode::Home | KeyCode::Char('g') => {
                    self.move_cursor(-(self.rows.len() as isize));
                    return Outcome::Changed;
                }
                KeyCode::End | KeyCode::Char('G') => {
                    self.move_cursor(self.rows.len() as isize);
                    return Outcome::Changed;
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    if let RowKey::Workspace(id) = self.selected {
                        if self.expanded.contains(&id) {
                            self.move_cursor(1);
                        } else if !w.entries_of(Some(id)).is_empty() {
                            self.expanded.insert(id);
                        }
                    }
                    self.ensure_rows(w);
                    return Outcome::Changed;
                }
                KeyCode::Left | KeyCode::Char('h') => {
                    match self.selected.clone() {
                        RowKey::Workspace(id) if self.expanded.contains(&id) => {
                            self.expanded.remove(&id);
                        }
                        RowKey::Instance(iid) => {
                            if let Some(entry) = w.entry(&iid) {
                                self.selected = match entry.workspace_id() {
                                    Some(ws) => RowKey::Workspace(ws),
                                    None => RowKey::UnattributedSandboxes,
                                };
                            }
                        }
                        _ => {}
                    }
                    self.ensure_rows(w);
                    return Outcome::Changed;
                }
                KeyCode::Char(' ') => {
                    if let RowKey::Workspace(id) = self.selected {
                        self.toggle_expand(id);
                        self.ensure_rows(w);
                    }
                    return Outcome::Changed;
                }
                KeyCode::Char('*') => {
                    for ws in &w.workspaces {
                        self.expanded.insert(ws.id);
                    }
                    self.ensure_rows(w);
                    return Outcome::Changed;
                }
                KeyCode::Char('-') => {
                    self.expanded.clear();
                    self.ensure_rows(w);
                    return Outcome::Changed;
                }
                KeyCode::Enter => return self.activate(w, cx),
                KeyCode::Tab => {
                    self.md.open_drawer(cx.focus);
                    return Outcome::Changed;
                }
                _ => {}
            }
        }
        if in_detail {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.detail_cursor = self.detail_cursor.saturating_sub(1);
                    self.detail_scroll.ensure_visible(self.detail_cursor);
                    return Outcome::Changed;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.detail_cursor =
                        (self.detail_cursor + 1).min(self.detail_rows.len().saturating_sub(1));
                    self.detail_scroll.ensure_visible(self.detail_cursor);
                    return Outcome::Changed;
                }
                KeyCode::PageDown => {
                    self.detail_scroll.page_down();
                    return Outcome::Changed;
                }
                KeyCode::PageUp => {
                    self.detail_scroll.page_up();
                    return Outcome::Changed;
                }
                KeyCode::Enter => {
                    if let (RowKey::Instance(id), Some(DetailRow::Pane { pane, .. })) = (
                        self.selected.clone(),
                        self.detail_rows.get(self.detail_cursor),
                    ) {
                        let pane = *pane;
                        if w.instance(&id).is_some_and(|i| i.status.is_live()) {
                            cx.go(Go::Attach {
                                instance: id,
                                pane: Some(pane),
                            });
                            return Outcome::Changed;
                        }
                    }
                    return self.activate(w, cx);
                }
                KeyCode::Esc | KeyCode::Left | KeyCode::BackTab => {
                    self.md.close_drawer(cx.focus);
                    return Outcome::Changed;
                }
                KeyCode::Tab => {
                    cx.focus_next();
                    return Outcome::Changed;
                }
                _ => {}
            }
        }
        if let Some(i) = action_focus {
            let (o, fired) = self.actions[i].on_key(key);
            if fired {
                let name = self.actions[i].id;
                let names = [
                    "reconnect",
                    "session",
                    "shell",
                    "inspect",
                    "power-start",
                    "power-stop",
                    "stop",
                    "purge",
                    "launch",
                    "edit",
                    "prewarm",
                    "delete",
                    "new",
                ];
                for n in names {
                    if DETAIL.sub(n) == name {
                        return self.fire_action(n, w, cx);
                    }
                }
            }
            if o.consumed() {
                return o;
            }
            match key.code {
                KeyCode::Left | KeyCode::Char('h') => {
                    if i > 0 {
                        cx.focus.focus(self.actions[i - 1].id);
                    } else {
                        cx.focus.focus(DETAIL);
                    }
                    return Outcome::Changed;
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    if i + 1 < self.actions.len() {
                        cx.focus.focus(self.actions[i + 1].id);
                    }
                    return Outcome::Changed;
                }
                KeyCode::Esc => {
                    self.md.close_drawer(cx.focus);
                    return Outcome::Changed;
                }
                _ => {}
            }
        }
        if focus == Some(ROSTER) {
            match key.code {
                KeyCode::Esc | KeyCode::Left => {
                    cx.focus.focus(TREE);
                    return Outcome::Changed;
                }
                _ => {}
            }
        }

        Outcome::Ignored
    }

    fn pointer(&mut self, ev: &PointerEvent, w: &mut World, cx: &mut Cx) -> Outcome {
        if let Some(o) = self.md.on_pointer(ev) {
            return o;
        }
        let (id, pos) = (ev.id, ev.pos());
        if let Some((axis, delta)) = ev.wheel() {
            return match axis {
                Axis::Vertical => self.wheel(id, delta),
                Axis::Horizontal => Outcome::Ignored,
            };
        }
        if let Some(o) = scrollbar::pointer(ev, TREE, &mut self.scroll, TrackClick::Jump)
            .or_else(|| scrollbar::pointer(ev, DETAIL, &mut self.detail_scroll, TrackClick::Jump))
        {
            return o;
        }
        match ev.phase() {
            Some(Phase::Click) => {
                if ClickPolicy::DATA.activates(ev.count(), false) {
                    let d = self.double_click(id, w, cx);
                    if d.consumed() {
                        return d;
                    }
                }
                self.click(id, w, cx)
            }
            Some(Phase::Secondary) => self.secondary(id, pos, w, cx),
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
            ("launch", ModalResult::PickedMany(keys)) => {
                let choices: Vec<LaunchChoice> = keys
                    .iter()
                    .filter_map(|key| {
                        self.launch_picker_targets
                            .iter()
                            .find(|target| target.key == *key)
                            .map(|target| target.choice.clone())
                    })
                    .collect();
                if choices.is_empty()
                    || choices.len() != keys.len()
                    || self.pending_launch.is_none()
                {
                    return Outcome::Changed;
                }
                let mut seen = HashSet::new();
                if choices
                    .iter()
                    .any(|choice| !seen.insert(choice.tuple.agent))
                {
                    cx.status("Choose only one provider for each agent");
                    self.launch_queue.clear();
                    self.launch_collected.clear();
                    self.launch_chaining = false;
                    return Outcome::Changed;
                }
                if let Some(pending) = self.pending_launch.clone() {
                    for choice in &choices {
                        if !self.launch_still_offered(w, &pending, choice) {
                            self.launch_options_changed(cx);
                            return Outcome::Changed;
                        }
                    }
                }
                self.launch_queue = choices;
                self.launch_collected = Vec::new();
                self.launch_chaining = true;
                self.advance_launch_queue(w, cx)
            }
            ("launch", ModalResult::PickerEvent(PickerAction::SelectionConflict { .. })) => {
                cx.status("Choose only one provider for each agent");
                Outcome::Changed
            }
            ("launch", ModalResult::Cancelled) => {
                self.pending_launch = None;
                self.launch_queue.clear();
                self.launch_collected.clear();
                self.launch_chaining = false;
                Outcome::Changed
            }
            ("account", ModalResult::Cancelled) => {
                self.choosing = None;
                self.pending_launch = None;
                self.pending_session = None;
                self.launch_queue.clear();
                self.launch_collected.clear();
                self.launch_chaining = false;
                Outcome::Changed
            }
            ("account", ModalResult::Picked(i)) => {
                let Some(Choosing {
                    start,
                    tuple,
                    accounts,
                }) = self.choosing.take()
                else {
                    return Outcome::Changed;
                };
                let account = accounts.get(i).cloned();
                if self.launch_chaining {
                    let Some(pending) = self.pending_launch.clone() else {
                        self.launch_options_changed(cx);
                        return Outcome::Changed;
                    };
                    let Some(account) = account else {
                        self.launch_options_changed(cx);
                        return Outcome::Changed;
                    };
                    let Some(queued) = self.launch_queue.first().cloned() else {
                        self.launch_options_changed(cx);
                        return Outcome::Changed;
                    };
                    if queued.tuple != tuple {
                        self.launch_options_changed(cx);
                        return Outcome::Changed;
                    }
                    let mut explicit = queued;
                    explicit.account = AccountSelection::Explicit(account);
                    if !self.launch_still_offered(w, &pending, &explicit) {
                        self.launch_options_changed(cx);
                        return Outcome::Changed;
                    }
                    self.launch_queue.remove(0);
                    self.launch_collected.push(explicit);
                    return self.advance_launch_queue(w, cx);
                }
                match start {
                    Start::Launch { workspace, role } => {
                        let Some(account) = account else {
                            self.launch_options_changed(cx);
                            return Outcome::Changed;
                        };
                        let effective = effective_launch_offer(w, tuple, workspace, Some(&role));
                        let choice = LaunchChoice {
                            tuple,
                            model: effective.model,
                            account: AccountSelection::Explicit(account),
                        };
                        if !self.launch_still_offered(w, &(workspace, role.clone()), &choice) {
                            self.launch_options_changed(cx);
                            return Outcome::Changed;
                        }
                        let Some(instance) = choice.instance() else {
                            self.launch_options_changed(cx);
                            return Outcome::Changed;
                        };
                        self.pending_launch = None;
                        cx.go(Go::LaunchMany {
                            workspace,
                            role,
                            instances: vec![instance],
                            task: None,
                        });
                    }
                    Start::Session { instance } => cx.go(Go::NewSession {
                        instance,
                        agent: Some(tuple.agent),
                        account,
                    }),
                }
                Outcome::Changed
            }
            ("session", ModalResult::Picked(i)) => {
                let Some((agent, account)) = self.session_picker_targets.get(i).cloned() else {
                    return Outcome::Changed;
                };
                let Some(instance) = self.pending_session.take() else {
                    return Outcome::Changed;
                };
                let shell = account.as_deref() == Some("__shell__");
                if account.is_none() && !shell {
                    self.choose_account(
                        Start::Session { instance },
                        TupleIdentity::new(agent, agent.provider()),
                        w,
                        cx,
                    );
                    return Outcome::Changed;
                }
                cx.go(Go::NewSession {
                    instance,
                    agent: if shell { None } else { Some(agent) },
                    account: if shell { None } else { account },
                });
                Outcome::Changed
            }
            (
                "stop",
                ModalResult::Dialog {
                    action: Some(1), ..
                },
            ) => {
                let Some(id) = id_for_row_key(&tag.key) else {
                    return Outcome::Changed;
                };
                match w.entry(&id) {
                    None => {
                        cx.status("Sandbox is already gone · nothing to stop");
                        return Outcome::Changed;
                    }
                    Some(InstanceEntry::Unavailable(unavailable)) => {
                        let display = unavailable.display();
                        cx.status(format!(
                            "Cannot stop {}: agent metadata unavailable · {}",
                            display.sandbox_name, display.reason
                        ));
                        return Outcome::Changed;
                    }
                    Some(InstanceEntry::Available(instance)) if !instance.status.stoppable() => {
                        cx.status(format!(
                            "{} is not running · nothing to stop",
                            id.short_suffix()
                        ));
                        return Outcome::Changed;
                    }
                    Some(InstanceEntry::Available(_)) => {}
                }
                self.busy_rows
                    .push((RowKey::Instance(id.clone()), "stopping…"));
                w.perform(Effect::Stop { instance: id });
                cx.progress(job::STOP, "Stopping…");
                Outcome::Changed
            }
            (
                "purge",
                ModalResult::Dialog {
                    action: Some(1), ..
                },
            ) => {
                let Some(id) = id_for_row_key(&tag.key) else {
                    return Outcome::Changed;
                };
                match w.entry(&id) {
                    None => {
                        cx.status("Sandbox is already gone · nothing to purge");
                        return Outcome::Changed;
                    }
                    Some(InstanceEntry::Unavailable(unavailable)) => {
                        let display = unavailable.display();
                        cx.status(format!(
                            "Cannot purge {} while its work state is unknown: {}",
                            display.sandbox_name, display.reason
                        ));
                        return Outcome::Changed;
                    }
                    Some(InstanceEntry::Available(_)) => {}
                }
                self.busy_rows
                    .push((RowKey::Instance(id.clone()), "purging…"));
                w.perform(Effect::Purge { instance: id });
                cx.progress(job::PURGE, "Purging…");
                Outcome::Changed
            }
            (
                "delete",
                ModalResult::Dialog {
                    action: Some(1), ..
                },
            ) => {
                let id = tag.n as WorkspaceId;
                let row = RowKey::Workspace(id);
                if w.workspace(id).is_none() {
                    cx.status("That workspace is already gone · nothing to delete");
                    return Outcome::Changed;
                }
                if let Some((_, what)) = self.busy_rows.iter().find(|(k, _)| *k == row) {
                    cx.status(format!("Workspace is {what}"));
                    return Outcome::Changed;
                }
                self.busy_rows.push((row, "deleting…"));
                w.perform(Effect::DeleteWorkspace { id });
                Outcome::Changed
            }
            ("info", ModalResult::Info(InfoResult::Copy(v))) => {
                cx.copy(v);
                Outcome::Changed
            }
            (_, ModalResult::Cancelled)
            | (
                _,
                ModalResult::Dialog {
                    action: Some(0), ..
                },
            ) => {
                if tag.kind == "stop" || tag.kind == "purge" || tag.kind == "delete" {
                    cx.status("Cancelled · nothing changed");
                }
                self.pending_launch = None;
                self.pending_session = None;
                Outcome::Changed
            }
            _ => Outcome::Changed,
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        self.ensure_rows(w);
        self.clock.painted(w);
        let t = ctx.theme;
        self.wide = area.width >= 150;
        self.md.detail_owns = self.actions.iter().map(|b| b.id).collect();
        let l = self.md.layout(area, buf, ctx, t.canvas);
        if !l.master.is_empty() {
            self.draw_tree(l.master, buf, ctx, w);
        }
        if l.mode != Mode::Split {
            if l.mode == Mode::Drawer {
                self.draw_detail(l.detail, buf, ctx, w, true);
            } else {
                self.draw_summary(l.summary, buf, ctx, w);
            }
            self.md.finish(ctx);
            self.menu.render(area, buf, ctx);
            return;
        }
        let right = l.detail;
        if self.wide {
            let (detail, roster) = Split::new(55, 40, 36).horizontal(right, 2);
            self.draw_detail(detail, buf, ctx, w, false);
            self.draw_roster(roster, buf, ctx, w);
        } else {
            self.draw_detail(right, buf, ctx, w, false);
        }

        self.menu.render(area, buf, ctx);
    }

    fn hints(&self, focus: Option<WidgetId>, w: &World) -> Vec<Hint> {
        if self.menu.is_open() {
            return termrock::widgets::row_menu::hints();
        }
        let mut v = vec![];
        if focus == Some(DETAIL) {
            v.push(hint("↑↓", "Move"));
            if let RowKey::Instance(id) = &self.selected
                && w.instance(id).is_some()
            {
                v.push(hint("Enter", "Attach pane"));
            }
            v.push(hint("Tab", "Actions"));
            v.push(hint("Esc", "Back"));
            return v;
        }
        if let Some(b) = self.actions.iter().find(|b| Some(b.id) == focus) {
            v.push(hint("←→", "Choose"));
            v.push(b.hint());
            v.push(hint("Esc", "Back"));
            return v;
        }
        match &self.selected {
            RowKey::Instance(id) => {
                if let Some(InstanceEntry::Unavailable(unavailable)) = w.entry(id) {
                    if unavailable.phase.ready() {
                        v.push(hint("t", "Stop sandbox"));
                    } else if unavailable.phase.startable() {
                        v.push(hint("r", "Start sandbox"));
                    }
                    v.push(hint("i", "Inspect"));
                    v.push(hint("←", "Back"));
                    return v;
                }
                let live = w.instance(id).is_some_and(|i| i.status.is_live());
                let restorable = w
                    .instance(id)
                    .is_some_and(|i| i.status.reconnectable() && !i.status.is_live());
                if live {
                    v.push(hint("Enter", "Reconnect"));
                    v.push(hint("a", "New session"));
                    v.push(hint("!", "Open shell"));
                    v.push(hint("t", "Stop"));
                } else if restorable {
                    v.push(hint("Enter", "Restore"));
                }
                v.push(hint("i", "Inspect"));
                v.push(hint("p", "Purge…"));
                v.push(hint("←", "Back"));
            }
            RowKey::Workspace(_) => {
                v.push(hint("Enter", "Launch"));
                v.push(hint("→", "Expand"));
                v.push(hint("e", "Edit"));
                v.push(hint("n", "New"));
                v.push(hint("d", "Delete…"));
                v.push(hint("w", "Prewarm"));
                v.push(hint("o", "GitHub"));
            }
            RowKey::CurrentDir => {
                v.push(hint("Enter", "Launch"));
                v.push(hint("n", "New"));
                if w.cwd_workspace().is_some() {
                    v.push(hint("e", "Edit"));
                }
            }
            RowKey::NewWorkspace => {
                v.push(hint("Enter", "Setup"));
            }
            RowKey::UnattributedSandboxes => {}
        }
        v.push(hint("Tab", "Details"));
        v.push(hint("c", "Accounts"));
        v.push(hint("u", "Usage"));
        v.push(hint("s", "Settings"));
        v.push(hint("?", "Help"));
        v.push(hint("q", "Quit"));
        v
    }

    fn crumb(&self, w: &World) -> String {
        match &self.selected {
            RowKey::CurrentDir => "Workspaces".into(),
            RowKey::Workspace(id) => format!(
                "Workspaces › {}",
                w.workspace(*id).map(|x| x.name.as_str()).unwrap_or("?")
            ),
            RowKey::Instance(id) => {
                let ws = w
                    .entry(id)
                    .and_then(InstanceEntry::workspace_id)
                    .and_then(|x| w.workspace(x))
                    .map(|x| x.name.clone())
                    .unwrap_or("current directory".into());
                match w.entry(id) {
                    Some(InstanceEntry::Unavailable(i)) => {
                        let display = i.display();
                        format!(
                            "Workspaces › {} › {} · {}",
                            display.workspace_name, display.sandbox_name, display.gateway
                        )
                    }
                    _ => format!("Workspaces › {ws} › {}", id.short_suffix()),
                }
            }
            RowKey::NewWorkspace => "Workspaces › new workspace".into(),
            RowKey::UnattributedSandboxes => "Workspaces › unattributed sandboxes".into(),
        }
    }

    fn animating(&self, _w: &World) -> bool {
        !self.busy_rows.is_empty() || self.still_inside.is_some()
    }

    fn on_esc_top(&mut self, _w: &mut World, _cx: &mut Cx) -> Outcome {
        Outcome::Ignored
    }
}

fn supported_launch_tuple(
    world: &World,
    tuple: TupleIdentity,
) -> Option<&crate::domain::tuple::TupleOffer> {
    world.surfaces.tuples.iter().find(|offer| {
        offer.agent == tuple.agent
            && offer.provider == tuple.provider
            && offer.state == crate::domain::tuple::TupleState::Supported
    })
}

impl Screen for ManagerScreen {
    fn layer_open(&self) -> bool {
        self.menu.hides_page_focus()
    }

    fn strip_right(&self, w: &World) -> Vec<Segment> {
        let mut v = vec![];
        if !self.busy_rows.is_empty() {
            v.push(
                Segment::new(
                    format!(
                        "{} {}",
                        spinner_frame(w.clock.now_ms as u64 / 80),
                        self.busy_rows[0].1.trim_end_matches('…')
                    ),
                    Tone::Secondary,
                )
                .priority(6),
            );
        }
        v
    }
}

impl ManagerScreen {
    fn draw_summary(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        let t = ctx.theme;
        let bg = t.surface;
        let title = match &self.selected {
            RowKey::CurrentDir => "Current directory".to_owned(),
            RowKey::Workspace(id) => w.workspace(*id).map(|x| x.name.clone()).unwrap_or_default(),
            RowKey::Instance(id) => match w.entry(id) {
                Some(InstanceEntry::Unavailable(unavailable)) => unavailable.display().sandbox_name,
                _ => format!("Instance {}", id.short_suffix()),
            },
            RowKey::NewWorkspace => "New workspace".into(),
            RowKey::UnattributedSandboxes => "Unattributed sandboxes".into(),
        };
        let inner = Panel::card(Some(&title))
            .meta("Tab details")
            .render(area, buf, t);
        let one = |s: String, tone: Tone| vec![Span::new(s, tone)];
        let lines: Vec<Vec<Span>> = match &self.selected {
            RowKey::CurrentDir | RowKey::Workspace(_) => match self.selected_workspace(w) {
                Some(ws) => vec![
                    one(
                        format!(
                            "{} · {} · {}",
                            ws.workdir,
                            w.surfaces.vocabulary.count(ws.mounts.len()),
                            plural(ws.env_count(), "var", "vars")
                        ),
                        Tone::Secondary,
                    ),
                    one(
                        format!(
                            "Roles {}{} · Auth {}",
                            ws.roles
                                .default
                                .as_deref()
                                .map(|d| format!("{d} ★"))
                                .unwrap_or("none".into()),
                            match &ws.roles.allowed {
                                crate::domain::workspace::AllowedRoles::All => " · all".to_owned(),
                                crate::domain::workspace::AllowedRoles::Custom(l) =>
                                    format!(" · {} allowed", l.len()),
                            },
                            w.account_for(
                                TupleIdentity::new(Agent::ClaudeCode, Agent::ClaudeCode.provider()),
                                Some(ws),
                                None,
                                None,
                            )
                            .label(&w.accounts)
                        ),
                        Tone::Muted,
                    ),
                ]
                .into_iter()
                .chain(w.entries_of(Some(ws.id)).iter().take(2).map(|entry| {
                    let tone = if entry.as_available().is_some() {
                        Tone::Secondary
                    } else {
                        Tone::Warning
                    };
                    entry_line(entry, tone, &w.surfaces.vocabulary)
                }))
                .collect(),
                None => vec![one(
                    format!(
                        "{} · not saved · Enter launches, n creates a workspace",
                        w.tilde(&w.cwd)
                    ),
                    Tone::Muted,
                )],
            },
            RowKey::Instance(id) => match w.entry(id) {
                Some(InstanceEntry::Available(i)) => vec![
                    vec![
                        Span::new(
                            format!("{} · {} · ", i.role_label(), i.agent.label()),
                            Tone::Secondary,
                        ),
                        Span::new(
                            w.surfaces.vocabulary.status_label(i.status),
                            status_word_tone(i.status, Tone::Secondary),
                        ),
                        Span::new(
                            format!(" · {}", w.surfaces.vocabulary.dirty_summary(i)),
                            Tone::Secondary,
                        ),
                    ],
                    one(
                        match &i.daemon {
                            DaemonSnapshot::Tabs(tabs) => fill_in(
                                &w.surfaces.vocabulary.daemon.list_tabs,
                                &[
                                    ("tabs", &plural(tabs.len(), "tab", "tabs")),
                                    (
                                        "panes",
                                        &plural(
                                            tabs.iter().map(|t| t.panes.len()).sum(),
                                            "pane",
                                            "panes",
                                        ),
                                    ),
                                ],
                            ),
                            DaemonSnapshot::NoTabs => {
                                w.surfaces.vocabulary.daemon.list_no_tabs.clone()
                            }
                            DaemonSnapshot::Unavailable => {
                                w.surfaces.vocabulary.daemon.list_unavailable.clone()
                            }
                        },
                        Tone::Muted,
                    ),
                ],
                Some(InstanceEntry::Unavailable(i)) => {
                    let display = i.display();
                    let power = if i.phase.ready() {
                        "stop available · t"
                    } else if i.phase.startable() {
                        "start available · r"
                    } else {
                        "start only available while stopped"
                    };
                    vec![
                        one(
                            format!("Gateway {} · phase {}", display.gateway, i.phase.label()),
                            Tone::Warning,
                        ),
                        one(format!("Power {power}"), Tone::Muted),
                        one(
                            format!("Agent metadata unavailable: {}", display.reason),
                            Tone::Warning,
                        ),
                    ]
                }
                None => vec![],
            },
            RowKey::NewWorkspace => vec![one(
                "Enter starts the five-step create chain".into(),
                Tone::Muted,
            )],
            RowKey::UnattributedSandboxes => vec![one(
                "These named sandboxes have no configured workspace".into(),
                Tone::Warning,
            )],
        };
        for (i, l) in lines.iter().enumerate() {
            if inner.y + i as u16 >= inner.bottom() {
                break;
            }
            termrock::widgets::viewport::render_spans(
                buf,
                inner.x,
                inner.y + i as u16,
                inner.width,
                t,
                l,
                bg,
            );
        }
        ctx.clickable(DETAIL, area);
    }
}

impl ManagerScreen {
    fn click(&mut self, id: WidgetId, w: &mut World, cx: &mut Cx) -> Outcome {
        self.ensure_rows(w);
        if self.menu.is_open() {
            let (o, chosen) = self.menu.on_click(id);
            return match chosen {
                Some((target, cmd)) => self.run_menu(target, cmd, w, cx),
                None => o,
            };
        }
        for i in self.scroll.visible_range() {
            if Self::toggle_id(i) == id {
                if let RowKey::Workspace(ws) = self.rows[i].key.clone() {
                    self.selected = RowKey::Workspace(ws);
                    self.toggle_expand(ws);
                    self.ensure_rows(w);
                }
                cx.focus.focus(TREE);
                return Outcome::Changed;
            }
            if Self::row_id(i) == id {
                let same = self.selected == self.rows[i].key;
                self.selected = self.rows[i].key.clone();
                self.detail_cursor = 0;
                self.md.close_drawer(cx.focus);
                if same {
                    return self.activate(w, cx);
                }
                return Outcome::Changed;
            }
        }
        if id == DETAIL {
            self.md.open_drawer(cx.focus);
            return Outcome::Changed;
        }
        for i in 0..self.detail_rows.len() {
            if Self::detail_row_id(i) == id {
                let was = self.detail_cursor == i && cx.focus.is(DETAIL);
                self.detail_cursor = i;
                cx.focus.focus(DETAIL);
                if was
                    && let (RowKey::Instance(iid), Some(DetailRow::Pane { pane, .. })) =
                        (self.selected.clone(), self.detail_rows.get(i))
                {
                    let pane = *pane;
                    cx.go(Go::Attach {
                        instance: iid,
                        pane: Some(pane),
                    });
                }
                return Outcome::Changed;
            }
        }
        for i in 0..self.actions.len() {
            if self.actions[i].id == id {
                cx.focus.focus(id);
                if self.actions[i].on_click() {
                    let names = [
                        "reconnect",
                        "session",
                        "shell",
                        "inspect",
                        "stop",
                        "purge",
                        "launch",
                        "edit",
                        "prewarm",
                        "delete",
                        "new",
                    ];
                    for n in names {
                        if DETAIL.sub(n) == id {
                            return self.fire_action(n, w, cx);
                        }
                    }
                }
                return Outcome::Changed;
            }
        }
        if id == ROSTER {
            cx.focus.focus(ROSTER);
            return Outcome::Changed;
        }

        if let Some(entry) = w
            .running_entries()
            .into_iter()
            .find(|e| ROSTER.part(roster_row(e.id())) == id)
        {
            let inst = entry.id().clone();
            self.select_instance(&inst, w);
            cx.focus.focus(TREE);
            return Outcome::Changed;
        }
        Outcome::Ignored
    }

    fn secondary(&mut self, id: WidgetId, pos: Position, w: &mut World, cx: &mut Cx) -> Outcome {
        self.ensure_rows(w);

        let in_detail = id == DETAIL
            || (0..self.detail_rows.len()).any(|k| Self::detail_row_id(k) == id)
            || self.actions.iter().any(|b| b.id == id);
        if in_detail && let Some(i) = self.cursor() {
            if self.actions.iter().any(|b| b.id == id) {
                cx.focus.focus(id);
            } else {
                cx.focus.focus(DETAIL);
            }
            self.open_menu(i, ContextAnchor::Pointer(pos), w);
            return Outcome::Changed;
        }
        let Some(i) = self
            .scroll
            .visible_range()
            .find(|&i| Self::row_id(i) == id || Self::toggle_id(i) == id)
        else {
            return Outcome::Ignored;
        };
        if self.menu.config.moves_cursor && self.selected != self.rows[i].key {
            self.selected = self.rows[i].key.clone();
            self.detail_cursor = 0;
            self.detail_scroll.jump_start();
        }
        self.md.close_drawer(cx.focus);
        self.open_menu(i, ContextAnchor::Pointer(pos), w);
        Outcome::Changed
    }

    fn double_click(&mut self, id: WidgetId, w: &mut World, cx: &mut Cx) -> Outcome {
        if self.menu.is_open() {
            return Outcome::Ignored;
        }
        for i in self.scroll.visible_range() {
            if Self::row_id(i) == id {
                self.selected = self.rows[i].key.clone();
                return self.activate(w, cx);
            }
        }
        Outcome::Ignored
    }

    fn wheel(&mut self, id: WidgetId, delta: i32) -> Outcome {
        if id == TREE || id == scrollbar::id_for(TREE) {
            self.scroll.scroll_by(delta as isize);
            return Outcome::Changed;
        }
        if id == DETAIL || id == scrollbar::id_for(DETAIL) {
            self.detail_scroll.scroll_by(delta as isize);
            return Outcome::Changed;
        }
        Outcome::Ignored
    }
}
