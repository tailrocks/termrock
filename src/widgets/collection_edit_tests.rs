use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::core::event::Key;
use crate::core::hit::HitRegistry;
use crate::core::id::WidgetId;
use crate::theme::{ColorLevel, Theme};
use crate::ui::ctx::{Interaction, RenderCtx};
use crate::widgets::grid::{
    CellKind, CellValue, ColumnSpec, DataGrid, GridAction, GridRows, RowTotal,
};
use crate::widgets::option_editor::{ColumnEditor, Options};
use crate::widgets::table::{Cell, Column, DataTable, TableAction};
use ratatui::layout::Constraint;

fn key(code: KeyCode) -> Key {
    Key {
        code,
        mods: KeyModifiers::NONE,
    }
}

fn editor() -> ColumnEditor {
    ColumnEditor::Select(Options::values(&["a", "b"]))
}

fn table(texts: &[&str]) -> DataTable {
    let rows = texts.iter().map(|t| vec![Cell::new(*t)]).collect();
    DataTable::try_new(
        WidgetId::of("t.edit"),
        vec![Column::new("Kind", Constraint::Length(10)).editor(editor())],
        rows,
    )
    .unwrap()
    .cell_nav(true)
}

fn texts(t: &DataTable) -> Vec<String> {
    t.rows().iter().map(|r| r[0].text.clone()).collect()
}

fn table_open_on(t: &mut DataTable, display: usize) {
    for _ in 0..display {
        let _ = t.on_key(&key(KeyCode::Down));
    }
    let _ = t.on_key(&key(KeyCode::Enter));
    assert!(t.choice.is_some(), "Enter opens the choice editor");
}

#[derive(Debug, Clone, Copy)]
enum Finish {
    Enter,
    Tab,
    Blur,
}

const FINISHES: [Finish; 3] = [Finish::Enter, Finish::Tab, Finish::Blur];

fn finish_table(t: &mut DataTable, how: Finish) -> Option<TableAction> {
    match how {
        Finish::Enter => t
            .on_key(&key(KeyCode::Enter))
            .into_actions()
            .into_iter()
            .next(),
        Finish::Tab => t
            .on_key(&key(KeyCode::Tab))
            .into_actions()
            .into_iter()
            .next(),
        Finish::Blur => t.on_blur().into_actions().into_iter().next(),
    }
}

#[test]
fn issue32_table_remove_cancels_choice() {
    for how in FINISHES {
        let mut t = table(&["a", "b", "c"]);
        table_open_on(&mut t, 1);
        t.remove_rows(&[0]);
        assert!(!t.is_editing(), "{how:?}: the session closes with the rows");
        let act = finish_table(&mut t, how);
        assert!(
            !matches!(act, Some(TableAction::Committed { .. })),
            "{how:?}: {act:?}"
        );
        assert_eq!(texts(&t), ["b", "c"], "{how:?}: no row is rewritten");

        let mut t = table(&["a", "b"]);
        table_open_on(&mut t, 1);
        t.remove_rows(&[0]);
        let act = finish_table(&mut t, how);
        assert!(
            !matches!(act, Some(TableAction::Committed { .. })),
            "{how:?}: {act:?}"
        );
        assert_eq!(texts(&t), ["b"], "{how:?}: the shifted index never panics");
    }
}

#[test]
fn issue32_table_set_row_cancels_sessions() {
    let mut t = table(&["a", "b", "c"]);
    table_open_on(&mut t, 1);
    t.try_set_row(2, vec![Cell::new("z")]).unwrap();
    assert!(!t.is_editing(), "a data mutation cancels conservatively");
    assert_eq!(finish_table(&mut t, Finish::Enter), None);
    assert_eq!(texts(&t), ["a", "b", "z"]);
}

#[test]
fn issue32_table_no_op_removal_keeps_editor() {
    let mut t = table(&["a", "b", "c"]);
    table_open_on(&mut t, 1);
    t.remove_rows(&[]);
    t.remove_rows(&[7, 9]);
    assert!(t.choice.is_some(), "nothing was removed");
    assert_eq!(
        t.on_key(&key(KeyCode::Enter))
            .into_actions()
            .into_iter()
            .next(),
        Some(TableAction::Committed { row: 1, col: 0 })
    );
}

