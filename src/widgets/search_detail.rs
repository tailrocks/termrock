//! Query-driven result browsing: the headless [`SearchSession`] and the composed
//! [`SearchDetail`] widget.
//!
//! [`SearchDetail`] puts a query box, keyed results and a preview side by side.
//! It routes keys and pointer events by focus and by hit region, paints rows
//! and cells through host callbacks, and resets the preview only when the
//! selected key changes. Everything below describes the session it owns.
//!
//! A session owns what every finder repeats: the query text, the request
//! lifecycle that keeps a slow or superseded result from replacing a newer one,
//! and one selection over the stable keys of the accepted results. It draws
//! nothing and holds no borrowed source. A host ranks on
//! [`SearchAction::QueryRequested`], builds a [`SearchSource`] whose stamp is
//! [`QueryToken::result_stamp`], and hands both back through
//! [`SearchSession::submit`].
//!
//! Selection lives in one [`KeyedState`](crate::core::keyed::KeyedState) over
//! the selectable keys (items and cells), so it survives reorder, filter and
//! refresh by identity. Headings, spacers and cell containers carry keys of
//! their own and are never selectable.

#[cfg(test)]
mod cost_tests;
mod detail;
mod projection;
mod session;

use std::sync::atomic::{AtomicU64, Ordering};

use crate::core::edit::TextCmd;
use crate::core::id::ItemKey;
use crate::core::keyed::KeyedSourceError;
use crate::core::keymap::{Actions, KeyBinding, KeyMap};
use crate::core::list::{KeyedRowSource, SourceStamp};
use crate::core::selection::ClickPolicy;
use crate::ui::context::{ContextConfig, ContextRequest};
use ratatui::crossterm::event::KeyCode;

pub use detail::{QueryView, RowPaint, SearchDetail, SearchDetailSource, paint_query_default};
pub use session::{SearchPhase, SearchSession};

/// What a visible result row is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResultRowKind {
    /// A group label. Not selectable, no cells.
    Heading,
    /// One selectable row. Its single cell aliases the row key.
    Item,
    /// A row of separately selectable cells, each with its own key.
    Cells,
    /// A blank separator. Not selectable, no cells.
    Spacer,
}

/// The results a host builds for one query.
///
/// The inherited [`RowSource`](crate::core::list::RowSource) methods describe
/// the visible rows, `source_len`/`source_key` every authoritative key: row
/// keys of all kinds and every cell key, each once. An [`ResultRowKind::Item`]
/// row and its sole cell are one entity and one key.
///
/// The stamp must equal [`QueryToken::result_stamp`] of the request it answers.
/// Its `data` is the input stamp's, so it does not move when only the query
/// does: the authoritative keys are the whole catalog (hidden records
/// included), and the visible rows are the ranked subset. A key shown that the
/// previous source of the same `data` did not list is refused as
/// `UnknownVisible`.
pub trait SearchSource: KeyedRowSource {
    fn row_kind(&self, row: usize) -> ResultRowKind;

    /// Zero for headings and spacers, one for an item, the cell count for a
    /// [`ResultRowKind::Cells`] row.
    fn cell_count(&self, row: usize) -> usize;

    /// Stable identity of a cell. Never derive it from the column.
    fn cell_key(&self, row: usize, cell: usize) -> ItemKey;

    fn cell_label(&self, row: usize, cell: usize) -> &str;

    fn cell_enabled(&self, row: usize, cell: usize) -> bool;

    /// Overrides [`SearchConfig::click`] for this cell. Evaluated against the
    /// accepted projection that produced the hit.
    fn cell_click_policy(&self, _row: usize, _cell: usize) -> Option<ClickPolicy> {
        None
    }
}

static NEXT_OWNER: AtomicU64 = AtomicU64::new(1);

pub(crate) fn fresh_owner() -> u64 {
    NEXT_OWNER
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
        .expect("search session owner ids exhausted")
}

/// Proof of which request a result answers.
///
/// The owner is private: a token from another session never validates, even
/// when its generation and source stamp are equal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueryToken {
    pub(crate) owner: u64,
    pub generation: u64,
    pub source: SourceStamp,
}

