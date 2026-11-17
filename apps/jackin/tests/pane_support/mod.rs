#![allow(dead_code)]

//! The test side of a terminal pane: a link source that hands out links which
//! store what the pane sends and replay what a test pushes, so every
//! assertion reads that stored state and the pane's own answers.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use jackin_console::domain::agent::Agent;
use jackin_console::domain::instance::InstanceId;
use jackin_console::domain::vocabulary::{PaneWords, Vocabulary};
use jackin_console::panes::{
    Lease, LinkEvent, LinkFault, LinkSource, PaneBody, PaneCommand, PaneLink, PanePolicy,
    PaneRequest, PaneSize, PaneTiming, Sent, TerminalPane,
};
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;
use termrock::core::event::Key;

pub const INSTANCE: &str = "jk-test";

pub fn instance_id() -> InstanceId {
    InstanceId::simulated(INSTANCE)
}

#[derive(Debug, Default)]
pub struct LinkLog {
    pub commands: Vec<PaneCommand>,
    pub sizes: Vec<PaneSize>,
    pub inbox: VecDeque<LinkEvent>,
    pub drains: Vec<i64>,
    pub full: bool,
    pub dropped: bool,
}

pub type Log = Arc<Mutex<LinkLog>>;

#[derive(Debug)]
pub struct TestLink(Log);

impl Drop for TestLink {
    fn drop(&mut self) {
        self.0.lock().expect("log").dropped = true;
    }
}

impl PaneLink for TestLink {
    fn send(&self, command: PaneCommand) -> Sent {
        let mut log = self.0.lock().expect("log");
        if log.full {
            return Sent::Full;
        }
        log.commands.push(command);
        Sent::Queued
    }

    fn set_size(&self, size: PaneSize) {
        self.0.lock().expect("log").sizes.push(size);
    }

    fn drain(&self, now_ms: i64, output_budget: usize) -> Vec<LinkEvent> {
        let mut log = self.0.lock().expect("log");
        log.drains.push(now_ms);
        let mut out = Vec::new();
        let mut bytes = 0;
        while let Some(event) = log.inbox.pop_front() {
            if let LinkEvent::Output(chunk) = &event {
                if bytes > 0 && bytes + chunk.len() > output_budget {
                    log.inbox.push_front(event);
                    break;
                }
                bytes += chunk.len();
            }
            out.push(event);
        }
        out
    }
}

#[derive(Debug, Default)]
pub struct Bench {
    links: Mutex<Vec<Log>>,
    refusals: Mutex<VecDeque<LinkFault>>,
    restarted: Mutex<Vec<usize>>,
}

impl Bench {
    fn link(&self, size: PaneSize) -> Result<Box<dyn PaneLink>, LinkFault> {
        if let Some(reason) = self.refusals.lock().expect("refusals").pop_front() {
            return Err(reason);
        }
        let log = Log::default();
        log.lock().expect("log").sizes.push(size);
        self.links.lock().expect("links").push(log.clone());
        Ok(Box::new(TestLink(log)))
    }
}

impl LinkSource for Bench {
    fn open(&self, _: &PaneRequest, size: PaneSize) -> Result<Box<dyn PaneLink>, LinkFault> {
        self.link(size)
    }

    fn restart(&self, _: &PaneRequest, size: PaneSize) -> Result<Box<dyn PaneLink>, LinkFault> {
        let link = self.link(size);
        if link.is_ok() {
            self.restarted
                .lock()
                .expect("restarted")
                .push(self.opened() - 1);
        }
        link
    }
}

impl Bench {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn refuse_next(&self, reason: &str) {
        self.refuse_with(LinkFault::new(reason));
    }

    pub fn refuse_with(&self, fault: LinkFault) {
        self.refusals.lock().expect("refusals").push_back(fault);
    }

    /// The numbers of the links that were opened by a restart.
    pub fn restarted(&self) -> Vec<usize> {
        self.restarted.lock().expect("restarted").clone()
    }

    pub fn opened(&self) -> usize {
        self.links.lock().expect("links").len()
    }

    pub fn log(&self, n: usize) -> Log {
        self.links.lock().expect("links")[n].clone()
    }

    pub fn push(&self, n: usize, event: LinkEvent) {
        self.log(n).lock().expect("log").inbox.push_back(event);
    }

    pub fn fill(&self, n: usize, full: bool) {
        self.log(n).lock().expect("log").full = full;
    }

    pub fn dropped(&self, n: usize) -> bool {
        self.log(n).lock().expect("log").dropped
    }

    pub fn output(&self, n: usize, bytes: &[u8]) {
        self.push(n, LinkEvent::Output(bytes.to_vec()));
    }

    pub fn commands(&self, n: usize) -> Vec<PaneCommand> {
        self.log(n).lock().expect("log").commands.clone()
    }

    pub fn sizes(&self, n: usize) -> Vec<PaneSize> {
        self.log(n).lock().expect("log").sizes.clone()
    }
}

pub fn words() -> PaneWords {
    Vocabulary::openshell().pane
}

pub fn codex() -> PaneRequest {
    PaneRequest::new(&instance_id(), Some(Agent::Codex), None)
}

pub fn shell() -> PaneRequest {
    PaneRequest::new(&instance_id(), None, None)
}

pub fn pane_for(request: PaneRequest, bench: &Arc<Bench>, policy: PanePolicy) -> TerminalPane {
    let mut pane = TerminalPane::new(1, request, bench.clone(), PaneTiming::default(), 0);
    pane.set_policy(&Arc::new(policy));
    pane
}

pub fn pane(bench: &Arc<Bench>) -> TerminalPane {
    pane_for(codex(), bench, PanePolicy::default())
}

pub fn size(rows: u16, cols: u16) -> PaneSize {
    PaneSize { rows, cols }
}

/// The inner area of a pane that gives its program `cols` columns and `rows`
/// rows: one more column goes to the scroll track.
pub fn inner(cols: u16, rows: u16) -> Rect {
    Rect::new(0, 0, cols + 1, rows)
}

pub fn key(spec: &str) -> Key {
    let (mods, name) = match spec.strip_prefix("ctrl+") {
        Some(rest) => (KeyModifiers::CONTROL, rest),
        None => (KeyModifiers::NONE, spec),
    };
    let code = match name {
        "enter" => KeyCode::Enter,
        "esc" => KeyCode::Esc,
        "up" => KeyCode::Up,
        "f10" => KeyCode::F(10),
        "right" => KeyCode::Right,
        "down" => KeyCode::Down,
        "tab" => KeyCode::Tab,
        c if c.chars().count() == 1 => KeyCode::Char(c.chars().next().expect("a char")),
        other => panic!("unknown key {other}"),
    };
    Key { code, mods }
}

/// A pane with a live program: sized, attached, and past its replay window.
pub fn live(bench: &Arc<Bench>) -> TerminalPane {
    let mut pane = pane(bench);
    pane.sync_size(inner(80, 24));
    bench.push(0, LinkEvent::Attached);
    bench.push(0, LinkEvent::Lease(Lease::Owner));
    pane.tick(0);
    pane.tick(800);
    pane
}

pub fn row(pane: &TerminalPane, row: u16) -> String {
    let view = pane.grid().state().view(0);
    (0..view.cols())
        .filter_map(|col| view.cell(row, col))
        .filter(|cell| !cell.is_wide_continuation())
        .map(|cell| match cell.text() {
            "" => " ".to_owned(),
            text => text.to_owned(),
        })
        .collect::<String>()
        .trim_end()
        .to_owned()
}

pub fn input(bytes: &[u8]) -> PaneCommand {
    PaneCommand::Input(bytes.to_vec())
}
