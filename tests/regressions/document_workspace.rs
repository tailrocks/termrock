//! Lifecycle conformance for `DocumentWorkspace`, driven through a minimal
//! text document that owns a real `CodeEditor`.

use std::borrow::Cow;
use std::cell::Cell;

use crate::common::*;
use junie_tui::core::id::{ItemKey, WidgetId};
use junie_tui::core::response::Response;
use junie_tui::widgets::code::CodeEditor;
use junie_tui::widgets::document_workspace::{
    CloseDecision, CloseToken, DocumentWorkspace, OpenKind, OperationToken, WorkspaceAction,
    WorkspaceDocument, WorkspaceError,
};
use junie_tui::widgets::tabs::{TabAction, Tabs};
use ratatui::crossterm::event::{KeyCode, KeyModifiers};

const ID: WidgetId = WidgetId::of("t.workspace");

struct TextDoc {
    title: String,
    dirty: bool,
    editor: CodeEditor,
}

struct TextView<'a> {
    doc: &'a mut TextDoc,
}

impl TextView<'_> {
    fn scroll_to(&mut self, line: usize) {
        self.doc.editor.scroll.v.scroll_to(line);
    }

    fn caret_to(&mut self, offset: usize) {
        self.doc.editor.jump_to(offset);
    }

    fn scroll_offset(&self) -> usize {
        self.doc.editor.scroll.v.offset
    }

    fn caret(&self) -> usize {
        self.doc.editor.cursor_offset()
    }

    fn text(&self) -> &str {
        self.doc.editor.text()
    }
}

impl WorkspaceDocument for TextDoc {
    type View<'a> = TextView<'a>;

    fn title(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.title)
    }

    fn dirty(&self) -> bool {
        self.dirty
    }

    fn focus_target(&self) -> Option<WidgetId> {
        Some(self.editor.id)
    }

    fn view(&mut self) -> TextView<'_> {
        TextView { doc: self }
    }
}

fn doc(title: &str, text: &str) -> TextDoc {
    TextDoc {
        title: title.into(),
        dirty: false,
        editor: CodeEditor::new(ID.sub(title), text),
    }
}

fn ws() -> DocumentWorkspace<TextDoc> {
    DocumentWorkspace::new(ID)
}

fn open(w: &mut DocumentWorkspace<TextDoc>, title: &str) -> ItemKey {
    let (key, _) = w.open(doc(title, ""), OpenKind::Pinned);
    key
}

fn type_text(w: &mut DocumentWorkspace<TextDoc>, key: ItemKey, text: &str) {
    let (_, resp) = w
        .update(key, |d| {
            d.editor.begin_edit();
            for c in text.chars() {
                d.editor.on_key(&key_of(KeyCode::Char(c)));
            }
            d.dirty = true;
        })
        .expect("open document");
    drop(resp);
}

fn key_of(code: KeyCode) -> junie_tui::core::event::Key {
    chord(code, KeyModifiers::NONE)
}

fn acts(resp: Response<WorkspaceAction<TextDoc>>) -> Vec<String> {
    resp.into_actions()
        .into_iter()
        .map(|a| match a {
            WorkspaceAction::Activated(k) => format!("activated:{k:?}"),
            WorkspaceAction::PreviewPromoted(k) => format!("promoted:{k:?}"),
            WorkspaceAction::CloseRequested(t) => format!("close-requested:{:?}", t.document),
            WorkspaceAction::CancelOperation(t) => format!("cancel:{:?}", t.document),
            WorkspaceAction::Closed { key, .. } => format!("closed:{key:?}"),
        })
        .collect()
}

fn close_token(w: &mut DocumentWorkspace<TextDoc>, key: ItemKey) -> CloseToken {
    let resp = w.request_close(key);
    resp.into_actions()
        .into_iter()
        .find_map(|a| match a {
            WorkspaceAction::CloseRequested(t) => Some(t),
            _ => None,
        })
        .expect("a close token")
}

