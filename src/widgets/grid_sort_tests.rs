use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use crate::core::event::Key;
use crate::core::id::WidgetId;
use crate::widgets::grid::{
    CellKind, CellValue, ColumnSpec, DataGrid, GridAction, GridRows, ORDER_REBUILDS, RowTotal,
};
use crate::widgets::table::SortDir;

fn int(n: i64) -> CellValue {
    CellValue::Int(n)
}

fn rows(items: &[(Option<i64>, &str)]) -> Vec<Vec<CellValue>> {
    items
        .iter()
        .map(|(n, tag)| {
            vec![
                n.map_or(CellValue::Null, int),
                CellValue::Text((*tag).into()),
            ]
        })
        .collect()
}

fn page(items: &[(Option<i64>, &str)], more: bool) -> GridRows {
    GridRows {
        rows: rows(items),
        total: RowTotal::Exact(items.len()),
        more,
    }
}

fn grid(local_sort: bool, items: &[(Option<i64>, &str)]) -> DataGrid {
    let mut g = DataGrid::new(
        WidgetId::of("sort-tests"),
        vec![
            ColumnSpec::new("n", CellKind::Number),
            ColumnSpec::new("tag", CellKind::Text),
        ],
    );
    g.local_sort = local_sort;
    g.set_rows(page(items, false));
    g
}

fn press(g: &mut DataGrid, code: KeyCode) -> Option<GridAction> {
    g.on_key(&Key {
        code,
        mods: KeyModifiers::NONE,
    })
    .1
}

fn press_shift(g: &mut DataGrid, code: KeyCode) {
    g.on_key(&Key {
        code,
        mods: KeyModifiers::SHIFT,
    });
}

fn shown(g: &DataGrid) -> Vec<String> {
    (0..g.len())
        .map(|d| g.value(g.source_row(d), 1).text())
        .collect()
}

fn cursor_tag(g: &DataGrid) -> String {
    g.value(g.source_row(g.cursor.0), 1).text()
}

fn move_to(g: &mut DataGrid, display: usize) {
    while g.cursor.0 < display {
        press(g, KeyCode::Down);
    }
    while g.cursor.0 > display {
        press(g, KeyCode::Up);
    }
}

fn edit_cursor_cell(g: &mut DataGrid, text: &str) {
    press(g, KeyCode::Enter);
    assert!(g.is_editing(), "Enter opens the cell editor");
    for _ in 0..8 {
        press(g, KeyCode::Backspace);
    }
    for ch in text.chars() {
        press(g, KeyCode::Char(ch));
    }
    press(g, KeyCode::Enter);
    assert!(!g.is_editing(), "Enter commits the edit: {text}");
}

fn rebuilds() -> usize {
    ORDER_REBUILDS.with(|n| n.get())
}

#[test]
fn issue46_append_preserves_active_local_sort() {
    let mut g = grid(true, &[(Some(2), "two"), (Some(4), "four")]);
    press(&mut g, KeyCode::Char('s'));
    assert_eq!(g.sort, Some((0, SortDir::Asc)));
    move_to(&mut g, 1);
    assert_eq!(cursor_tag(&g), "four");

    g.append_rows(page(
        &[(Some(1), "one"), (Some(3), "three"), (None, "null")],
        false,
    ));

    assert_eq!(shown(&g), ["one", "two", "three", "four", "null"]);
    assert_eq!(cursor_tag(&g), "four", "cursor follows its record");

    press(&mut g, KeyCode::Char('s'));
    assert_eq!(g.sort, Some((0, SortDir::Desc)));
    g.append_rows(page(&[(Some(3), "three-b"), (None, "null-b")], false));
    assert_eq!(
        shown(&g),
        ["null-b", "null", "four", "three-b", "three", "two", "one"],
        "Desc keeps Null first and reverses ties"
    );
}

#[test]
fn issue46_commit_reorders_changed_sort_column() {
    let mut g = grid(
        true,
        &[(Some(2), "two"), (Some(1), "one"), (Some(5), "five")],
    );
    press(&mut g, KeyCode::Char('s'));
    assert_eq!(shown(&g), ["one", "two", "five"]);
    move_to(&mut g, 1);
    let src = g.source_row(1);
    g.selection.select_only(src, |_| true);

    edit_cursor_cell(&mut g, "9");
    assert_eq!(
        shown(&g),
        ["one", "two", "five"],
        "a pending edit does not move its row"
    );

    g.apply_commit_result(Ok(()));

    assert_eq!(shown(&g), ["one", "five", "two"]);
    assert_eq!(g.value(g.source_row(2), 0), &int(9));
    assert_eq!(cursor_tag(&g), "two", "cursor stays on the edited record");
    assert!(g.selection.contains(g.source_row(2)), "selection follows");
    assert!(g.pending.is_empty());
}