#[test]
fn issue32_stale_generation_cannot_commit() {
    let mut t = table(&["a", "b", "c"]);
    table_open_on(&mut t, 1);
    let stale = t.choice.take();
    t.try_set_rows(vec![
        vec![Cell::new("x")],
        vec![Cell::new("y")],
        vec![Cell::new("z")],
    ])
    .unwrap();
    t.choice = stale;
    let act = t
        .on_key(&key(KeyCode::Enter))
        .into_actions()
        .into_iter()
        .next();
    assert!(
        !matches!(act, Some(TableAction::Committed { .. })),
        "{act:?}"
    );
    assert!(t.choice.is_none(), "a stale session is closed");
    assert_eq!(texts(&t), ["x", "y", "z"]);

    let mut t = table(&["a", "b", "c"]);
    table_open_on(&mut t, 1);
    let stale = t.choice.take();
    t.remove_rows(&[1]);
    t.choice = stale;
    let _ = t.on_key(&key(KeyCode::Enter));
    assert_eq!(texts(&t), ["a", "c"], "same index, different generation");
}

#[test]
fn issue32_stale_popup_click_cannot_commit() {
    let mut t = table(&["a", "b", "c"]);
    table_open_on(&mut t, 1);
    let option = t.choice.as_ref().unwrap().editor.option_id(0);
    let stale = t.choice.take();
    t.remove_rows(&[0]);
    t.choice = stale;
    let act = t
        .on_click_editor(option)
        .expect("the popup owns its option")
        .into_actions()
        .into_iter()
        .next();
    assert!(
        !matches!(act, Some(TableAction::Committed { .. })),
        "{act:?}"
    );
    assert_eq!(texts(&t), ["b", "c"]);
}

#[test]
fn issue32_redraw_removes_popup_regions() {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut t = table(&["a", "b", "c"]);
    table_open_on(&mut t, 1);
    let option = t.choice.as_ref().unwrap().editor.option_id(0);
    let paint = |t: &mut DataTable| {
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 12));
        let mut hits = HitRegistry::default();
        let mut ring = crate::core::focus::FocusRing::default();
        let interaction = Interaction {
            focus: Some(t.id),
            ..Default::default()
        };
        let mut ctx = RenderCtx::new(&theme, interaction, &mut hits, &mut ring);
        t.render(Rect::new(0, 0, 40, 12), &mut buf, &mut ctx, Color::Reset);
        ctx.flush_overlays(&mut buf);
        hits.part_of(option).is_some()
    };
    assert!(paint(&mut t), "the open popup registers its options");
    t.remove_rows(&[0]);
    assert!(!paint(&mut t), "the old popup left the hit map");
}

fn grid(texts: &[&str]) -> DataGrid {
    let mut g = DataGrid::new(
        WidgetId::of("g.edit"),
        vec![ColumnSpec::new("kind", CellKind::Text).editor(editor())],
    )
    .editable(true);
    g.try_set_rows(rows(texts)).unwrap();
    g
}

fn rows(texts: &[&str]) -> GridRows {
    GridRows {
        rows: texts
            .iter()
            .map(|t| vec![CellValue::Text((*t).into())])
            .collect(),
        total: RowTotal::Exact(texts.len()),
        more: false,
    }
}

fn commit_ok(g: &mut DataGrid) {
    let snapshot = g.snapshot_changes();
    let token = g.begin_submission(snapshot.clone()).unwrap();
    g.complete_submission(token, Ok(snapshot.identity_receipt()))
        .unwrap();
}

fn grid_open_on(g: &mut DataGrid, display: usize) {
    for _ in 0..display {
        let _ = g.on_key(&key(KeyCode::Down));
    }
    let _ = g.on_key(&key(KeyCode::Enter));
    assert!(g.choice.is_some(), "Enter opens the choice editor");
}

fn finish_grid(g: &mut DataGrid, how: Finish) -> Option<GridAction> {
    match how {
        Finish::Enter => g
            .on_key(&key(KeyCode::Enter))
            .into_actions()
            .into_iter()
            .next(),
        Finish::Tab => g
            .on_key(&key(KeyCode::Tab))
            .into_actions()
            .into_iter()
            .next(),
        Finish::Blur => g.on_blur().into_actions().into_iter().next(),
    }
}

#[test]
fn issue32_grid_replace_cancels_choice() {
    for how in FINISHES {
        for replacement in [&["x", "y", "z"][..], &[][..]] {
            let mut g = grid(&["a", "b", "c"]);
            grid_open_on(&mut g, 1);
            g.try_set_rows(rows(replacement)).unwrap();
            assert!(g.choice.is_none() && g.edit.is_none(), "{how:?}");
            let act = finish_grid(&mut g, how);
            assert!(
                !matches!(act, Some(GridAction::CellChanged { .. })),
                "{how:?}: {act:?}"
            );
            assert!(g.pending().is_empty(), "{how:?}: nothing staged");
        }
    }
}

