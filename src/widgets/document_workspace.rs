//! A keyed document workspace: identity, activation, preview, close and task
//! ownership for a tab strip of documents.
//!
//! [`DocumentWorkspace`] is a controller over the existing [`Tabs`]
//! presentation. It owns the documents (`D`), their order, the active and the
//! preview document, and the tokens that tie a delayed task or a pending close
//! to the exact document that asked for it. It owns no content: titles,
//! editors, grids, backends and domain policy stay with the application's `D`.
//!
//! # Rules
//!
//! * Document identity is an [`ItemKey`] allocated from a process-wide counter
//!   and never reused, so a token issued for a closed document can never match
//!   a document opened later, even under the same title or in a recreated
//!   workspace.
//! * One background operation runs per document. [`DocumentWorkspace::begin_operation`]
//!   supersedes the previous one, and [`DocumentWorkspace::finish_operation`]
//!   runs its closure only for the exact generation that is still running.
//! * Content mutation and non-content work are separate. A close token is
//!   invalidated exactly when document content changes.
//!   [`DocumentWorkspace::update`] changes a document and advances the content
//!   revision. [`DocumentWorkspace::view_mut`] hands out
//!   [`WorkspaceDocument::View`], a facet the document type restricts to
//!   everything that is not content: painting, scrolling, caret and selection
//!   movement, polling a running operation and view marks such as "unseen".
//!   It never advances the revision, so none of that can stale a pending
//!   close. [`DocumentWorkspace::end_operation`] finishes an operation through
//!   the same facet, and [`DocumentWorkspace::finish_operation`] finishes one
//!   whose result changes content.
//! * A dirty or running document is never closed silently. A close request
//!   yields a [`CloseToken`] naming the document, its content revision and the
//!   request. Resolving it validates all three. The save-and-close protocol
//!   ([`DocumentWorkspace::begin_close_save`] and its completions) locks the
//!   document against mutation while the backend runs.
//! * Every transition returns a [`Response`] whose [`WorkspaceAction`]s are in
//!   the order the host must handle them. The controller never tears down a
//!   host resource; [`WorkspaceAction::Closed`] hands the removed document back
//!   so the host finalizes it.
//!
//! The tokens are logical identities, not security capabilities: stale or
//! mismatched values are rejected, but a caller that forges a current value is
//! not defended against.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::core::id::{ItemKey, Part, PartRef, WidgetId};
use crate::core::response::Response;
use crate::widgets::tabs::{TabItem, Tabs};

static NEXT_DOCUMENT_KEY: AtomicU64 = AtomicU64::new(0);

fn fresh_document_key() -> u64 {
    loop {
        let n = NEXT_DOCUMENT_KEY.load(Ordering::SeqCst);
        let next = n.checked_add(1).expect("document key space exhausted");
        if NEXT_DOCUMENT_KEY
            .compare_exchange(n, next, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            return n;
        }
    }
}

fn bump(counter: u64, what: &str) -> u64 {
    counter
        .checked_add(1)
        .unwrap_or_else(|| panic!("document workspace {what} exhausted"))
}

/// Names one accepted background operation of one document.
///
/// `generation` counts the document's operations; it is not a content
/// revision. Compare tokens, never display indices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OperationToken {
    pub document: ItemKey,
    pub generation: u64,
}

/// Names one close request for one document at one content revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CloseToken {
    pub document: ItemKey,
    pub revision: u64,
    pub request: u64,
}

/// How a document is opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenKind {
    /// Stays until it is closed.
    Pinned,
    /// A single replaceable slot: the next preview open replaces it, unless it
    /// holds changes or a running operation, and the first change promotes it.
    Preview,
}

/// What a user decided about a pending close request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseDecision {
    Cancel,
    Discard,
}

/// What a transition asks the host to do, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkspaceAction<D> {
    /// The document became the active one.
    Activated(ItemKey),
    /// The preview document became pinned.
    PreviewPromoted(ItemKey),
    /// A dirty or running document asked to close; present the choice and
    /// answer with [`DocumentWorkspace::resolve_close`] or the save protocol.
    CloseRequested(CloseToken),
    /// The host must stop the backend work this operation names.
    CancelOperation(OperationToken),
    /// The document left the workspace; the host finalizes its resources.
    Closed { key: ItemKey, document: D },
}

