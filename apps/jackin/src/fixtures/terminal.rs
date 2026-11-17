//! The scenario `capsule-terminal`: a Capsule whose tabs hold terminal panes in
//! the link states a pane can be in, each over a scripted link.

use std::sync::Arc;

use termrock::ui::layout::SplitDir;

use crate::domain::agent::Agent;
use crate::domain::instance::InstanceId;
use crate::panes::{
    Daemon, FailNext, LinkFault, PaneBody, PaneNode, PaneRequest, PaneSpawn, PaneTiming,
    TerminalPane,
};
use crate::sim::link::{Play, SimSource, Stream};
use crate::world::{World, refresh_snapshots};

const INSTANCE: &str = "jk-7f3a";

fn opened(
    request: &PaneRequest,
    play: Play,
    timing: PaneTiming,
) -> impl FnOnce(PaneSpawn) -> Box<dyn PaneBody> + '_ {
    move |spawn| {
        Box::new(TerminalPane::new(
            spawn.id,
            request.clone(),
            Arc::new(SimSource::new(play)),
            timing,
            spawn.now_ms,
        ))
    }
}

fn label(d: &mut Daemon, tab: usize, text: &str) {
    if let Some(t) = d.tabs.get_mut(tab) {
        t.custom_label = Some(text.into());
    }
}

/// The OpenShell world with one running instance whose seven tabs are:
/// `live` (Codex beside a pager), `pending` (a pane with no room), `attaching`,
/// `viewing` (read only), `detached`, `exited` and `failed`.
pub fn capsule_terminal(w: &mut World) {
    super::surfaces::fixed(w);
    let timing = w.backend.pane_timing();
    let now = w.now_ms();
    let instance = InstanceId::simulated(INSTANCE);
    let codex = PaneRequest::new(
        &instance,
        Some(Agent::Codex),
        Some("acct-codex-primary".into()),
    );
    let shell = PaneRequest::new(&instance, None, None);
    let mut d = Daemon::new("payments-platform");

    d.new_tab(
        &codex,
        now,
        opened(&codex, Play::Stays(Stream::Codex), timing),
    );
    d.split(
        SplitDir::Horizontal,
        false,
        &shell,
        now,
        opened(&shell, Play::Stays(Stream::Pager), timing),
    );
    if let Some(t) = d.active_tab_mut() {
        t.focused = 1;
    }
    label(&mut d, 0, "live");

    d.new_tab(
        &shell,
        now,
        opened(&shell, Play::Stays(Stream::Shell), timing),
    );
    d.split(
        SplitDir::Vertical,
        false,
        &codex,
        now,
        opened(&codex, Play::Stays(Stream::Codex), timing),
    );
    if let Some(PaneNode::Split { split, .. }) = d.active_tab_mut().map(|t| &mut t.root) {
        split.percent = 92;
    }
    label(&mut d, 1, "pending");

    d.new_tab(&codex, now, opened(&codex, Play::Hangs, timing));
    label(&mut d, 2, "attaching");

    d.new_tab(
        &codex,
        now,
        opened(&codex, Play::Watches(Stream::Codex), timing),
    );
    d.split(
        SplitDir::Horizontal,
        false,
        &shell,
        now,
        opened(&shell, Play::Stays(Stream::Shell), timing),
    );
    if let Some(t) = d.active_tab_mut() {
        t.focused = 6;
    }
    label(&mut d, 3, "viewing");

    d.new_tab(
        &codex,
        now,
        opened(&codex, Play::Detaches(Stream::Codex), timing),
    );
    label(&mut d, 4, "detached");

    d.new_tab(
        &codex,
        now,
        opened(&codex, Play::Exits(Stream::Codex), timing),
    );
    label(&mut d, 5, "exited");

    d.new_tab(
        &codex,
        now,
        opened(
            &codex,
            Play::Refuses(LinkFault::new("the gateway is unreachable").then(FailNext::RunDoctor)),
            timing,
        ),
    );
    label(&mut d, 6, "failed");

    d.active = 0;
    w.daemons.insert(instance.clone(), d);
    if let Some(i) = w
        .instances
        .iter_mut()
        .find(|entry| entry.id() == &instance)
        .and_then(crate::domain::instance::InstanceEntry::as_available_mut)
    {
        i.agents = vec![
            crate::domain::instance::AgentInstanceView {
                agent: Agent::Codex,
                provider: crate::domain::agent::Provider::OpenAi,
                account: Some("acct-codex-primary".into()),
                model: "gpt-5-codex".into(),
                branch: None,
                state: crate::domain::instance::AgentState::Working,
            },
            crate::domain::instance::AgentInstanceView {
                agent: Agent::Omp,
                provider: crate::domain::agent::Provider::OpenCode,
                account: None,
                model: "omp-default".into(),
                branch: None,
                state: crate::domain::instance::AgentState::Idle,
            },
        ];
        i.task = Some(crate::domain::instance::TaskView {
            text: "Ship the multi-agent launch".into(),
            statuses: vec![
                crate::domain::instance::InstanceStatusView {
                    agent: Agent::Codex,
                    text: "working".into(),
                },
                crate::domain::instance::InstanceStatusView {
                    agent: Agent::Omp,
                    text: "idle".into(),
                },
            ],
        });
    }
    refresh_snapshots(w);
    w.sync_arbiter();
}
