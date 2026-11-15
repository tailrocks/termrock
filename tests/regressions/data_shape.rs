use crate::common::*;
use junie_tui::core::data::{DataError, ShapeError};
use junie_tui::core::event::Outcome;
use junie_tui::core::id::WidgetId;
use junie_tui::widgets::grid::{
    CellKind, CellValue, ColumnSpec, DataGrid, GridCmd, GridDataError, GridRows, RowTotal,
};
use junie_tui::widgets::option_editor::{ColumnEditor, Options};
use junie_tui::widgets::table::{Cell, Column, DataTable, SortDir};
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Constraint, Rect};

fn columns() -> Vec<Column> {
    vec![
        Column::new("name", Constraint::Length(10)).editable(),
        Column::new("owner", Constraint::Length(10)).editable(),
    ]
}

fn full(a: &str, b: &str) -> Vec<Cell> {
    vec![Cell::new(a), Cell::new(b)]
}

fn table() -> DataTable {
    DataTable::try_new(
        WidgetId::of("t"),
        columns(),
        vec![full("x", "1"), full("y", "2")],
    )
    .expect("rectangular fixture")
    .cell_nav(true)
}

fn shape(row: usize, expected: usize, actual: usize) -> DataError {
    DataError::Shape(ShapeError {
        row,
        expected,
        actual,
    })
}

fn texts(t: &DataTable) -> Vec<Vec<String>> {
    t.rows()
        .iter()
        .map(|r| r.iter().map(|c| c.text.clone()).collect())
        .collect()
}

fn paint(t: &mut DataTable) -> Vec<String> {
    let (buf, _) = draw(None, |buf, ctx| {
        t.render(Rect::new(0, 0, 40, 6), buf, ctx, ctx.theme.canvas)
    });
    (0..6).map(|y| row(&buf, y)).collect()
}

fn grid_columns() -> Vec<ColumnSpec> {
    vec![
        ColumnSpec::new("a", CellKind::Text),
        ColumnSpec::new("b", CellKind::Text),
    ]
}

fn grid_rows(rows: Vec<Vec<CellValue>>) -> GridRows {
    let n = rows.len();
    GridRows {
        rows,
        total: RowTotal::Exact(n),
        more: false,
    }
}

fn text(s: &str) -> CellValue {
    CellValue::Text(s.to_owned())
}

fn grid() -> DataGrid {
    let mut g = DataGrid::new(WidgetId::of("g"), grid_columns());
    g.local_sort = true;
    g.try_set_rows(grid_rows(vec![
        vec![text("a0"), text("b0")],
        vec![text("a1"), text("b1")],
    ]))
    .expect("rectangular fixture");
    g
}

fn paint_grid(g: &mut DataGrid) {
    draw(None, |buf, ctx| {
        g.render(Rect::new(0, 0, 40, 6), buf, ctx, ctx.theme.canvas)
    });
}

#[test]
fn issue42_table_rejects_short_and_long_rows() {
    assert_eq!(
        DataTable::try_new(WidgetId::of("t"), columns(), vec![vec![Cell::new("x")]]).err(),
        Some(shape(0, 2, 1)),
    );
    assert_eq!(
        DataTable::try_new(
            WidgetId::of("t"),
            columns(),
            vec![
                full("x", "1"),
                vec![Cell::new("a"), Cell::new("b"), Cell::new("c")]
            ],
        )
        .err(),
        Some(shape(1, 2, 3)),
    );

    let mut t = table();
    t.sort_by(0);
    t.cursor_row = 1;
    let before = texts(&t);
    let order: Vec<usize> = (0..t.len()).map(|d| t.source_row(d)).collect();

    assert_eq!(
        t.try_set_rows(vec![full("p", "q"), vec![Cell::new("short")]]),
        Err(shape(1, 2, 1)),
    );
    assert_eq!(
        t.try_set_row(0, vec![Cell::new("short")]),
        Err(shape(0, 2, 1)),
    );
    assert_eq!(
        t.try_set_row(9, full("p", "q")),
        Err(DataError::MissingRow(9))
    );
    assert_eq!(
        t.try_set_model(
            vec![Column::new("only", Constraint::Length(4))],
            vec![full("p", "q")],
        ),
        Err(shape(0, 1, 2)),
    );

    assert_eq!(texts(&t), before, "rejected input leaves the rows alone");
    assert_eq!(
        t.columns().len(),
        2,
        "rejected model leaves the schema alone"
    );
    assert_eq!(t.sort, Some((0, SortDir::Asc)));
    assert_eq!(t.cursor_row, 1);
    let kept: Vec<usize> = (0..t.len()).map(|d| t.source_row(d)).collect();
    assert_eq!(kept, order);

    t.try_set_row(0, full("p", "q")).expect("full-width row");
    assert_eq!(texts(&t)[0], ["p", "q"]);
}

