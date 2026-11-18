//! A lazy node that is a root asks for its children on the first open, like
//! a lazy node anywhere else in the tree.

use crate::common::*;
use ratatui::crossterm::event::KeyCode;
use termrock::core::event::Outcome;
use termrock::core::id::WidgetId;
use termrock::widgets::tree::{TreeAction, TreeNode, TreeView};

const ID: WidgetId = WidgetId::of("t.lazy_root");

fn actions(tree: &mut TreeView, code: KeyCode) -> (Outcome, Vec<TreeAction>) {
    let r = tree.on_key(&key(code));
    let outcome = r.outcome();
    (outcome, r.into_actions().into_iter().collect())
}

fn roots() -> Vec<TreeNode> {
    vec![
        TreeNode::dir("local", vec![TreeNode::leaf("a.txt")]),
        TreeNode::lazy("remote"),
    ]
}

#[test]
fn a_lazy_root_starts_closed_and_the_first_open_asks_for_its_children() {
    let mut tree = TreeView::new(ID, roots());
    assert!(
        !tree.rows()[2].expanded,
        "an unloaded root is shown closed, not open and empty"
    );
    let _ = actions(&mut tree, KeyCode::Down);
    let _ = actions(&mut tree, KeyCode::Down);
    assert_eq!(tree.cursor_path(), Some(&[1usize][..]));
    let (outcome, acts) = actions(&mut tree, KeyCode::Enter);
    assert_eq!(outcome, Outcome::Changed);
    assert_eq!(
        acts,
        vec![TreeAction::Expand(vec![1])],
        "the first open asks the host for the children"
    );
    assert!(tree.rows()[2].busy, "the row shows it is loading");
}

#[test]
fn a_loaded_root_starts_open() {
    let tree = TreeView::new(ID, roots());
    assert!(tree.rows()[0].expanded, "a root with children starts open");
    assert_eq!(tree.rows().len(), 3);
}