#[test]
fn issue32_grid_cancel_edit_closes_choice_idempotently() {
    let mut g = grid(&["a", "b"]);
    grid_open_on(&mut g, 0);
    assert!(g.cancel_edit());
    assert!(g.choice.is_none());
    assert!(!g.cancel_edit());
}

#[test]
fn issue32_grid_structural_paths_cancel_choice() {
    let mut g = grid(&["a", "b"]);
    g.insert_row().unwrap();
    let _ = g.on_key(&key(KeyCode::Up));
    let _ = g.on_key(&key(KeyCode::Enter));
    assert!(g.choice.is_some());
    g.discard().unwrap();
    assert!(g.choice.is_none(), "discard closes the session");
    assert_eq!(g.rows().len(), 2);

    let mut g = grid(&["a", "b"]);
    let _ = g.on_key(&key(KeyCode::Enter));
    commit_ok(&mut g);
    assert!(g.choice.is_none(), "an accepted commit closes the session");

    let mut g = grid(&["a", "b"]);
    g.insert_row().unwrap();
    let _ = g.on_key(&key(KeyCode::Up));
    let _ = g.on_key(&key(KeyCode::Enter));
    assert!(g.undo().unwrap().is_some());
    assert!(g.choice.is_none(), "undoing an insert closes the session");
}

#[test]
fn issue32_append_keeps_existing_editor_but_replace_cancels() {
    let mut g = grid(&["a", "b"]);
    grid_open_on(&mut g, 1);
    g.try_append_rows(rows(&["c"])).unwrap();
    assert!(g.choice.is_some(), "append does not renumber old rows");
    assert_eq!(
        g.on_key(&key(KeyCode::Enter))
            .into_actions()
            .into_iter()
            .next(),
        Some(GridAction::CellChanged { row: 1, col: 0 })
    );

    let mut g = grid(&["a", "b"]);
    grid_open_on(&mut g, 1);
    let stale = g.choice.take();
    g.try_set_rows(rows(&["a", "b2"])).unwrap();
    g.choice = stale;
    let act = g
        .on_key(&key(KeyCode::Enter))
        .into_actions()
        .into_iter()
        .next();
    assert!(
        !matches!(act, Some(GridAction::CellChanged { .. })),
        "{act:?}"
    );
    assert!(g.choice.is_none());
    assert!(g.pending().is_empty());
}

fn text_table(texts: &[&str]) -> DataTable {
    let rows = texts.iter().map(|t| vec![Cell::new(*t)]).collect();
    DataTable::try_new(
        WidgetId::of("t.text"),
        vec![Column::new("Name", Constraint::Length(10)).editable()],
        rows,
    )
    .unwrap()
    .validator(|_col, text| text.contains('!').then(|| "no bang".to_owned()))
    .cell_nav(true)
}

fn text_table_open_on(t: &mut DataTable, display: usize) {
    for _ in 0..display {
        let _ = t.on_key(&key(KeyCode::Down));
    }
    let _ = t.on_key(&key(KeyCode::Enter));
    assert!(t.edit.is_some(), "Enter opens the text editor");
}

fn cells(texts: &[&str]) -> Vec<Vec<Cell>> {
    texts.iter().map(|t| vec![Cell::new(*t)]).collect()
}

#[test]
fn issue32_table_text_structural_change_closes_editor() {
    type Mutate = fn(&mut DataTable);
    let cases: [(&str, Mutate, &[&str]); 3] = [
        ("remove_rows", |t| t.remove_rows(&[0]), &["b", "c"]),
        (
            "set_row",
            |t| t.try_set_row(1, vec![Cell::new("z")]).unwrap(),
            &["a", "z", "c"],
        ),
        (
            "set_rows",
            |t| t.try_set_rows(cells(&["x", "y", "z"])).unwrap(),
            &["x", "y", "z"],
        ),
    ];
    for (name, mutate, expected) in cases {
        let mut t = text_table(&["a", "b", "c"]);
        text_table_open_on(&mut t, 1);
        mutate(&mut t);
        assert!(!t.is_editing(), "{name}: the text session closes");
        assert_eq!(t.commit_edit(), None, "{name}: nothing to commit");
        assert_eq!(texts(&t), expected, "{name}: no row is rewritten");
    }
}

