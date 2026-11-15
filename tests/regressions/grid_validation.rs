use std::cell::RefCell;
use std::rc::Rc;

use crate::common::*;
use junie_tui::core::event::Outcome;
use junie_tui::core::focus::BlurAction;
use junie_tui::core::id::WidgetId;
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::widgets::grid::{
    CellEditResult, CellKind, CellValue, ColumnSpec, DataGrid, FinishEdit, GridAction,
    GridEditError, GridRows, RowTotal, default_validator,
};
use junie_tui::widgets::option_editor::EditorConfig;
use junie_tui::widgets::option_editor::{ColumnEditor, OptionItem, Options};
use junie_tui::widgets::table::{Cell, Column, DataTable};
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Constraint, Position, Rect};
use ratatui::style::Color;

type Log = Rc<RefCell<Vec<String>>>;

fn log() -> Log {
    Rc::new(RefCell::new(Vec::new()))
}

fn calls(log: &Log) -> Vec<String> {
    log.borrow().clone()
}

fn grid_with(
    columns: Vec<ColumnSpec>,
    rows: Vec<Vec<CellValue>>,
    log: &Log,
    reject: &'static [&'static str],
) -> DataGrid {
    let seen = log.clone();
    let mut grid = DataGrid::new(WidgetId::of("t.grid"), columns).validator(move |spec, text| {
        seen.borrow_mut().push(text.to_owned());
        if reject.contains(&text) {
            return Err(format!("rejected {text:?}"));
        }
        default_validator(spec, text)
    });
    grid.try_set_rows(GridRows {
        rows,
        total: RowTotal::Unknown,
        more: false,
    })
    .expect("rectangular fixture");
    grid
}

fn press(grid: &mut DataGrid, code: KeyCode) -> (Outcome, Option<GridAction>) {
    grid.on_key(&key(code))
        .try_into_single()
        .expect("at most one action")
}

fn untouched(grid: &mut DataGrid) {
    assert!(grid.pending().is_empty(), "pending {:?}", grid.pending());
    assert!(grid.undo().is_none(), "an undo record was pushed");
}

fn draw_grid(grid: &mut DataGrid) -> ratatui::buffer::Buffer {
    let (buf, _) = draw(None, |buf, ctx| {
        grid.render(Rect::new(0, 0, 60, 8), buf, ctx, Color::Reset);
    });
    buf
}

fn marks(buf: &ratatui::buffer::Buffer, y: u16) -> bool {
    row(buf, y).contains('!')
}

fn opts(values: &[&str]) -> Options {
    Options::new(values.iter().map(|v| OptionItem::new(v)))
}

