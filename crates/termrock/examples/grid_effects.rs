//! A data grid reports what the user did as typed actions, in order. The host
//! turns them into effects: save a cell, then move focus.
//!
//! `cargo run -p termrock --example grid_effects`

mod common;

use common::{frame, key};
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use termrock::core::id::WidgetId;
use termrock::core::response::Response;
use termrock::widgets::grid::{
    CellKind, CellValue, ColumnSpec, DataGrid, GridAction, GridRows, RowTotal,
};

/// What the host does for each action. Actions it does not handle are skipped.
#[derive(Debug, PartialEq, Eq)]
enum Effect {
    Save { row: usize, col: usize },
    FocusNext,
    FocusPrevious,
}

fn effects(response: Response<GridAction>) -> Vec<Effect> {
    response
        .actions()
        .filter_map(|action| match action {
            GridAction::CellChanged { row, col } => Some(Effect::Save {
                row: *row,
                col: *col,
            }),
            GridAction::LeaveForward => Some(Effect::FocusNext),
            GridAction::LeaveBackward => Some(Effect::FocusPrevious),
            _ => None,
        })
        .collect()
}

fn grid() -> DataGrid {
    let mut grid = DataGrid::new(
        WidgetId::of("example.grid"),
        vec![ColumnSpec::new("Name", CellKind::Text)],
    )
    .editable(true);
    grid.try_set_rows(GridRows {
        rows: vec![vec![CellValue::Text("a".into())]],
        total: RowTotal::Exact(1),
        more: false,
    })
    .expect("a rectangular row");
    grid
}

fn screen(grid: &mut DataGrid) -> String {
    let area = Rect::new(0, 0, 24, 6);
    frame(area.width, area.height, |buf, ctx| {
        grid.render(area, buf, ctx, ctx.theme.canvas);
    })
    .0
}

fn edit_then_tab(grid: &mut DataGrid) -> Vec<Effect> {
    let _ = grid.begin_edit();
    let _ = grid.on_key(&key(KeyCode::Char('z')));
    effects(grid.on_key(&key(KeyCode::Tab)))
}

fn main() {
    let mut grid = grid();
    println!("{}", screen(&mut grid));
    println!("effects: {:?}", edit_then_tab(&mut grid));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_edit_is_reported_before_the_focus_leaves() {
        assert_eq!(
            edit_then_tab(&mut grid()),
            [Effect::Save { row: 0, col: 0 }, Effect::FocusNext],
        );
    }

    #[test]
    fn a_key_that_changes_nothing_yields_no_effects() {
        assert_eq!(effects(grid().on_key(&key(KeyCode::Char('x')))), []);
    }
}