/// Why a token or key was refused. Nothing was changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceError {
    MissingDocument,
    StaleClose,
    StaleOperation,
    /// A close-save owns the document; only its own completion may proceed.
    SaveInProgress,
    /// The saved state still has changes, so the document stays open.
    StillDirty,
}

impl std::fmt::Display for WorkspaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::MissingDocument => "The document is no longer open",
            Self::StaleClose => "That close request is out of date",
            Self::StaleOperation => "That operation is out of date",
            Self::SaveInProgress => "Saving: wait for the save to finish",
            Self::StillDirty => "The document still has unsaved changes",
        })
    }
}

impl std::error::Error for WorkspaceError {}

/// What the workspace needs to know about a document.
pub trait WorkspaceDocument {
    /// The restricted facet [`DocumentWorkspace::view_mut`] and
    /// [`DocumentWorkspace::end_operation`] hand out. It should expose only
    /// what is not content (painting, view-only navigation, operation progress
    /// and results, view marks), and its fields should be private so a caller
    /// cannot reach the document through it.
    type View<'a>
    where
        Self: 'a;

    fn title(&self) -> Cow<'_, str>;
    /// Whether the document holds changes that closing would lose.
    fn dirty(&self) -> bool;
    /// The widget that should take focus when this document becomes active.
    fn focus_target(&self) -> Option<WidgetId>;
    fn view(&mut self) -> Self::View<'_>;
}

struct SaveLock {
    close: CloseToken,
    operation: OperationToken,
}

struct Entry<D> {
    document: D,
    revision: u64,
    generation: u64,
    running: Option<u64>,
    close: Option<CloseToken>,
    save: Option<SaveLock>,
}

fn op(document: ItemKey, generation: u64) -> OperationToken {
    OperationToken {
        document,
        generation,
    }
}

/// The checked completion rule: the entry exists and this generation is the
/// one still running.
fn operation_matches<D>(entries: &BTreeMap<ItemKey, Entry<D>>, token: OperationToken) -> bool {
    entries
        .get(&token.document)
        .is_some_and(|entry| entry.running == Some(token.generation))
}

/// Owns a set of documents and the rules for opening, activating, previewing,
/// closing and running work in them.
pub struct DocumentWorkspace<D: WorkspaceDocument> {
    id: WidgetId,
    entries: BTreeMap<ItemKey, Entry<D>>,
    order: Vec<ItemKey>,
    active: Option<ItemKey>,
    preview: Option<ItemKey>,
    next_close_request: u64,
}

type Resp<D> = Response<WorkspaceAction<D>>;

impl<D: WorkspaceDocument> DocumentWorkspace<D> {
    pub fn new(id: WidgetId) -> Self {
        Self {
            id,
            entries: BTreeMap::new(),
            order: Vec::new(),
            active: None,
            preview: None,
            next_close_request: 0,
        }
    }

    pub fn id(&self) -> WidgetId {
        self.id
    }

    /// A render id for one document, scoped by the workspace id, the role
    /// ("editor", "grid") and the document key. It never depends on position.
    pub fn scoped_id(&self, key: ItemKey, role: &str) -> WidgetId {
        self.id.sub(role).part(PartRef::item(Part::ROW, key))
    }

    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    pub fn contains(&self, key: ItemKey) -> bool {
        self.entries.contains_key(&key)
    }

    pub fn active_key(&self) -> Option<ItemKey> {
        self.active
    }

    pub fn preview_key(&self) -> Option<ItemKey> {
        self.preview
    }

    pub fn ordered_keys(&self) -> &[ItemKey] {
        &self.order
    }

    pub fn key_at(&self, index: usize) -> Option<ItemKey> {
        self.order.get(index).copied()
    }

    pub fn index_of(&self, key: ItemKey) -> Option<usize> {
        self.order.iter().position(|k| *k == key)
    }

    pub fn get(&self, key: ItemKey) -> Option<&D> {
        self.entries.get(&key).map(|e| &e.document)
    }