#[test]
fn issue43_parser_sees_empty_every_editor() {
    // Text editor: clearing the field and committing reaches the parser once.
    let seen = log();
    let mut grid = grid_with(
        vec![ColumnSpec::new("name", CellKind::Text)],
        vec![vec![CellValue::Text("abc".into())]],
        &seen,
        &[""],
    );
    press(&mut grid, KeyCode::Enter);
    assert!(grid.edit.is_some());
    for _ in 0..3 {
        press(&mut grid, KeyCode::Backspace);
    }
    seen.borrow_mut().clear();
    let (_, action) = press(&mut grid, KeyCode::Enter);
    assert_eq!(calls(&seen), vec![String::new()], "text editor");
    assert_eq!(action, None);
    assert!(grid.edit.is_some(), "the rejected input stays editable");
    assert_eq!(grid.edit_error(), Some("rejected \"\""));
    untouched(&mut grid);

    // Select editor: an option whose value is empty.
    let seen = log();
    let mut grid = grid_with(
        vec![
            ColumnSpec::new("kind", CellKind::Text)
                .editor(ColumnEditor::Select(opts(&["", "a", "b"]))),
        ],
        vec![vec![CellValue::Text("a".into())]],
        &seen,
        &[""],
    );
    press(&mut grid, KeyCode::Enter);
    assert!(grid.choice.is_some());
    press(&mut grid, KeyCode::Up);
    let (_, action) = press(&mut grid, KeyCode::Enter);
    assert_eq!(calls(&seen), vec![String::new()], "select editor");
    assert_eq!(action, None);
    assert!(grid.choice.is_some(), "the popup stays open");
    assert_eq!(grid.edit_error(), Some("rejected \"\""));
    untouched(&mut grid);

    // Tags editor: clearing every tag.
    let seen = log();
    let mut grid = grid_with(
        vec![ColumnSpec::new("tags", CellKind::Text).editor(ColumnEditor::Tags(opts(&["a", "b"])))],
        vec![vec![CellValue::Text("a".into())]],
        &seen,
        &[""],
    );
    press(&mut grid, KeyCode::Enter);
    press(&mut grid, KeyCode::Char(' '));
    let (_, action) = press(&mut grid, KeyCode::Enter);
    assert_eq!(calls(&seen), vec![String::new()], "tags editor");
    assert_eq!(action, None);
    assert!(grid.choice.is_some());
    assert_eq!(grid.edit_error(), Some("rejected \"\""));
    untouched(&mut grid);

    // A non-nullable Number treats an empty Select like empty Number text.
    let seen = log();
    let mut grid = grid_with(
        vec![
            ColumnSpec::new("qty", CellKind::Number)
                .nullable(false)
                .editor(ColumnEditor::Select(opts(&["", "1", "2"]))),
        ],
        vec![vec![CellValue::Int(1)]],
        &seen,
        &[],
    );
    press(&mut grid, KeyCode::Enter);
    press(&mut grid, KeyCode::Up);
    let (_, action) = press(&mut grid, KeyCode::Enter);
    assert_eq!(calls(&seen), vec![String::new()], "number select");
    assert_eq!(action, None);
    assert!(grid.edit_error().is_some());
    untouched(&mut grid);
    assert_eq!(grid.value(0, 0), &CellValue::Int(1));
}

#[test]
fn issue43_boolean_and_clear_use_parser() {
    // Native Bool: true -> false goes through the parser as text.
    let seen = log();
    let mut grid = grid_with(
        vec![ColumnSpec::new("flag", CellKind::Bool)],
        vec![vec![CellValue::Bool(true)]],
        &seen,
        &["false"],
    );
    let (_, action) = press(&mut grid, KeyCode::Enter);
    assert_eq!(calls(&seen), vec!["false".to_owned()], "native bool");
    assert_eq!(action, None);
    assert_eq!(grid.value(0, 0), &CellValue::Bool(true));
    assert!(grid.cell_errors.contains_key(&(0, 0)));
    untouched(&mut grid);

    // Native Bool: false -> NULL when nullable goes through the parser too.
    let seen = log();
    let mut grid = grid_with(
        vec![ColumnSpec::new("flag", CellKind::Bool)],
        vec![vec![CellValue::Bool(false)]],
        &seen,
        &["null"],
    );
    let (_, action) = press(&mut grid, KeyCode::Enter);
    assert_eq!(calls(&seen), vec!["null".to_owned()], "native bool null");
    assert_eq!(action, None);
    assert_eq!(grid.value(0, 0), &CellValue::Bool(false));
    untouched(&mut grid);

    // Checkbox editor: the immediate toggle.
    let seen = log();
    let mut grid = grid_with(
        vec![
            ColumnSpec::new("on", CellKind::Text)
                .editor(ColumnEditor::Checkbox(opts(&["no", "yes"]))),
        ],
        vec![vec![CellValue::Text("yes".into())]],
        &seen,
        &["no"],
    );
    let (_, action) = press(&mut grid, KeyCode::Enter);
    assert_eq!(calls(&seen), vec!["no".to_owned()], "checkbox");
    assert_eq!(action, None);
    assert_eq!(grid.value(0, 0), &CellValue::Text("yes".into()));
    untouched(&mut grid);

    // Radio and Segmented popups.
    for (editor, step) in [
        (ColumnEditor::Radio(opts(&["a", "b"])), KeyCode::Down),
        (
            ColumnEditor::Segmented(opts(&["a", "b", "c"])),
            KeyCode::Right,
        ),
    ] {
        let seen = log();
        let mut grid = grid_with(
            vec![ColumnSpec::new("pick", CellKind::Text).editor(editor.clone())],
            vec![vec![CellValue::Text("a".into())]],
            &seen,
            &["b"],
        );
        press(&mut grid, KeyCode::Enter);
        press(&mut grid, step);
        let (_, action) = press(&mut grid, KeyCode::Enter);
        assert_eq!(calls(&seen), vec!["b".to_owned()], "{editor:?}");
        assert_eq!(action, None, "{editor:?}");
        assert!(grid.choice.is_some(), "{editor:?}");
        assert!(grid.edit_error().is_some(), "{editor:?}");
        untouched(&mut grid);
    }

    // Clear cell (Delete) on a nullable cell goes through the parser.
    let seen = log();
    let mut grid = grid_with(
        vec![ColumnSpec::new("qty", CellKind::Number)],
        vec![vec![CellValue::Int(4)]],
        &seen,
        &["null"],
    );
    let (_, action) = press(&mut grid, KeyCode::Delete);
    assert_eq!(calls(&seen), vec!["null".to_owned()], "clear cell");
    assert_eq!(action, None);
    assert_eq!(grid.value(0, 0), &CellValue::Int(4));
    assert!(grid.cell_errors.contains_key(&(0, 0)));
    untouched(&mut grid);
}

