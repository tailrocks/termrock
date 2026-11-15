use ratatui::crossterm::event::KeyCode;

use super::projection::{self, Move, Selectable, Slot};
use super::{
    MissingSelection, QueryRequest, QueryToken, SearchAction, SearchCmd, SearchConfig, SearchError,
    SearchFocusMode, SearchSource, fresh_owner,
};
use crate::core::edit::{EditKind, TextCmd};
use crate::core::event::Key;
use crate::core::id::ItemKey;
use crate::core::keyed::KeyedState;
use crate::core::keymap::lookup;
use crate::core::list::SourceStamp;
use crate::core::response::Response;
use crate::ui::context::{ContextAnchor, ContextRequest};
use crate::widgets::edit::{EditExit, EditField, EditResult};
use crate::widgets::input::TextInput;

type Reply = Response<SearchAction>;

#[derive(Debug)]
enum RequestState {
    Uninitialized,
    Pending(QueryRequest),
    Accepted(QueryToken),
    Failed(QueryToken),
}

impl RequestState {
    fn token(&self) -> Option<QueryToken> {
        match self {
            Self::Uninitialized => None,
            Self::Pending(request) => Some(request.token),
            Self::Accepted(token) | Self::Failed(token) => Some(*token),
        }
    }
}

/// The last result source that passed validation. It stays on display while a
/// newer request is pending or has failed.
#[derive(Debug)]
struct Accepted {
    stamp: SourceStamp,
    text: String,
    slots: Vec<Slot>,
}

/// Where the session is in its request lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchPhase {
    /// No input stamp yet, so no request has been made.
    Uninitialized,
    /// The newest request has no answer. The previous results stay on display
    /// but cannot be acted on.
    Loading,
    Ready,
    Failed,
}

/// Query text, request lifecycle and one keyed selection. See the
/// [module](super) documentation.
///
/// Not `Clone`: every session has its own owner, so a token never validates
/// against another session.
#[derive(Debug)]
pub struct SearchSession {
    owner: u64,
    generation: u64,
    input_stamp: Option<SourceStamp>,
    request: RequestState,
    committed: String,
    accepted: Option<Accepted>,
    state: KeyedState,
    query: TextInput,
    config: SearchConfig,
    error: Option<String>,
    column: usize,
    context_intent: Option<ItemKey>,
    press_was_selected: bool,
    /// A paint moved the selection off a cell it left out. The composition
    /// reports it with the next response.
    relocated: bool,
}

impl SearchSession {
    pub fn new(query: TextInput, config: SearchConfig) -> Self {
        Self {
            owner: fresh_owner(),
            generation: 0,
            input_stamp: None,
            request: RequestState::Uninitialized,
            committed: query.field.text().to_owned(),
            accepted: None,
            state: KeyedState::new(),
            query,
            config,
            error: None,
            column: 0,
            context_intent: None,
            press_was_selected: false,
            relocated: false,
        }
    }

    pub fn config(&self) -> &SearchConfig {
        &self.config
    }

    pub fn config_mut(&mut self) -> &mut SearchConfig {
        &mut self.config
    }

    pub fn query(&self) -> &str {
        self.query.field.text()
    }

    /// The query widget, for painting. Edit it only through the session.
    pub fn query_input(&self) -> &TextInput {
        &self.query
    }

    /// The query widget for the composition that paints and points at it.
    /// Text it changes must be reported through [`edited_query`](Self::edited_query).
    pub(super) fn query_input_mut(&mut self) -> &mut TextInput {
        &mut self.query
    }

    /// Requests results when the query text no longer matches the last
    /// request, after the query widget was edited outside the session.
    pub(super) fn edited_query(&mut self) -> Reply {
        self.after_text_edit()
    }

    /// The query the displayed results answer.
    pub fn accepted_query(&self) -> Option<&str> {
        self.accepted
            .as_ref()
            .map(|accepted| accepted.text.as_str())
    }

    /// Whether `key` is an enabled item or cell of the displayed results that
    /// the last paint placed.
    pub fn is_selectable(&self, key: ItemKey) -> bool {
        self.slot(key).is_some_and(|slot| slot.selectable())
    }

