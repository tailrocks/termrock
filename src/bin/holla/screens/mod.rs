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
use junie_tui::ui::layer::{LayerEntry, LayerTag, ShellLayers};
use junie_tui::ui::layers::LayerResult;
use junie_tui::widgets::menu::MenuAction;
use junie_tui::widgets::picker::PickerAction;
use junie_tui::widgets::scrollbar;
use junie_tui::widgets::statusbar::StatusItem;
use ratatui::layout::{Position, Rect};

use crate::domain::context::Scope;
use crate::sim::world::{Msg, World};

/// A detail or preview pane that describes one item. Every move of the
/// selection goes through `track_detail`, the single rule for the pane's
/// scroll: it rewinds only when the described item changes, so re-selecting
/// the current item (or a reorder that keeps it) keeps the reader's place.
pub trait DetailPane: Sized {
    type Item: PartialEq;

    /// The item the pane currently describes.
    fn described_item(&self) -> Self::Item;

    fn detail_scroll_mut(&mut self) -> &mut ScrollState;

    fn track_detail<R>(&mut self, mv: impl FnOnce(&mut Self) -> R) -> R {
        let before = self.described_item();
        let out = mv(self);
        if self.described_item() != before {
            self.detail_scroll_mut().jump_start();
        }
        out
    }
}

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

pub use junie_tui::shell::{open_context, row_anchor, row_choice};

impl ModalResult {
    pub fn menu_chosen(&self) -> Option<usize> {
        match *self {
            Self::MenuChosen(k) => Some(k),
            _ => None,
        }
    }
}

/// The one row menu a page has open. The page keeps the captured target and actions
/// here. The layer tag carries only a token, so a result for an earlier menu cannot
/// consume a newer request, and no row position or key string is ever replayed.
pub struct PendingMenu<C> {
    last: u64,
    pending: Option<(u64, C)>,
}

/// What a layer result meant for the pending menu.
pub enum MenuTake<C> {
    /// Another kind or token: the result is not for the pending request, which stays.
    Stale,
    /// The pending request was dismissed and retired without an action.
    Dismissed,
    /// The user chose item `index` of the pending request.
    Chosen(C, usize),
}

impl<C> Default for PendingMenu<C> {
    fn default() -> Self {
        Self {
            last: 0,
            pending: None,
        }
    }
}

impl<C> PendingMenu<C> {
    /// Replaces any earlier pending request and returns the tag key for the new menu.
    pub fn capture(&mut self, context: C) -> String {
        self.last = self
            .last
            .checked_add(1)
            .expect("row menu token space exhausted");
        self.pending = Some((self.last, context));
        self.last.to_string()
    }

    /// Takes the request this result answers. A result whose kind or token does not
    /// match the pending request is stale, and it leaves that request in place.
    pub fn take(&mut self, tag: &LayerTag, kind: &str, result: &ModalResult) -> MenuTake<C> {
        let token = (tag.kind == kind)
            .then(|| tag.key.parse::<u64>().ok())
            .flatten();
        match (self.pending.as_ref(), token) {
            (Some((pending, _)), Some(token)) if *pending == token => {}
            _ => return MenuTake::Stale,
        }
        let Some((_, context)) = self.pending.take() else {
            return MenuTake::Stale;
        };
        match result.menu_chosen() {
            Some(index) => MenuTake::Chosen(context, index),
            None => MenuTake::Dismissed,
        }
    }
}

pub trait Screen: junie_tui::shell::Page<Holla> {
    /// Whether this page needs its tick while another tab is shown. A page
    /// whose tick only refreshes a projection of the world returns false: the
    /// domain keeps running, and the page catches up once when it is entered.
    fn background_tick(&self) -> bool {
        true
    }

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

/// Drives a page the way the shell does: render to register hit regions, then route a
/// secondary click through the page's own `pointer` handler.
#[cfg(test)]
pub(crate) mod pointer_tests {
    use super::{Cx, Holla, LayerTag, World};
    use junie_tui::core::focus::{Focus, FocusRing};
    use junie_tui::core::hit::HitRegistry;
    use junie_tui::core::id::WidgetId;
    use junie_tui::core::intent::Phase;
    use junie_tui::core::pointer::PointerEvent;
    use junie_tui::shell::{Page as ShellPage, Request};
    use junie_tui::theme::{ColorLevel, Theme};
    use junie_tui::ui::ctx::{Interaction, RenderCtx};
    use ratatui::buffer::Buffer;
    use ratatui::crossterm::event::KeyModifiers;
    use ratatui::layout::{Position, Rect};

    /// A screen tall enough that every row of the test fixtures is visible.
    pub(crate) const AREA: Rect = Rect::new(0, 0, 120, 300);

    pub(crate) fn render_hits(page: &mut impl ShellPage<Holla>, w: &World) -> HitRegistry {
        let theme = Theme::for_level(ColorLevel::TrueColor);
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let mut buffer = Buffer::empty(AREA);
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        page.render(AREA, &mut buffer, &mut ctx, w);
        ctx.flush_overlays(&mut buffer);
        drop(ctx);
        hits
    }

    /// A cell inside the region a rendered widget or row registered under `id`.
    pub(crate) fn cell_of(hits: &HitRegistry, id: WidgetId) -> Position {
        let area = hits
            .regions()
            .iter()
            .find(|r| r.id == id)
            .expect("the row was rendered")
            .area;
        Position::new(area.x, area.y)
    }

    /// Delivers a secondary click at `pos` and returns the tag of the menu it opened.
    pub(crate) fn secondary_open(
        page: &mut impl ShellPage<Holla>,
        w: &mut World,
        hits: &HitRegistry,
        pos: Position,
    ) -> LayerTag {
        let id = hits.hit(pos).expect("a rendered row is under the pointer");
        let event = PointerEvent::new(hits, Phase::Secondary, id, pos, 1, KeyModifiers::NONE);
        let mut focus = Focus::default();
        let ring = FocusRing::default();
        let mut cx = Cx::new(&mut focus, &ring);
        page.pointer(&event, w, &mut cx);
        cx.requests
            .iter()
            .find_map(|r| match r {
                Request::Open(_, tag) => Some(tag.clone()),
                _ => None,
            })
            .expect("the secondary click opened the row menu")
    }
}