#[test]
fn issue43_null_intent_is_not_custom_text() {
    // The default parser: reserved token and explicit clear both give NULL.
    let seen = log();
    let mut grid = grid_with(
        vec![ColumnSpec::new("qty", CellKind::Number)],
        vec![vec![CellValue::Int(4)], vec![CellValue::Int(5)]],
        &seen,
        &[],
    );
    let (_, action) = press(&mut grid, KeyCode::Delete);
    assert_eq!(action, Some(GridAction::CellChanged { row: 0, col: 0 }));
    assert_eq!(grid.value(0, 0), &CellValue::Null);
    press(&mut grid, KeyCode::Down);
    press(&mut grid, KeyCode::Enter);
    press(&mut grid, KeyCode::Backspace);
    for ch in "NULL".chars() {
        press(&mut grid, KeyCode::Char(ch));
    }
    let (_, action) = press(&mut grid, KeyCode::Enter);
    assert_eq!(action, Some(GridAction::CellChanged { row: 1, col: 0 }));
    assert_eq!(grid.value(1, 0), &CellValue::Null);

    // A custom parser that turns the clear token into text is rejected.
    let mut grid = DataGrid::new(
        WidgetId::of("t.custom"),
        vec![ColumnSpec::new("note", CellKind::Text)],
    )
    .validator(|_, text| Ok(CellValue::Text(text.to_owned())));
    grid.try_set_rows(GridRows {
        rows: vec![vec![CellValue::Text("x".into())]],
        total: RowTotal::Unknown,
        more: false,
    })
    .expect("rectangular fixture");
    let (_, action) = press(&mut grid, KeyCode::Delete);
    assert_eq!(action, None);
    assert_eq!(grid.value(0, 0), &CellValue::Text("x".into()));
    assert!(grid.cell_errors.contains_key(&(0, 0)));
    untouched(&mut grid);

    // A non-nullable column cannot be cleared.
    let mut grid = grid_with(
        vec![ColumnSpec::new("qty", CellKind::Number).nullable(false)],
        vec![vec![CellValue::Int(4)]],
        &log(),
        &[],
    );
    let (_, action) = press(&mut grid, KeyCode::Delete);
    assert_eq!(action, None);
    assert_eq!(grid.value(0, 0), &CellValue::Int(4));
    assert!(grid.cell_errors.contains_key(&(0, 0)));
    untouched(&mut grid);
}