#[test]
fn issue46_append_sorts_by_saved_values_not_pending() {
    let mut g = grid(
        true,
        &[(Some(2), "two"), (Some(1), "one"), (Some(5), "five")],
    );
    press(&mut g, KeyCode::Char('s'));
    assert_eq!(shown(&g), ["one", "two", "five"]);
    move_to(&mut g, 1);
    assert_eq!(cursor_tag(&g), "two");
    edit_cursor_cell(&mut g, "9");
    assert!(!g.pending.is_empty(), "the edit is still pending");

    g.append_rows(page(&[(Some(3), "three")], false));

    assert_eq!(
        shown(&g),
        ["one", "two", "three", "five"],
        "the pending 9 does not move its row; order uses saved values"
    );
}

#[test]
fn issue46_anchor_follows_its_record() {
    let mut g = grid(
        true,
        &[(Some(1), "one"), (Some(2), "two"), (Some(3), "three")],
    );
    press(&mut g, KeyCode::Char('s'));
    move_to(&mut g, 2);
    assert_eq!(cursor_tag(&g), "three");
    press_shift(&mut g, KeyCode::Up);
    assert_eq!(g.anchor(), Some((2, 0)), "anchor on display 2");
    assert_eq!(g.cursor.0, 1);

    g.append_rows(page(&[(Some(0), "zero")], false));

    assert_eq!(shown(&g), ["zero", "one", "two", "three"]);
    assert_eq!(
        g.anchor(),
        Some((3, 0)),
        "anchor moves with its record to display 3"
    );
}

#[test]
fn issue46_commit_keeps_anchor_on_its_record() {
    let mut g = grid(
        true,
        &[(Some(1), "one"), (Some(2), "two"), (Some(3), "three")],
    );
    press(&mut g, KeyCode::Char('s'));
    move_to(&mut g, 2);
    press_shift(&mut g, KeyCode::Up);
    assert_eq!(g.anchor(), Some((2, 0)));
    move_to(&mut g, 1);
    assert_eq!(cursor_tag(&g), "two");
    edit_cursor_cell(&mut g, "9");

    g.apply_commit_result(Ok(()));

    assert_eq!(shown(&g), ["one", "three", "two"]);
    assert_eq!(
        g.anchor(),
        Some((1, 0)),
        "anchor follows the three record to display 1"
    );
}

#[test]
fn issue46_commit_elsewhere_keeps_sorted_order() {
    let mut g = grid(true, &[(Some(2), "two"), (Some(1), "one")]);
    press(&mut g, KeyCode::Char('s'));
    assert_eq!(shown(&g), ["one", "two"]);
    press(&mut g, KeyCode::Right);
    edit_cursor_cell(&mut g, "uno");
    g.apply_commit_result(Ok(()));
    assert_eq!(shown(&g), ["uno", "two"]);
    assert_eq!(g.sort, Some((0, SortDir::Asc)));
}

#[test]
fn issue46_inserted_drafts_stay_tail_until_commit() {
    let mut g = grid(true, &[(Some(3), "three"), (Some(1), "one")]);
    press(&mut g, KeyCode::Char('s'));
    press(&mut g, KeyCode::Char('+'));
    assert_eq!(g.pending.inserted.len(), 1);
    g.cursor.1 = 0;
    edit_cursor_cell(&mut g, "0");
    g.append_rows(page(&[(Some(2), "two")], false));

    let draft = g.source_row(3);
    assert!(
        g.pending.inserted.contains(&draft),
        "draft stays at the tail"
    );
    assert_eq!(
        (0..3)
            .map(|d| g.value(g.source_row(d), 0).text())
            .collect::<Vec<_>>(),
        ["1", "2", "3"],
        "base prefix is sorted"
    );

    g.apply_commit_result(Ok(()));
    assert_eq!(
        (0..4)
            .map(|d| g.value(g.source_row(d), 0).text())
            .collect::<Vec<_>>(),
        ["0", "1", "2", "3"],
        "the committed draft joins the sorted group"
    );
}

