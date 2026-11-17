mod pane_support;

use std::sync::Arc;

use jackin_console::domain::instance::AgentState;
use jackin_console::panes::{Daemon, Lease, LinkEvent, PaneId, PaneTiming, TerminalPane};
use pane_support::*;
use termrock::ui::layout::SplitDir;

fn make(
    bench: &Arc<Bench>,
) -> impl FnOnce(jackin_console::panes::PaneSpawn) -> Box<dyn jackin_console::panes::PaneBody> {
    let bench = bench.clone();
    move |spawn| {
        Box::new(TerminalPane::new(
            spawn.id,
            codex(),
            bench,
            PaneTiming::default(),
            spawn.now_ms,
        ))
    }
}

fn new_tab(daemon: &mut Daemon, bench: &Arc<Bench>) -> PaneId {
    let tab = daemon.new_tab(&codex(), 0, make(bench));
    daemon.tabs[tab].focused
}

fn split(daemon: &mut Daemon, bench: &Arc<Bench>) -> PaneId {
    daemon
        .split(SplitDir::Horizontal, false, &codex(), 0, make(bench))
        .expect("the focused pane is there")
}

fn start(daemon: &mut Daemon, bench: &Arc<Bench>, pane: PaneId, event: Option<LinkEvent>) {
    let n = bench.opened();
    let body = &mut daemon.pane_mut(pane).expect("pane").body;
    body.sync_size(inner(80, 24));
    bench.push(n, LinkEvent::Attached);
    bench.push(n, LinkEvent::Lease(Lease::Owner));
    body.tick(0);
    body.tick(800);
    if let Some(event) = event {
        bench.push(n, event);
        body.tick(900);
    }
}

fn active_pane(daemon: &Daemon) -> PaneId {
    daemon.active_tab().expect("an active tab").focused
}

#[test]
fn closing_a_tab_before_the_active_one_keeps_the_active_tab() {
    let bench = Bench::new();
    let mut daemon = Daemon::new("dev");
    let first = new_tab(&mut daemon, &bench);
    let second = new_tab(&mut daemon, &bench);
    let third = new_tab(&mut daemon, &bench);
    new_tab(&mut daemon, &bench);
    daemon.active = 2;

    daemon.close_tab(0);
    assert_eq!(daemon.tabs.len(), 3);
    assert_eq!(active_pane(&daemon), third);
    assert!(daemon.pane(first).is_none());

    daemon.close_pane(second);
    assert_eq!(
        active_pane(&daemon),
        third,
        "closing the only pane of a tab before it"
    );
}

#[test]
fn closing_the_active_tab_activates_the_next_one() {
    let bench = Bench::new();
    let mut daemon = Daemon::new("dev");
    new_tab(&mut daemon, &bench);
    new_tab(&mut daemon, &bench);
    let last = new_tab(&mut daemon, &bench);
    daemon.active = 1;
    daemon.close_tab(1);
    assert_eq!(active_pane(&daemon), last);
    daemon.close_tab(1);
    assert_eq!(daemon.active, 0);
}

#[test]
fn a_tab_with_a_live_pane_does_not_show_done() {
    let bench = Bench::new();
    let mut daemon = Daemon::new("dev");
    let exited = new_tab(&mut daemon, &bench);
    start(
        &mut daemon,
        &bench,
        exited,
        Some(LinkEvent::Exited(Some(0))),
    );
    let live = split(&mut daemon, &bench);
    start(&mut daemon, &bench, live, None);

    let tab = daemon.active_tab().expect("tab").clone();
    assert_eq!(
        daemon.pane(exited).expect("pane").body.state(),
        AgentState::Done
    );
    assert_eq!(
        daemon.tab_state(&tab),
        daemon.pane(live).expect("pane").body.state()
    );
}

#[test]
fn a_tab_shows_done_when_every_pane_is_done() {
    let bench = Bench::new();
    let mut daemon = Daemon::new("dev");
    let first = new_tab(&mut daemon, &bench);
    start(&mut daemon, &bench, first, Some(LinkEvent::Exited(Some(0))));
    let second = split(&mut daemon, &bench);
    start(
        &mut daemon,
        &bench,
        second,
        Some(LinkEvent::Exited(Some(0))),
    );

    let tab = daemon.active_tab().expect("tab").clone();
    assert_eq!(daemon.tab_state(&tab), AgentState::Done);
}

#[test]
fn a_blocked_pane_wins_over_every_other_state() {
    let bench = Bench::new();
    let mut daemon = Daemon::new("dev");
    let failed = new_tab(&mut daemon, &bench);
    start(
        &mut daemon,
        &bench,
        failed,
        Some(LinkEvent::Exited(Some(1))),
    );
    let done = split(&mut daemon, &bench);
    start(&mut daemon, &bench, done, Some(LinkEvent::Exited(Some(0))));
    let live = split(&mut daemon, &bench);
    start(&mut daemon, &bench, live, None);

    let tab = daemon.active_tab().expect("tab").clone();
    assert_eq!(
        daemon.pane(done).expect("pane").body.state(),
        AgentState::Done
    );
    assert_eq!(daemon.tab_state(&tab), AgentState::Blocked);
}