#[test]
fn issue43_immediate_rejection_is_visible() {
    let seen = log();
    let mut grid = grid_with(
        vec![
            ColumnSpec::new("on", CellKind::Text)
                .editor(ColumnEditor::Checkbox(opts(&["no", "yes"]))),
            ColumnSpec::new("other", CellKind::Text),
        ],
        vec![
            vec![CellValue::Text("yes".into()), CellValue::Text("o".into())],
            vec![CellValue::Text("no".into()), CellValue::Text("p".into())],
        ],
        &seen,
        &["no"],
    );
    // Row 1 has a stale error that must survive the recovery of row 0.
    grid.cell_errors.insert((1, 1), "stale".into());
    draw_grid(&mut grid);

    // A real double click on the cell toggles through the parser.
    let id = grid.cell_id(0, 0);
    let (out, action) = grid
        .on_click_with(id, Position::new(8, 2), KeyModifiers::NONE, 2)
        .try_into_single()
        .expect("at most one action");
    assert_eq!(out, Outcome::Changed);
    assert_eq!(action, None);
    assert_eq!(calls(&seen), vec!["no".to_owned()]);
    assert!(grid.cell_errors.contains_key(&(0, 0)));
    assert_eq!(grid.value(0, 0), &CellValue::Text("yes".into()));
    untouched(&mut grid);

    // Move the cursor off the cell so the error mark is painted.
    press(&mut grid, KeyCode::Down);
    let buf = draw_grid(&mut grid);
    assert!((0..8).any(|y| marks(&buf, y)), "no error mark rendered");

    // Accepting a valid transition clears only the target's error.
    let mut grid = grid_with(
        vec![
            ColumnSpec::new("on", CellKind::Text)
                .editor(ColumnEditor::Checkbox(opts(&["no", "yes"]))),
        ],
        vec![
            vec![CellValue::Text("no".into())],
            vec![CellValue::Text("no".into())],
        ],
        &log(),
        &[],
    );
    grid.cell_errors.insert((0, 0), "old".into());
    grid.cell_errors.insert((1, 0), "other".into());
    let (_, action) = press(&mut grid, KeyCode::Enter);
    assert_eq!(action, Some(GridAction::CellChanged { row: 0, col: 0 }));
    assert!(!grid.cell_errors.contains_key(&(0, 0)));
    assert_eq!(
        grid.cell_errors.get(&(1, 0)).map(String::as_str),
        Some("other")
    );
}

#[test]
fn issue43_table_immediate_rejection_is_visible() {
    let mut table = DataTable::try_new(
        WidgetId::of("t.table"),
        vec![
            Column::new("on", Constraint::Length(10))
                .editor(ColumnEditor::Checkbox(opts(&["no", "yes"]))),
        ],
        vec![vec![Cell::new("yes")], vec![Cell::new("yes")]],
    )
    .expect("rectangular fixture")
    .cell_nav(true)
    .validator(|_, text| (text == "no").then(|| "no is not allowed".to_owned()));

    let (out, action) = table
        .on_key(&key(KeyCode::Enter))
        .try_into_single()
        .expect("at most one action");
    assert_eq!(out, Outcome::Changed);
    assert_eq!(action, None);
    assert_eq!(table.rows()[0][0].text, "yes");
    assert_eq!(
        table.rows()[0][0].error.as_deref(),
        Some("no is not allowed")
    );
    assert_eq!(table.rows()[1][0].error, None);

    let _ = table.on_key(&key(KeyCode::Down));
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let (buf, _) = draw(None, |buf, ctx| {
        table.render(Rect::new(0, 0, 60, 8), buf, ctx, Color::Reset);
    });
    let flagged = (0..8u16)
        .any(|y| (0..60u16).any(|x| buf[(x, y)].symbol() != " " && buf[(x, y)].fg == theme.error));
    assert!(flagged, "the rejected cell is drawn in the error colour");
}

