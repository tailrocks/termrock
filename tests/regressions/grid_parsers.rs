use crate::common::*;
use junie_tui::core::focus::BlurAction;
use junie_tui::core::id::WidgetId;
use junie_tui::widgets::edit::parsers;
use junie_tui::widgets::grid::{
    CellEditResult, CellKind, CellValue, ColumnSpec, DataGrid, GridAction, GridEditError, GridRows,
    RowTotal, default_validator,
};
use junie_tui::widgets::option_editor::{ColumnEditor, OptionItem, Options};
use ratatui::crossterm::event::KeyCode;

fn spec(kind: CellKind) -> ColumnSpec {
    ColumnSpec::new("c", kind).nullable(false)
}

fn parse(kind: CellKind, text: &str) -> Result<CellValue, String> {
    default_validator(&spec(kind), text)
}

fn accepts(kind: CellKind, text: &str, expected: CellValue) {
    assert_eq!(parse(kind, text), Ok(expected), "{kind:?} {text:?}");
}

fn rejects(kind: CellKind, text: &str) {
    let got = parse(kind, text);
    assert!(
        got.is_err(),
        "{kind:?} {text:?} must be rejected, got {got:?}"
    );
}

fn json(text: &str) -> CellValue {
    CellValue::Json(text.to_owned())
}

fn text(text: &str) -> CellValue {
    CellValue::Text(text.to_owned())
}

#[test]
fn issue47_json_requires_complete_container() {
    for ok in [
        r#"{"a":1}"#,
        r#"[1, 2, 3]"#,
        r#"{ "k" : [ {"n": null}, "é\n" ], "u": "żółć" }"#,
        "[]",
        "{}",
    ] {
        accepts(CellKind::Json, ok, json(ok));
    }
    accepts(
        CellKind::Json,
        "  {\"b\": 1,\n \"a\": 2}  ",
        json("{\"b\": 1,\n \"a\": 2}"),
    );
    for bad in [
        "{not json}",
        "[1, 2",
        "{\"a\": 1} trailing",
        "[1] [2]",
        "{\"a\": 1,}",
        r#"{"a": "bad \q escape"}"#,
        "{'a': 1}",
        "42",
        "\"text\"",
        "true",
        "{",
        "]",
        "",
    ] {
        rejects(CellKind::Json, bad);
    }
}

#[test]
fn issue47_uuid_exact_group_grammar() {
    for ok in [
        "123e4567-e89b-12d3-a456-426614174000",
        "123E4567-e89B-12D3-a456-426614174000",
        "00000000-0000-0000-0000-000000000000",
        "ffffffff-ffff-ffff-ffff-ffffffffffff",
    ] {
        accepts(CellKind::Id, ok, text(ok));
    }
    for bad in [
        "------------------------------------",
        "123e4567-e89b-12d3-a456-42661417400",
        "123e4567-e89b-12d3-a456-4266141740000",
        "123e4567e-89b-12d3-a456-426614174000",
        "123e4567-e89b-12d3-a456-42661417400g",
        "123e4567-e89b-12d3-a456_426614174000",
        "{123e4567-e89b-12d3-a456-42661417400}",
        "123e4567e89b12d3a456426614174000",
        "urn:uuid:123e4567-e89b-12d3-a456-426614174000",
        "123e4567-e89b-12d3-a456-42661417400é",
        "",
    ] {
        rejects(CellKind::Id, bad);
    }
}