#[test]
fn issue46_delete_cursor_fallback_is_deterministic() {
    let all = [
        (Some(3), "c"),
        (Some(1), "a"),
        (Some(4), "d"),
        (Some(2), "b"),
    ];
    let mut g = grid(true, &all);
    press(&mut g, KeyCode::Char('s'));
    assert_eq!(shown(&g), ["a", "b", "c", "d"]);
    move_to(&mut g, 1);
    g.selection.select_only(g.source_row(3), |_| true);
    press(&mut g, KeyCode::Char('-'));
    g.apply_commit_result(Ok(()));
    assert_eq!(shown(&g), ["a", "c", "d"]);
    assert_eq!(cursor_tag(&g), "c", "next survivor in old display order");
    assert_eq!(g.selection.len(), 1);
    assert!(
        g.selection.contains(g.source_row(2)),
        "selection remapped to d"
    );

    move_to(&mut g, 2);
    press(&mut g, KeyCode::Char('-'));
    g.apply_commit_result(Ok(()));
    assert_eq!(shown(&g), ["a", "c"]);
    assert_eq!(cursor_tag(&g), "c", "falls back to the previous survivor");
    assert!(g.selection.is_empty());

    for _ in 0..2 {
        press(&mut g, KeyCode::Char('-'));
        press(&mut g, KeyCode::Up);
    }
    g.apply_commit_result(Ok(()));
    assert!(g.is_empty());
    assert_eq!(g.cursor.0, 0);
}

#[test]
fn issue46_delete_and_reorder_in_one_commit_keeps_cursor_fallback() {
    let mut g = grid(
        true,
        &[
            (Some(2), "two"),
            (Some(1), "one"),
            (Some(5), "five"),
            (Some(3), "three"),
        ],
    );
    press(&mut g, KeyCode::Char('s'));
    assert_eq!(shown(&g), ["one", "two", "three", "five"]);
    move_to(&mut g, 1);
    assert_eq!(cursor_tag(&g), "two");
    press(&mut g, KeyCode::Char('-'));
    move_to(&mut g, 3);
    assert_eq!(cursor_tag(&g), "five");
    edit_cursor_cell(&mut g, "0");
    move_to(&mut g, 1);
    assert_eq!(cursor_tag(&g), "two", "cursor rests on the deleted row");

    g.apply_commit_result(Ok(()));

    assert_eq!(shown(&g), ["five", "one", "three"]);
    assert_eq!(
        cursor_tag(&g),
        "three",
        "deleted cursor falls back to the next survivor in old display order"
    );
}

#[test]
fn issue46_remote_and_failure_do_not_sort() {
    let mut g = grid(false, &[(Some(2), "two"), (Some(1), "one")]);
    let action = press(&mut g, KeyCode::Char('s'));
    assert_eq!(
        action,
        Some(GridAction::SortRequested(Some((0, SortDir::Asc))))
    );
    g.append_rows(page(&[(Some(0), "zero")], false));
    assert_eq!(
        shown(&g),
        ["two", "one", "zero"],
        "remote order is host-owned"
    );

    let mut g = grid(true, &[(Some(2), "two"), (Some(1), "one")]);
    press(&mut g, KeyCode::Char('s'));
    let before = rebuilds();
    edit_cursor_cell(&mut g, "9");
    edit_cursor_cell(&mut g, "9");
    assert_eq!(rebuilds(), before, "typing and no-op edits never rebuild");
    g.apply_commit_result(Err((0, "rejected".into())));
    assert_eq!(rebuilds(), before, "a failed commit never rebuilds");
    assert_eq!(shown(&g), ["one", "two"]);
    assert!(!g.pending.is_empty(), "the draft survives a failed commit");
}

#[test]
fn issue46_empty_append_does_not_resort() {
    let mut g = grid(true, &[(Some(2), "two"), (Some(1), "one")]);
    press(&mut g, KeyCode::Char('s'));
    let before = rebuilds();
    g.append_rows(GridRows {
        rows: vec![],
        total: RowTotal::Exact(2),
        more: true,
    });
    assert_eq!(rebuilds(), before);
    assert!(g.more, "metadata still applies");
    assert_eq!(shown(&g), ["one", "two"]);

    g.append_rows(page(&[(Some(0), "zero")], false));
    assert_eq!(rebuilds(), before + 1, "a real append rebuilds once");
}

#[test]
fn issue46_empty_commit_and_stale_sort_column_are_safe() {
    let mut g = grid(true, &[(Some(2), "two"), (Some(1), "one")]);
    press(&mut g, KeyCode::Char('s'));
    let before = rebuilds();
    g.apply_commit_result(Ok(()));
    assert_eq!(rebuilds(), before, "an empty success does not reorder");

    g.sort = Some((9, SortDir::Asc));
    g.append_rows(page(&[(Some(0), "zero")], false));
    assert_eq!(
        shown(&g),
        ["two", "one", "zero"],
        "stale column keeps source order"
    );
}