    /// Documents in strip order.
    pub fn iter(&self) -> impl Iterator<Item = (ItemKey, &D)> + '_ {
        self.order
            .iter()
            .filter_map(|key| self.entries.get(key).map(|e| (*key, &e.document)))
    }

    /// The document's running operation, if one is in flight.
    pub fn running(&self, key: ItemKey) -> Option<OperationToken> {
        let entry = self.entries.get(&key)?;
        entry.running.map(|g| op(key, g))
    }

    /// Whether a close-save owns the document.
    pub fn save_locked(&self, key: ItemKey) -> bool {
        self.entries.get(&key).is_some_and(|e| e.save.is_some())
    }

    /// The document's content revision, for hosts that capture it.
    pub fn revision(&self, key: ItemKey) -> Option<u64> {
        self.entries.get(&key).map(|e| e.revision)
    }

    pub fn pending_close(&self, key: ItemKey) -> Option<CloseToken> {
        self.entries.get(&key)?.close
    }

    /// Opens `document`. See [`Self::open_with`] for a document that must know
    /// its own key while it is built.
    pub fn open(&mut self, document: D, kind: OpenKind) -> (ItemKey, Resp<D>) {
        self.open_with(|_| document, kind)
    }

    /// Opens the document `build` makes from the key the workspace allocates
    /// for it, so the document can derive its render ids from that key.
    ///
    /// A pinned open appends and activates. A preview open first retires the
    /// previous preview: a clean idle one is closed (`Closed` is emitted before
    /// `Activated`), a dirty or running one is promoted to pinned.
    pub fn open_with(
        &mut self,
        build: impl FnOnce(ItemKey) -> D,
        kind: OpenKind,
    ) -> (ItemKey, Resp<D>) {
        let key = ItemKey::num(fresh_document_key());
        let mut resp = Response::changed();
        if kind == OpenKind::Preview
            && let Some(old) = self.preview
        {
            if self.retains(old) {
                self.preview = None;
                resp.push_action(WorkspaceAction::PreviewPromoted(old));
            } else if let Some(document) = self.remove(old) {
                resp.push_action(WorkspaceAction::Closed { key: old, document });
            }
        }
        self.entries.insert(
            key,
            Entry {
                document: build(key),
                revision: 0,
                generation: 0,
                running: None,
                close: None,
                save: None,
            },
        );
        self.order.push(key);
        self.active = Some(key);
        if kind == OpenKind::Preview {
            self.preview = Some(key);
        }
        resp.push_action(WorkspaceAction::Activated(key));
        (key, resp)
    }

    fn retains(&self, key: ItemKey) -> bool {
        self.entries
            .get(&key)
            .is_some_and(|e| e.document.dirty() || e.running.is_some() || e.save.is_some())
    }

    /// Removes `key` from every index. The active document moves to the
    /// survivor at its old position, else the previous one, else none.
    fn remove(&mut self, key: ItemKey) -> Option<D> {
        let entry = self.entries.remove(&key)?;
        let at = self.order.iter().position(|k| *k == key);
        if let Some(at) = at {
            self.order.remove(at);
        }
        if self.preview == Some(key) {
            self.preview = None;
        }
        if self.active == Some(key) {
            self.active = at.and_then(|at| {
                self.order
                    .get(at)
                    .or_else(|| at.checked_sub(1).and_then(|p| self.order.get(p)))
                    .copied()
            });
        }
        Some(entry.document)
    }

    /// Emits the actions of closing `key`: the running operation's
    /// cancellation, `Closed`, then the successor's activation.
    fn close_now(&mut self, key: ItemKey, resp: &mut Resp<D>) {
        let was_active = self.active == Some(key);
        let cancel = self
            .entries
            .get(&key)
            .and_then(|e| e.running.map(|g| op(key, g)));
        let Some(document) = self.remove(key) else {
            return;
        };
        if let Some(token) = cancel {
            resp.push_action(WorkspaceAction::CancelOperation(token));
        }
        resp.push_action(WorkspaceAction::Closed { key, document });
        if was_active && let Some(next) = self.active {
            resp.push_action(WorkspaceAction::Activated(next));
        }
    }

    /// Makes `key` the active document. Activating the current key, or a key
    /// that is not open, changes nothing.
    pub fn activate(&mut self, key: ItemKey) -> Resp<D> {
        if !self.entries.contains_key(&key) || self.active == Some(key) {
            return Response::ignored();
        }
        self.active = Some(key);
        Response::emit(WorkspaceAction::Activated(key))
    }

    /// Moves `key` to `new_index` (clamped). Identity, active and preview are
    /// unchanged.
    pub fn reorder(&mut self, key: ItemKey, new_index: usize) -> Resp<D> {
        let Some(from) = self.index_of(key) else {
            return Response::ignored();
        };
        let to = new_index.min(self.order.len() - 1);
        if from == to {
            return Response::ignored();
        }
        let moved = self.order.remove(from);
        self.order.insert(to, moved);
        Response::changed()
    }

    /// Pins the preview document. Anything else is a no-op.
    pub fn promote(&mut self, key: ItemKey) -> Resp<D> {
        if self.preview != Some(key) {
            return Response::ignored();
        }
        self.preview = None;
        Response::emit(WorkspaceAction::PreviewPromoted(key))
    }

    /// The document's restricted view facet. It does not advance the content
    /// revision and works while a close-save holds the document. Operation
    /// polling and view marks go through it, not through [`Self::update`].
    pub fn view_mut(&mut self, key: ItemKey) -> Result<D::View<'_>, WorkspaceError> {
        self.entries
            .get_mut(&key)
            .map(|e| e.document.view())
            .ok_or(WorkspaceError::MissingDocument)
    }

    fn promote_if_dirty(&mut self, key: ItemKey, resp: &mut Resp<D>) {
        if self.preview == Some(key) && self.entries.get(&key).is_some_and(|e| e.document.dirty()) {
            self.preview = None;
            resp.push_action(WorkspaceAction::PreviewPromoted(key));
        }
    }

    /// The content mutation gateway. Refused with [`WorkspaceError::SaveInProgress`]
    /// before `f` runs while a close-save holds the document. Afterwards the
    /// content revision advances and a preview that became dirty is promoted.
    /// Never use it for painting, scrolling, caret or selection work.
    pub fn update<R>(
        &mut self,
        key: ItemKey,
        f: impl FnOnce(&mut D) -> R,
    ) -> Result<(R, Resp<D>), WorkspaceError> {
        let entry = self
            .entries
            .get_mut(&key)
            .ok_or(WorkspaceError::MissingDocument)?;
        if entry.save.is_some() {
            return Err(WorkspaceError::SaveInProgress);
        }
        let out = f(&mut entry.document);
        entry.revision = bump(entry.revision, "content revision");
        let mut resp = Response::changed();
        self.promote_if_dirty(key, &mut resp);
        Ok((out, resp))
    }

    /// Starts the document's one background operation. The previous ordinary
    /// operation, if any, is superseded and reported through
    /// [`WorkspaceAction::CancelOperation`]. Operations in other documents are
    /// untouched.
    pub fn begin_operation(
        &mut self,
        key: ItemKey,
    ) -> Result<(OperationToken, Resp<D>), WorkspaceError> {
        let entry = self
            .entries
            .get_mut(&key)
            .ok_or(WorkspaceError::MissingDocument)?;
        if entry.save.is_some() {
            return Err(WorkspaceError::SaveInProgress);
        }
        let generation = bump(entry.generation, "operation generation");
        let mut resp = Response::changed();
        if let Some(previous) = entry.running.take() {
            resp.push_action(WorkspaceAction::CancelOperation(op(key, previous)));
        }
        entry.generation = generation;
        entry.running = Some(generation);
        let token = op(key, generation);
        self.promote_if_dirty(key, &mut resp);
        Ok((token, resp))
    }

    /// Clears the running mark of the exact generation `token` names and
    /// returns its entry. The check precedes any write: a missing document, a
    /// superseded or cancelled generation, an already finished one or a
    /// close-save's own operation is refused.
    fn take_running(&mut self, token: OperationToken) -> Result<&mut Entry<D>, WorkspaceError> {
        if !self.entries.contains_key(&token.document) {
            return Err(WorkspaceError::MissingDocument);
        }
        if !operation_matches(&self.entries, token) {
            return Err(WorkspaceError::StaleOperation);
        }
        let entry = self
            .entries
            .get_mut(&token.document)
            .ok_or(WorkspaceError::MissingDocument)?;
        if entry.save.as_ref().is_some_and(|s| s.operation == token) {
            return Err(WorkspaceError::SaveInProgress);
        }
        entry.running = None;
        Ok(entry)
    }

    /// Applies an operation's result to exactly the document and generation
    /// that started it, when the result changes content. The check precedes
    /// the write: a missing document, a superseded or cancelled generation, or
    /// an already finished one is refused without calling `apply`. The content
    /// revision advances and a preview that became dirty is promoted. A result
    /// that is not content ends through [`Self::end_operation`].
    pub fn finish_operation<R>(
        &mut self,
        token: OperationToken,
        apply: impl FnOnce(&mut D) -> R,
    ) -> Result<(R, Resp<D>), WorkspaceError> {
        let entry = self.take_running(token)?;
        let out = apply(&mut entry.document);
        entry.revision = bump(entry.revision, "content revision");
        let mut resp = Response::changed();
        self.promote_if_dirty(token.document, &mut resp);
        Ok((out, resp))
    }

    /// Ends an operation whose result is not content, such as a query run
    /// that only filled its result pane. It has the checks of
    /// [`Self::finish_operation`], applies through the document's view facet
    /// and leaves the content revision alone, so a pending close stays valid.
    pub fn end_operation<R>(
        &mut self,
        token: OperationToken,
        apply: impl FnOnce(D::View<'_>) -> R,
    ) -> Result<(R, Resp<D>), WorkspaceError> {
        let entry = self.take_running(token)?;
        let out = apply(entry.document.view());
        Ok((out, Response::changed()))
    }

    /// Cancels the document's ordinary running operation. A close-save's
    /// operation is cancelled by [`Self::cancel_close_save`] instead.
    pub fn cancel_operation(&mut self, key: ItemKey) -> Resp<D> {
        let Some(entry) = self.entries.get_mut(&key) else {
            return Response::ignored();
        };
        if entry.save.is_some() {
            return Response::ignored();
        }
        let Some(generation) = entry.running.take() else {
            return Response::ignored();
        };
        Response::emit(WorkspaceAction::CancelOperation(op(key, generation)))
    }

    /// Asks to close `key`. A clean idle document closes at once; a dirty or
    /// running one stays and a [`CloseToken`] is emitted for the host to
    /// resolve. A repeated request at the same content revision reuses its
    /// token; a changed revision issues a new one.
    pub fn request_close(&mut self, key: ItemKey) -> Resp<D> {
        let Some(entry) = self.entries.get_mut(&key) else {
            return Response::ignored();
        };
        if entry.save.is_some() {
            return Response::ignored();
        }
        if entry.document.dirty() || entry.running.is_some() {
            let revision = entry.revision;
            let token = match entry.close {
                Some(t) if t.revision == revision => t,
                _ => {
                    let request = self.next_close_request;
                    self.next_close_request = bump(request, "close request counter");
                    let token = CloseToken {
                        document: key,
                        revision,
                        request,
                    };
                    if let Some(entry) = self.entries.get_mut(&key) {
                        entry.close = Some(token);
                    }
                    token
                }
            };
            return Response::emit(WorkspaceAction::CloseRequested(token));
        }
        let mut resp = Response::changed();
        self.close_now(key, &mut resp);
        resp
    }

    fn close_entry(&self, token: CloseToken) -> Result<&Entry<D>, WorkspaceError> {
        let entry = self
            .entries
            .get(&token.document)
            .ok_or(WorkspaceError::StaleClose)?;
        if entry.close != Some(token) || entry.revision != token.revision {
            return Err(WorkspaceError::StaleClose);
        }
        Ok(entry)
    }

    /// Answers a pending close request. Cancel keeps the document; Discard
    /// cancels its operation and closes only the token's document. A stale
    /// token is refused. Discard is refused with `SaveInProgress` while a
    /// close-save holds the document, and Cancel then cancels that save.
    pub fn resolve_close(
        &mut self,
        token: CloseToken,
        decision: CloseDecision,
    ) -> Result<Resp<D>, WorkspaceError> {
        let entry = self.close_entry(token)?;
        let save = entry.save.as_ref().map(|s| (s.close, s.operation));
        match decision {
            CloseDecision::Cancel => {
                if let Some((close, operation)) = save {
                    return self.cancel_close_save(close, operation);
                }
                if let Some(entry) = self.entries.get_mut(&token.document) {
                    entry.close = None;
                }
                Ok(Response::changed())
            }
            CloseDecision::Discard => {
                if save.is_some() {
                    return Err(WorkspaceError::SaveInProgress);
                }
                let mut resp = Response::changed();
                self.close_now(token.document, &mut resp);
                Ok(resp)
            }
        }
    }

    /// Starts saving a document that asked to close. It validates the token,
    /// installs the save lock and the dedicated operation atomically, and
    /// supersedes any ordinary operation. While locked, [`Self::update`] and
    /// [`Self::begin_operation`] return `SaveInProgress`; [`Self::view_mut`]
    /// keeps working.
    pub fn begin_close_save(
        &mut self,
        close: CloseToken,
    ) -> Result<(OperationToken, Resp<D>), WorkspaceError> {
        if self.close_entry(close)?.save.is_some() {
            return Err(WorkspaceError::SaveInProgress);
        }
        let key = close.document;
        let entry = self
            .entries
            .get_mut(&key)
            .ok_or(WorkspaceError::StaleClose)?;
        let generation = bump(entry.generation, "operation generation");
        let mut resp = Response::changed();
        if let Some(previous) = entry.running.take() {
            resp.push_action(WorkspaceAction::CancelOperation(op(key, previous)));
        }
        entry.generation = generation;
        entry.running = Some(generation);
        let operation = op(key, generation);
        entry.save = Some(SaveLock { close, operation });
        Ok((operation, resp))
    }

    /// Checks that `close` and `operation` are the live close-save pair.
    /// Hosts run their fallible backend and grid receipt checks first, then
    /// call this before [`Self::complete_close_save`].
    pub fn validate_close_save(
        &self,
        close: CloseToken,
        operation: OperationToken,
    ) -> Result<(), WorkspaceError> {
        let entry = self
            .entries
            .get(&close.document)
            .ok_or(WorkspaceError::StaleClose)?;
        let lock = entry.save.as_ref().ok_or(WorkspaceError::StaleClose)?;
        if lock.close != close {
            return Err(WorkspaceError::StaleClose);
        }
        if lock.operation != operation || !operation_matches(&self.entries, operation) {
            return Err(WorkspaceError::StaleOperation);
        }
        Ok(())
    }

    /// Finishes a successful save. `apply_saved` marks exactly this document
    /// clean; the document then closes. If it is still dirty afterwards, the
    /// lock is released, the document stays and `StillDirty` is returned.
    pub fn complete_close_save(
        &mut self,
        close: CloseToken,
        operation: OperationToken,
        apply_saved: impl FnOnce(&mut D),
    ) -> Result<Resp<D>, WorkspaceError> {
        self.validate_close_save(close, operation)?;
        let key = close.document;
        let entry = self
            .entries
            .get_mut(&key)
            .ok_or(WorkspaceError::StaleClose)?;
        entry.running = None;
        entry.save = None;
        apply_saved(&mut entry.document);
        entry.revision = bump(entry.revision, "content revision");
        if entry.document.dirty() {
            entry.close = None;
            return Err(WorkspaceError::StillDirty);
        }
        let mut resp = Response::changed();
        self.close_now(key, &mut resp);
        Ok(resp)
    }

    /// Ends a failed save. The document, its close request and its prompt stay
    /// available so the host can show the error.
    pub fn fail_close_save(
        &mut self,
        close: CloseToken,
        operation: OperationToken,
    ) -> Result<Resp<D>, WorkspaceError> {
        self.validate_close_save(close, operation)?;
        if let Some(entry) = self.entries.get_mut(&close.document) {
            entry.running = None;
            entry.save = None;
        }
        Ok(Response::changed())
    }

    /// Cancels a save in progress. It validates the exact pair, emits the
    /// operation's cancellation once, clears the lock and the close request and
    /// keeps the document. Both tokens are stale afterwards.
    pub fn cancel_close_save(
        &mut self,
        close: CloseToken,
        operation: OperationToken,
    ) -> Result<Resp<D>, WorkspaceError> {
        self.validate_close_save(close, operation)?;
        if let Some(entry) = self.entries.get_mut(&close.document) {
            entry.running = None;
            entry.save = None;
            entry.close = None;
        }
        Ok(Response::emit(WorkspaceAction::CancelOperation(operation)))
    }

    /// Builds the strip items from the ordered documents: title, a dirty mark
    /// while idle and a busy mark while an operation runs. The active tab
    /// follows the active key.
    pub fn sync_tabs(&self, tabs: &mut Tabs) {
        self.sync_tabs_with(tabs, |_, item| item);
    }

    /// Like [`Self::sync_tabs`], with `decorate` adding the host's own marks
    /// (prefix, error) to each item.
    pub fn sync_tabs_with(&self, tabs: &mut Tabs, decorate: impl Fn(&D, TabItem) -> TabItem) {
        let items = self
            .order
            .iter()
            .filter_map(|key| self.entries.get(key))
            .map(|e| {
                let busy = e.running.is_some();
                let item = TabItem::new(&e.document.title())
                    .closable()
                    .dirty(e.document.dirty() && !busy)
                    .busy(busy);
                decorate(&e.document, item)
            })
            .collect();
        tabs.set_items(items);
        let active = self.active.and_then(|k| self.index_of(k)).unwrap_or(0);
        tabs.set_active(active);
    }
}