    /// Records which cells of `row` the paint placed. A cell it left out is
    /// skipped by navigation and cannot run until a later paint places it.
    ///
    /// Placement is part of what the selection may rest on. When the paint
    /// leaves out the selected cell, the selection moves to the nearest placed
    /// cell, or clears when none is placed, so no key or detail action can
    /// reach a cell the user cannot see.
    pub(super) fn place_cells(&mut self, row: usize, placed: &[usize]) {
        let Some(accepted) = self.accepted.as_mut() else {
            return;
        };
        let start = accepted.slots.partition_point(|s| s.row < row);
        let end = accepted.slots.partition_point(|s| s.row <= row);
        for slot in &mut accepted.slots[start..end] {
            slot.placed = placed.contains(&slot.cell);
        }
        let Some(at) = self
            .state
            .cursor()
            .and_then(|key| self.state.visible_index(key))
            .filter(|&at| (start..end).contains(&at) && !accepted.slots[at].placed)
        else {
            return;
        };
        let target = projection::nearest_selectable(&accepted.slots, at);
        let moved = match target {
            Some(to) => {
                let slot = accepted.slots[to];
                self.column = slot.cell;
                self.state.set_cursor(slot.key)
            }
            None => self.state.clear_cursor(),
        };
        self.relocated |= moved;
        self.drop_stale_intent();
    }

    /// Whether a paint moved the selection since the last response.
    pub(super) fn take_relocation(&mut self) -> bool {
        std::mem::take(&mut self.relocated)
    }

    pub(super) fn has_relocation(&self) -> bool {
        self.relocated
    }

    /// The selected key, when it may act: displayed results that answer the
    /// newest request and a cell or item the last paint placed.
    fn actionable(&self) -> Option<ItemKey> {
        self.state
            .cursor()
            .filter(|&key| self.is_interactive() && self.is_selectable(key))
    }