#[test]
fn issue42_empty_schema_is_inert() {
    assert_eq!(
        DataTable::try_new(WidgetId::of("t"), vec![], vec![vec![]]).err(),
        Some(DataError::EmptySchemaWithRows),
    );
    let mut t = DataTable::try_new(WidgetId::of("t"), vec![], vec![])
        .expect("empty schema with no rows")
        .type_ahead(true)
        .cell_nav(true);
    assert_eq!(
        t.try_set_rows(vec![vec![]]),
        Err(DataError::EmptySchemaWithRows)
    );
    paint(&mut t);
    for code in [
        KeyCode::Down,
        KeyCode::Right,
        KeyCode::Enter,
        KeyCode::F(2),
        KeyCode::Char('s'),
        KeyCode::Char('x'),
        KeyCode::Char(' '),
    ] {
        let (out, action) = t
            .on_key(&key(code))
            .try_into_single()
            .expect("at most one action");
        assert_eq!(out, Outcome::Ignored, "{code:?}");
        assert!(action.is_none());
    }
    assert_eq!(t.sort_by(0), Outcome::Consumed);
    assert_eq!(t.start_edit().outcome(), Outcome::Consumed);
    assert!(!t.is_editing());

    let mut g = DataGrid::new(WidgetId::of("g"), vec![]);
    assert_eq!(
        g.try_set_rows(grid_rows(vec![vec![]])),
        Err(GridDataError::Shape(DataError::EmptySchemaWithRows)),
    );
    paint_grid(&mut g);
    assert_eq!(g.begin_edit().outcome(), Outcome::Consumed);
    assert!(
        g.insert_row().is_none(),
        "no phantom row in an empty schema"
    );
    assert!(g.is_empty());
    for cmd in [
        GridCmd::Sort,
        GridCmd::ClearCell,
        GridCmd::Copy,
        GridCmd::CopyWithHeader,
    ] {
        g.run(cmd);
    }
    assert!(g.pending().is_empty());
}

#[test]
fn issue42_grid_sparse_rows_normalize_to_null() {
    let mut g = DataGrid::new(WidgetId::of("g"), grid_columns());
    g.local_sort = true;
    g.try_set_rows(grid_rows(vec![
        vec![text("z")],
        vec![],
        vec![text("a"), text("b")],
    ]))
    .expect("short rows are padded");
    assert!(g.rows().iter().all(|r| r.len() == 2));
    assert_eq!(g.rows()[0], [text("z"), CellValue::Null]);
    assert_eq!(g.rows()[1], [CellValue::Null, CellValue::Null]);

    g.cursor = (0, 1);
    g.run(GridCmd::Sort);
    assert_eq!(
        g.sort,
        Some((1, SortDir::Asc)),
        "sorting the padded column works"
    );
    paint_grid(&mut g);

    g.cursor = (0, 0);
    let _ = g.begin_edit();
    assert!(g.is_editing(), "a padded cell opens the editor");
    g.cancel_edit();
    assert!(g.duplicate_row().is_some());
    assert!(g.rows().iter().all(|r| r.len() == 2));

    assert_eq!(
        g.try_set_rows(grid_rows(vec![
            vec![text("a")],
            vec![text("a"), text("b"), text("c")]
        ])),
        Err(GridDataError::Shape(shape(1, 2, 3))),
        "extra cells are rejected, not dropped",
    );
    let len = g.len();
    assert_eq!(
        g.try_append_rows(grid_rows(vec![vec![text("a"), text("b"), text("c")]])),
        Err(GridDataError::Shape(shape(len, 2, 3))),
        "append reports the eventual source row",
    );
    assert_eq!(g.len(), len);
}

