//! A text-document consumer of `DocumentWorkspace` that prints every
//! transition and the actions it returns, in the order a host handles them.
//!
//! Run it with `cargo run --example document_workspace`.

use std::borrow::Cow;

use junie_tui::core::id::{ItemKey, WidgetId};
use junie_tui::core::response::Response;
use junie_tui::widgets::document_workspace::{
    CloseDecision, DocumentWorkspace, OpenKind, WorkspaceAction, WorkspaceDocument,
};
use junie_tui::widgets::tabs::Tabs;

struct Note {
    title: String,
    text: String,
    saved: String,
}

/// The view facet: painting and caret movement only. A caller of `view_mut`
/// cannot reach the text through it.
struct NoteView<'a> {
    note: &'a Note,
}

impl NoteView<'_> {
    fn paint(&self) -> String {
        format!("[{}] {}", self.note.title, self.note.text)
    }
}

impl WorkspaceDocument for Note {
    type View<'a> = NoteView<'a>;

    fn title(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.title)
    }

    fn dirty(&self) -> bool {
        self.text != self.saved
    }

    fn focus_target(&self) -> Option<WidgetId> {
        None
    }

    fn view(&mut self) -> NoteView<'_> {
        NoteView { note: self }
    }
}

fn note(title: &str) -> Note {
    Note {
        title: title.into(),
        text: String::new(),
        saved: String::new(),
    }
}

fn show(step: &str, resp: Response<WorkspaceAction<Note>>) {
    let actions: Vec<String> = resp
        .into_actions()
        .into_iter()
        .map(|action| match action {
            WorkspaceAction::Activated(key) => format!("Activated({key:?})"),
            WorkspaceAction::PreviewPromoted(key) => format!("PreviewPromoted({key:?})"),
            WorkspaceAction::CloseRequested(token) => format!(
                "CloseRequested(revision {}, request {})",
                token.revision, token.request
            ),
            WorkspaceAction::CancelOperation(token) => {
                format!("CancelOperation(generation {})", token.generation)
            }
            WorkspaceAction::Closed { key, document } => {
                format!(
                    "Closed({key:?}, {:?}) for the host to finalize",
                    document.title
                )
            }
        })
        .collect();
    println!("{step:<38} {}", actions.join(", "));
}

fn keys(w: &DocumentWorkspace<Note>) -> Vec<ItemKey> {
    w.ordered_keys().to_vec()
}

fn main() {
    let mut w: DocumentWorkspace<Note> = DocumentWorkspace::new(WidgetId::of("example.notes"));

    let (a, resp) = w.open(note("a"), OpenKind::Pinned);
    show("open a (pinned)", resp);
    let (p1, resp) = w.open(note("p1"), OpenKind::Preview);
    show("open p1 (preview)", resp);
    let (p2, resp) = w.open(note("p2"), OpenKind::Preview);
    show("open p2 (replaces the clean preview)", resp);
    println!("{:<38} {:?}", "order after the replacement", keys(&w));
    assert!(w.get(p1).is_none());

    show("activate a", w.activate(a));
    show("activate a again (no-op)", w.activate(a));
    show("reorder p2 to the front", w.reorder(p2, 0));

    let ((), resp) = w
        .update(p2, |n| n.text.push_str("draft"))
        .expect("p2 is open");
    show("edit p2 (first change pins it)", resp);
    let (p3, resp) = w.open(note("p3"), OpenKind::Preview);
    show("open p3 (p2 is pinned, so it stays)", resp);

    let (task, resp) = w.begin_operation(p3).expect("p3 is open");
    show("begin an operation in p3", resp);
    let (_, resp) = w.begin_operation(p3).expect("p3 is open");
    show("begin another (supersedes the first)", resp);
    println!(
        "{:<38} {:?}",
        "finish the superseded one",
        w.finish_operation(task, |_| ()).map(|_| ())
    );

    show("request close of p2 (dirty)", w.request_close(p2));
    let token = w.pending_close(p2).expect("a close token");
    println!(
        "{:<38} {:?}",
        "view work leaves the token valid",
        w.view_mut(p2).map(|view| view.paint())
    );
    let ((), resp) = w
        .update(p2, |n| n.text.push('!'))
        .expect("p2 is still open");
    show("edit p2 after the request", resp);
    println!(
        "{:<38} {:?}",
        "resolve the stale token",
        w.resolve_close(token, CloseDecision::Discard).map(|_| ())
    );

    show("request close again", w.request_close(p2));
    let token = w.pending_close(p2).expect("a fresh close token");
    let (operation, resp) = w.begin_close_save(token).expect("a fresh token");
    show("begin the save-and-close", resp);
    println!(
        "{:<38} {:?}",
        "edit during the save",
        w.update(p2, |_| ()).map(|_| ())
    );
    println!(
        "{:<38} {:?}",
        "paint during the save",
        w.view_mut(p2).map(|view| view.paint())
    );
    let resp = w
        .complete_close_save(token, operation, |n| n.saved = n.text.clone())
        .expect("the exact pair");
    show("the backend saved; the tab closes", resp);

    let mut tabs = Tabs::new(WidgetId::of("example.strip"), &[]);
    w.sync_tabs(&mut tabs);
    let labels: Vec<&str> = tabs
        .items()
        .iter()
        .map(|item| item.label.as_str())
        .collect();
    println!("{:<38} {labels:?}", "the strip projects the order");
}