#[test]
fn issue47_gregorian_and_timestamp_boundaries() {
    for ok in [
        "2025-06-01",
        "2025-06-01 12:30:00+00",
        "2025-06-01 12:30:00",
        "2025-06-01T12:30:00Z",
        "2025-06-01T12:30:00.1Z",
        "2025-06-01T12:30:00.123456789-05:30",
        "2025-06-01 23:59:59+23:59",
        "2024-02-29",
        "2000-02-29",
        "0001-01-01",
        "9999-12-31T00:00:00-00",
        "2025-04-30",
        "2025-12-31",
    ] {
        accepts(CellKind::Timestamp, ok, text(ok));
    }
    accepts(CellKind::Timestamp, " 2025-06-01 ", text("2025-06-01"));
    for bad in [
        "2026-99-99",
        "2026-13-01",
        "2026-00-10",
        "2026-01-00",
        "2026-01-32",
        "2026-04-31",
        "2026-02-29",
        "1900-02-29",
        "2100-02-29",
        "0000-01-01",
        "20260601",
        "2026-6-1",
        "2026/06/01",
        "abcd-ef-gh",
        "2026-06-01x",
        "2026-06-01T",
        "2026-06-01+01",
        "2026-06-01Z",
        "2026-06-01 24:00:00",
        "2026-06-01 12:60:00",
        "2026-06-01 12:00:60",
        "2026-06-01 12:00",
        "2026-06-01 12:00:0",
        "2026-06-01 12:00:00.",
        "2026-06-01 12:00:00.1234567890",
        "2026-06-01 12:00:00z",
        "2026-06-01 12:00:00+24",
        "2026-06-01 12:00:00+01:60",
        "2026-06-01 12:00:00+0100",
        "2026-06-01 12:00:00+1:00",
        "2026-06-01 12:00:00 UTC",
        "2026-06-01 12:00:00Zjunk",
        "2026-06-01\u{a0}12:00:00",
        "２０２６-06-01",
        "2026-06-01é",
        "",
    ] {
        rejects(CellKind::Timestamp, bad);
    }
}

#[test]
fn issue47_numeric_nonfinite_rejected() {
    for bad in [
        "NaN",
        "nan",
        "inf",
        "-inf",
        "+inf",
        "Infinity",
        "-infinity",
        "1e999",
        "-1e999",
        "1e309",
    ] {
        rejects(CellKind::Number, bad);
    }
    accepts(
        CellKind::Number,
        "9223372036854775807",
        CellValue::Int(i64::MAX),
    );
    accepts(
        CellKind::Number,
        "-9223372036854775808",
        CellValue::Int(i64::MIN),
    );
    accepts(
        CellKind::Number,
        "1.7976931348623157e308",
        CellValue::Num(f64::MAX),
    );
    accepts(
        CellKind::Number,
        "-1.7976931348623157e308",
        CellValue::Num(f64::MIN),
    );
    accepts(CellKind::Number, "5e-324", CellValue::Num(5e-324));
    accepts(CellKind::Number, "1e5", CellValue::Num(1e5));
    // Beyond i64 the int-first grammar falls through to a finite float.
    accepts(
        CellKind::Number,
        "9223372036854775808",
        CellValue::Num(9_223_372_036_854_775_808.0),
    );
}

fn grid(column: ColumnSpec, value: CellValue) -> DataGrid {
    let mut grid = DataGrid::new(WidgetId::of("t.grid47"), vec![column]);
    grid.set_rows(GridRows {
        rows: vec![vec![value]],
        total: RowTotal::Unknown,
        more: false,
    });
    grid
}

fn press(grid: &mut DataGrid, code: KeyCode) -> Option<GridAction> {
    grid.on_key(&key(code)).1
}

fn replace(grid: &mut DataGrid, with: &str) {
    for _ in 0..draft(grid).chars().count() {
        press(grid, KeyCode::Backspace);
    }
    for ch in with.chars() {
        press(grid, KeyCode::Char(ch));
    }
}

fn untouched(grid: &mut DataGrid, original: &CellValue) {
    assert!(grid.pending().is_empty(), "pending {:?}", grid.pending());
    assert!(grid.undo().is_none(), "an undo record was pushed");
    assert_eq!(grid.value(0, 0), original, "the effective value moved");
}