fn type_text(grid: &mut DataGrid, text: &str) {
    for ch in text.chars() {
        press(grid, KeyCode::Char(ch));
    }
}

fn replace_text(grid: &mut DataGrid, old_len: usize, text: &str) {
    for _ in 0..old_len {
        press(grid, KeyCode::Backspace);
    }
    type_text(grid, text);
}

#[test]
fn issue43_finish_session_resolves_text_choice_once() {
    // No session: succeeds without consulting the parser.
    let seen = log();
    let mut grid = grid_with(
        vec![ColumnSpec::new("name", CellKind::Text)],
        vec![vec![CellValue::Text("abc".into())]],
        &seen,
        &["bad"],
    );
    assert_eq!(grid.finish_edit_session(), Ok(FinishEdit::NoSession));
    assert!(calls(&seen).is_empty());

    // Text session, rejected: the live input and a visible error remain.
    press(&mut grid, KeyCode::Enter);
    replace_text(&mut grid, 3, "bad");
    seen.borrow_mut().clear();
    let err = grid.finish_edit_session();
    assert!(matches!(err, Err(GridEditError::Validation(_))), "{err:?}");
    assert_eq!(calls(&seen), vec!["bad".to_owned()]);
    assert!(grid.edit.is_some());
    assert_eq!(grid.edit.as_ref().unwrap().field.text(), "bad");
    assert_eq!(grid.edit_error(), Some("rejected \"bad\""));
    untouched(&mut grid);

    // Text session, accepted and changed.
    seen.borrow_mut().clear();
    replace_text(&mut grid, 3, "xyz");
    seen.borrow_mut().clear();
    assert_eq!(
        grid.finish_edit_session(),
        Ok(FinishEdit::Accepted {
            row: 0,
            col: 0,
            result: CellEditResult::Changed
        })
    );
    assert_eq!(calls(&seen), vec!["xyz".to_owned()]);
    assert!(grid.edit.is_none());
    assert_eq!(grid.value(0, 0), &CellValue::Text("xyz".into()));

    // Text session, accepted and unchanged: closes, records nothing.
    let seen = log();
    let mut grid = grid_with(
        vec![
            ColumnSpec::new("name", CellKind::Text),
            ColumnSpec::new("b", CellKind::Text),
        ],
        vec![vec![CellValue::Text("abc".into()), CellValue::Null]],
        &seen,
        &[],
    );
    press(&mut grid, KeyCode::Enter);
    assert_eq!(
        grid.finish_edit_session(),
        Ok(FinishEdit::Accepted {
            row: 0,
            col: 0,
            result: CellEditResult::Unchanged
        })
    );
    assert_eq!(calls(&seen), vec!["abc".to_owned()]);
    assert!(grid.edit.is_none());
    assert_eq!(grid.cursor, (0, 0), "finishing never navigates");
    untouched(&mut grid);

    // Choice sessions commit their selection once, whatever the blur policy.
    let seen = log();
    let mut grid = grid_with(
        vec![
            ColumnSpec::new("kind", CellKind::Text)
                .editor(ColumnEditor::Select(opts(&["a", "b", "bad"]))),
        ],
        vec![vec![CellValue::Text("a".into())]],
        &seen,
        &["bad"],
    );
    press(&mut grid, KeyCode::Enter);
    press(&mut grid, KeyCode::Down);
    assert_eq!(
        grid.finish_edit_session(),
        Ok(FinishEdit::Accepted {
            row: 0,
            col: 0,
            result: CellEditResult::Changed
        })
    );
    assert_eq!(calls(&seen), vec!["b".to_owned()]);
    assert!(grid.choice.is_none());
    assert_eq!(grid.value(0, 0), &CellValue::Text("b".into()));

    // A rejected selection keeps the popup and shows the error.
    press(&mut grid, KeyCode::Enter);
    press(&mut grid, KeyCode::Down);
    seen.borrow_mut().clear();
    let err = grid.finish_edit_session();
    assert!(matches!(err, Err(GridEditError::Validation(_))), "{err:?}");
    assert_eq!(calls(&seen), vec!["bad".to_owned()]);
    assert!(grid.choice.is_some());
    assert_eq!(grid.edit_error(), Some("rejected \"bad\""));

    // Tags: the joined selection is parsed once.
    let seen = log();
    let mut grid = grid_with(
        vec![ColumnSpec::new("tags", CellKind::Text).editor(ColumnEditor::Tags(opts(&["a", "b"])))],
        vec![vec![CellValue::Text("a".into())]],
        &seen,
        &[],
    );
    press(&mut grid, KeyCode::Enter);
    press(&mut grid, KeyCode::Down);
    press(&mut grid, KeyCode::Char(' '));
    assert_eq!(
        grid.finish_edit_session(),
        Ok(FinishEdit::Accepted {
            row: 0,
            col: 0,
            result: CellEditResult::Changed
        })
    );
    assert_eq!(calls(&seen), vec!["a, b".to_owned()]);

    // No committable selection: NoChoice, popup stays, parser untouched.
    let seen = log();
    let mut grid = grid_with(
        vec![
            ColumnSpec::new("pick", CellKind::Text).editor(ColumnEditor::Combo(opts(&["a", "b"]))),
        ],
        vec![vec![CellValue::Text("a".into())]],
        &seen,
        &[],
    );
    press(&mut grid, KeyCode::Enter);
    type_text(&mut grid, "zz");
    assert_eq!(grid.finish_edit_session(), Err(GridEditError::NoChoice));
    assert!(calls(&seen).is_empty());
    assert!(grid.choice.is_some());
    assert!(grid.edit_error().is_some());
    untouched(&mut grid);
}

