pub mod activity;
pub mod cleanup;
pub mod disk;
pub mod files;
pub mod finder;
pub mod modals;
pub mod plan;
pub mod review;
pub mod snapshot;

use junie_tui::core::event::Outcome;
use junie_tui::core::scroll::ScrollState;
use junie_tui::theme::Tone;
use junie_tui::ui::layer::{LayerEntry, ShellLayers};
use junie_tui::ui::layers::LayerResult;
use junie_tui::widgets::menu::MenuAction;
use junie_tui::widgets::picker::PickerAction;
use junie_tui::widgets::scrollbar;
use junie_tui::widgets::statusbar::StatusItem;
use ratatui::layout::{Position, Rect};

use crate::domain::context::Scope;
use crate::sim::world::{Msg, World};

pub type Layers = ShellLayers<ModalResult, (), usize>;

pub type LayerSlot = LayerEntry<ModalResult, (), usize>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModalResult {
    Dialog {
        action: Option<usize>,
        text: Option<String>,
    },
    Picked(usize),
    Cancelled,
    MenuChosen(usize),
    Closed,

    PickerEvent(PickerAction),

    Other(LayerResult),
}

impl From<LayerResult> for ModalResult {
    fn from(r: LayerResult) -> Self {
        match r {
            LayerResult::Dialog { action, text } => Self::Dialog { action, text },
            LayerResult::Picker(PickerAction::Chosen(i) | PickerAction::ChosenAlt(i)) => {
                Self::Picked(i)
            }
            LayerResult::Picker(PickerAction::Cancelled) => Self::Cancelled,
            LayerResult::Picker(ev) => Self::PickerEvent(ev),
            LayerResult::Menu(MenuAction::Chosen(i)) => Self::MenuChosen(i),
            LayerResult::Menu(MenuAction::Dismissed) => Self::Cancelled,
            LayerResult::Help => Self::Closed,
            other => Self::Other(other),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Go {
    Run {
        item: String,
        args: Vec<(String, String)>,
    },

    Alternatives {
        item: String,
        anchor: Rect,
    },

    Push(Page),

    Pop,

    Here,

    Scope(Scope),

    ConfirmPlan(String),

    Gate1Accepted(GateTarget),

    Trusted {
        config: String,
        then: String,
    },

    CleanupPlan(Vec<String>),

    Restart(String),

    CloseTab,

    Reload,

    Exec {
        label: String,
        argv: Vec<crate::domain::exec::Command>,
    },

    Osc52(String),

    Analyze(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateTarget {
    Plan(String),
    Item {
        item: String,
        args: Vec<(String, String)>,
    },

    Cleanup(crate::domain::cleanup::DeletePlan),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Page {
    Finder {
        group: Option<&'static str>,
    },
    Disk {
        path: String,
    },
    PlanReview {
        plan: String,
    },
    Review(GateTarget),
    Trust {
        config: String,
        then: String,
    },
    Args {
        item: String,
    },
    Snapshot {
        kind: String,
    },

    Files {
        path: String,
        query: Option<String>,
    },

    Find,

    DiskOverview,

    TopFiles,

    Cleanup {
        category: Option<String>,
    },

    CleanupGate {
        plan: crate::domain::cleanup::DeletePlan,
    },

    Report {
        index: usize,
    },

    Config {
        path: String,
    },
}

pub type Request = junie_tui::shell::Request<ModalResult, (), Go>;

pub type Cx<'a> = junie_tui::shell::Cx<'a, ModalResult, (), Go>;

#[derive(Default)]
pub struct StatusBits {
    pub center: Option<StatusItem>,
    pub right: Vec<StatusItem>,
}

pub struct Holla;

impl junie_tui::shell::PageTypes for Holla {
    type State = World;
    type Msg = Msg;
    type Result = ModalResult;
    type LayerState = ();
    type Go = Go;
}

pub use junie_tui::shell::{open_context, rebuild_on_tick, row_anchor, row_choice};

impl ModalResult {
    pub fn menu_chosen(&self) -> Option<usize> {
        match *self {
            Self::MenuChosen(k) => Some(k),
            _ => None,
        }
    }
}

pub trait Screen: junie_tui::shell::Page<Holla> {
    fn on_context_key(&mut self, _w: &mut World, _cx: &mut Cx) -> Outcome {
        Outcome::Ignored
    }
    fn status(&self, _w: &World) -> StatusBits {
        StatusBits::default()
    }

    fn typing_hot(&self) -> bool {
        false
    }

    fn as_finder(&mut self) -> Option<&mut finder::FinderPage> {
        None
    }

    fn as_files(&mut self) -> Option<&mut files::FilesPage> {
        None
    }
}

pub fn ticks_label(ticks: u64) -> String {
    junie_tui::ui::text::duration(
        std::time::Duration::from_millis(ticks * crate::sim::world::TICK_MS as u64),
        junie_tui::ui::text::DurationStyle::Spoken,
    )
}

pub fn risk_tone(risk: crate::domain::action::Risk) -> Tone {
    use crate::domain::action::Risk;
    match risk {
        Risk::ReadOnly => Tone::Faint,
        Risk::Mutating => Tone::Muted,
        Risk::Destructive => Tone::Error,
        Risk::Privileged => Tone::Warning,
    }
}

pub(crate) fn scroll_press(area: Rect, pos: Position, scroll: &mut ScrollState) -> Outcome {
    let track = Rect::new(area.right().saturating_sub(1), area.y, 1, area.height);
    if scrollbar::press(track, pos, scroll) {
        Outcome::Changed
    } else {
        Outcome::Consumed
    }
}

pub(crate) fn scroll_drag(area: Rect, pos: Position, scroll: &mut ScrollState) -> Outcome {
    let track = Rect::new(area.right().saturating_sub(1), area.y, 1, area.height);
    if scrollbar::drag(track, pos, scroll) {
        Outcome::Changed
    } else {
        Outcome::Consumed
    }
}