#[test]
fn issue42_shape_failure_is_atomic() {
    let mut t = table();
    t.cursor_col = 1;
    let _ = t.on_key(&key(KeyCode::Enter));
    assert!(t.is_editing());
    let before = texts(&t);
    assert!(
        t.try_set_rows(vec![full("p", "q"), full("r", "s"), vec![Cell::new("bad")]])
            .is_err()
    );
    assert!(
        t.is_editing(),
        "a rejected replacement keeps the open editor"
    );
    assert_eq!(texts(&t), before);

    let mut t = DataTable::try_new(
        WidgetId::of("t"),
        vec![
            Column::new("pick", Constraint::Length(8))
                .editable()
                .editor(ColumnEditor::Select(Options::values(&[
                    "one", "two", "three",
                ]))),
        ],
        vec![vec![Cell::new("one")]],
    )
    .expect("rectangular fixture")
    .cell_nav(true);
    let _ = t.on_key(&key(KeyCode::Enter));
    assert!(t.choice.is_some(), "choice editor opened");
    assert!(t.try_set_rows(vec![vec![]]).is_err());
    assert!(
        t.choice.is_some(),
        "a rejected replacement keeps the choice editor"
    );

    let mut g = grid();
    g.record_cell(0, 1, text("edited"));
    g.toggle_delete(1);
    g.cell_errors.insert((0, 0), "kept".into());
    g.cursor = (0, 0);
    let _ = g.begin_edit();
    assert!(g.is_editing());
    let (pending, rows) = (g.pending().clone(), g.rows().to_vec());
    assert!(
        g.try_set_rows(grid_rows(vec![
            vec![text("n0"), text("n1")],
            vec![text("n0"), text("n1"), text("overflow")],
        ]))
        .is_err()
    );
    assert!(
        g.try_set_model(
            vec![ColumnSpec::new("only", CellKind::Text)],
            grid_rows(vec![vec![text("a"), text("b")]])
        )
        .is_err()
    );
    assert_eq!(
        *g.pending(),
        pending,
        "staged changes survive a rejected replacement"
    );
    assert_eq!(g.rows(), rows.as_slice());
    assert_eq!(g.columns().len(), 2);
    assert!(g.is_editing());
    assert_eq!(g.cell_errors.get(&(0, 0)).map(String::as_str), Some("kept"));
    assert_eq!(g.cursor, (0, 0));
}

#[test]
fn issue42_model_replacement_rebuilds_dependent_state() {
    let mut t = table();
    t.sort_by(1);
    t.cursor_col = 1;
    t.try_set_model(
        vec![Column::new("solo", Constraint::Length(8))],
        vec![vec![Cell::new("b")], vec![Cell::new("a")]],
    )
    .expect("one column, full rows");
    assert_eq!(t.sort, None, "sort on a vanished column is cleared");
    assert_eq!(t.cursor_col, 0);
    assert_eq!(t.numeric.len(), 1);
    assert_eq!(texts(&t), [["b"], ["a"]]);
    paint(&mut t);
    t.sort_by(0);
    assert_eq!(t.source_row(0), 1);

    let rows = || vec![full("a", "10"), full("b", "9")];
    let order = |t: &DataTable| (0..t.len()).map(|d| t.source_row(d)).collect::<Vec<_>>();
    let mut t = table().numeric(&[1]);
    t.try_set_rows(rows()).expect("rectangular");
    t.sort_by(1);
    assert_eq!(order(&t), [1, 0], "a numeric column sorts 9 before 10");

    t.try_set_model(columns(), rows())
        .expect("same schema, refreshed rows");
    assert_eq!(
        t.sort,
        Some((1, SortDir::Asc)),
        "same column keeps its sort"
    );
    assert_eq!(order(&t), [1, 0], "and keeps its numeric flag");

    let mut other = columns();
    other[1] = Column::new("label", Constraint::Length(10));
    t.try_set_model(other, rows())
        .expect("column 1 now means something else");
    assert_eq!(t.sort, None, "a sort on a changed column is cleared");
    t.sort_by(1);
    assert_eq!(
        order(&t),
        [0, 1],
        "the replaced column no longer inherits numeric ordering"
    );

    let mut g = grid();
    g.sort = Some((1, SortDir::Desc));
    g.cursor = (1, 1);
    g.try_set_model(
        vec![ColumnSpec::new("solo", CellKind::Text)],
        grid_rows(vec![vec![text("x")], vec![text("y")]]),
    )
    .expect("one column, full rows");
    assert_eq!(g.sort, None);
    assert_eq!(g.cursor.1, 0);
    assert_eq!(g.columns().len(), 1);
    paint_grid(&mut g);
}

#[test]
fn issue42_table_update_column_is_checked() {
    let mut t = table();
    assert_eq!(
        t.update_column(5, |_| unreachable!("closure runs only for real columns")),
        Err(5)
    );
    assert_eq!(
        t.update_column(0, |_| {}),
        Ok(false),
        "an equal update is a no-op"
    );
    t.cursor_col = 1;
    let _ = t.on_key(&key(KeyCode::Enter));
    assert!(t.is_editing());
    assert_eq!(t.update_column(0, |c| c.title = "renamed".into()), Ok(true));
    assert!(t.is_editing(), "retitling another column keeps the editor");
    assert_eq!(t.columns()[0].title, "renamed");
    assert_eq!(t.update_column(1, |c| c.editable = false), Ok(true));
    assert!(
        !t.is_editing(),
        "making the edited column read-only closes its editor"
    );
}