#[test]
fn workspace_identity_survives_reorder_and_close() {
    let mut w = ws();
    let (a, b, c) = (open(&mut w, "a"), open(&mut w, "b"), open(&mut w, "c"));
    drop(w.activate(b));
    drop(w.reorder(c, 0));
    assert_eq!(w.ordered_keys(), [c, a, b]);
    assert_eq!(w.active_key(), Some(b), "reorder keeps the active document");
    drop(w.request_close(a));
    assert_eq!(w.ordered_keys(), [c, b]);
    assert_eq!(w.active_key(), Some(b));
    assert_eq!(w.get(b).map(|d| d.title.as_str()), Some("b"));
}

#[test]
fn workspace_keys_are_never_reused_across_controllers() {
    let mut first = ws();
    let a = open(&mut first, "same");
    drop(first.request_close(a));
    let b = open(&mut first, "same");
    assert_ne!(a, b, "close and reopen under one title is a new document");
    let mut second = ws();
    let c = open(&mut second, "same");
    assert!(c != a && c != b, "a second controller never repeats a key");
}

#[test]
fn workspace_open_pinned_activates_and_leaves_others_alone() {
    let mut w = ws();
    let a = open(&mut w, "a");
    type_text(&mut w, a, "x");
    let (b, resp) = w.open(doc("b", ""), OpenKind::Pinned);
    assert_eq!(acts(resp), [format!("activated:{b:?}")]);
    assert_eq!(w.ordered_keys(), [a, b]);
    assert!(w.get(a).is_some_and(|d| d.dirty));
}

#[test]
fn workspace_preview_replaces_a_clean_idle_preview_with_closed_before_activated() {
    let mut w = ws();
    let (p1, _) = w.open(doc("p1", ""), OpenKind::Preview);
    let (p2, resp) = w.open(doc("p2", ""), OpenKind::Preview);
    assert_eq!(
        acts(resp),
        [format!("closed:{p1:?}"), format!("activated:{p2:?}")]
    );
    assert_eq!(w.ordered_keys(), [p2]);
    assert_eq!(w.preview_key(), Some(p2));
}

#[test]
fn workspace_preview_promotes_before_replacement() {
    let mut w = ws();
    let (dirty, _) = w.open(doc("dirty", ""), OpenKind::Preview);
    type_text(&mut w, dirty, "draft");
    assert_eq!(w.preview_key(), None, "the first edit pinned it");

    let (running, _) = w.open(doc("running", ""), OpenKind::Preview);
    let (token, _) = w.begin_operation(running).expect("open");
    let (next, resp) = w.open(doc("next", ""), OpenKind::Preview);
    assert_eq!(
        acts(resp),
        [
            format!("promoted:{running:?}"),
            format!("activated:{next:?}")
        ],
        "a running preview is promoted, not discarded"
    );
    assert_eq!(w.ordered_keys(), [dirty, running, next]);
    assert_eq!(w.preview_key(), Some(next));
    assert!(w.finish_operation(token, |_| ()).is_ok());
}

#[test]
fn workspace_first_update_that_dirties_a_preview_promotes_once() {
    let mut w = ws();
    let (p, _) = w.open(doc("p", ""), OpenKind::Preview);
    let (_, clean) = w.update(p, |_| ()).expect("open");
    assert!(acts(clean).is_empty(), "a clean update does not promote");
    let (_, first) = w.update(p, |d| d.dirty = true).expect("open");
    assert_eq!(acts(first), [format!("promoted:{p:?}")]);
    let (_, second) = w.update(p, |_| ()).expect("open");
    assert!(acts(second).is_empty(), "promotion is emitted once");
}

#[test]
fn workspace_activating_the_current_key_is_a_no_op() {
    let mut w = ws();
    let a = open(&mut w, "a");
    let resp = w.activate(a);
    assert!(!resp.is_changed() && !resp.has_actions());
    assert!(!w.activate(ItemKey::num(u64::MAX)).has_actions());
}