#[test]
fn issue32_table_text_stale_session_cannot_commit() {
    let mut t = text_table(&["a", "b", "c"]);
    text_table_open_on(&mut t, 1);
    let stale = t.edit.take();
    t.try_set_rows(cells(&["x", "y", "z"])).unwrap();
    t.edit = stale;
    assert_eq!(t.commit_edit(), Some(TableAction::Cancelled));
    assert!(!t.is_editing(), "a stale session is closed");
    assert_eq!(texts(&t), ["x", "y", "z"]);
}

#[test]
fn issue32_table_text_invalid_value_keeps_session() {
    let mut t = text_table(&["a", "b"]);
    text_table_open_on(&mut t, 1);
    let _ = t.on_key(&key(KeyCode::Char('!')));
    let act = t
        .on_key(&key(KeyCode::Enter))
        .into_actions()
        .into_iter()
        .next();
    assert_eq!(act, None, "a rejected value commits nothing");
    assert!(t.is_editing(), "the live session stays open");
    assert_eq!(t.edit_error(), Some("no bang"));
    assert_eq!(texts(&t), ["a", "b"]);
}

fn grid_text(texts: &[&str]) -> DataGrid {
    let mut g = DataGrid::new(
        WidgetId::of("g.text"),
        vec![ColumnSpec::new("name", CellKind::Text)],
    )
    .editable(true)
    .validator(|_spec, text| {
        if text.contains('!') {
            Err("no bang".to_owned())
        } else {
            Ok(CellValue::Text(text.to_owned()))
        }
    });
    g.try_set_rows(rows(texts)).unwrap();
    g
}

fn grid_text_open_on(g: &mut DataGrid, display: usize) {
    for _ in 0..display {
        let _ = g.on_key(&key(KeyCode::Down));
    }
    let act = g.begin_edit().into_actions().into_iter().next();
    assert_eq!(act, None);
    assert!(g.edit.is_some(), "begin_edit opens the text editor");
}

#[test]
fn issue32_grid_text_structural_change_closes_editor() {
    let mut g = grid_text(&["a", "b", "c"]);
    grid_text_open_on(&mut g, 1);
    g.try_set_rows(rows(&["x", "y", "z"])).unwrap();
    assert!(!g.is_editing(), "set_rows closes the text session");
    assert_eq!(g.commit_edit(), None);
    assert!(g.pending().is_empty(), "nothing staged on the replacement");

    let mut g = grid_text(&["a", "b", "c"]);
    grid_text_open_on(&mut g, 1);
    g.discard().unwrap();
    assert!(!g.is_editing(), "discard closes the text session");
    assert_eq!(g.commit_edit(), None);
    assert!(g.pending().is_empty());

    let mut g = grid_text(&["a", "b"]);
    g.insert_row().unwrap();
    let inserted = g.len() - 1;
    grid_text_open_on(&mut g, inserted);
    g.toggle_delete(inserted).unwrap();
    assert!(
        !g.is_editing(),
        "removing the inserted row closes the session"
    );
    assert_eq!(g.commit_edit(), None);
    assert_eq!(g.len(), 2);
    assert!(g.pending().is_empty());
}

#[test]
fn issue32_grid_text_stale_session_cannot_commit() {
    let mut g = grid_text(&["a", "b", "c"]);
    grid_text_open_on(&mut g, 1);
    let stale = g.edit.take();
    g.try_set_rows(rows(&["x", "y", "z"])).unwrap();
    g.edit = stale;
    assert_eq!(g.commit_edit(), None, "the stale session commits nothing");
    assert!(!g.is_editing(), "a stale session is closed");
    assert!(
        g.pending().is_empty(),
        "no cell is staged on replacement data"
    );
}

#[test]
fn issue32_grid_text_invalid_value_keeps_session() {
    let mut g = grid_text(&["a", "b"]);
    grid_text_open_on(&mut g, 1);
    let _ = g.on_key(&key(KeyCode::Char('!')));
    let act = g
        .on_key(&key(KeyCode::Enter))
        .into_actions()
        .into_iter()
        .next();
    assert_eq!(act, None, "a rejected value commits nothing");
    assert!(g.is_editing(), "the live session stays open");
    assert_eq!(g.edit_error(), Some("no bang"));
    assert!(g.pending().is_empty());
}