    pub fn phase(&self) -> SearchPhase {
        match self.request {
            RequestState::Uninitialized => SearchPhase::Uninitialized,
            RequestState::Pending(_) => SearchPhase::Loading,
            RequestState::Accepted(_) => SearchPhase::Ready,
            RequestState::Failed(_) => SearchPhase::Failed,
        }
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Whether the displayed results answer the newest request. Activation,
    /// context and detail actions need it.
    pub fn is_interactive(&self) -> bool {
        matches!(self.request, RequestState::Accepted(_))
    }

    pub fn accepted_stamp(&self) -> Option<SourceStamp> {
        self.accepted.as_ref().map(|accepted| accepted.stamp)
    }

    /// Refuses a source that is not the last accepted projection, so a host
    /// cannot paint rows the selection does not describe.
    pub fn verify_display(&self, stamp: SourceStamp) -> Result<(), SearchError> {
        match self.accepted_stamp() {
            Some(accepted) if accepted == stamp => Ok(()),
            _ => Err(SearchError::UnsubmittedProjection),
        }
    }

    pub fn selected_key(&self) -> Option<ItemKey> {
        self.state.cursor()
    }

    /// `(row, cell)` of the selection in the accepted source, for painting.
    pub fn selected_cell(&self) -> Option<(usize, usize)> {
        self.slot(self.state.cursor()?).map(|s| (s.row, s.cell))
    }

    /// The selectable keys of the accepted source.
    pub fn selectable_len(&self) -> usize {
        self.state.visible_len()
    }

    fn slot(&self, key: ItemKey) -> Option<&Slot> {
        let at = self.state.visible_index(key)?;
        self.accepted.as_ref()?.slots.get(at)
    }

    fn can_navigate(&self) -> bool {
        self.accepted.is_some() && (self.is_interactive() || self.config.navigate_while_loading)
    }

    fn requery(&mut self) -> Reply {
        let Some(source) = self.input_stamp else {
            return Response::changed();
        };
        self.generation = self
            .generation
            .checked_add(1)
            .expect("query generation exhausted");
        let request = QueryRequest {
            token: QueryToken {
                owner: self.owner,
                generation: self.generation,
                source,
            },
            text: self.query.field.text().to_owned(),
        };
        self.committed.clone_from(&request.text);
        self.error = None;
        self.request = RequestState::Pending(request.clone());
        Response::emit(SearchAction::QueryRequested(request))
    }

    fn after_text_edit(&mut self) -> Reply {
        if self.query.field.text() == self.committed {
            Response::changed()
        } else {
            self.requery()
        }
    }

    pub fn set_query(&mut self, text: &str) -> Reply {
        if self.query.field.text() == text {
            return Response::ignored();
        }
        self.query.field.set_text(text);
        self.after_text_edit()
    }

    /// Edits the query field directly, for example its key map. A text change
    /// requests results like any other edit.
    pub fn configure_query(&mut self, configure: impl FnOnce(&mut EditField)) -> Reply {
        configure(&mut self.query.field);
        self.after_text_edit()
    }

    /// Installs or replaces the input stamp. The first stamp makes the first
    /// request; an equal one later changes nothing.
    pub fn refresh_source(&mut self, stamp: SourceStamp) -> Reply {
        if self.input_stamp == Some(stamp) {
            return Response::ignored();
        }
        self.input_stamp = Some(stamp);
        self.requery()
    }

    /// Requests the current query again, the way out of [`SearchPhase::Failed`].
    pub fn retry(&mut self) -> Reply {
        if self.input_stamp.is_none() {
            return Response::ignored();
        }
        self.requery()
    }

    fn current_token(&self, token: QueryToken) -> Result<(), SearchError> {
        if self.request.token() == Some(token) {
            Ok(())
        } else {
            Err(SearchError::StaleQuery)
        }
    }

    /// Accepts the results for `token`.
    ///
    /// Every check precedes every change: a refused call leaves the pending
    /// request pending, so the host can correct its source and submit again.
    /// Submitting the accepted source again is a no-op.
    pub fn submit<S: SearchSource + ?Sized>(
        &mut self,
        token: QueryToken,
        source: &S,
    ) -> Result<Reply, SearchError> {
        self.current_token(token)?;
        let request = match &self.request {
            RequestState::Pending(request) => request,
            RequestState::Failed(_) => return Err(SearchError::AlreadyCompleted),
            RequestState::Accepted(_) => {
                return if self.accepted_stamp() == Some(source.stamp()) {
                    Ok(Response::ignored())
                } else {
                    Err(SearchError::AlreadyCompleted)
                };
            }
            RequestState::Uninitialized => return Err(SearchError::StaleQuery),
        };
        let expected = token.result_stamp();
        let actual = source.stamp();
        if actual != expected {
            return Err(SearchError::StampMismatch { expected, actual });
        }
        let slots = projection::build(source)?;
        let before = self.state.cursor();
        self.state.reconcile(&Selectable {
            source,
            slots: &slots,
        })?;

        let query_changed = self
            .accepted
            .as_ref()
            .is_none_or(|accepted| accepted.text != request.text);
        let lost = before.is_some_and(|key| self.state.cursor() != Some(key));
        let first = slots.iter().find(|slot| slot.enabled).map(|slot| slot.key);
        let take_first = (query_changed && self.config.select_first_on_query)
            || (lost && self.config.missing_selection == MissingSelection::FirstEnabled);
        if let Some(first) = first.filter(|_| take_first) {
            self.state.set_cursor(first);
        }
        let after = self.state.cursor();
        if after != before {
            self.column = after
                .and_then(|key| slots.iter().find(|slot| slot.key == key))
                .map_or(0, |slot| slot.cell);
        }
        self.accepted = Some(Accepted {
            stamp: actual,
            text: request.text.clone(),
            slots,
        });
        self.request = RequestState::Accepted(token);
        self.error = None;
        self.drop_stale_intent();
        Ok(if after == before {
            Response::changed()
        } else {
            Response::emit(SearchAction::SelectionChanged(after))
        })
    }

    /// Records that the newest request failed. The last accepted results stay
    /// on display, read-only.
    pub fn fail(&mut self, token: QueryToken, message: String) -> Result<Reply, SearchError> {
        self.current_token(token)?;
        if !matches!(self.request, RequestState::Pending(_)) {
            return Err(SearchError::AlreadyCompleted);
        }
        self.request = RequestState::Failed(token);
        self.error = Some(message);
        self.context_intent = None;
        Ok(Response::changed())
    }

    fn drop_stale_intent(&mut self) {
        let key = self.context_intent;
        let alive = key.is_some_and(|key| self.actionable() == Some(key));
        if !alive {
            self.context_intent = None;
        }
    }

    // ---- keyboard ---------------------------------------------------------

    /// The command a key is bound to. Context keys are the composition's:
    /// it reads [`ContextConfig`](crate::ui::context::ContextConfig) itself.
    pub fn command_for(&self, key: &Key) -> Option<SearchCmd> {
        lookup(&self.config.keymap, key)
    }

    /// Routes one key. `query_focused` says whether the query (or, in
    /// [`SearchFocusMode::Combined`], the single owner) has focus; `page` is
    /// the number of result rows the host shows.
    ///
    /// Focus and detail commands are never run here: they return ignored so the
    /// composition that owns focus can read [`command_for`](Self::command_for).
    pub fn on_key(&mut self, key: &Key, query_focused: bool, page: usize) -> Reply {
        let cmd = self.command_for(key);
        if cmd.is_some_and(SearchCmd::is_host_owned) {
            return Response::ignored();
        }
        match self.config.focus_mode {
            SearchFocusMode::Combined => self.combined_key(key, cmd, page),
            SearchFocusMode::Separate if query_focused => self.separate_query_key(key, cmd, page),
            SearchFocusMode::Separate => {
                cmd.map_or_else(Response::ignored, |cmd| self.run(cmd, page))
            }
        }
    }

    fn separate_query_key(&mut self, key: &Key, cmd: Option<SearchCmd>, page: usize) -> Reply {
        let (reply, edit) = self.edit_key(key);
        if edit.unused {
            return match cmd {
                Some(
                    cmd @ (SearchCmd::Up
                    | SearchCmd::Down
                    | SearchCmd::PageUp
                    | SearchCmd::PageDown),
                ) => self.run(cmd, page),
                _ => Response::ignored(),
            };
        }
        if edit.exit == Some(EditExit::Commit) {
            return self.activate();
        }
        reply
    }

    fn combined_key(&mut self, key: &Key, cmd: Option<SearchCmd>, page: usize) -> Reply {
        if let Some(cmd) = cmd {
            return self.run(cmd, page);
        }
        match key.code {
            KeyCode::Esc if key.plain() => return self.escape(),
            KeyCode::Backspace if key.plain() && self.query().is_empty() => {
                return Response::consumed();
            }
            _ => {}
        }
        self.edit_key(key).0
    }

    /// Clears the query selection, then the query, then gives up so the host
    /// can step out further.
    fn escape(&mut self) -> Reply {
        if self.query.field.buffer.has_selection() {
            self.query.field.buffer.clear_selection();
            return Response::changed();
        }
        if self.query().is_empty() {
            return Response::ignored();
        }
        self.query.field.record(EditKind::Other);
        self.query.field.buffer.set_text("");
        self.after_text_edit()
    }

    fn edit_key(&mut self, key: &Key) -> (Reply, EditResult) {
        let field = &self.query.field;
        let command = field.text_cmd(key).and_then(|cmd| {
            let effective = match cmd {
                TextCmd::SelectAll => !field.text().is_empty(),
                TextCmd::Undo => field.can_undo(),
                TextCmd::Redo => field.can_redo(),
                _ => return None,
            };
            Some(SearchAction::QueryCommand { cmd, effective })
        });
        let result = self.query.field.on_key(key);
        if result.unused {
            return (Response::ignored(), result);
        }
        let mut reply = Response::changed();
        if let Some(action) = command {
            reply.push_action(action);
        }
        (reply.merge(self.after_text_edit()), result)
    }

    /// Pastes into the query. Ignored unless `query_focused`, or
    /// [`SearchConfig::type_to_query_from_detail`] sends detail input back.
    pub fn on_paste(&mut self, text: &str, query_focused: bool) -> Reply {
        if !query_focused && !self.config.type_to_query_from_detail {
            return Response::ignored();
        }
        let flat = self.config.paste_mode.flatten(text);
        if flat.is_empty() || !self.query.field.paste(&flat) {
            return Response::consumed();
        }
        self.after_text_edit()
    }

    /// Runs a result command. Focus and detail commands return ignored.
    pub fn run(&mut self, cmd: SearchCmd, page: usize) -> Reply {
        match cmd {
            SearchCmd::Activate => self.activate(),
            SearchCmd::Up => self.navigate(Move::Up),
            SearchCmd::Down => self.navigate(Move::Down),
            SearchCmd::Left => self.navigate(Move::Left),
            SearchCmd::Right => self.navigate(Move::Right),
            SearchCmd::Home => self.navigate(Move::Home),
            SearchCmd::End => self.navigate(Move::End),
            SearchCmd::PageUp => self.navigate(Move::PageUp(page)),
            SearchCmd::PageDown => self.navigate(Move::PageDown(page)),
            SearchCmd::Context
            | SearchCmd::FocusQuery
            | SearchCmd::FocusResults
            | SearchCmd::OpenDetail
            | SearchCmd::CloseDetail => Response::ignored(),
        }
    }

    fn navigate(&mut self, mv: Move) -> Reply {
        let from = self
            .state
            .cursor()
            .and_then(|key| self.state.visible_index(key));
        let horizontal = matches!(mv, Move::Left | Move::Right);
        let slots = self.accepted.as_ref().map_or(&[][..], |a| &a.slots[..]);
        if horizontal && !from.is_some_and(|at| slots[at].in_cells) {
            return Response::ignored();
        }
        if !self.can_navigate() {
            return Response::consumed();
        }
        let Some(to) = projection::target(slots, from, self.column, mv) else {
            return Response::consumed();
        };
        let slot = slots[to];
        let reply = self.move_cursor(slot.key);
        if horizontal {
            self.column = slot.cell;
        }
        reply
    }

    fn move_cursor(&mut self, key: ItemKey) -> Reply {
        if !self.state.set_cursor(key) {
            return Response::consumed();
        }
        self.drop_stale_intent();
        Response::emit(SearchAction::SelectionChanged(Some(key)))
    }

    /// Selects `key` if it is an enabled key of the displayed results.
    pub fn select(&mut self, key: ItemKey) -> Reply {
        if !self.can_navigate() {
            return Response::consumed();
        }
        let Some(cell) = self.slot(key).map(|slot| slot.cell) else {
            return Response::ignored();
        };
        let reply = self.move_cursor(key);
        if self.state.cursor() == Some(key) {
            self.column = cell;
        }
        reply
    }

    fn activate(&mut self) -> Reply {
        match self.actionable() {
            Some(key) => Response::emit(SearchAction::Activate(key)),
            None => Response::consumed(),
        }
    }

    /// Emits a host-named detail action against the selected key, captured now
    /// so it never resolves a position later.
    pub fn detail_action(&mut self, name: &'static str) -> Reply {
        match self.actionable() {
            Some(key) => Response::emit(SearchAction::DetailAction { key, name }),
            None => Response::consumed(),
        }
    }

    /// Emits activation of detail item `index` against the selected key.
    pub fn detail_item(&mut self, index: usize) -> Reply {
        match self.actionable() {
            Some(key) => Response::emit(SearchAction::DetailItem { key, index }),
            None => Response::consumed(),
        }
    }

    // ---- pointer ----------------------------------------------------------

    /// A primary press on `key`: selects it and captures it for the release.
    /// A key that is not an enabled key of the displayed results is ignored.
    pub fn press(&mut self, key: ItemKey, layout_generation: u64) -> Reply {
        if !self.can_navigate() || !self.state.capture_press(key, layout_generation) {
            return Response::ignored();
        }
        self.press_was_selected = self.state.cursor() == Some(key);
        Response::consumed().merge(self.select(key))
    }

    /// The release matching [`press`](Self::press). Every release consumes the
    /// capture. It activates when the cell's click policy fires for `count`
    /// clicks, and only if the row has not moved or the layout changed since
    /// the press.
    pub fn release(&mut self, key: ItemKey, layout_generation: u64, count: u8) -> Reply {
        let again = std::mem::take(&mut self.press_was_selected);
        if !self.state.release_press(key, layout_generation) || !self.is_interactive() {
            return Response::consumed();
        }
        let policy = self.slot(key).map_or(self.config.click, |slot| {
            slot.policy.unwrap_or(self.config.click)
        });
        if policy.activates(count, again) {
            Response::emit(SearchAction::Activate(key))
        } else {
            Response::consumed()
        }
    }

    /// A press and release in one input. `SelectionChanged` comes before
    /// `Activate`.
    pub fn click(&mut self, key: ItemKey, layout_generation: u64, count: u8) -> Reply {
        let down = self.press(key, layout_generation);
        if !down.is_consumed() {
            return down;
        }
        down.merge(self.release(key, layout_generation, count))
    }

    /// Drops the press capture when the pointer left the row before release.
    pub fn cancel_press(&mut self) {
        self.state.cancel_press();
        self.press_was_selected = false;
    }

    /// Cancels a press captured under another layout generation.
    pub fn invalidate_layout(&mut self, generation: u64) -> bool {
        self.state.invalidate_layout(generation)
    }

    // ---- context ----------------------------------------------------------

    /// Holds a keyboard context request for the selected key. The key is
    /// captured now; the anchor comes from the repainted layout through
    /// [`emit_context`](Self::emit_context). Which keys ask, and whether a
    /// pointer request moves the cursor, are
    /// [`ContextConfig`](crate::ui::context::ContextConfig)'s, applied by the
    /// composition.
    pub fn begin_context(&mut self) -> Reply {
        match self.actionable() {
            Some(key) => {
                self.context_intent = Some(key);
                Response::changed()
            }
            _ => Response::consumed(),
        }
    }

    pub fn context_intent(&self) -> Option<ItemKey> {
        self.context_intent
    }

    /// Emits the request started by [`begin_context`](Self::begin_context), or
    /// nothing when its key is no longer the selection.
    pub fn emit_context(&mut self, anchor: ContextAnchor) -> Reply {
        self.drop_stale_intent();
        match self.context_intent.take() {
            Some(key) => Response::emit(SearchAction::ContextRequested(ContextRequest {
                target: key,
                anchor,
            })),
            None => Response::consumed(),
        }
    }
}