#[test]
fn workspace_close_picks_the_survivor_at_the_old_position_then_the_previous() {
    let mut w = ws();
    let (a, b, c) = (open(&mut w, "a"), open(&mut w, "b"), open(&mut w, "c"));
    drop(w.activate(b));
    drop(w.request_close(b));
    assert_eq!(w.active_key(), Some(c), "next at the old position");
    drop(w.request_close(c));
    assert_eq!(w.active_key(), Some(a), "previous when none follows");
    drop(w.request_close(a));
    assert_eq!(w.active_key(), None);
}

#[test]
fn workspace_closing_an_inactive_document_keeps_the_active_one() {
    let mut w = ws();
    let (a, b) = (open(&mut w, "a"), open(&mut w, "b"));
    drop(w.activate(a));
    let resp = w.request_close(b);
    assert_eq!(acts(resp).len(), 1, "only Closed for the inactive target");
    assert_eq!(w.active_key(), Some(a));
}

#[test]
fn workspace_switch_preserves_editor_history() {
    let mut w = ws();
    let (a, b) = (open(&mut w, "a"), open(&mut w, "b"));
    drop(w.activate(a));
    type_text(&mut w, a, "one\ntwo\nthree\nfour\nfive\nsix\n");
    {
        let mut view = w.view_mut(a).expect("open");
        view.scroll_to(3);
        view.caret_to(4);
    }
    let before = {
        let view = w.view_mut(a).expect("open");
        (view.text().to_owned(), view.caret(), view.scroll_offset())
    };
    for _ in 0..3 {
        drop(w.activate(b));
        type_text(&mut w, b, "b");
        drop(w.activate(a));
        let view = w.view_mut(a).expect("open");
        assert_eq!(
            (view.text().to_owned(), view.caret(), view.scroll_offset()),
            before,
            "switching rebuilds nothing"
        );
    }
    let (_, resp) = w
        .update(a, |d| {
            d.editor.begin_edit();
            d.editor
                .on_key(&chord(KeyCode::Char('z'), KeyModifiers::CONTROL));
        })
        .expect("open");
    drop(resp);
    assert_ne!(
        w.get(a).map(|d| d.editor.text().to_owned()),
        Some(before.0),
        "A's own undo history survived the switches"
    );
    assert_eq!(w.get(b).map(|d| d.editor.text()), Some("bbb"));
}

#[test]
fn workspace_stale_close_never_targets_new_document() {
    let mut w = ws();
    let old = open(&mut w, "same");
    type_text(&mut w, old, "x");
    let token = close_token(&mut w, old);
    drop(
        w.resolve_close(token, CloseDecision::Discard)
            .expect("fresh token"),
    );
    let reopened = open(&mut w, "same");
    type_text(&mut w, reopened, "y");
    assert_eq!(
        w.resolve_close(token, CloseDecision::Discard).err(),
        Some(WorkspaceError::StaleClose)
    );
    assert_eq!(w.get(reopened).map(|d| d.editor.text()), Some("y"));

    let fresh = close_token(&mut w, reopened);
    type_text(&mut w, reopened, "z");
    assert_eq!(
        w.resolve_close(fresh, CloseDecision::Discard).err(),
        Some(WorkspaceError::StaleClose),
        "an edit after the request makes its token stale"
    );
    assert!(w.contains(reopened));
}

#[test]
fn workspace_completion_uses_originating_token() {
    let mut w = ws();
    let (a, b, c) = (open(&mut w, "a"), open(&mut w, "b"), open(&mut w, "c"));
    let (run_a, _) = w.begin_operation(a).expect("open");
    drop(w.activate(b));
    drop(w.request_close(c));
    let ((), resp) = w
        .finish_operation(run_a, |d| d.title.push_str("-done"))
        .expect("a still runs");
    drop(resp);
    assert_eq!(w.get(a).map(|d| d.title.as_str()), Some("a-done"));
    assert_eq!(w.get(b).map(|d| d.title.as_str()), Some("b"));

    let (run_b, _) = w.begin_operation(b).expect("open");
    drop(w.cancel_operation(b));
    drop(w.request_close(b));
    let called = Cell::new(false);
    let err = w
        .finish_operation(run_b, |_| called.set(true))
        .err()
        .expect("b is gone");
    assert_eq!(err, WorkspaceError::MissingDocument);
    assert!(!called.get());
}