#[test]
fn issue43_one_commit_is_one_parser_call() {
    // A parser whose answer changes on the second call proves that a commit
    // never validates and then parses again.
    for editor in [None, Some(ColumnEditor::Select(opts(&["a", "b"])))] {
        let count = Rc::new(RefCell::new(0usize));
        let seen = count.clone();
        let mut spec = ColumnSpec::new("v", CellKind::Text);
        if let Some(e) = editor.clone() {
            spec = spec.editor(e);
        }
        let mut grid =
            DataGrid::new(WidgetId::of("t.flaky"), vec![spec]).validator(move |_, text| {
                *seen.borrow_mut() += 1;
                if *seen.borrow() == 1 {
                    Ok(CellValue::Text(text.to_owned()))
                } else {
                    Err("second call".to_owned())
                }
            });
        grid.try_set_rows(GridRows {
            rows: vec![vec![CellValue::Text("a".into())]],
            total: RowTotal::Unknown,
            more: false,
        })
        .expect("rectangular fixture");
        press(&mut grid, KeyCode::Enter);
        if editor.is_some() {
            press(&mut grid, KeyCode::Down);
        } else {
            type_text(&mut grid, "z");
        }
        *count.borrow_mut() = 0;
        let (_, action) = press(&mut grid, KeyCode::Enter);
        assert_eq!(*count.borrow(), 1, "{editor:?}");
        assert_eq!(
            action,
            Some(GridAction::CellChanged { row: 0, col: 0 }),
            "{editor:?}"
        );
        assert!(!grid.is_editing(), "{editor:?}");
    }
}