#[test]
fn issue42_public_stale_cursor_indices_safe() {
    let mut t = table();
    t.cursor_row = 99;
    t.cursor_col = 99;
    t.sort = Some((99, SortDir::Asc));
    paint(&mut t);
    for code in [
        KeyCode::Enter,
        KeyCode::F(2),
        KeyCode::Down,
        KeyCode::Char(' '),
    ] {
        t.cursor_row = 99;
        t.cursor_col = 99;
        let _ = t.on_key(&key(code));
        t.cancel_edit();
    }
    t.cursor_col = 99;
    let _ = t.start_edit();
    t.cancel_edit();
    let _ = t.on_click_header(99);
    t.try_set_rows(vec![full("b", "1"), full("a", "2")])
        .expect("rectangular");
    assert_eq!(
        t.source_row(0),
        0,
        "stale sort column is ignored, not applied"
    );
    assert!(t.out_of_set_cells().is_empty());

    t.edit = Some(junie_tui::widgets::table::EditState {
        row: 99,
        col: 99,
        field: junie_tui::widgets::edit::EditField::single("x".to_owned()),
    });
    assert_eq!(t.commit_edit(), None);
    assert_eq!(
        texts(&t),
        [["b", "1"], ["a", "2"]],
        "no phantom cell appears"
    );

    let mut g = grid();
    g.sort = Some((99, SortDir::Asc));
    g.cursor = (99, 99);
    paint_grid(&mut g);
    for code in [
        KeyCode::Enter,
        KeyCode::F(2),
        KeyCode::Delete,
        KeyCode::Down,
    ] {
        g.cursor = (99, 99);
        let _ = g.on_key(&key(code));
        g.cancel_edit();
    }
    for cmd in [
        GridCmd::Sort,
        GridCmd::ClearCell,
        GridCmd::Copy,
        GridCmd::CopyWithHeader,
        GridCmd::FilterOnCell,
        GridCmd::FollowReference,
    ] {
        g.cursor = (99, 99);
        g.run(cmd);
    }
    g.try_set_rows(grid_rows(vec![vec![text("a")], vec![text("b")]]))
        .expect("padded");
    assert!(
        g.pending()
            .cells()
            .all(|(key, c, _)| g.source_index(key).is_some_and(|r| r < g.rows().len()) && c < 2),
        "no pending entry points outside the model",
    );
}

#[test]
fn issue42_grid_model_rejects_unusable_width_ranges() {
    let mut g = grid();
    g.cursor = (1, 1);
    let mut inverted = grid_columns();
    inverted[1].min_width = 20;
    inverted[1].max_width = 5;
    let err = g
        .try_set_model(inverted, grid_rows(vec![vec![text("x"), text("y")]]))
        .expect_err("min_width above max_width");
    assert_eq!(
        err,
        GridDataError::ColumnWidth {
            column: 1,
            min_width: 20,
            max_width: 5
        }
    );
    assert!(err.to_string().contains("column 1"));

    let mut zero = grid_columns();
    zero[0].max_width = 0;
    zero[0].min_width = 0;
    assert!(matches!(
        g.try_set_model(zero, grid_rows(vec![])),
        Err(GridDataError::ColumnWidth { column: 0, .. })
    ));
    assert_eq!(g.rows().len(), 2, "the live model is untouched");
    assert_eq!(g.columns().len(), 2);
    assert_eq!(g.cursor, (1, 1));
}

#[test]
fn issue42_table_width_only_vs_semantic_column_update() {
    let mut t = table();
    t.cursor_col = 1;
    let _ = t.on_key(&key(KeyCode::Enter));
    assert!(t.is_editing());
    t.sizing.set(0, 14);
    t.sizing.set(1, 16);

    assert_eq!(
        t.update_column(0, |c| c.width = Constraint::Length(20)),
        Ok(true)
    );
    assert_eq!(t.sizing.user(0), None, "a width change resets that sizing");
    assert_eq!(t.sizing.user(1), Some(16), "other columns keep theirs");
    assert!(t.is_editing(), "a width-only change keeps the open editor");

    assert_eq!(t.update_column(0, |c| c.title = "renamed".into()), Ok(true));
    assert_eq!(t.sizing.user(1), Some(16), "a title change keeps sizing");
    assert!(t.is_editing());

    assert_eq!(
        t.update_column(1, |c| c.editor =
            ColumnEditor::for_options(Options::values(&["a", "b"]))),
        Ok(true)
    );
    assert!(
        !t.is_editing(),
        "an editor change is semantic and closes it"
    );
    assert_eq!(t.sizing.user(1), Some(16));
}
