//! Compound widget inputs return every semantic action once, in order.

mod common;

use common::*;
use junie_tui::core::event::Outcome;
use junie_tui::core::id::WidgetId;
use junie_tui::core::response::{Actions, Flow, Invalidate, Response};
use junie_tui::widgets::grid::{
    CellKind, CellValue, ColumnSpec, DataGrid, GridAction, GridRows, RowTotal,
};
use junie_tui::widgets::option_editor::{ColumnEditor, Options};
use junie_tui::widgets::table::{Cell, Column, DataTable, TableAction};
use junie_tui::widgets::tree::{BranchAction, TreeAction, TreeNode, TreeView};
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Constraint;

const ID: WidgetId = WidgetId::of("t.batch");

fn text(s: &str) -> Vec<junie_tui::core::event::Key> {
    s.chars().map(|c| key(KeyCode::Char(c))).collect()
}

fn actions<A>(r: Response<A>) -> Vec<A> {
    r.into_actions().into_iter().collect()
}

// ----- Response / Actions ---------------------------------------------------

#[test]
fn response_zero_one_are_inline() {
    let none: Response<u8> = Response::changed();
    assert!(matches!(none.into_actions(), Actions::None));
    let one = Response::emit(7u8);
    assert!(matches!(one.into_actions(), Actions::One(7)));
    let mut two = Response::emit(1u8);
    two.push_action(2);
    assert!(matches!(two.into_actions(), Actions::Many(v) if v == vec![1, 2]));
}

#[test]
fn response_merge_preserves_order_and_flags() {
    let mut first = Response::<char>::emit('A');
    first.push_action('B');
    let second = Response::emit('C').relayout();
    let merged = first.merge(second);
    assert_eq!(merged.invalidate(), Invalidate::Layout);
    assert_eq!(merged.flow(), Flow::Consumed);
    let (outcome, got) = merged.into_parts();
    assert_eq!(outcome, Outcome::Changed);
    assert_eq!(got.into_iter().collect::<String>(), "ABC");
}

#[test]
fn response_bitor_keeps_every_action() {
    let batch = Response::emit(1u8) | Response::emit(2) | Response::consumed();
    assert_eq!(actions(batch), vec![1, 2]);
}

#[test]
fn response_single_conversion_preserves_batch_on_error() {
    let mut two = Response::emit('a');
    two.push_action('b');
    let back = two.try_into_single().expect_err("two actions");
    assert_eq!(back.actions().copied().collect::<Vec<_>>(), vec!['a', 'b']);
    assert_eq!(actions(back), vec!['a', 'b']);
    let (outcome, single) = Response::emit('z').try_into_single().expect("one action");
    assert_eq!((outcome, single), (Outcome::Changed, Some('z')));
}

#[test]
fn response_layout_survives_nested_action_dispatch() {
    let inner: Response<u8> = Response::emit(3).relayout();
    let outer = inner.map_actions(|n| u32::from(n) * 2);
    let (flow, invalidate, got) = outer.into_raw_parts();
    assert_eq!((flow, invalidate), (Flow::Consumed, Invalidate::Layout));
    assert_eq!(got.into_iter().collect::<Vec<_>>(), vec![6]);
    let drained = Response::emit(1u8).relayout().on_actions(|_| {});
    assert_eq!(drained.invalidate(), Invalidate::Layout);
}

// ----- DataTable ------------------------------------------------------------

fn table() -> DataTable {
    let mut t = DataTable::new(
        ID,
        vec![
            Column::new("Name", Constraint::Length(12)).editable(),
            Column::new("Kind", Constraint::Length(12))
                .editor(ColumnEditor::Select(Options::values(&["a", "b", "c"]))),
        ],
        vec![vec![Cell::new("one"), Cell::new("a")]],
    );
    t.cell_nav = true;
    t
}

#[test]
fn table_tab_emits_commit_then_leave() {
    let mut t = table();
    t.cursor_col = 1;
    let _ = t.begin_edit();
    // Open the choice editor, pick "b" with the keyboard, then Tab off the last column.
    let _ = t.on_key(&key(KeyCode::Down));
    let r = t.on_key(&key(KeyCode::Tab));
    assert_eq!(
        actions(r),
        vec![
            TableAction::Committed { row: 0, col: 1 },
            TableAction::LeaveForward
        ]
    );
}

#[test]
fn table_tab_text_edge_commits_before_leave_and_shift_tab_backward() {
    let mut t = table();
    t.cursor_col = 0;
    let _ = t.begin_edit();
    for k in text("x") {
        let _ = t.on_key(&k);
    }
    let r = t.on_key(&chord(
        KeyCode::BackTab,
        ratatui::crossterm::event::KeyModifiers::SHIFT,
    ));
    assert_eq!(
        actions(r),
        vec![
            TableAction::Committed { row: 0, col: 0 },
            TableAction::LeaveBackward
        ]
    );
    assert_eq!(t.rows[0][0].text, "onex");
}