#[test]
fn workspace_superseded_operation_is_rejected() {
    let mut w = ws();
    let (a, b) = (open(&mut w, "a"), open(&mut w, "b"));
    let (old, _) = w.begin_operation(a).expect("open");
    let (other, _) = w.begin_operation(b).expect("open");
    let (new, resp) = w.begin_operation(a).expect("open");
    assert_eq!(acts(resp), [format!("cancel:{a:?}")]);
    assert_ne!(old, new);
    let called = Cell::new(false);
    let err = w
        .finish_operation(old, |_| called.set(true))
        .err()
        .expect("superseded");
    assert_eq!(err, WorkspaceError::StaleOperation);
    assert!(!called.get(), "the old apply closure is never called");
    assert!(w.finish_operation(other, |_| ()).is_ok(), "b is unaffected");
    assert!(w.finish_operation(new, |_| ()).is_ok());
    assert_eq!(
        w.finish_operation(new, |_| ()).err(),
        Some(WorkspaceError::StaleOperation),
        "a completion applies once"
    );
}

fn dirty_with_save(
    w: &mut DocumentWorkspace<TextDoc>,
    title: &str,
) -> (ItemKey, CloseToken, OperationToken) {
    let key = open(w, title);
    type_text(w, key, "draft");
    let close = close_token(w, key);
    let (op, _) = w.begin_close_save(close).expect("fresh token");
    (key, close, op)
}

#[test]
fn workspace_save_close_is_atomic_and_targeted() {
    let mut w = ws();
    let (a, close, op) = dirty_with_save(&mut w, "a");
    let b = open(&mut w, "b");
    type_text(&mut w, b, "keep");
    drop(w.activate(b));
    let resp = w
        .complete_close_save(close, op, |d| d.dirty = false)
        .expect("exact pair");
    assert_eq!(acts(resp), [format!("closed:{a:?}")]);
    assert_eq!(w.ordered_keys(), [b]);
    assert!(w.get(b).is_some_and(|d| d.dirty), "b is unchanged");
    assert_eq!(w.active_key(), Some(b));
}

#[test]
fn workspace_paint_and_navigation_preserve_close_token() {
    let mut w = ws();
    let a = open(&mut w, "a");
    type_text(&mut w, a, "1\n2\n3\n4\n5\n6\n");
    let close = close_token(&mut w, a);
    let revision = w.revision(a);
    for i in 0..5 {
        let mut view = w.view_mut(a).expect("open");
        view.scroll_to(i);
        view.caret_to(i);
    }
    assert_eq!(w.revision(a), revision, "view work never advances content");
    assert_eq!(w.request_close(a).into_actions().len(), 1);
    assert_eq!(w.pending_close(a), Some(close), "the same token is reused");
    let (op, _) = w.begin_close_save(close).expect("still valid");
    for i in 0..5 {
        w.view_mut(a).expect("open").scroll_to(i);
    }
    assert_eq!(w.revision(a), revision);
    assert!(
        w.complete_close_save(close, op, |d| d.dirty = false)
            .is_ok(),
        "the token survived painting and navigation"
    );
}

#[test]
fn workspace_save_lock_allows_view_but_rejects_mutation() {
    let mut w = ws();
    let (a, close, op) = dirty_with_save(&mut w, "a");
    w.view_mut(a).expect("view works").scroll_to(0);
    let touched = Cell::new(false);
    assert_eq!(
        w.update(a, |_| touched.set(true)).err(),
        Some(WorkspaceError::SaveInProgress)
    );
    assert!(!touched.get(), "the mutation closure never runs");
    assert_eq!(
        w.begin_operation(a).err(),
        Some(WorkspaceError::SaveInProgress)
    );
    assert_eq!(
        w.resolve_close(close, CloseDecision::Discard).err(),
        Some(WorkspaceError::SaveInProgress),
        "ordinary Discard waits for the save"
    );
    assert_eq!(
        w.finish_operation(op, |_| touched.set(true)).err(),
        Some(WorkspaceError::SaveInProgress),
        "an ordinary completion cannot consume a close-save token"
    );
    assert!(!touched.get());
    assert!(w.validate_close_save(close, op).is_ok(), "tokens unchanged");
}