#[test]
fn issue47_parser_error_through_grid_commit() {
    let cases = [
        (CellKind::Timestamp, "2026-02-30", "2026-03-01 08:00:00+00"),
        (CellKind::Timestamp, "2026-99-99", "2026-03-01"),
        (
            CellKind::Id,
            "------------------------------------",
            "123e4567-e89b-12d3-a456-426614174000",
        ),
        (CellKind::Number, "1e999", "42"),
    ];
    for (kind, bad, good) in cases {
        for how in ["enter", "tab", "blur"] {
            let original = CellValue::Null;
            let mut g = grid(ColumnSpec::new("c", kind), original.clone());
            g.blur_action = BlurAction::Commit;
            press(&mut g, KeyCode::Enter);
            replace(&mut g, bad);
            let action = match how {
                "enter" => press(&mut g, KeyCode::Enter),
                "tab" => press(&mut g, KeyCode::Tab),
                _ => g.on_blur().1,
            };
            let ctx = format!("{kind:?} {bad:?} via {how}");
            assert_eq!(action, None, "{ctx}: no CellChanged on rejection");
            assert!(g.edit.is_some(), "{ctx}: the session stays open");
            assert_eq!(draft(&g), bad, "{ctx}: the draft text is retained");
            assert!(g.edit_error().is_some(), "{ctx}: the error is visible");
            untouched(&mut g, &original);

            replace(&mut g, good);
            let action = press(&mut g, KeyCode::Enter);
            assert!(
                matches!(action, Some(GridAction::CellChanged { .. })),
                "{ctx}: correction commits, got {action:?}"
            );
            assert!(g.edit.is_none(), "{ctx}: correction closes the editor");
            assert_eq!(g.edit_error(), None, "{ctx}: error cleared");
        }
    }
}

#[test]
fn issue47_parser_error_through_constrained_json_choice() {
    let options = Options::new(["{\"a\": 1}", "{bad}", "[1, 2"].map(OptionItem::new));
    let column =
        || ColumnSpec::new("doc", CellKind::Json).editor(ColumnEditor::Select(options.clone()));
    let original = json("{\"a\": 1}");
    let mut g = grid(column(), original.clone());
    press(&mut g, KeyCode::Enter);
    assert!(g.choice.is_some(), "the constrained editor opens a popup");
    press(&mut g, KeyCode::Down);
    let action = press(&mut g, KeyCode::Enter);
    assert_eq!(action, None, "an invalid option is not committed");
    assert!(g.choice.is_some(), "the popup stays open");
    assert!(g.edit_error().is_some(), "the rejection is visible");
    untouched(&mut g, &original);
    press(&mut g, KeyCode::Up);
    let action = press(&mut g, KeyCode::Enter);
    assert_eq!(action, None, "the current option is an accepted no-op");
    assert!(g.choice.is_none());
    assert_eq!(g.edit_error(), None);

    let mut g = grid(column(), original.clone());
    press(&mut g, KeyCode::Enter);
    press(&mut g, KeyCode::Down);
    press(&mut g, KeyCode::Down);
    press(&mut g, KeyCode::Enter);
    assert!(g.edit_error().is_some(), "the truncated option is rejected");
    untouched(&mut g, &original);
}

#[test]
fn issue47_try_edit_text_reports_grammar_errors() {
    let mut g = grid(
        ColumnSpec::new("when", CellKind::Timestamp),
        CellValue::Null,
    );
    for bad in ["2026-02-30", "2026-06-01 12:00", "nope"] {
        let got = g.try_edit_text(0, 0, bad);
        assert!(
            matches!(got, Err(GridEditError::Validation(_))),
            "{bad:?}: {got:?}"
        );
    }
    assert!(g.pending().is_empty());
    assert_eq!(
        g.try_edit_text(0, 0, "2026-06-01 12:00:00+00"),
        Ok(CellEditResult::Changed)
    );
    assert_eq!(g.value(0, 0), &text("2026-06-01 12:00:00+00"));
}

#[test]
fn issue47_deep_json_rejects_without_mutating_draft() {
    // Native Json editors request a viewer, so the gateway is the edit path.
    let original = json("{\"a\": 1}");
    let mut g = grid(ColumnSpec::new("doc", CellKind::Json), original.clone());
    let deep = format!("{}{}", "[".repeat(100_000), "]".repeat(100_000));
    let got = g.try_edit_text(0, 0, &deep);
    assert!(
        matches!(got, Err(GridEditError::Validation(_))),
        "a pathologically deep container is rejected, got {got:?}"
    );
    assert!(g.edit.is_none(), "the gateway opens no session");
    untouched(&mut g, &original);
    let ok = format!("{}{}", "[".repeat(100), "]".repeat(100));
    accepts(CellKind::Json, &ok, json(&ok));
    assert_eq!(g.try_edit_text(0, 0, &ok), Ok(CellEditResult::Changed));
    assert_eq!(g.value(0, 0), &json(&ok));
}