impl QueryToken {
    /// The stamp the submitted [`SearchSource`] must report.
    pub fn result_stamp(&self) -> SourceStamp {
        SourceStamp {
            instance: self.source.instance,
            data: self.source.data,
            view: self.generation,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryRequest {
    pub token: QueryToken,
    pub text: String,
}

/// The ordered semantic effects of one input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SearchAction {
    /// Rank `text` and answer with [`SearchSession::submit`] or
    /// [`SearchSession::fail`].
    QueryRequested(QueryRequest),
    /// The selected key changed. `None` means nothing is selectable.
    SelectionChanged(Option<ItemKey>),
    Activate(ItemKey),
    ContextRequested(ContextRequest<ItemKey>),
    DetailAction {
        key: ItemKey,
        name: &'static str,
    },
    /// A query edit command ran. `effective` is whether it changed anything:
    /// select-all on an empty query, or undo with nothing to undo, did not.
    QueryCommand {
        cmd: TextCmd,
        effective: bool,
    },
    /// A detail item was activated while `key` was selected.
    DetailItem {
        key: ItemKey,
        index: usize,
    },
}

/// Why a call was refused. Every refusal leaves the session unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SearchError {
    /// The token is from another session or is no longer the latest request.
    StaleQuery,
    /// The latest request already completed with a different outcome.
    AlreadyCompleted,
    /// The source reports another stamp than [`QueryToken::result_stamp`].
    StampMismatch {
        expected: SourceStamp,
        actual: SourceStamp,
    },
    /// Row `row` breaks the kind's cell rules.
    InconsistentRow {
        row: usize,
    },
    InvalidSource(KeyedSourceError),
    /// The source shown is not the last accepted projection.
    UnsubmittedProjection,
}

impl From<KeyedSourceError> for SearchError {
    fn from(error: KeyedSourceError) -> Self {
        Self::InvalidSource(error)
    }
}

/// Commands the keymap maps keys to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchCmd {
    FocusQuery,
    FocusResults,
    OpenDetail,
    CloseDetail,
    Activate,
    Context,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
}

impl SearchCmd {
    /// Focus, detail and context commands belong to the composition that owns
    /// focus and layout; a session never runs them.
    pub const fn is_host_owned(self) -> bool {
        matches!(
            self,
            Self::FocusQuery
                | Self::FocusResults
                | Self::OpenDetail
                | Self::CloseDetail
                | Self::Context
        )
    }
}

impl Actions for SearchCmd {
    fn label(self) -> &'static str {
        match self {
            Self::FocusQuery => "Focus search",
            Self::FocusResults => "Focus results",
            Self::OpenDetail => "Open detail",
            Self::CloseDetail => "Close detail",
            Self::Activate => "Open",
            Self::Context => "Actions",
            Self::Up => "Previous result",
            Self::Down => "Next result",
            Self::Left => "Previous cell",
            Self::Right => "Next cell",
            Self::Home => "First result",
            Self::End => "Last result",
            Self::PageUp => "Page up",
            Self::PageDown => "Page down",
        }
    }

    fn default_keymap() -> KeyMap<Self> {
        vec![
            (KeyBinding::ctrl('f'), Self::FocusQuery),
            (KeyBinding::key(KeyCode::Tab), Self::FocusResults),
            (KeyBinding::ctrl('o'), Self::OpenDetail),
            (KeyBinding::key(KeyCode::Enter), Self::Activate),
            (KeyBinding::key(KeyCode::Up), Self::Up),
            (KeyBinding::key(KeyCode::Down), Self::Down),
            (KeyBinding::key(KeyCode::Left), Self::Left),
            (KeyBinding::key(KeyCode::Right), Self::Right),
            (KeyBinding::key(KeyCode::Home), Self::Home),
            (KeyBinding::key(KeyCode::End), Self::End),
            (KeyBinding::key(KeyCode::PageUp), Self::PageUp),
            (KeyBinding::key(KeyCode::PageDown), Self::PageDown),
        ]
    }
}

/// Where the cursor lands when its row is gone after a refresh.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MissingSelection {
    /// The nearest enabled key at the old position.
    #[default]
    NearestPosition,
    /// The first enabled key.
    FirstEnabled,
}