#[test]
fn issue43_blur_rejection_retains_choice_and_error() {
    let seen = log();
    let mut grid = grid_with(
        vec![
            ColumnSpec::new("kind", CellKind::Text).editor(ColumnEditor::Select(opts(&["a", "b"]))),
        ],
        vec![vec![CellValue::Text("a".into())]],
        &seen,
        &["b"],
    )
    .editor_config(EditorConfig {
        blur: BlurAction::Commit,
        ..EditorConfig::default()
    });
    press(&mut grid, KeyCode::Enter);
    press(&mut grid, KeyCode::Down);
    let (_, action) = grid
        .on_blur()
        .try_into_single()
        .expect("at most one action");
    assert_eq!(action, None);
    assert_eq!(calls(&seen), vec!["b".to_owned()]);
    assert!(grid.choice.is_some(), "focus movement is not acceptance");
    assert_eq!(grid.edit_error(), Some("rejected \"b\""));
    untouched(&mut grid);
}

#[test]
fn issue43_accepted_noop_can_tab() {
    let seen = log();
    let mut grid = grid_with(
        vec![
            ColumnSpec::new("a", CellKind::Text),
            ColumnSpec::new("b", CellKind::Text),
        ],
        vec![vec![
            CellValue::Text("one".into()),
            CellValue::Text("two".into()),
        ]],
        &seen,
        &[],
    );
    press(&mut grid, KeyCode::Enter);
    seen.borrow_mut().clear();
    let (_, action) = press(&mut grid, KeyCode::Tab);
    assert_eq!(calls(&seen), vec!["one".to_owned()]);
    assert_eq!(action, None, "an unchanged value emits no CellChanged");
    assert_eq!(grid.cursor, (0, 1), "Tab still navigates");
    assert!(grid.edit.as_ref().is_some_and(|e| e.col == 1));
    untouched(&mut grid);

    // A rejected value blocks Tab.
    let seen = log();
    let mut grid = grid_with(
        vec![
            ColumnSpec::new("a", CellKind::Text),
            ColumnSpec::new("b", CellKind::Text),
        ],
        vec![vec![
            CellValue::Text("one".into()),
            CellValue::Text("two".into()),
        ]],
        &seen,
        &["bad"],
    );
    press(&mut grid, KeyCode::Enter);
    replace_text(&mut grid, 3, "bad");
    let (_, action) = press(&mut grid, KeyCode::Tab);
    assert_eq!(action, None);
    assert_eq!(grid.cursor, (0, 0));
    assert!(grid.edit_error().is_some());
    untouched(&mut grid);
}

#[test]
fn issue43_try_edit_text_is_the_same_gateway() {
    let seen = log();
    let mut grid = grid_with(
        vec![
            ColumnSpec::new("qty", CellKind::Number).nullable(false),
            ColumnSpec::new("ro", CellKind::Text).read_only(),
        ],
        vec![vec![CellValue::Int(1), CellValue::Null]],
        &seen,
        &[],
    );
    assert_eq!(grid.try_edit_text(0, 0, "1"), Ok(CellEditResult::Unchanged));
    untouched(&mut grid);
    assert!(matches!(
        grid.try_edit_text(0, 0, ""),
        Err(GridEditError::Validation(_))
    ));
    assert!(matches!(
        grid.try_edit_text(0, 0, "x"),
        Err(GridEditError::Validation(_))
    ));
    assert_eq!(grid.try_edit_text(0, 0, "7"), Ok(CellEditResult::Changed));
    assert_eq!(grid.value(0, 0), &CellValue::Int(7));
    assert_eq!(
        grid.try_edit_text(0, 1, "x"),
        Err(GridEditError::InvalidTarget)
    );
    assert_eq!(
        grid.try_edit_text(5, 0, "1"),
        Err(GridEditError::InvalidTarget)
    );
    assert_eq!(
        grid.try_edit_text(0, 9, "1"),
        Err(GridEditError::InvalidTarget)
    );
    assert_eq!(
        calls(&seen),
        vec!["1", "", "x", "7"],
        "ineligible targets never reach the parser"
    );
}