#[test]
fn issue48_default_nonfinite_rejected() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let original = CellValue::Num(bad);
        let mut g = grid(
            ColumnSpec::new("n", CellKind::Number).nullable(true),
            original.clone(),
        );
        press(&mut g, KeyCode::Enter);
        let seeded = draft(&g);
        let revision = g.revision();
        let action = press(&mut g, KeyCode::Enter);
        assert_eq!(action, None, "{bad}: seed {seeded:?} must not commit");
        assert!(g.edit.is_some(), "{bad}: the session stays open");
        assert!(g.edit_error().is_some(), "{bad}: the rejection is visible");
        assert!(g.pending().is_empty());
        assert!(g.undo().is_none());
        assert_eq!(
            g.revision(),
            revision,
            "{bad}: the rejected commit bumped the revision"
        );
        let kept = g.value(0, 0);
        assert!(
            matches!(kept, CellValue::Num(n) if n.to_bits() == bad.to_bits()),
            "{bad}: the imported value must stay as it was, got {kept:?}"
        );
    }
}

fn draft(grid: &DataGrid) -> String {
    grid.edit
        .as_ref()
        .map(|e| e.field.text().to_owned())
        .unwrap_or_default()
}

#[test]
fn issue47_json_and_uuid_through_gateway() {
    let mut g = grid(ColumnSpec::new("doc", CellKind::Json), CellValue::Null);
    for bad in ["{not json}", "42", "[1] x", ""] {
        let got = g.try_edit_text(0, 0, bad);
        assert!(
            matches!(got, Err(GridEditError::Validation(_))),
            "{bad:?}: {got:?}"
        );
    }
    assert!(g.pending().is_empty());
    let src = "{\"b\": 1,\n  \"a\": [true]}";
    assert_eq!(g.try_edit_text(0, 0, src), Ok(CellEditResult::Changed));
    assert_eq!(g.value(0, 0), &json(src), "spelling is not reserialised");

    let mut g = grid(ColumnSpec::new("id", CellKind::Id), CellValue::Null);
    let got = g.try_edit_text(0, 0, "------------------------------------");
    assert!(matches!(got, Err(GridEditError::Validation(_))), "{got:?}");
    let uuid = "123E4567-e89b-12D3-a456-426614174000";
    assert_eq!(g.try_edit_text(0, 0, uuid), Ok(CellEditResult::Changed));
    assert_eq!(g.value(0, 0), &text(uuid), "case is preserved");
}

#[test]
fn issue47_public_helpers_validate_the_exact_text() {
    assert!(parsers::json_container("[1]").is_ok());
    assert!(parsers::json_container("1").is_err());
    assert!(parsers::canonical_uuid("123e4567-e89b-12d3-a456-426614174000").is_ok());
    assert!(parsers::canonical_uuid(" 123e4567-e89b-12d3-a456-426614174000").is_err());
    assert!(parsers::date_or_timestamp("2025-06-01 12:30:00+00").is_ok());
    // The helpers never trim: trimming is the grid parser's job.
    assert!(parsers::date_or_timestamp("2026-06-01 ").is_err());
    assert!(parsers::date_or_timestamp(" 2026-06-01").is_err());
    let msg = parsers::date_or_timestamp("2026-02-30").unwrap_err();
    assert!(msg.message().contains("no day 30"), "{msg}");
    let msg = parsers::json_container("{not json}").unwrap_err();
    assert!(
        msg.message().starts_with("Must be a JSON object or array"),
        "{msg}"
    );
    let msg = parsers::canonical_uuid("nope").unwrap_err();
    assert!(msg.message().contains("UUID"), "{msg}");
}
