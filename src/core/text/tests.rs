use super::TextBuffer;
use crate::core::edit::{EditKind, UndoStack};

#[test]
fn text_revision_follows_undo_and_redo() {
    let mut buf = TextBuffer::multi("one");
    let mut undo = UndoStack::new();
    undo.record(&buf, EditKind::Other);
    buf.insert_str(" two");
    let edited = buf.revision();
    assert!(undo.undo(&mut buf));
    assert_eq!(buf.text(), "one");
    assert_ne!(buf.revision(), edited, "undo changes the content ticket");
    let undone = buf.revision();
    assert!(undo.redo(&mut buf));
    assert_eq!(buf.text(), "one two");
    assert_ne!(buf.revision(), undone);
}
