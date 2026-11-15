use std::sync::Arc;

use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::hit::Axis;
use junie_tui::theme::Tone;
use junie_tui::ui::layout::SplitDir;
use junie_tui::widgets::viewport_ops::ViewportOps;
use ratatui::layout::Rect;

use crate::domain::account::AccountId;
use crate::domain::agent::Agent;
use crate::domain::instance::{AgentState, InstanceId};
use crate::domain::vocabulary::PaneWords;

mod clipboard;
mod link;
mod policy;
mod session;
mod terminal;

pub use link::{
    DetachCause, EndKind, FailNext, Lease, LinkEvent, LinkFault, LinkSource, LinkStatus,
    PaneCommand, PaneLink, PaneSize, Sent,
};
pub use policy::{ClipboardWrite, PanePolicy, PaneTiming};
pub use terminal::{TerminalPane, sanitize_title};

pub use junie_tui::ui::pane_tree::PaneId;

/// The rows of history a terminal pane keeps.
pub const SCROLLBACK: usize = 2_000;

pub const MIN_PANE_COLS: u16 = 20;
pub const MIN_PANE_ROWS: u16 = 4;
pub const MAX_LABEL: usize = 16;

pub use junie_tui::ui::pane_tree::{Direction, PaneNode, Seam, nearest};

#[derive(Debug, Clone)]
pub struct Tab {
    pub custom_label: Option<String>,
    pub root: PaneNode,
    pub focused: PaneId,
    pub zoomed: Option<PaneId>,
}