#[test]
fn table_tab_unchanged_text_at_edge_only_leaves() {
    let mut t = table();
    t.cursor_col = 0;
    let _ = t.begin_edit();
    let r = t.on_key(&chord(
        KeyCode::BackTab,
        ratatui::crossterm::event::KeyModifiers::SHIFT,
    ));
    assert_eq!(actions(r), vec![TableAction::LeaveBackward]);
    assert!(!t.is_editing());
}

#[test]
fn table_tab_invalid_edit_stays_and_emits_nothing() {
    let mut t = table().validator(|_, s| s.contains('!').then(|| "no bangs".into()));
    t.cursor_col = 0;
    let _ = t.begin_edit();
    for k in text("!") {
        let _ = t.on_key(&k);
    }
    let r = t.on_key(&key(KeyCode::Tab));
    assert!(t.is_editing(), "the editor stays open");
    assert_eq!(r.actions().len(), 0);
    assert_eq!(t.cursor_col, 0, "no movement");
}

#[test]
fn table_tab_into_toggle_cell_does_not_flip_it() {
    let mut t = DataTable::new(
        ID,
        vec![
            Column::new("Name", Constraint::Length(12)).editable(),
            Column::new("On", Constraint::Length(6))
                .editor(ColumnEditor::Checkbox(Options::values(&["off", "on"]))),
        ],
        vec![vec![Cell::new("one"), Cell::new("off")]],
    );
    t.cell_nav = true;
    let _ = t.begin_edit();
    for k in text("x") {
        let _ = t.on_key(&k);
    }
    let r = t.on_key(&key(KeyCode::Tab));
    assert_eq!(actions(r), vec![TableAction::Committed { row: 0, col: 0 }]);
    assert_eq!(t.cursor_col, 1, "focus moved to the toggle");
    assert_eq!(t.rows[0][1].text, "off", "traversal never flips the value");
    let r = t.on_key(&key(KeyCode::Enter));
    assert_eq!(actions(r), vec![TableAction::Committed { row: 0, col: 1 }]);
    assert_eq!(
        t.rows[0][1].text, "on",
        "explicit activation still flips it"
    );
}

// ----- DataGrid -------------------------------------------------------------

fn grid(kinds: &[CellKind], row: Vec<CellValue>) -> DataGrid {
    let cols = kinds
        .iter()
        .enumerate()
        .map(|(i, k)| ColumnSpec::new(&format!("c{i}"), *k))
        .collect();
    let mut g = DataGrid::new(ID, cols).editable(true);
    g.set_rows(GridRows {
        rows: vec![row],
        total: RowTotal::Exact(1),
        more: false,
    });
    g
}

#[test]
fn grid_tab_edge_emits_change_then_leave() {
    let mut g = grid(&[CellKind::Text], vec![CellValue::Text("a".into())]);
    let _ = g.begin_edit();
    for k in text("z") {
        let _ = g.on_key(&k);
    }
    let r = g.on_key(&key(KeyCode::Tab));
    assert_eq!(
        actions(r),
        vec![
            GridAction::CellChanged { row: 0, col: 0 },
            GridAction::LeaveForward
        ]
    );
}

fn choice_grid(kinds: &[CellKind]) -> DataGrid {
    let cols = kinds
        .iter()
        .enumerate()
        .map(|(i, k)| {
            let spec = ColumnSpec::new(&format!("c{i}"), *k);
            if *k == CellKind::Enum {
                spec.editor(ColumnEditor::Select(Options::values(&["a", "b", "c"])))
            } else {
                spec
            }
        })
        .collect();
    let mut g = DataGrid::new(ID, cols).editable(true);
    g.set_rows(GridRows {
        rows: vec![
            kinds
                .iter()
                .map(|k| match k {
                    CellKind::Enum => CellValue::Text("a".into()),
                    _ => CellValue::Text("x".into()),
                })
                .collect(),
        ],
        total: RowTotal::Exact(1),
        more: false,
    });
    g
}

/// A valid edit that leaves the value as it was must not block traversal:
/// Tab at the edge still reports the leave and closes the editor. (Whether the
/// no-op also reports `CellChanged` is #48's equality rule, so only the
/// movement is asserted.)
#[test]
fn grid_tab_valid_noop_edge_still_leaves() {
    let mut g = grid(&[CellKind::Text], vec![CellValue::Text("a".into())]);
    let _ = g.begin_edit();
    let got = actions(g.on_key(&key(KeyCode::Tab)));
    assert_eq!(got.last(), Some(&GridAction::LeaveForward));
    assert!(got.len() <= 2, "at most the commit and the leave: {got:?}");
    assert!(!g.is_editing(), "the editor closed");
    assert_eq!(*g.value(0, 0), CellValue::Text("a".into()), "value kept");
}