#[test]
fn workspace_failed_save_keeps_document_and_intent() {
    let mut w = ws();
    let (a, close, op) = dirty_with_save(&mut w, "a");
    let resp = w.fail_close_save(close, op).expect("exact pair");
    assert!(acts(resp).is_empty(), "no Closed event");
    assert!(w.contains(a) && !w.save_locked(a));
    assert_eq!(
        w.pending_close(a),
        Some(close),
        "the prompt stays available"
    );
    assert!(w.begin_operation(a).is_ok(), "the lock is released");

    let (op2, _) = w.begin_close_save(close).expect("retry on the same token");
    assert_eq!(
        w.complete_close_save(close, op2, |_| ()).err(),
        Some(WorkspaceError::StillDirty),
        "a save that leaves changes keeps the document"
    );
    assert!(w.contains(a) && !w.save_locked(a));
    assert!(w.get(a).is_some_and(|d| d.dirty));
}

#[test]
fn workspace_close_effect_order_is_stable() {
    let mut w = ws();
    let (a, b) = (open(&mut w, "a"), open(&mut w, "b"));
    drop(w.activate(a));
    let (token, _) = w.begin_operation(a).expect("open");
    let close = close_token(&mut w, a);
    let resp = w
        .resolve_close(close, CloseDecision::Discard)
        .expect("fresh");
    assert_eq!(
        acts(resp),
        [
            format!("cancel:{a:?}"),
            format!("closed:{a:?}"),
            format!("activated:{b:?}")
        ]
    );
    assert_eq!(
        w.finish_operation(token, |_| ()).err(),
        Some(WorkspaceError::MissingDocument)
    );
}

#[test]
fn workspace_cancel_close_save_is_targeted() {
    let mut w = ws();
    let (a, close, op) = dirty_with_save(&mut w, "a");
    let (b, close_b, op_b) = dirty_with_save(&mut w, "b");
    assert_eq!(
        w.cancel_close_save(close, op_b).err(),
        Some(WorkspaceError::StaleOperation),
        "a mismatched pair is refused"
    );
    assert!(w.save_locked(a) && w.save_locked(b), "nothing was cleared");
    let resp = w.cancel_close_save(close, op).expect("exact pair");
    assert_eq!(acts(resp), [format!("cancel:{a:?}")], "cancelled once");
    assert!(
        !w.save_locked(a) && w.save_locked(b),
        "b's lock is untouched"
    );
    assert!(w.contains(a));
    assert_eq!(
        w.cancel_close_save(close, op).err(),
        Some(WorkspaceError::StaleClose),
        "both tokens are stale afterwards"
    );
    assert!(w.validate_close_save(close_b, op_b).is_ok());
}

#[test]
fn workspace_cancel_during_save_cancels_the_save() {
    let mut w = ws();
    let (a, close, _) = dirty_with_save(&mut w, "a");
    let resp = w
        .resolve_close(close, CloseDecision::Cancel)
        .expect("exact pending token");
    assert_eq!(acts(resp), [format!("cancel:{a:?}")]);
    assert!(w.contains(a) && !w.save_locked(a));
}

#[test]
fn workspace_resolve_cancel_clears_only_that_request() {
    let mut w = ws();
    let (a, b) = (open(&mut w, "a"), open(&mut w, "b"));
    type_text(&mut w, a, "x");
    type_text(&mut w, b, "y");
    let (ca, cb) = (close_token(&mut w, a), close_token(&mut w, b));
    drop(w.resolve_close(ca, CloseDecision::Cancel).expect("fresh"));
    assert_eq!(w.pending_close(a), None);
    assert_eq!(w.pending_close(b), Some(cb));
    assert!(w.contains(a) && w.contains(b));
}