impl Tab {
    pub fn leaves(&self) -> Vec<PaneId> {
        self.root.leaves()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneKindTag {
    Agent,
    Shell,
    Script,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneKey {
    /// The capsule swallowed the key: nothing reached the pane.
    Consumed,
    /// The pane took the key.
    Forwarded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneNote {
    pub text: String,
    pub tone: Tone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteOutcome {
    Sent,
    NeedsConfirm {
        lines: usize,
    },
    TooLarge,
    /// The link's queue is full: nothing was sent.
    Full,
    /// The pane has no program to take it.
    Dropped,
}

/// What a pane shows in place of its screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaneFace {
    /// The screen of the program.
    Grid,
    /// The last screen, dimmed: nothing changes on it any more.
    Dimmed,
    /// An empty state: there is no screen to show yet.
    Empty { title: String, hint: Option<String> },
}

/// What a pane asks the console to do for it. The console takes them from the
/// pane on its tick and runs them on the screen's own request channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaneEffect {
    Status {
        text: String,
        tone: Tone,
    },
    /// Copy `text` as the operator's copy; the status reads `Copied {what}`.
    Copy {
        text: String,
        what: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaneKind {
    Agent {
        agent: Agent,
        account: Option<AccountId>,
    },
    Shell,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneRequest {
    pub instance: InstanceId,
    pub kind: PaneKind,
}

impl PaneRequest {
    pub fn new(instance: &InstanceId, agent: Option<Agent>, account: Option<AccountId>) -> Self {
        Self {
            instance: instance.clone(),
            kind: match agent {
                Some(agent) => PaneKind::Agent { agent, account },
                None => PaneKind::Shell,
            },
        }
    }

    pub fn agent(&self) -> Option<Agent> {
        match self.kind {
            PaneKind::Agent { agent, .. } => Some(agent),
            PaneKind::Shell => None,
        }
    }

    pub fn account(&self) -> Option<&AccountId> {
        match &self.kind {
            PaneKind::Agent { account, .. } => account.as_ref(),
            PaneKind::Shell => None,
        }
    }
}

/// What the daemon decides before a body exists and hands to the backend that
/// builds it: the id the pane will have, the daemon's workspace name and the
/// time of the request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneSpawn {
    pub id: PaneId,
    pub workspace: String,
    pub now_ms: i64,
}

/// Why a pane of some kind may not start in an instance now. The new-tab
/// picker words it with `PaneWords::refusal`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaneRefusal {
    /// The instance's main process has its one pane already (one pane for
    /// each main process).
    MainProcessOpen { agent: Agent },
}

/// The inside of a pane. The capsule draws, sizes, scrolls and types into a
/// pane only through this trait, so it does not know what runs behind it.
///
/// There is no `Send` bound: `TextViewport` holds an `Rc` (the edit menu's
/// validator), and the `World` that owns the panes is not `Send` either.
pub trait PaneBody: std::fmt::Debug {
    fn kind(&self) -> PaneKindTag;
    fn state(&self) -> AgentState;
    fn title(&self) -> Option<&str>;
    /// The one row the pane says over its bottom edge when its link is not
    /// live, in the words the console gives it.
    fn note(&self, words: &PaneWords) -> Option<PaneNote>;
    /// A word after the pane's name in its border.
    fn suffix(&self, _words: &PaneWords) -> Option<String> {
        None
    }
    fn face(&self, _words: &PaneWords) -> PaneFace {
        PaneFace::Grid
    }
    fn viewport(&self) -> &dyn ViewportOps;
    fn viewport_mut(&mut self) -> &mut dyn ViewportOps;
    /// Whether a program is on the other end to take what is typed. The
    /// hardware cursor belongs to such a pane only.
    fn accepts_input(&self) -> bool {
        true
    }
    fn key(&mut self, key: &Key, now_ms: i64) -> PaneKey;
    /// The operator clicked in the pane.
    fn gesture(&mut self, _now_ms: i64) {}
    /// A wheel step the pane takes for itself, or `None` to let the viewport
    /// scroll.
    fn wheel(&mut self, _axis: Axis, _delta: i32) -> Option<Outcome> {
        None
    }
    fn paste(&mut self, text: &str, now_ms: i64) -> PasteOutcome;
    /// A paste the operator confirmed after `NeedsConfirm`.
    fn paste_confirmed(&mut self, text: &str, now_ms: i64) -> PasteOutcome {
        self.paste(text, now_ms)
    }
    fn tick(&mut self, now_ms: i64) -> bool;
    /// What the pane asks of the console since the last call.
    fn take_effects(&mut self, _words: &PaneWords) -> Vec<PaneEffect> {
        Vec::new()
    }
    /// The console's policy for what a program may do through a pane.
    fn set_policy(&mut self, _policy: &Arc<PanePolicy>) {}
    fn sync_size(&mut self, inner: Rect);
    fn focus_changed(&mut self, focused: bool, now_ms: i64);
    fn redraw(&mut self);
    fn clear(&mut self);
    fn touched(&self) -> &[String];
    fn sends_prefix(&self) -> bool;
    fn close_needs_confirm(&self) -> bool;
}

#[derive(Debug)]
pub struct Pane {
    pub id: PaneId,
    pub label: String,
    pub agent: Option<Agent>,
    pub account: Option<AccountId>,
    pub body: Box<dyn PaneBody>,
}

#[derive(Debug)]
pub struct Daemon {
    pub workspace: String,
    pub tabs: Vec<Tab>,
    pub active: usize,
    pub panes: Vec<Pane>,
    pub next_id: u64,

    pub attached_by: Option<String>,
    pub policy: Arc<PanePolicy>,
}

impl Daemon {
    pub fn new(workspace: &str) -> Self {
        Self {
            workspace: workspace.to_owned(),
            tabs: vec![],
            active: 0,
            panes: vec![],
            next_id: 1,
            attached_by: None,
            policy: Arc::new(PanePolicy::default()),
        }
    }

    /// Tells every pane the operator is no longer looking at it.
    pub fn blur(&mut self, now_ms: i64) {
        for pane in &mut self.panes {
            pane.body.focus_changed(false, now_ms);
        }
    }

    /// Gives the daemon, and every pane in it, the console's policy.
    pub fn set_policy(&mut self, policy: &Arc<PanePolicy>) {
        self.policy = Arc::clone(policy);
        for pane in &mut self.panes {
            pane.body.set_policy(policy);
        }
    }

    pub fn pane(&self, id: PaneId) -> Option<&Pane> {
        self.panes.iter().find(|p| p.id == id)
    }

    pub fn pane_mut(&mut self, id: PaneId) -> Option<&mut Pane> {
        self.panes.iter_mut().find(|p| p.id == id)
    }

    pub fn active_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active)
    }

    pub fn active_tab_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.get_mut(self.active)
    }

    pub fn focused_pane(&self) -> Option<PaneId> {
        self.active_tab().map(|t| t.focused)
    }

    fn alloc(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn new_pane(
        &mut self,
        request: &PaneRequest,
        now_ms: i64,
        make: impl FnOnce(PaneSpawn) -> Box<dyn PaneBody>,
    ) -> PaneId {
        let id = self.alloc();
        let agent = request.agent();
        let mut body = make(PaneSpawn {
            id,
            workspace: self.workspace.clone(),
            now_ms,
        });
        body.set_policy(&self.policy);
        self.panes.push(Pane {
            id,
            label: agent.map_or_else(|| "Shell".to_owned(), |a| a.label().to_owned()),
            agent,
            account: request.account().cloned(),
            body,
        });
        id
    }

    pub fn new_tab(
        &mut self,
        request: &PaneRequest,
        now_ms: i64,
        make: impl FnOnce(PaneSpawn) -> Box<dyn PaneBody>,
    ) -> usize {
        let pid = self.new_pane(request, now_ms, make);
        self.tabs.push(Tab {
            custom_label: None,
            root: PaneNode::Leaf(pid),
            focused: pid,
            zoomed: None,
        });
        self.active = self.tabs.len() - 1;
        self.active
    }

    pub fn split(
        &mut self,
        dir: SplitDir,
        new_first: bool,
        request: &PaneRequest,
        now_ms: i64,
        make: impl FnOnce(PaneSpawn) -> Box<dyn PaneBody>,
    ) -> Option<PaneId> {
        let target = self.focused_pane()?;
        let pid = self.new_pane(request, now_ms, make);
        let tab = self.active_tab_mut()?;
        if tab.root.split_leaf(target, pid, dir, new_first) {
            tab.focused = pid;
            tab.zoomed = None;
            Some(pid)
        } else {
            self.panes.retain(|p| p.id != pid);
            None
        }
    }

    pub fn close_pane(&mut self, id: PaneId) -> bool {
        let Some(ti) = self.tabs.iter().position(|t| t.leaves().contains(&id)) else {
            return false;
        };
        self.panes.retain(|p| p.id != id);
        let tab = &mut self.tabs[ti];
        if tab.leaves().len() <= 1 {
            self.remove_tab(ti);
            return true;
        }
        tab.root.remove_leaf(id);
        if tab.focused == id {
            tab.focused = tab.leaves()[0];
        }
        if tab.zoomed == Some(id) {
            tab.zoomed = None;
        }
        false
    }

    pub fn close_tab(&mut self, i: usize) {
        if i >= self.tabs.len() {
            return;
        }
        let leaves = self.tabs[i].leaves();
        self.panes.retain(|p| !leaves.contains(&p.id));
        self.remove_tab(i);
    }

    fn remove_tab(&mut self, i: usize) {
        self.tabs.remove(i);
        if i < self.active {
            self.active -= 1;
        } else if self.active >= self.tabs.len() {
            self.active = self.tabs.len().saturating_sub(1);
        }
    }

    pub fn tab_label(&self, tab: &Tab, account_suffix: &dyn Fn(&Pane) -> Option<String>) -> String {
        if let Some(c) = &tab.custom_label {
            return c.clone();
        }
        let leaves = tab.leaves();
        let mut agents: Vec<String> = vec![];
        let mut has_shell = false;
        for id in &leaves {
            if let Some(p) = self.pane(*id) {
                match p.agent {
                    Some(a) => {
                        let mut l = a.label().to_owned();
                        if let Some(s) = account_suffix(p) {
                            l = format!("{l} ({s})");
                        }
                        if !agents.contains(&l) {
                            agents.push(l);
                        }
                    }
                    None => has_shell = true,
                }
            }
        }
        let base = match (agents.len(), has_shell) {
            (0, _) => "Shell".to_owned(),
            (1, false) => agents[0].clone(),
            (_, false) => "Agents".to_owned(),
            (_, true) => "Mix".to_owned(),
        };
        if leaves.len() > 1 {
            format!("{base} ({})", leaves.len())
        } else {
            base
        }
    }

    pub fn tab_state(&self, tab: &Tab) -> AgentState {
        let states: Vec<AgentState> = tab
            .leaves()
            .iter()
            .filter_map(|id| self.pane(*id).map(|p| p.body.state()))
            .collect();
        states
            .iter()
            .copied()
            .filter(|s| *s != AgentState::Done)
            .max_by_key(|s| s.rank())
            .or(states.first().copied())
            .unwrap_or(AgentState::Unknown)
    }

    pub fn tick(&mut self, now_ms: i64) -> bool {
        let mut changed = false;
        for p in &mut self.panes {
            junie_tui::record_work!(SchedulingPolls, 1usize);
            changed |= p.body.tick(now_ms);
        }
        changed
    }

    pub fn touched_files(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .panes
            .iter()
            .flat_map(|p| p.body.touched().iter().cloned())
            .collect();
        v.sort();
        v.dedup();
        v
    }

    pub fn snapshot(&self) -> crate::domain::instance::DaemonSnapshot {
        use crate::domain::instance::{DaemonSnapshot, PaneSnapshot, TabSnapshot};
        if self.tabs.is_empty() {
            return DaemonSnapshot::NoTabs;
        }
        DaemonSnapshot::Tabs(
            self.tabs
                .iter()
                .enumerate()
                .map(|(i, t)| TabSnapshot {
                    label: self.tab_label(t, &|_| None),
                    active: i == self.active,
                    panes: t
                        .leaves()
                        .iter()
                        .filter_map(|id| self.pane(*id))
                        .map(|p| PaneSnapshot {
                            label: p.label.clone(),
                            agent: p.agent,
                            state: p.body.state(),
                            focused: t.focused == p.id,
                        })
                        .collect(),
                })
                .collect(),
        )
    }
}