/// A valid no-op edit between two cells moves on and opens the next editor.
#[test]
fn grid_tab_valid_noop_moves_to_next_cell_and_edits() {
    let mut g = grid(
        &[CellKind::Text, CellKind::Text],
        vec![CellValue::Text("a".into()), CellValue::Text("b".into())],
    );
    let _ = g.begin_edit();
    let got = actions(g.on_key(&key(KeyCode::Tab)));
    assert!(!got.contains(&GridAction::LeaveForward), "{got:?}");
    assert_eq!(g.cursor.1, 1, "focus moved");
    assert!(g.is_editing(), "the next cell is open for editing");
}

#[test]
fn grid_choice_tab_emits_change_then_leave() {
    let mut g = choice_grid(&[CellKind::Enum]);
    let _ = g.begin_edit();
    let _ = g.on_key(&key(KeyCode::Down));
    let r = g.on_key(&key(KeyCode::Tab));
    assert_eq!(
        actions(r),
        vec![
            GridAction::CellChanged { row: 0, col: 0 },
            GridAction::LeaveForward
        ]
    );
    assert_eq!(*g.value(0, 0), CellValue::Text("b".into()));
}

#[test]
fn grid_choice_shift_tab_emits_change_then_leave_backward() {
    let mut g = choice_grid(&[CellKind::Enum]);
    let _ = g.begin_edit();
    let _ = g.on_key(&key(KeyCode::Down));
    let r = g.on_key(&chord(
        KeyCode::BackTab,
        ratatui::crossterm::event::KeyModifiers::SHIFT,
    ));
    assert_eq!(
        actions(r),
        vec![
            GridAction::CellChanged { row: 0, col: 0 },
            GridAction::LeaveBackward
        ]
    );
}

#[test]
fn grid_choice_tab_into_next_cell_reports_only_the_commit() {
    let mut g = choice_grid(&[CellKind::Enum, CellKind::Text]);
    let _ = g.begin_edit();
    let _ = g.on_key(&key(KeyCode::Down));
    let r = g.on_key(&key(KeyCode::Tab));
    assert_eq!(actions(r), vec![GridAction::CellChanged { row: 0, col: 0 }]);
    assert_eq!(g.cursor.1, 1, "focus moved to the text cell");
    assert!(g.is_editing(), "the next editor opened");
}

#[test]
fn grid_invalid_tab_emits_no_leave() {
    let mut g = grid(&[CellKind::Number], vec![CellValue::Int(1)]);
    let _ = g.begin_edit();
    for k in text("x") {
        let _ = g.on_key(&k);
    }
    let r = g.on_key(&key(KeyCode::Tab));
    assert!(g.is_editing(), "the editor stays with its error");
    assert_eq!(r.actions().len(), 0, "no change and no leave");
}

#[test]
fn grid_traversal_does_not_activate_checkbox_or_viewer() {
    for (kind, value) in [
        (CellKind::Bool, CellValue::Bool(false)),
        (CellKind::Json, CellValue::Json("{}".into())),
        (CellKind::Text, CellValue::Text("x".repeat(200))),
    ] {
        let mut g = grid(
            &[CellKind::Text, kind],
            vec![CellValue::Text("a".into()), value.clone()],
        );
        let _ = g.begin_edit();
        for k in text("z") {
            let _ = g.on_key(&k);
        }
        let r = g.on_key(&key(KeyCode::Tab));
        assert_eq!(
            actions(r),
            vec![GridAction::CellChanged { row: 0, col: 0 }],
            "{kind:?}: only the commit is reported"
        );
        assert_eq!(g.cursor.1, 1, "{kind:?}: focus moved");
        assert_eq!(*g.value(0, 1), value, "{kind:?}: value untouched");
        let r = g.on_key(&key(KeyCode::Enter));
        assert!(r.has_actions(), "{kind:?}: explicit activation still acts");
    }
}

// ----- TreeView -------------------------------------------------------------

#[test]
fn tree_both_emits_expand_then_activate() {
    let mut t = TreeView::new(
        ID,
        vec![TreeNode::dir("root", vec![TreeNode::lazy("lazy")])],
    )
    .branch_activate(BranchAction::Both);
    let _ = t.on_key(&key(KeyCode::Down));
    let r = t.on_key(&key(KeyCode::Enter));
    assert_eq!(
        actions(r),
        vec![
            TreeAction::Expand(vec![0, 0]),
            TreeAction::Activate(vec![0, 0])
        ]
    );
    assert_eq!(t.selected, Some(vec![0, 0]));
}

#[test]
fn tree_both_collapse_still_activates() {
    let mut t = TreeView::new(ID, vec![TreeNode::dir("b", vec![TreeNode::leaf("c")])])
        .branch_activate(BranchAction::Both);
    let r = t.on_key(&key(KeyCode::Enter));
    assert_eq!(actions(r), vec![TreeAction::Activate(vec![0])]);
}
