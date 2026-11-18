use crate::common::*;
use junie_tui::core::data::{DataError, ShapeError};
use junie_tui::core::id::WidgetId;
use junie_tui::widgets::grid::{
    CellKind, CellValue, ColumnSpec, DataGrid, GridAction, GridDataError, GridRows, RowTotal,
};
use junie_tui::widgets::option_editor::{ColumnEditor, Options};
use junie_tui::widgets::table::{Cell, Column, DataTable, TableAction};
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Constraint;

fn columns() -> Vec<Column> {
    vec![
        Column::new("name", Constraint::Length(10)).editable(),
        Column::new("owner", Constraint::Length(10)).editable(),
    ]
}

fn full(a: &str, b: &str) -> Vec<Cell> {
    vec![Cell::new(a), Cell::new(b)]
}

fn texts(t: &DataTable) -> Vec<Vec<String>> {
    t.rows()
        .iter()
        .map(|r| r.iter().map(|c| c.text.clone()).collect())
        .collect()
}

fn type_chars(t: &mut DataTable, s: &str) {
    for c in s.chars() {
        let _ = t.on_key(&key(KeyCode::Char(c)));
    }
}

fn text(s: &str) -> CellValue {
    CellValue::Text(s.to_owned())
}

#[test]
fn issue32_failed_shape_change_keeps_editor() {
    let mut t = DataTable::try_new(
        WidgetId::of("t"),
        columns(),
        vec![full("x", "1"), full("y", "2")],
    )
    .expect("rectangular fixture")
    .cell_nav(true);
    let _ = t.on_key(&key(KeyCode::Enter));
    type_chars(&mut t, "zz");
    assert_eq!(draft(&t), "xzz");

    let before = texts(&t);
    let shape = DataError::Shape(ShapeError {
        row: 1,
        expected: 2,
        actual: 1,
    });
    assert_eq!(
        t.try_set_rows(vec![full("p", "q"), vec![Cell::new("short")]]),
        Err(shape),
    );
    assert!(
        t.is_editing(),
        "a rejected replacement keeps the text editor"
    );
    assert_eq!(draft(&t), "xzz", "the draft survives the rejection");
    assert_eq!(texts(&t), before, "rows are untouched by the rejection");

    assert_eq!(
        t.commit_edit(),
        Some(TableAction::Committed { row: 0, col: 0 }),
        "the surviving draft commits to its original row",
    );
    assert_eq!(texts(&t)[0][0], "xzz");

    let mut t = DataTable::try_new(
        WidgetId::of("t"),
        vec![
            Column::new("pick", Constraint::Length(8))
                .editable()
                .editor(ColumnEditor::Select(Options::values(&[
                    "one", "two", "three",
                ]))),
        ],
        vec![vec![Cell::new("one")], vec![Cell::new("one")]],
    )
    .expect("rectangular fixture")
    .cell_nav(true);
    let _ = t.on_key(&key(KeyCode::Enter));
    let _ = t.on_key(&key(KeyCode::Down));
    assert!(t.choice.is_some(), "choice editor opened");

    assert!(t.try_set_rows(vec![vec![], vec![]]).is_err());
    assert!(
        t.choice.is_some(),
        "a rejected replacement keeps the choice editor"
    );
    assert_eq!(texts(&t)[0][0], "one");

    let (_, action) = t
        .on_key(&key(KeyCode::Enter))
        .try_into_single()
        .expect("at most one action");
    assert_eq!(action, Some(TableAction::Committed { row: 0, col: 0 }));
    assert_eq!(texts(&t)[0][0], "two", "the highlighted option commits");

    let mut g = DataGrid::new(
        WidgetId::of("g"),
        vec![
            ColumnSpec::new("a", CellKind::Text),
            ColumnSpec::new("b", CellKind::Text),
        ],
    );
    g.try_set_rows(GridRows {
        rows: vec![vec![text("a0"), text("b0")], vec![text("a1"), text("b1")]],
        total: RowTotal::Exact(2),
        more: false,
    })
    .expect("rectangular fixture");
    let _ = g.on_key(&key(KeyCode::Enter));
    let _ = g.on_key(&key(KeyCode::Char('z')));
    assert!(g.is_editing());
    assert_eq!(grid_draft(&g), "a0z");

    let pending = g.pending().clone();
    let rejected = g.try_set_rows(GridRows {
        rows: vec![
            vec![text("n0"), text("n1")],
            vec![text("n0"), text("n1"), text("overflow")],
        ],
        total: RowTotal::Exact(2),
        more: false,
    });
    assert!(matches!(rejected, Err(GridDataError::Shape(_))));
    assert!(
        g.is_editing(),
        "a rejected replacement keeps the grid editor"
    );
    assert_eq!(
        grid_draft(&g),
        "a0z",
        "the grid draft survives the rejection"
    );
    assert_eq!(*g.pending(), pending);
    assert_eq!(g.value(0, 0), &text("a0"));

    assert_eq!(
        g.commit_edit(),
        Some(GridAction::CellChanged { row: 0, col: 0 }),
        "the surviving grid draft commits to its original row",
    );
    assert_eq!(g.value(0, 0), &text("a0z"));
}

fn draft(t: &DataTable) -> String {
    t.edit
        .as_ref()
        .expect("text editor open")
        .field
        .text()
        .to_owned()
}

fn grid_draft(g: &DataGrid) -> String {
    g.edit
        .as_ref()
        .expect("grid editor open")
        .field
        .text()
        .to_owned()
}
