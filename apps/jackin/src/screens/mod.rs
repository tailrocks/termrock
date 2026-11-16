pub mod accounts;
pub mod capsule;
pub mod cockpit;
pub mod config;
pub mod doctor;
pub mod editor;
pub mod manager;
pub mod modals;
pub mod prelude;
pub mod role_editor;
pub mod roles;
pub mod row_meta;
pub mod settings;
pub mod usage;

use termrock::core::event::Outcome;
use termrock::widgets::option_list::Item;
use termrock::widgets::segments::Segment;

use crate::domain::account::AccountId;
use crate::domain::agent::Agent;
use crate::domain::instance::InstanceId;
use crate::domain::onepassword::OpReference;
use crate::domain::workspace::{RoleName, Workspace, WorkspaceId};
use crate::world::{Msg, World};
use modals::{BrowserResult, FormAction, FormDialog, FormValues, InfoResult};
use termrock::ui::layer::Layer;
pub use termrock::ui::layer::LayerTag;
use termrock::ui::layers::LayerResult;
use termrock::widgets::menu::MenuAction;
use termrock::widgets::picker::PickerAction;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModalResult {
    Dialog {
        action: Option<usize>,
        text: Option<String>,
    },
    Picked(usize),
    PickedMany(Vec<termrock::core::id::ItemKey>),
    PickedAlt(usize),

    Scope,

    PickerEvent(PickerAction),
    Cancelled,
    Browser(BrowserResult),
    Choice(Option<usize>),
    Form(Option<FormValues>),

    FormAction(String, FormValues),

    FormEvent(FormAction),
    Op(Option<OpReference>),
    Info(InfoResult),

    Help,
    Custom(String),
}

impl From<LayerResult> for ModalResult {
    fn from(r: LayerResult) -> Self {
        match r {
            LayerResult::Dialog { action, text } => ModalResult::Dialog { action, text },
            LayerResult::Picker(PickerAction::Chosen(i))
            | LayerResult::Menu(MenuAction::Chosen(i)) => ModalResult::Picked(i),
            LayerResult::Picker(PickerAction::ChosenAlt(i)) => ModalResult::PickedAlt(i),
            LayerResult::Picker(PickerAction::ChosenMany(indices)) => {
                ModalResult::PickedMany(indices)
            }
            LayerResult::Picker(PickerAction::NextScope | PickerAction::PrevScope) => {
                ModalResult::Scope
            }
            LayerResult::Picker(PickerAction::Cancelled) | LayerResult::Menu(_) => {
                ModalResult::Cancelled
            }
            LayerResult::Picker(ev) => ModalResult::PickerEvent(ev),
            LayerResult::Form(FormAction::Cancel) => ModalResult::Form(None),
            LayerResult::Form(ev) => ModalResult::FormEvent(ev),
            LayerResult::Choice(c) => ModalResult::Choice(c),
            LayerResult::Info(r) => ModalResult::Info(r),
            LayerResult::Help => ModalResult::Help,
            LayerResult::File(r) => ModalResult::Browser(r),
        }
    }
}

impl From<termrock::widgets::inspect_changes::InspectOutcome> for ModalResult {
    fn from(o: termrock::widgets::inspect_changes::InspectOutcome) -> Self {
        use termrock::widgets::inspect_changes::InspectOutcome;
        ModalResult::Custom(
            match o {
                InspectOutcome::Back => "back",
                InspectOutcome::Close => "close",
            }
            .into(),
        )
    }
}

pub type AppLayer = dyn Layer<ModalResult, World>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Go {
    Manager,
    Settings,
    Accounts {
        select: Option<AccountId>,
    },
    Usage {
        select: Option<AccountId>,
    },
    Roles {
        select: Option<RoleName>,
    },
    RoleEditor {
        role: RoleName,
    },
    Doctor,

    Editor {
        workspace: Option<WorkspaceId>,
        pending: Option<Box<Workspace>>,
    },
    Prelude,
    LaunchMany {
        workspace: Option<WorkspaceId>,
        role: String,
        instances: Vec<crate::domain::tuple::InstanceChoice>,
        task: Option<String>,
    },
    Attach {
        instance: InstanceId,
        pane: Option<u64>,
    },
    NewSession {
        instance: InstanceId,
        agent: Option<Agent>,
        account: Option<AccountId>,
    },

    Detach,

    InstanceEnded {
        instance: InstanceId,
        purge: bool,
    },

    PowerStart {
        instance: InstanceId,
    },
    PowerStop {
        instance: InstanceId,
    },

    LaunchFailedAck {
        instance: Option<InstanceId>,
    },
    CloseTop {
        tag: LayerTag,
        then: Option<termrock::core::id::WidgetId>,
    },
    Quit,
}

pub fn post_refusal(cx: &mut Cx) {
    if let Some(reason) = termrock::core::refusal::take_refusal() {
        cx.refused(reason);
    }
}

pub type Request = termrock::shell::Request<ModalResult, World, Go>;

pub type Cx<'a> = termrock::shell::Cx<'a, ModalResult, World, Go>;

pub struct Jackin;

impl termrock::shell::PageTypes for Jackin {
    type State = World;
    type Msg = Msg;
    type Result = ModalResult;
    type LayerState = World;
    type Go = Go;
}

pub trait Screen: termrock::shell::Page<Jackin> {
    fn picker_items(&mut self, _tag: &LayerTag, _query: &str, _w: &World) -> Option<Vec<Item>> {
        None
    }

    fn form_changed(&mut self, _tag: &LayerTag, _form: &mut FormDialog, _w: &World) {}

    /// A backend reply is about to be delivered to the screen, and `form` is
    /// one of its open forms. A form that waits for the reply drops the wait
    /// when it no longer holds what the request was made from.
    fn reply_arrives(&mut self, _tag: &LayerTag, _form: &mut FormDialog, _m: &Msg) {}

    fn strip_right(&self, _w: &World) -> Vec<Segment> {
        vec![]
    }

    fn layer_open(&self) -> bool {
        false
    }

    fn job_continues(&self, _m: &Msg) -> bool {
        false
    }

    fn request_leave(&mut self, _w: &mut World, cx: &mut Cx) -> Outcome {
        cx.go(Go::Manager);
        Outcome::Changed
    }
}

pub fn health_spans(
    s: &crate::domain::usage::OverallSummary,
) -> Vec<termrock::widgets::viewport::Span> {
    use crate::domain::usage::HealthWord;
    use termrock::theme::Tone;
    use termrock::widgets::viewport::Span;
    let word = Span::new(
        s.health.label(),
        match s.health {
            HealthWord::Blocked | HealthWord::Degraded => Tone::Error,
            HealthWord::Attention => Tone::Warning,
            _ => Tone::Normal,
        },
    );
    let word = if matches!(s.health, HealthWord::Blocked | HealthWord::Degraded) {
        word.bold()
    } else {
        word
    };
    let mut v = vec![word];
    for (text, tone) in s.issue_parts() {
        v.push(Span::new(" · ", Tone::Muted));
        v.push(Span::new(text, tone));
    }
    v
}