/// Who owns keyboard focus between the query and the results.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchFocusMode {
    /// The query and the results are separate focus owners. While the query is
    /// focused, text editing has first refusal.
    #[default]
    Separate,
    /// One owner: result keys navigate, everything else edits the query.
    Combined,
}

/// Who owns the result cursor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectionOwner {
    /// The session moves the cursor for keys and clicks.
    #[default]
    Session,
    /// The host's own keyed list moves the cursor and reports it with
    /// [`SearchSession::set_selection`]. The session still gates actions on it
    /// and reconciles it when results are accepted, but navigation keys and
    /// presses never move it.
    External,
}

/// How pasted text enters the query.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum QueryPasteMode {
    /// Delete every line break.
    #[default]
    StripLineBreaks,
    /// Replace each line break with a space, then trim the ends.
    JoinLinesWithSpaceTrim,
}

impl QueryPasteMode {
    pub(crate) fn flatten(self, text: &str) -> String {
        match self {
            Self::StripLineBreaks => text.chars().filter(|c| !matches!(c, '\n' | '\r')).collect(),
            Self::JoinLinesWithSpaceTrim => text
                .split(['\n', '\r'])
                .collect::<Vec<_>>()
                .join(" ")
                .trim()
                .to_owned(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SearchConfig {
    pub keymap: KeyMap<SearchCmd>,
    pub click: ClickPolicy,
    pub select_first_on_query: bool,
    pub missing_selection: MissingSelection,
    pub context: ContextConfig,
    pub focus_mode: SearchFocusMode,
    pub paste_mode: QueryPasteMode,
    /// Whether text typed or pasted while a detail owns focus returns to the
    /// query. The composition decides what "detail" is.
    pub type_to_query_from_detail: bool,
    /// Whether navigation moves the retained cursor while a request is pending.
    /// Activation, context and detail actions stay disabled either way.
    pub navigate_while_loading: bool,
    pub selection_owner: SelectionOwner,
    /// Whether a new query returns the preview to its top even when the same
    /// key stays selected. Holla does; the default keeps the scroll.
    pub reset_preview_on_query: bool,
    /// Whether accepted results that keep the selected key leave its preview
    /// scroll alone. Turn it off to return to the top on every accepted
    /// result.
    pub preserve_preview_on_same_key: bool,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            keymap: SearchCmd::default_keymap(),
            click: ClickPolicy::DATA,
            select_first_on_query: true,
            missing_selection: MissingSelection::default(),
            context: ContextConfig::DEFAULT,
            focus_mode: SearchFocusMode::default(),
            paste_mode: QueryPasteMode::default(),
            type_to_query_from_detail: false,
            navigate_while_loading: false,
            selection_owner: SelectionOwner::default(),
            reset_preview_on_query: false,
            preserve_preview_on_same_key: true,
        }
    }
}

impl SearchConfig {
    pub fn keymap(mut self, keymap: KeyMap<SearchCmd>) -> Self {
        self.keymap = keymap;
        self
    }

    pub fn click(mut self, click: ClickPolicy) -> Self {
        self.click = click;
        self
    }

    pub fn select_first_on_query(mut self, on: bool) -> Self {
        self.select_first_on_query = on;
        self
    }

    pub fn missing_selection(mut self, policy: MissingSelection) -> Self {
        self.missing_selection = policy;
        self
    }

    pub fn context(mut self, context: ContextConfig) -> Self {
        self.context = context;
        self
    }

    pub fn focus_mode(mut self, mode: SearchFocusMode) -> Self {
        self.focus_mode = mode;
        self
    }

    pub fn paste_mode(mut self, mode: QueryPasteMode) -> Self {
        self.paste_mode = mode;
        self
    }

    pub fn type_to_query_from_detail(mut self, on: bool) -> Self {
        self.type_to_query_from_detail = on;
        self
    }

    pub fn navigate_while_loading(mut self, on: bool) -> Self {
        self.navigate_while_loading = on;
        self
    }

    pub fn selection_owner(mut self, owner: SelectionOwner) -> Self {
        self.selection_owner = owner;
        self
    }

    pub fn reset_preview_on_query(mut self, on: bool) -> Self {
        self.reset_preview_on_query = on;
        self
    }

    pub fn preserve_preview_on_same_key(mut self, on: bool) -> Self {
        self.preserve_preview_on_same_key = on;
        self
    }
}