#[test]
fn workspace_invalid_preflight_never_invokes_apply_saved() {
    let mut w = ws();
    let (a, close, op) = dirty_with_save(&mut w, "a");
    let (_, other_close, other_op) = dirty_with_save(&mut w, "b");
    let called = Cell::new(false);
    for (c, o) in [(close, other_op), (other_close, op)] {
        assert!(w.validate_close_save(c, o).is_err());
        assert!(
            w.complete_close_save(c, o, |_| called.set(true)).is_err(),
            "a mismatched pair is refused"
        );
    }
    assert!(!called.get(), "apply_saved never ran");
    assert!(w.save_locked(a), "the refused calls cleared nothing");
    drop(w.cancel_close_save(close, op).expect("exact pair"));
    assert_eq!(
        w.complete_close_save(close, op, |_| called.set(true)).err(),
        Some(WorkspaceError::StaleClose)
    );
    assert!(!called.get());
}

#[test]
fn workspace_tabs_projection_translates_events_by_key() {
    let mut w = ws();
    let (a, b, c) = (open(&mut w, "a"), open(&mut w, "b"), open(&mut w, "c"));
    let mut tabs = Tabs::new(ID.sub("tabs"), &[]);
    w.sync_tabs(&mut tabs);
    assert_eq!(
        tabs.items
            .iter()
            .map(|i| i.label.as_str())
            .collect::<Vec<_>>(),
        ["a", "b", "c"]
    );
    assert_eq!(tabs.active, 2, "the strip follows the active key");

    let (_, act) = tabs.on_key(&key_of(KeyCode::Left));
    let Some(TabAction::Activated(i)) = act else {
        panic!("keyboard activation expected, got {act:?}");
    };
    let key = w.key_at(i).expect("same projection");
    assert_eq!(key, b);
    drop(w.activate(key));
    assert_eq!(w.active_key(), Some(b));

    drop(w.reorder(c, 0));
    w.sync_tabs(&mut tabs);
    assert_eq!(tabs.items[0].label, "c");
    assert_eq!(
        w.key_at(tabs.active),
        w.active_key(),
        "projection is by key"
    );
    assert_eq!(w.key_at(1), Some(a));
}

#[test]
fn workspace_tabs_projection_marks_dirty_and_busy() {
    let mut w = ws();
    let (a, b) = (open(&mut w, "a"), open(&mut w, "b"));
    type_text(&mut w, a, "x");
    let (_, _) = w.begin_operation(b).expect("open");
    let mut tabs = Tabs::new(ID.sub("tabs"), &[]);
    w.sync_tabs_with(&mut tabs, |d, item| item.prefix(&d.title.to_uppercase()));
    assert!(tabs.items[0].dirty && !tabs.items[0].busy);
    assert!(tabs.items[1].busy && !tabs.items[1].dirty);
    assert_eq!(tabs.items[1].prefix.as_deref(), Some("B"));
}

#[test]
fn workspace_scoped_ids_follow_the_key_not_the_position() {
    let mut w = ws();
    let (a, b) = (open(&mut w, "a"), open(&mut w, "b"));
    let before = w.scoped_id(b, "editor");
    drop(w.reorder(b, 0));
    drop(w.request_close(a));
    assert_eq!(w.scoped_id(b, "editor"), before);
    assert_ne!(w.scoped_id(b, "editor"), w.scoped_id(b, "grid"));
    assert_ne!(w.scoped_id(a, "editor"), before);
}

#[test]
fn workspace_discard_running_preview_cancels_and_closes() {
    let mut w = ws();
    let (p, _) = w.open(doc("p", ""), OpenKind::Preview);
    let (token, _) = w.begin_operation(p).expect("open");
    let close = close_token(&mut w, p);
    let resp = w
        .resolve_close(close, CloseDecision::Discard)
        .expect("fresh");
    assert_eq!(
        acts(resp),
        [format!("cancel:{p:?}"), format!("closed:{p:?}")]
    );
    assert_eq!(w.preview_key(), None);
    assert!(w.finish_operation(token, |_| ()).is_err());
}
