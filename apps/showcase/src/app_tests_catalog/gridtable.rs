//! Lane `gridtable` of the #14 burn-down: this lane's callable scenarios.
//!
//! Each scenario builds real components, sends real input, reads typed
//! outcomes and renders to a `Buffer`. Register one `ScenarioCase` per
//! `ScenarioId` variant from the `lane:gridtable` block in `../coverage.rs`. Edit only
//! this file and `../coverage/lane_gridtable.rs`.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Constraint, Position, Rect};
use termrock::core::event::{ClientPlatform, Key, Outcome};
use termrock::core::focus::BlurAction;
use termrock::core::focus::{Focus, FocusRing};
use termrock::core::hit::{Axis, HitRegistry};
use termrock::core::id::WidgetId;
use termrock::core::intent::{DismissReason, Phase};
use termrock::core::keymap::{KeyBinding, NavKeys};
use termrock::core::list::{TypeAheadPrecedence, route_key};
use termrock::core::pointer::{MouseTarget, PointerEvent};
use termrock::core::response::Response;
use termrock::core::scroll::{HScroll, TrackClick};
use termrock::core::selection::{ClickPolicy, SelectGesture, SelectionMode, TriState};
use termrock::theme::{ColorLevel, Glyphs, Strings, Theme, Tone};
use termrock::ui::columns::HeaderAction;
use termrock::ui::ctx::{Interaction, RenderCtx};
use termrock::ui::layer::{Layer, LayerCx, LayerOut};
use termrock::ui::reveal::Reveal;
use termrock::widgets::diff::{
    DiffFile, DiffHunk, DiffLine, DiffLineKind, DiffMode, DiffStatus, DiffView, UnreadFile,
    review_lines, review_lines_with, unified_lines, unified_lines_with,
};
use termrock::widgets::empty::EmptyState;
use termrock::widgets::grid::{
    CellEditResult, CellKind, CellParser, CellValue, ColumnSpec, DEFAULT_WIDTH_SAMPLE_ROWS,
    DataGrid, EditState, GridAction, GridChoice, GridCmd, GridRows, GridUndo, GridUndoKind,
    PendingChanges, RowState, RowTotal, cell_text, cell_text_with, cell_validator,
    default_validator,
};
use termrock::widgets::inspect_changes::{
    ChangeSet, ChangeWords, InspectChanges, InspectMode, InspectOutcome,
};
use termrock::widgets::option_editor::{
    Affordance, ColumnEditor, EditorConfig, OptionItem, Options,
};
use termrock::widgets::scrollbar;
use termrock::widgets::table::{
    Align, Cell, ChoiceEdit, Column, DataTable, SortDir, TableAction, TableCmd,
};
use termrock::widgets::viewport::line_text;

use super::{Painted, ScenarioCase, key, paint, paint_hits, text_of};
use crate::coverage::ScenarioId;

#[rustfmt::skip]
pub const CASES: &[ScenarioCase] = &[
    ScenarioCase { id: ScenarioId::DiffModelAndView, run: diff_model_and_view },
    ScenarioCase { id: ScenarioId::InspectChangesModes, run: inspect_changes_modes },
    ScenarioCase { id: ScenarioId::GridValuesAndDraft, run: grid_values_and_draft },
    ScenarioCase { id: ScenarioId::GridKeysAndCommands, run: grid_keys_and_commands },
    ScenarioCase { id: ScenarioId::GridPointerAndLabels, run: grid_pointer_and_labels },
    ScenarioCase { id: ScenarioId::GridEditSessions, run: grid_edit_sessions },
    ScenarioCase { id: ScenarioId::TableModelAndSelection, run: table_model_and_selection },
    ScenarioCase { id: ScenarioId::TableKeysAndEdits, run: table_keys_and_edits },
    ScenarioCase { id: ScenarioId::TablePointerAndChrome, run: table_pointer_and_chrome },
];

fn sample_file() -> DiffFile {
    DiffFile {
        path: "src/a.rs".into(),
        status: DiffStatus::Modified,
        hunks: vec![
            DiffHunk {
                old_start: 10,
                new_start: 10,
                lines: vec![
                    DiffLine::context("keep"),
                    DiffLine::remove("old name"),
                    DiffLine::add("new name"),
                    DiffLine::add("extra"),
                ],
            },
            DiffHunk {
                old_start: 40,
                new_start: 41,
                lines: vec![DiffLine::remove("gone")],
            },
        ],
    }
}

fn plain(lines: &[Vec<termrock::widgets::viewport::Span>]) -> Vec<String> {
    lines.iter().map(|l| line_text(l)).collect()
}

fn diff_model_and_view() {
    // Line kinds come from the three constructors.
    assert_eq!(DiffLine::context("c").kind, DiffLineKind::Context);
    assert_eq!(DiffLine::add("a").kind, DiffLineKind::Add);
    assert_eq!(DiffLine::remove("r").kind, DiffLineKind::Remove);

    // Hunk and file arithmetic, headers and summaries in both glyph sets.
    let file = sample_file();
    let (first, second) = (&file.hunks[0], &file.hunks[1]);
    assert_eq!((first.old_len(), first.new_len()), (2, 3));
    assert_eq!((second.old_len(), second.new_len()), (1, 0));
    assert_eq!(first.header(), "@@ -10,2 +10,3 @@");
    assert_eq!(second.header(), "@@ -40,1 +41,0 @@");
    assert_eq!((file.additions(), file.deletions()), (2, 2));
    assert_eq!(file.summary(), "+2 −2 · 2 hunks");
    assert_eq!(file.summary_with(&Glyphs::ASCII), "+2 -2 | 2 hunks");
    assert_eq!(file.header(), "M src/a.rs");
    let renamed = DiffFile {
        status: DiffStatus::Renamed {
            from: "src/old.rs".into(),
        },
        hunks: vec![file.hunks[0].clone()],
        ..file.clone()
    };
    assert_eq!(renamed.header(), "R src/old.rs → src/a.rs");
    assert_eq!(
        renamed.header_with(&Glyphs::ASCII),
        "R src/old.rs > src/a.rs"
    );
    assert_eq!(renamed.summary(), "+2 −1 · 1 hunk", "one hunk is singular");

    // Every status names itself, marks itself and takes its own tone.
    let statuses = [
        (DiffStatus::Added, "A", "added", Tone::Success),
        (DiffStatus::Modified, "M", "modified", Tone::Warning),
        (DiffStatus::Deleted, "D", "deleted", Tone::Error),
        (
            DiffStatus::Renamed { from: "x".into() },
            "R",
            "renamed",
            Tone::Secondary,
        ),
    ];
    for (status, marker, label, tone) in statuses {
        assert_eq!(
            (status.marker(), status.label(), status.tone()),
            (marker, label, tone)
        );
    }

    // The two modes toggle into each other and carry their labels.
    assert_eq!(DiffMode::Unified.label(), "unified");
    assert_eq!(DiffMode::Review.label(), "review");
    assert_eq!(DiffMode::Unified.toggled(), DiffMode::Review);
    assert_eq!(DiffMode::Review.toggled(), DiffMode::Unified);

    // The pure line builders: unified numbers each side, review pairs a
    // removal with its addition and reports the narrow fallback.
    let unified = plain(&unified_lines(&file));
    assert_eq!(unified[0], "M src/a.rs  +2 −2 · 2 hunks");
    assert_eq!(unified[1], "@@ -10,2 +10,3 @@");
    assert!(unified[2].contains("10") && unified[2].contains("  keep"));
    assert!(unified[3].contains("- old name"), "{}", unified[3]);
    assert!(unified[4].contains("+ new name"), "{}", unified[4]);
    let ascii = plain(&unified_lines_with(&file, &Glyphs::ASCII, &Strings::ASCII));
    assert_eq!(ascii[0], "M src/a.rs  +2 -2 | 2 hunks");
    let empty = DiffFile {
        hunks: vec![],
        ..file.clone()
    };
    assert_eq!(
        plain(&unified_lines(&empty)).last().map(String::as_str),
        Some("(no textual changes)")
    );

    let wide = plain(&review_lines(&file, 100));
    assert!(wide[0].ends_with("review"), "{}", wide[0]);
    assert!(wide[1].starts_with("Old") && wide[1].contains("New"));
    assert!(
        wide.iter()
            .any(|l| l.contains("old name") && l.contains("new name")),
        "a removal sits beside its addition: {wide:?}"
    );
    assert_eq!(
        plain(&review_lines(&file, 20)),
        unified,
        "too narrow for two columns, review falls back to unified"
    );
    let wide_ascii = plain(&review_lines_with(
        &file,
        100,
        &Glyphs::ASCII,
        &Strings::ASCII,
    ));
    assert!(
        wide_ascii[0].contains("+2 -2 | 2 hunks | "),
        "{}",
        wide_ascii[0]
    );

    // The view: identity, empty state, file swap, mode switch and layout.
    let id = WidgetId::of("scenario.diff.view");
    let mut view = DiffView::new(id);
    assert_eq!(view.id(), id);
    assert!(view.file().is_none());
    let area = Rect::new(0, 0, 100, 12);
    let bg = Theme::for_level(ColorLevel::TrueColor).surface;
    let buf = paint(area, |a, b, ctx| view.render(a, b, ctx, bg));
    assert!(text_of(&buf).contains("No file selected"));

    view.set_file(Some(file.clone()));
    assert_eq!(view.file(), Some(&file));
    assert_eq!(view.layout_mode(100), DiffMode::Unified);
    view.set_mode(DiffMode::Review);
    assert_eq!(view.mode, DiffMode::Review);
    assert_eq!(view.layout_mode(100), DiffMode::Review);
    assert_eq!(
        view.layout_mode(30),
        DiffMode::Unified,
        "a narrow pane keeps the unified layout while the mode stays review"
    );
    view.layout(100);
    let buf = paint(area, |a, b, ctx| view.render(a, b, ctx, bg));
    let text = text_of(&buf);
    assert!(text.contains("Old") && text.contains("New"), "{text}");
    assert_eq!(view.toggle_mode(), DiffMode::Unified);
    let buf = paint(area, |a, b, ctx| view.render(a, b, ctx, bg));
    assert!(!text_of(&buf).contains("Old"));
    assert!(text_of(&buf).contains("@@ -10,2 +10,3 @@"));

    // Keys and pointer scroll and select through the shared viewport.
    let long = DiffFile {
        hunks: vec![DiffHunk {
            old_start: 1,
            new_start: 1,
            lines: (0..40)
                .map(|i| DiffLine::add(format!("line {i}")))
                .collect(),
        }],
        ..file.clone()
    };
    view.set_file(Some(long));
    let small = Rect::new(0, 0, 60, 8);
    paint(small, |a, b, ctx| view.render(a, b, ctx, bg));
    assert_eq!(view.term.scroll.offset, 0);
    let (outcome, _) = view.on_key(&key(KeyCode::Down));
    assert_eq!(outcome, Outcome::Changed);
    assert_eq!(view.term.scroll.offset, 1, "Down scrolls one line");
    let press = view.on_click(Position::new(3, 2));
    let drag = view.on_drag(Position::new(8, 3));
    assert!(matches!(
        (press, drag),
        (
            Outcome::Changed | Outcome::Consumed,
            Outcome::Changed | Outcome::Consumed
        )
    ));
    assert!(
        view.term.selected_text().is_some_and(|t| !t.is_empty()),
        "a press and a drag select the text between them"
    );
    view.set_file(Some(sample_file()));
    assert_eq!(view.term.scroll.offset, 0, "a new file starts at the top");
}

struct InspectOut(InspectOutcome);

impl From<InspectOutcome> for InspectOut {
    fn from(o: InspectOutcome) -> Self {
        Self(o)
    }
}

fn inspect_screen() -> Rect {
    Rect::new(0, 0, 120, 36)
}

fn inspect_draw(ic: &mut InspectChanges) -> (Buffer, HitRegistry) {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let screen = inspect_screen();
    let mut buf = Buffer::empty(screen);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    Layer::<InspectOut, ()>::render(ic, screen, &mut buf, &mut ctx, &());
    (buf, hits)
}

fn inspect_key(ic: &mut InspectChanges, focus: &mut Focus, code: KeyCode) -> Outcome {
    let ring = FocusRing::default();
    let mut cx = LayerCx {
        focus,
        ring: &ring,
        app: &(),
    };
    Layer::<InspectOut, ()>::on_key(ic, &key(code), &mut cx)
}

fn inspect_result(ic: &mut InspectChanges) -> Option<InspectOutcome> {
    match Layer::<InspectOut, ()>::take_result(ic)? {
        LayerOut::Close(InspectOut(o)) | LayerOut::Event(InspectOut(o)) => Some(o),
    }
}

fn changed_file(path: &str, status: DiffStatus) -> DiffFile {
    DiffFile {
        path: path.into(),
        status,
        hunks: vec![DiffHunk {
            old_start: 1,
            new_start: 1,
            lines: vec![DiffLine::context("same"), DiffLine::add("fresh")],
        }],
    }
}

fn inspect_changes_modes() {
    let id = WidgetId::of("scenario.inspect");
    let set = ChangeSet {
        files: vec![
            DiffFile {
                hunks: vec![DiffHunk {
                    old_start: 1,
                    new_start: 1,
                    lines: (0..80).map(|i| DiffLine::add(format!("row {i}"))).collect(),
                }],
                ..changed_file("src/a.rs", DiffStatus::Modified)
            },
            changed_file("src/b.rs", DiffStatus::Added),
        ],
        unpushed: 2,
        omitted: 5,
        unread: vec![UnreadFile {
            path: "assets/logo.bin".into(),
            status: DiffStatus::Modified,
        }],
    };
    assert_eq!((set.additions(), set.deletions()), (81, 0));
    assert!(!set.is_empty());
    assert_eq!(set.summary(), "3 files · +81 −0");
    let blank = ChangeSet {
        files: vec![],
        unpushed: 0,
        omitted: 0,
        unread: vec![],
    };
    assert!(blank.is_empty());
    assert_eq!(blank.summary(), "0 files · +0 −0");

    assert_eq!(InspectMode::Compact.label(), "compact");
    assert_eq!(InspectMode::Advanced.label(), "advanced");

    // Compact: the list, the omitted words and the unread marker.
    let words = ChangeWords {
        omitted: "{count} hidden".into(),
        unread: "skipped".into(),
    };
    assert_eq!(ChangeWords::default().unread, "not read");
    let mut ic = InspectChanges::new(id, "Changes", set.clone(), InspectMode::Compact)
        .words(words)
        .returns_to_exit(true);
    assert_eq!(ic.initial_focus(), id, "compact focuses the list itself");
    let text = text_of(&inspect_draw(&mut ic).0);
    assert!(text.contains("5 hidden"), "custom omitted words: {text}");
    assert!(text.contains("skipped"), "custom unread word: {text}");
    assert!(text.contains("2 commits not pushed"));

    // Advanced: the tree takes initial focus and the diff pane is aimed by wheel.
    let mut adv = InspectChanges::new(id, "Changes", set, InspectMode::Advanced);
    assert_eq!(adv.initial_focus(), id.sub("tree"));
    let (buf, _) = inspect_draw(&mut adv);
    assert!(text_of(&buf).contains("advanced"));
    let area = adv.area;
    assert!(!area.is_empty(), "the frame reports where it drew");
    let over_diff = Position::new(area.right() - 4, area.y + area.height / 2);
    assert_eq!(adv.wheel_at(Axis::Vertical, 1, over_diff), Outcome::Changed);
    assert_eq!(
        adv.wheel_at(Axis::Horizontal, 1, Position::new(area.x + 2, area.y + 4)),
        Outcome::Consumed,
        "a horizontal wheel off the diff is swallowed"
    );

    // Back versus Close, once with returns_to_exit and once without.
    let leave = |returns: bool, act: fn(&mut InspectChanges)| {
        let mut ic = InspectChanges::new(
            id,
            "Changes",
            ChangeSet {
                files: vec![changed_file("a.rs", DiffStatus::Modified)],
                unpushed: 0,
                omitted: 0,
                unread: vec![],
            },
            InspectMode::Compact,
        )
        .returns_to_exit(returns);
        inspect_draw(&mut ic);
        act(&mut ic);
        inspect_result(&mut ic)
    };
    let esc: fn(&mut InspectChanges) = |ic| {
        inspect_key(ic, &mut Focus::default(), KeyCode::Esc);
    };
    let dismiss: fn(&mut InspectChanges) = |ic| {
        Layer::<InspectOut, ()>::on_dismiss(ic, DismissReason::OutsideClick);
    };
    assert_eq!(leave(true, esc), Some(InspectOutcome::Back));
    assert_eq!(leave(false, esc), Some(InspectOutcome::Close));
    assert_eq!(leave(true, dismiss), Some(InspectOutcome::Back));
    assert_eq!(leave(false, dismiss), Some(InspectOutcome::Close));

    let mut focus = Focus::default();
    // A key moves between the modes and the view switch flips the diff.
    let mut ic = InspectChanges::new(
        id,
        "Changes",
        ChangeSet {
            files: vec![changed_file("a.rs", DiffStatus::Modified)],
            unpushed: 0,
            omitted: 0,
            unread: vec![],
        },
        InspectMode::Compact,
    );
    inspect_draw(&mut ic);
    assert_eq!(ic.mode, InspectMode::Compact);
    inspect_key(&mut ic, &mut focus, KeyCode::Char('m'));
    assert_eq!(ic.mode, InspectMode::Advanced);
    let text = text_of(&inspect_draw(&mut ic).0);
    assert!(text.contains("unified · v switches"), "{text}");
    inspect_key(&mut ic, &mut focus, KeyCode::Char('v'));
    let text = text_of(&inspect_draw(&mut ic).0);
    assert!(text.contains("review · v switches"), "{text}");
}

fn ctrl(c: char) -> termrock::core::event::Key {
    termrock::core::event::Key {
        code: KeyCode::Char(c),
        mods: KeyModifiers::CONTROL,
    }
}

fn demo_columns() -> Vec<ColumnSpec> {
    let kinds = ["alpha", "beta"].map(OptionItem::new);
    vec![
        ColumnSpec::new("id", CellKind::Id)
            .primary()
            .type_label("uuid"),
        ColumnSpec::new("name", CellKind::Text).nullable(false),
        ColumnSpec::new("seats", CellKind::Number),
        ColumnSpec::new("kind", CellKind::Enum).enum_values(&["alpha", "beta"]),
        ColumnSpec::new("team", CellKind::Text).references("teams"),
        ColumnSpec::new("locked", CellKind::Text).read_only(),
        ColumnSpec::new("pick", CellKind::Text).editor(ColumnEditor::Select(Options::new(kinds))),
    ]
}

fn demo_rows(n: usize) -> GridRows {
    GridRows {
        rows: (0..n)
            .map(|i| {
                vec![
                    CellValue::Text(format!("00000000-0000-4000-8000-00000000000{i}")),
                    CellValue::Text(format!("name{i}")),
                    CellValue::Int(10 + i as i64),
                    CellValue::Text("alpha".into()),
                    CellValue::Text(format!("team{i}")),
                    CellValue::Text("fixed".into()),
                    CellValue::Text("alpha".into()),
                ]
            })
            .collect(),
        total: RowTotal::Exact(n),
        more: false,
    }
}

fn demo_grid(name: &'static str, n: usize) -> DataGrid {
    let mut grid = DataGrid::new(WidgetId::of(name), demo_columns());
    grid.try_set_rows(demo_rows(n)).expect("rectangular rows");
    grid
}

fn grid_values_and_draft() {
    // Every value kind shows, copies and seeds an editor with its own text.
    let samples = [
        (CellValue::Null, "NULL", "NULL"),
        (CellValue::Default, "DEFAULT", "DEFAULT"),
        (CellValue::Text("hi".into()), "hi", "hi"),
        (CellValue::Int(7), "7", "7"),
        (CellValue::Num(1.5), "1.50", "1.5"),
        (CellValue::Bool(true), "true", "true"),
        (
            CellValue::Json("{\"a\":1}".into()),
            "{\"a\":1}",
            "{\"a\":1}",
        ),
    ];
    for (value, shown, seeded) in samples {
        assert_eq!(value.text(), shown);
        assert_eq!(value.text_ref(), shown);
        assert_eq!(value.edit_text(), seeded);
    }
    assert_eq!(
        CellValue::Num(2.0).edit_text(),
        "2.0",
        "an integral number keeps its decimal point so it stays a number"
    );

    // The built-in grammar, one input per column kind, good and bad.
    let uuid = "123e4567-e89b-12d3-a456-426614174000";
    let kinds = [
        (CellKind::Text, "x", CellValue::Text("x".into()), None),
        (
            CellKind::Id,
            uuid,
            CellValue::Text(uuid.into()),
            Some("nope"),
        ),
        (CellKind::Number, "12", CellValue::Int(12), Some("abc")),
        (CellKind::Bool, "yes", CellValue::Bool(true), Some("maybe")),
        (
            CellKind::Timestamp,
            "2024-05-06",
            CellValue::Text("2024-05-06".into()),
            Some("yesterday"),
        ),
        (
            CellKind::Json,
            "{\"a\":1}",
            CellValue::Json("{\"a\":1}".into()),
            Some("nope"),
        ),
        (
            CellKind::Enum,
            "alpha",
            CellValue::Text("alpha".into()),
            Some("gamma"),
        ),
    ];
    for (kind, good, expected, bad) in kinds {
        let spec = ColumnSpec::new("c", kind).enum_values(&["alpha", "beta"]);
        assert_eq!(default_validator(&spec, good), Ok(expected), "{kind:?}");
        if let Some(bad) = bad {
            assert!(
                default_validator(&spec, bad).is_err(),
                "{kind:?} rejects {bad}"
            );
        }
    }
    let number = ColumnSpec::new("n", CellKind::Number);
    assert_eq!(default_validator(&number, "1.5"), Ok(CellValue::Num(1.5)));
    assert_eq!(default_validator(&number, ""), Ok(CellValue::Null));
    assert!(default_validator(&number.clone().nullable(false), "").is_err());
    assert_eq!(
        default_validator(&number, "DEFAULT"),
        Ok(CellValue::Default)
    );
    let parser: CellParser = cell_validator(|spec, text| {
        default_validator(spec, text).map(|v| match v {
            CellValue::Text(t) => CellValue::Text(t.to_uppercase()),
            other => other,
        })
    });
    assert_eq!(
        parser.validate_in(&ColumnSpec::new("t", CellKind::Text), "abc"),
        Ok(CellValue::Text("ABC".into()))
    );

    // Cell text: NULL, empty text, JSON squeezed to a narrow column, ids cut
    // in the middle, controls replaced.
    assert_eq!(cell_text(&CellValue::Null, CellKind::Text, 8), "NULL");
    assert_eq!(
        cell_text(&CellValue::Text(String::new()), CellKind::Text, 8),
        "''"
    );
    let json = CellValue::Json("{\"a\":1}".into());
    assert_eq!(cell_text(&json, CellKind::Json, 4), "{…}");
    assert_eq!(
        cell_text_with(&json, CellKind::Json, 4, &Glyphs::ASCII),
        "{~}"
    );
    let id = CellValue::Text(uuid.into());
    let shown = cell_text(&id, CellKind::Id, 12);
    assert!(
        shown.starts_with("123e") && shown.ends_with("000") && shown.contains('…'),
        "{shown}"
    );
    assert!(!cell_text(&CellValue::Text("a\nb".into()), CellKind::Text, 8).contains('\n'));
    assert_eq!(DEFAULT_WIDTH_SAMPLE_ROWS, 200);

    // Column spec builders land on the spec.
    let columns = demo_columns();
    assert!(columns[0].primary && columns[0].type_label == "uuid");
    assert!(!columns[1].nullable && columns[2].nullable);
    assert_eq!(columns[3].enum_values, ["alpha", "beta"]);
    assert_eq!(columns[4].references.as_deref(), Some("teams"));
    assert!(columns[5].read_only);
    assert!(matches!(columns[6].editor, ColumnEditor::Select(_)));
    assert!(
        !ColumnSpec::new("j", CellKind::Json).sortable,
        "JSON never sorts"
    );

    // A grid over those columns: shape accessors and the row sample width.
    let mut grid = demo_grid("scenario.grid.values", 3);
    assert_eq!((grid.len(), grid.is_empty()), (3, false));
    assert_eq!(grid.rows().len(), 3);
    assert_eq!(grid.columns().len(), 7);
    assert_eq!(grid.value(1, 1), &CellValue::Text("name1".into()));
    assert_eq!(grid.source_row(2), 2);
    assert_eq!(grid.width_sample_rows, DEFAULT_WIDTH_SAMPLE_ROWS);
    let wide_rows = || GridRows {
        rows: (0..4)
            .map(|i| {
                vec![CellValue::Text(if i == 0 {
                    "a".into()
                } else {
                    "x".repeat(30)
                })]
            })
            .collect(),
        total: RowTotal::Exact(4),
        more: false,
    };
    let measured = |sample: usize| {
        let mut wide = DataGrid::new(
            WidgetId::of("scenario.grid.sample"),
            vec![ColumnSpec::new("t", CellKind::Text)],
        )
        .width_sample_rows(sample);
        assert_eq!(wide.width_sample_rows, sample);
        wide.try_set_rows(wide_rows()).expect("rectangular rows");
        let theme = Theme::for_level(ColorLevel::TrueColor);
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        termrock::ui::measure::Measure::measure(&wide, ratatui::layout::Size::new(80, 20), &ctx)
            .width
    };
    assert!(
        measured(200) > measured(1),
        "sampling only the first row leaves the column narrow"
    );
    let empty = DataGrid::new(WidgetId::of("scenario.grid.none"), demo_columns());
    assert!(empty.is_empty());

    // Draft: edit, delete, insert, duplicate, then undo and discard.
    grid.cursor = (0, 1);
    assert_eq!(
        grid.record_cell(0, 2, CellValue::Int(99)),
        Ok(CellEditResult::Changed)
    );
    assert_eq!(grid.row_state(0), RowState::Modified);
    let first = grid.row_key(0).expect("row 0 has a key");
    assert!(grid.pending().is_dirty(first, 2));
    assert_eq!(grid.pending().value(first, 2), Some(&CellValue::Int(99)));
    assert_eq!(
        grid.value(0, 2),
        &CellValue::Int(99),
        "the draft shows through"
    );
    assert_eq!(grid.toggle_delete(1), Ok(Some(GridAction::RowDeleted(1))));
    assert_eq!(grid.row_state(1), RowState::Deleted);
    grid.row_errors.insert(2, "bad row".into());
    assert_eq!(grid.row_state(2), RowState::Error);
    assert_eq!(grid.row_state(99), RowState::Clean);
    assert_eq!(grid.insert_row(), Ok(Some(GridAction::RowInserted(3))));
    assert_eq!(grid.row_state(3), RowState::Inserted);
    grid.cursor = (0, 1);
    assert_eq!(grid.duplicate_row(), Ok(Some(GridAction::RowInserted(4))));
    assert_eq!(grid.value(4, 1), &CellValue::Text("name0".into()));
    assert_eq!(
        grid.value(4, 0),
        &CellValue::Default,
        "a copy asks for a fresh key"
    );
    let pending: &PendingChanges = grid.pending();
    assert_eq!(pending.counts(), (1, 2, 1));
    assert_eq!(pending.total(), 4);
    assert!(!pending.is_empty());
    assert_eq!(pending.dirty_rows().len(), 1);
    assert_eq!(
        grid.pending_label().as_deref(),
        Some("1 update · 2 inserts · 1 delete")
    );

    assert!(matches!(
        grid.undo(),
        Ok(Some(GridUndo {
            kind: GridUndoKind::Insert,
            ..
        }))
    ));
    assert_eq!(grid.len(), 4, "undoing a copy removes the row");
    assert_eq!(grid.discard(), Ok(true));
    assert!(grid.pending().is_empty());
    assert_eq!(grid.len(), 3, "discard drops the inserted rows");
    assert_eq!(grid.discard(), Ok(false), "nothing left to drop");
    assert_eq!(grid.pending_label(), None);

    // A custom parser and the editable switch decide what an edit may do.
    let mut upper = demo_grid("scenario.grid.upper", 1).validator(|spec, text| {
        default_validator(spec, text).map(|v| match v {
            CellValue::Text(t) => CellValue::Text(t.to_uppercase()),
            other => other,
        })
    });
    assert_eq!(
        upper.try_edit_text(0, 1, "shout"),
        Ok(CellEditResult::Changed)
    );
    assert_eq!(upper.value(0, 1), &CellValue::Text("SHOUT".into()));
    let mut frozen = demo_grid("scenario.grid.frozen", 1).editable(false);
    assert_eq!(
        frozen.toggle_delete(0),
        Ok(None),
        "a read-only grid refuses edits quietly"
    );
    assert_eq!(frozen.insert_row(), Ok(None));
}

fn grid_press(grid: &mut DataGrid, code: KeyCode) -> Option<GridAction> {
    grid_press_key(grid, key(code))
}

fn grid_press_key(grid: &mut DataGrid, key: Key) -> Option<GridAction> {
    grid.on_key(&key)
        .try_into_single()
        .expect("at most one action")
        .1
}

fn char_key(c: char) -> KeyCode {
    KeyCode::Char(c)
}

fn grid_keys_and_commands() {
    let mut grid = demo_grid("scenario.grid.keys", 3);
    let area = Rect::new(0, 0, 110, 14);
    let bg = Theme::for_level(ColorLevel::TrueColor).surface;
    paint(area, |a, b, ctx| grid.render(a, b, ctx, bg));

    // Arrows move the cursor cell by cell.
    assert_eq!(grid_press(&mut grid, KeyCode::Down), None);
    assert_eq!(grid_press(&mut grid, KeyCode::Right), None);
    assert_eq!(grid.cursor, (1, 1));

    // One key per command, each reporting the action a host acts on.
    assert_eq!(
        grid_press_key(&mut grid, ctrl('s')),
        Some(GridAction::CommitRequested)
    );
    assert_eq!(
        grid_press(&mut grid, char_key('U')),
        Some(GridAction::DiscardRequested)
    );
    assert_eq!(
        grid_press(&mut grid, char_key('p')),
        Some(GridAction::PreviewSql)
    );
    assert_eq!(
        grid_press(&mut grid, char_key('r')),
        Some(GridAction::Refresh)
    );
    assert_eq!(
        grid_press(&mut grid, KeyCode::F(5)),
        Some(GridAction::Refresh)
    );
    assert_eq!(
        grid_press(&mut grid, char_key('/')),
        Some(GridAction::OpenFilters)
    );
    assert_eq!(
        grid_press(&mut grid, char_key('F')),
        Some(GridAction::ClearFilters)
    );
    assert_eq!(
        grid_press(&mut grid, char_key('f')),
        Some(GridAction::FilterOnCell {
            col: 1,
            value: CellValue::Text("name1".into())
        })
    );
    assert_eq!(
        grid_press(&mut grid, char_key('y')),
        Some(GridAction::Copy("name1".into()))
    );
    let Some(GridAction::Copy(table)) = grid_press(&mut grid, char_key('Y')) else {
        panic!("Y copies the row with its header");
    };
    assert!(table.starts_with("id\tname\tseats"), "{table}");
    assert!(table.contains("name1"));

    // Sorting cycles ascending, descending, off; the grid only asks, the host sorts.
    assert_eq!(
        grid_press(&mut grid, char_key('s')),
        Some(GridAction::SortRequested(Some((1, SortDir::Asc))))
    );
    assert_eq!(
        grid_press(&mut grid, char_key('s')),
        Some(GridAction::SortRequested(Some((1, SortDir::Desc))))
    );
    assert_eq!(
        grid_press(&mut grid, char_key('s')),
        Some(GridAction::SortRequested(None))
    );
    assert_eq!(
        grid_press(&mut grid, char_key('S')),
        Some(GridAction::SortRequested(None))
    );

    // Row edits and their undo.
    grid.cursor = (1, 1);
    assert_eq!(
        grid_press(&mut grid, char_key('-')),
        Some(GridAction::RowDeleted(1))
    );
    assert_eq!(grid.row_state(1), RowState::Deleted);
    assert_eq!(
        grid_press(&mut grid, char_key('-')),
        Some(GridAction::RowDeleted(1))
    );
    assert_eq!(grid.row_state(1), RowState::Clean, "the mark toggles");
    assert_eq!(
        grid_press(&mut grid, char_key('+')),
        Some(GridAction::RowInserted(3))
    );
    assert_eq!(
        grid_press_key(&mut grid, ctrl('d')),
        Some(GridAction::RowInserted(4))
    );
    let mut undone = Vec::new();
    while let Some(GridAction::Undone(u)) = grid_press(&mut grid, char_key('u')) {
        undone.push(u.kind);
    }
    assert_eq!(
        undone,
        [
            GridUndoKind::Insert,
            GridUndoKind::Insert,
            GridUndoKind::DeleteMark,
            GridUndoKind::DeleteMark
        ]
    );

    // Clearing a nullable cell, following a reference, opening a viewer.
    grid.cursor = (0, 2);
    assert_eq!(
        grid_press(&mut grid, KeyCode::Delete),
        Some(GridAction::CellChanged { row: 0, col: 2 })
    );
    assert_eq!(grid.value(0, 2), &CellValue::Null);
    grid.cursor = (2, 4);
    assert_eq!(
        grid_press_key(&mut grid, ctrl(']')),
        Some(GridAction::FollowReference { row: 2, col: 4 })
    );
    grid.cursor = (2, 5);
    assert!(matches!(
        grid_press(&mut grid, KeyCode::Enter),
        Some(GridAction::ReadOnly { reason }) if reason.contains("locked")
    ));
    let mut json = DataGrid::new(
        WidgetId::of("scenario.grid.json"),
        vec![ColumnSpec::new("doc", CellKind::Json)],
    );
    json.try_set_rows(GridRows {
        rows: vec![vec![CellValue::Json("{\"a\": 1}".into())]],
        total: RowTotal::Exact(1),
        more: false,
    })
    .expect("rectangular rows");
    assert_eq!(
        grid_press(&mut json, KeyCode::Enter),
        Some(GridAction::OpenViewer { row: 0, col: 0 })
    );

    // A paged source offers the "more" row; a read-only grid activates rows.
    let mut paged = demo_grid("scenario.grid.paged", 2);
    paged
        .try_set_rows(GridRows {
            more: true,
            ..demo_rows(2)
        })
        .expect("rectangular rows");
    let more_area = Rect::new(0, 0, 110, 14);
    let more_bg = Theme::for_level(ColorLevel::TrueColor).surface;
    paged.set_loading(true);
    assert!(paged.loading);
    let fetching = paint(more_area, |a, b, ctx| paged.render(a, b, ctx, more_bg));
    assert!(
        text_of(&fetching).contains("fetching") && !text_of(&fetching).contains("fetches more"),
        "a loading grid says it is fetching"
    );
    paged.set_loading(false);
    let idle = paint(more_area, |a, b, ctx| paged.render(a, b, ctx, more_bg));
    assert!(text_of(&idle).contains("fetches more"));
    paged.cursor = (2, 0);
    assert_eq!(
        grid_press(&mut paged, KeyCode::Enter),
        Some(GridAction::FetchMore)
    );
    let mut browse = demo_grid("scenario.grid.browse", 2).editable(false);
    browse.cursor = (1, 1);
    assert_eq!(
        grid_press(&mut browse, KeyCode::Enter),
        Some(GridAction::Activated(1))
    );

    // Typing in a cell: Enter opens the editor, Enter commits, Esc cancels.
    grid.cursor = (0, 1);
    assert!(!grid.is_editing());
    assert_eq!(grid_press(&mut grid, KeyCode::Enter), None);
    assert!(grid.is_editing());
    for c in "!".chars() {
        grid_press(&mut grid, KeyCode::Char(c));
    }
    assert_eq!(
        grid_press(&mut grid, KeyCode::Enter),
        Some(GridAction::CellChanged { row: 0, col: 1 })
    );
    assert_eq!(grid.value(0, 1), &CellValue::Text("name0!".into()));
    grid_press(&mut grid, KeyCode::Enter);
    grid_press(&mut grid, KeyCode::Esc);
    assert!(!grid.is_editing());

    // Tab walks to the next writable cell and reports where it leaves the grid.
    let mut two = DataGrid::new(
        WidgetId::of("scenario.grid.tabs"),
        vec![
            ColumnSpec::new("a", CellKind::Text),
            ColumnSpec::new("b", CellKind::Text),
        ],
    );
    two.try_set_rows(GridRows {
        rows: vec![vec![
            CellValue::Text("1".into()),
            CellValue::Text("2".into()),
        ]],
        total: RowTotal::Exact(1),
        more: false,
    })
    .expect("rectangular rows");
    two.cursor = (0, 1);
    grid_press(&mut two, KeyCode::Enter);
    assert_eq!(
        grid_press(&mut two, KeyCode::Tab),
        Some(GridAction::LeaveForward)
    );
    two.cursor = (0, 0);
    grid_press(&mut two, KeyCode::Enter);
    assert_eq!(
        grid_press(&mut two, KeyCode::BackTab),
        Some(GridAction::LeaveBackward)
    );

    // Selecting rows from the keyboard.
    assert!(!grid.has_selection());
    grid_press(&mut grid, KeyCode::Char(' '));
    assert!(grid.has_selection());
    grid_press(&mut grid, KeyCode::Esc);
    assert!(!grid.has_selection());
    assert!(grid.select_rows(SelectGesture::All, 0));
    assert_eq!(grid.selection.len(), 3);
    assert!(grid.has_selection());

    // Every command through `run`, the entry point a menu uses.
    let mut cmds = demo_grid("scenario.grid.cmds", 3);
    cmds.cursor = (0, 2);
    let outcome = |cmds: &mut DataGrid, cmd: GridCmd| cmds.run(cmd);
    assert_eq!(
        outcome(&mut cmds, GridCmd::ClearCell),
        (
            Outcome::Changed,
            Some(GridAction::CellChanged { row: 0, col: 2 })
        )
    );
    assert_eq!(
        outcome(&mut cmds, GridCmd::ToggleDelete),
        (Outcome::Changed, Some(GridAction::RowDeleted(0)))
    );
    assert_eq!(
        outcome(&mut cmds, GridCmd::InsertRow),
        (Outcome::Changed, Some(GridAction::RowInserted(3)))
    );
    cmds.cursor = (1, 2);
    assert_eq!(
        outcome(&mut cmds, GridCmd::DuplicateRow),
        (Outcome::Changed, Some(GridAction::RowInserted(4)))
    );
    assert!(matches!(
        outcome(&mut cmds, GridCmd::Copy).1,
        Some(GridAction::Copy(_))
    ));
    assert!(matches!(
        outcome(&mut cmds, GridCmd::CopyWithHeader).1,
        Some(GridAction::Copy(t)) if t.starts_with("id\t")
    ));
    assert_eq!(
        outcome(&mut cmds, GridCmd::Commit).1,
        Some(GridAction::CommitRequested)
    );
    cmds.cursor = (1, 1);
    assert_eq!(
        outcome(&mut cmds, GridCmd::Sort).1,
        Some(GridAction::SortRequested(Some((1, SortDir::Asc))))
    );
    assert_eq!(
        outcome(&mut cmds, GridCmd::ClearSort).1,
        Some(GridAction::SortRequested(None))
    );
    assert!(matches!(
        outcome(&mut cmds, GridCmd::FilterOnCell).1,
        Some(GridAction::FilterOnCell { col: 1, .. })
    ));
    assert_eq!(
        outcome(&mut cmds, GridCmd::OpenFilters).1,
        Some(GridAction::OpenFilters)
    );
    assert_eq!(
        outcome(&mut cmds, GridCmd::ClearFilters).1,
        Some(GridAction::ClearFilters)
    );
    assert_eq!(
        outcome(&mut cmds, GridCmd::Refresh).1,
        Some(GridAction::Refresh)
    );
    assert!(matches!(
        outcome(&mut cmds, GridCmd::Undo).1,
        Some(GridAction::Undone(_))
    ));
    assert_eq!(
        outcome(&mut cmds, GridCmd::Discard).1,
        Some(GridAction::DiscardRequested)
    );
    assert_eq!(
        outcome(&mut cmds, GridCmd::PreviewSql).1,
        Some(GridAction::PreviewSql)
    );
    cmds.cursor = (1, 4);
    assert_eq!(
        outcome(&mut cmds, GridCmd::FollowReference).1,
        Some(GridAction::FollowReference { row: 1, col: 4 })
    );
    cmds.cursor = (1, 1);
    assert_eq!(
        outcome(&mut cmds, GridCmd::FollowReference),
        (Outcome::Consumed, None),
        "a column without a reference has nothing to follow"
    );
    assert_eq!(
        outcome(&mut cmds, GridCmd::SelectAll),
        (Outcome::Changed, None)
    );
    assert!(cmds.has_selection());
    let mut view_only = demo_grid("scenario.grid.view-only", 1).editable(false);
    assert_eq!(
        view_only.run(GridCmd::InsertRow),
        (Outcome::Consumed, None),
        "a read-only grid cannot insert"
    );

    // The context menu offers the commands that apply and disables the rest.
    let items = cmds.context_items();
    let find = |items: &[(GridCmd, termrock::widgets::menu::MenuItem)], cmd: GridCmd| {
        items
            .iter()
            .find(|(c, _)| *c == cmd)
            .map(|(_, i)| i.item.disabled)
    };
    assert_eq!(find(&items, GridCmd::InsertRow), Some(false));
    let locked = view_only.context_items();
    assert_eq!(find(&locked, GridCmd::InsertRow), Some(true));
    assert_eq!(find(&locked, GridCmd::Copy), Some(false));

    // Key policy is a builder: a remapped command, a custom duplicate key, vim
    // navigation and type-ahead with its precedence and column.
    let mut remapped = demo_grid("scenario.grid.remap", 3)
        .keymap(vec![(KeyBinding::char('x'), GridCmd::Copy)])
        .duplicate_keys(&[key(char_key('k'))])
        .nav(NavKeys::VIM);
    assert!(matches!(
        grid_press(&mut remapped, char_key('x')),
        Some(GridAction::Copy(_))
    ));
    assert_eq!(
        grid_press(&mut remapped, char_key('y')),
        None,
        "the default key is gone"
    );
    assert_eq!(
        grid_press(&mut remapped, char_key('k')),
        Some(GridAction::RowInserted(3))
    );
    remapped.cursor = (0, 0);
    grid_press(&mut remapped, char_key('j'));
    assert_eq!(remapped.cursor.0, 1, "vim j moves down");

    let mut typed = demo_grid("scenario.grid.typeahead", 3)
        .type_ahead(true)
        .type_ahead_column(Some(1))
        .type_ahead_precedence(TypeAheadPrecedence::TypeAhead)
        .type_ahead_timeout(std::time::Duration::from_millis(400));
    for c in "name2".chars() {
        grid_press(&mut typed, char_key(c));
    }
    assert_eq!(
        typed.cursor.0, 2,
        "typing a prefix jumps to the matching row"
    );
    assert_eq!(
        grid_press(&mut typed, char_key('s')),
        None,
        "with type-ahead first a letter is a search, not the sort command"
    );
    // Routed like the app does, the precedence decides whether a letter that
    // starts a search is the sort shortcut or the first letter of a prefix.
    let first_s = |precedence: TypeAheadPrecedence| {
        let mut fresh = demo_grid("scenario.grid.precedence", 3)
            .type_ahead(true)
            .type_ahead_column(Some(1))
            .type_ahead_precedence(precedence);
        let mut action = None;
        route_key(&key(char_key('s')), || {
            let (outcome, found) = one(fresh.on_key(&key(char_key('s'))));
            action = action.take().or(found);
            outcome
        });
        action
    };
    assert_eq!(first_s(TypeAheadPrecedence::TypeAhead), None);
    assert_eq!(
        first_s(TypeAheadPrecedence::Shortcuts),
        Some(GridAction::SortRequested(Some((0, SortDir::Asc))))
    );
    // A zero timeout forgets the prefix before every letter, so the word
    // never builds up (the pause only guarantees the clock moved on).
    let mut forgetful = demo_grid("scenario.grid.forgetful", 3)
        .type_ahead(true)
        .type_ahead_column(Some(1))
        .type_ahead_precedence(TypeAheadPrecedence::TypeAhead)
        .type_ahead_timeout(std::time::Duration::ZERO);
    for c in "name2".chars() {
        std::thread::sleep(std::time::Duration::from_millis(2));
        grid_press(&mut forgetful, char_key(c));
    }
    assert_eq!(
        forgetful.cursor.0, 0,
        "each letter starts a new search, so the prefix never matches"
    );
}

fn one(resp: Response<GridAction>) -> (Outcome, Option<GridAction>) {
    resp.try_into_single().expect("at most one action")
}

fn area_of(painted: &Painted, id: WidgetId) -> Rect {
    painted
        .hits
        .regions()
        .iter()
        .find(|r| r.id == id)
        .map(|r| r.area)
        .unwrap_or_else(|| panic!("the frame registers no region for {id:?}"))
}

fn grid_pointer_and_labels() {
    let area = Rect::new(0, 0, 110, 14);
    let bg = Theme::for_level(ColorLevel::TrueColor).surface;
    let mut grid = demo_grid("scenario.grid.pointer", 3)
        .resizable_columns(true)
        .seam_hit_w(3)
        .toggle_mods(KeyModifiers::ALT)
        .bar_labels(["See SQL", "Throw away", "Persist"]);
    let painted = paint_hits(area, |a, b, ctx| grid.render(a, b, ctx, bg));
    let text = text_of(&painted.buf);
    assert!(text.contains("name1") && text.contains("seats"), "{text}");
    assert_eq!(grid.rows_label(), "rows 1–3 of 3");
    assert_eq!(
        grid.cols_label(),
        None,
        "every column fits, so no column range"
    );
    assert_eq!(grid.position_label(), "rows 1–3 of 3");

    // Row totals phrase themselves by how much the source knows.
    let totals = [
        (RowTotal::Exact(3), false, "rows 1–3 of 3"),
        (RowTotal::Estimated(1000), false, "rows 1–3 of ~1,000"),
        (RowTotal::Unknown, false, "rows 1–3 of 3"),
        (
            RowTotal::Exact(120),
            true,
            "rows 1–3 of 3 loaded · 120 total",
        ),
        (
            RowTotal::Estimated(500),
            true,
            "rows 1–3 of 3 loaded · ~500 total",
        ),
    ];
    for (total, more, label) in totals {
        let mut paged = demo_grid("scenario.grid.totals", 3);
        paged
            .try_set_rows(GridRows {
                total,
                more,
                ..demo_rows(3)
            })
            .expect("rectangular rows");
        paint(area, |a, b, ctx| paged.render(a, b, ctx, bg));
        assert_eq!(paged.rows_label(), label, "{total:?}");
    }

    // The footer row of a paged grid is a button that asks for more.
    let mut paged = demo_grid("scenario.grid.more", 3);
    paged
        .try_set_rows(GridRows {
            more: true,
            ..demo_rows(3)
        })
        .expect("rectangular rows");
    paint(area, |a, b, ctx| paged.render(a, b, ctx, bg));
    assert_eq!(
        one(paged.on_click(paged.more_id(), Position::new(2, 6))),
        (Outcome::Changed, Some(GridAction::FetchMore))
    );
    assert_eq!(paged.cursor.0, 3, "the cursor lands on the footer row");

    // Cells: click selects, double click edits, drag extends a range.
    let cell = area_of(&painted, grid.cell_id(1, 2));
    assert_eq!(
        one(grid.on_click(grid.cell_id(1, 2), cell.as_position())),
        (Outcome::Changed, None)
    );
    assert_eq!(grid.cursor, (1, 2));
    let (outcome, action) = one(grid.on_click_with(
        grid.cell_id(1, 1),
        area_of(&painted, grid.cell_id(1, 1)).as_position(),
        KeyModifiers::NONE,
        2,
    ));
    assert_eq!((outcome, action), (Outcome::Changed, None));
    assert!(grid.is_editing(), "a double click opens the editor");
    let inside = area_of(&painted, grid.cell_id(1, 1)).as_position();
    assert_eq!(
        grid.on_press(grid.cell_id(1, 1), inside, 1),
        Outcome::Changed
    );
    assert_eq!(grid.on_drag(grid.cell_id(1, 1), inside), Outcome::Changed);
    assert!(grid.cancel_edit());
    let from = grid.cell_id(0, 1);
    let to = area_of(&painted, grid.cell_id(2, 3)).as_position();
    one(grid.on_click(from, area_of(&painted, from).as_position()));
    assert_eq!(grid.on_drag(from, to), Outcome::Changed);
    assert!(
        grid.has_selection(),
        "dragging across cells selects a range"
    );
    assert!(matches!(
        grid_press_key(&mut grid, ctrl('c')),
        Some(GridAction::Copy(t)) if t.lines().count() == 3
    ));
    grid.context_selection(1, 2);
    assert_eq!(
        grid.cursor,
        (2, 3),
        "a context click inside the range keeps it"
    );
    grid.context_selection(0, 5);
    assert_eq!(
        grid.cursor,
        (0, 5),
        "a context click outside the range moves the cursor"
    );
    assert!(!grid.has_selection());

    // Row numbers select rows: plain replaces, the toggle modifier adds.
    one(grid.on_click_with(
        grid.rownum_id(0),
        Position::new(1, 3),
        KeyModifiers::NONE,
        1,
    ));
    assert_eq!(grid.selection.len(), 1);
    one(grid.on_click_with(grid.rownum_id(2), Position::new(1, 5), KeyModifiers::ALT, 1));
    assert_eq!(
        grid.selection.len(),
        2,
        "the configured toggle modifier adds a row"
    );
    // A modifier no platform default uses proves the builder, not the default.
    let mut hyper = demo_grid("scenario.grid.hyper", 3).toggle_mods(KeyModifiers::HYPER);
    paint(area, |a, b, ctx| hyper.render(a, b, ctx, bg));
    one(hyper.on_click_with(
        hyper.rownum_id(0),
        Position::new(1, 3),
        KeyModifiers::NONE,
        1,
    ));
    one(hyper.on_click_with(
        hyper.rownum_id(2),
        Position::new(1, 5),
        KeyModifiers::HYPER,
        1,
    ));
    assert_eq!(hyper.selection.len(), 2, "the custom chord adds a row");
    one(hyper.on_click_with(
        hyper.rownum_id(1),
        Position::new(1, 4),
        KeyModifiers::ALT | KeyModifiers::CONTROL,
        1,
    ));
    assert_eq!(
        hyper.selection.len(),
        1,
        "a chord outside the configured one replaces the selection"
    );

    // Headers sort, the menu lists what a column offers and applies it.
    let header = area_of(&painted, grid.header_id(1));
    assert_eq!(
        one(grid.on_click(grid.header_id(1), header.as_position())),
        (
            Outcome::Changed,
            Some(GridAction::SortRequested(Some((1, SortDir::Asc))))
        )
    );
    let menu = grid.header_menu(1);
    let offered: Vec<HeaderAction> = menu.iter().map(|(a, _)| *a).collect();
    assert!(offered.contains(&HeaderAction::SortAsc) && offered.contains(&HeaderAction::Autofit));
    let ev = PointerEvent::new(
        &painted.hits,
        Phase::Click,
        grid.header_id(1),
        header.as_position(),
        1,
        KeyModifiers::NONE,
    );
    assert_eq!(grid.header_menu_at(&ev), Some(1));
    assert_eq!(
        grid.on_header_action(2, HeaderAction::SortDesc),
        (
            Outcome::Changed,
            Some(GridAction::SortRequested(Some((2, SortDir::Desc))))
        )
    );
    assert_eq!(
        grid.on_header_action(2, HeaderAction::SortAsc).1,
        Some(GridAction::SortRequested(Some((2, SortDir::Asc))))
    );
    assert_eq!(
        grid.on_header_action(2, HeaderAction::ClearSort).1,
        Some(GridAction::SortRequested(None))
    );

    // Dragging a column seam resizes it; a double press fits it; reset undoes both.
    let before = area_of(&painted, grid.header_id(2)).x;
    let seam = area_of(&painted, grid.seam_id(1));
    assert_eq!(
        grid.on_press(grid.seam_id(1), seam.as_position(), 1),
        Outcome::Changed
    );
    assert_eq!(
        grid.on_drag(grid.seam_id(1), Position::new(seam.x + 6, seam.y)),
        Outcome::Changed
    );
    one(grid.on_click(grid.seam_id(1), seam.as_position()));
    let wider = paint_hits(area, |a, b, ctx| grid.render(a, b, ctx, bg));
    let x_wider = area_of(&wider, grid.header_id(2)).x;
    assert!(x_wider > before, "the next column moved right");
    assert_eq!(seam.width, 3, "the seam hit area is as wide as configured");
    grid.autofit(1);
    let fitted = paint_hits(area, |a, b, ctx| grid.render(a, b, ctx, bg));
    assert!(
        area_of(&fitted, grid.header_id(2)).x < x_wider,
        "autofit pulls the dragged column back to its content"
    );
    let seam = area_of(&fitted, grid.seam_id(1));
    assert_eq!(
        grid.on_press(grid.seam_id(1), seam.as_position(), 1),
        Outcome::Changed
    );
    grid.on_drag(grid.seam_id(1), Position::new(seam.x + 6, seam.y));
    let seam = area_of(
        &paint_hits(area, |a, b, ctx| grid.render(a, b, ctx, bg)),
        grid.seam_id(1),
    );
    assert_eq!(
        grid.on_press(grid.seam_id(1), seam.as_position(), 2),
        Outcome::Changed
    );
    let refit = paint_hits(area, |a, b, ctx| grid.render(a, b, ctx, bg));
    assert_eq!(
        area_of(&refit, grid.header_id(2)).x,
        area_of(&fitted, grid.header_id(2)).x,
        "a double press on the seam fits the column again"
    );
    // Fixed widths: no seams, no resize entries in the header menu.
    let mut locked = demo_grid("scenario.grid.fixed-widths", 3).resizable_columns(false);
    let still = paint_hits(area, |a, b, ctx| locked.render(a, b, ctx, bg));
    assert!(
        !still
            .hits
            .regions()
            .iter()
            .any(|r| r.id == locked.seam_id(1)),
        "fixed widths register no seam"
    );
    assert!(
        !locked
            .header_menu(1)
            .iter()
            .any(|(a, _)| *a == HeaderAction::Autofit),
        "fixed widths offer no autofit"
    );
    assert_eq!(
        grid.on_header_action(1, HeaderAction::Autofit),
        (Outcome::Changed, None)
    );
    assert_eq!(
        grid.on_header_action(1, HeaderAction::ResetWidths),
        (Outcome::Changed, None)
    );
    let reset = paint_hits(area, |a, b, ctx| grid.render(a, b, ctx, bg));
    assert_eq!(
        area_of(&reset, grid.header_id(2)).x,
        before,
        "reset restores the widths"
    );

    // The change bar appears with a draft; its three buttons answer click and key.
    assert_eq!(
        grid.record_cell(0, 2, CellValue::Int(5)),
        Ok(CellEditResult::Changed)
    );
    let barred = paint_hits(area, |a, b, ctx| grid.render(a, b, ctx, bg));
    let bar_text = text_of(&barred.buf);
    assert!(
        bar_text.contains("Persist") && bar_text.contains("Throw away"),
        "{bar_text}"
    );
    let [preview, discard, save] = grid.bar_ids();
    assert_eq!(
        one(grid.on_click(discard, area_of(&barred, discard).as_position())).1,
        Some(GridAction::DiscardRequested)
    );
    assert_eq!(
        one(grid.on_click(save, area_of(&barred, save).as_position())).1,
        Some(GridAction::CommitRequested)
    );
    assert_eq!(
        one(grid.on_click(preview, area_of(&barred, preview).as_position())).1,
        Some(GridAction::PreviewSql)
    );
    assert_eq!(
        grid.on_bar_key(save, &key(KeyCode::Enter)),
        (Outcome::Changed, Some(GridAction::CommitRequested))
    );
    assert_eq!(
        grid.on_bar_key(WidgetId::of("elsewhere"), &key(KeyCode::Enter))
            .1,
        None
    );

    // A narrow viewport scrolls columns: badges, the range label, the arrows.
    let narrow = Rect::new(0, 0, 40, 10);
    let mut tight = demo_grid("scenario.grid.narrow", 3);
    let with_badges = paint(narrow, |a, b, ctx| tight.render(a, b, ctx, bg));
    let label = tight
        .cols_label()
        .expect("seven columns overflow forty cells");
    assert!(
        label.starts_with("cols 1–") && label.ends_with("of 7"),
        "{label}"
    );
    assert!(tight.position_label().contains(&label));
    one(tight.on_click(tight.right_id(), Position::new(39, 3)));
    paint(narrow, |a, b, ctx| tight.render(a, b, ctx, bg));
    assert!(tight.cols_label().is_some_and(|l| l.starts_with("cols 2–")));
    one(tight.on_click(tight.left_id(), Position::new(0, 3)));
    paint(narrow, |a, b, ctx| tight.render(a, b, ctx, bg));
    assert!(tight.cols_label().is_some_and(|l| l.starts_with("cols 1–")));
    let mut bare = demo_grid("scenario.grid.nobadges", 3).overflow_badges(false);
    let without = paint(narrow, |a, b, ctx| bare.render(a, b, ctx, bg));
    assert_ne!(
        text_of(&with_badges),
        text_of(&without),
        "badges are drawn only on request"
    );
    let mut still = demo_grid("scenario.grid.noscroll", 3).horizontal(HScroll::off());
    let fixed = paint(narrow, |a, b, ctx| still.render(a, b, ctx, bg));
    assert_ne!(
        text_of(&with_badges),
        text_of(&fixed),
        "turning horizontal scroll off changes the frame"
    );

    // An empty grid shows its own empty state.
    let mut none = DataGrid::new(WidgetId::of("scenario.grid.empty"), demo_columns())
        .empty(EmptyState::new("Nothing here yet"));
    let blank = paint(area, |a, b, ctx| none.render(a, b, ctx, bg));
    assert!(text_of(&blank).contains("Nothing here yet"));
    assert_eq!(none.rows_label(), "0 rows");

    // A navigation grid activates on the first click; the track policy decides
    // what a scrollbar click does on a tall grid.
    let mut nav = demo_grid("scenario.grid.nav", 3)
        .editable(false)
        .click(ClickPolicy::NAVIGATION);
    let shown = paint_hits(area, |a, b, ctx| nav.render(a, b, ctx, bg));
    assert_eq!(
        one(nav.on_click(
            nav.cell_id(2, 1),
            area_of(&shown, nav.cell_id(2, 1)).as_position()
        )),
        (Outcome::Changed, Some(GridAction::Activated(2)))
    );
    let offset_after_track_click = |policy: TrackClick| {
        let mut tall = DataGrid::new(
            WidgetId::of("scenario.grid.tall"),
            vec![ColumnSpec::new("n", CellKind::Number)],
        )
        .track_click(policy);
        tall.try_set_rows(GridRows {
            rows: (0..200).map(|i| vec![CellValue::Int(i)]).collect(),
            total: RowTotal::Exact(200),
            more: false,
        })
        .expect("rectangular rows");
        let small = Rect::new(0, 0, 30, 12);
        let drawn = paint_hits(small, |a, b, ctx| tall.render(a, b, ctx, bg));
        let bar = drawn
            .hits
            .regions()
            .iter()
            .find(|r| scrollbar::is_bar_of(r.id, tall.id))
            .map(|r| (r.id, r.area))
            .expect("a vertical scrollbar");
        let _ = tall.on_click(bar.0, Position::new(bar.1.x, bar.1.bottom() - 1));
        tall.scroll.offset
    };
    assert!(
        offset_after_track_click(TrackClick::Jump) > offset_after_track_click(TrackClick::Page),
        "jump lands under the click, page advances one screen"
    );
}

fn grid_edit_sessions() {
    let area = Rect::new(0, 0, 110, 14);
    let bg = Theme::for_level(ColorLevel::TrueColor).surface;
    let mut grid = demo_grid("scenario.grid.sessions", 3).editor_config(EditorConfig {
        blur: BlurAction::Commit,
        affordance: Affordance::Always,
        ..EditorConfig::default()
    });
    assert_eq!(
        grid.choice_id(),
        WidgetId::of("scenario.grid.sessions").sub("choice")
    );
    paint(area, |a, b, ctx| grid.render(a, b, ctx, bg));

    // A text session: open, type, commit.
    grid.cursor = (0, 1);
    assert!(grid.cell_editable());
    assert_eq!(grid.read_only_reason_at_cursor(), None);
    assert_eq!(one(grid.begin_edit()), (Outcome::Changed, None));
    let session: &EditState = grid.edit.as_ref().expect("a text session");
    assert_eq!((session.row, session.col), (0, 1));
    assert!(grid.is_editing());
    assert_eq!(grid.on_paste("x"), Outcome::Changed);
    assert_eq!(
        grid.commit_edit(),
        Some(GridAction::CellChanged { row: 0, col: 1 })
    );
    assert_eq!(grid.value(0, 1), &CellValue::Text("name0x".into()));
    assert!(!grid.is_editing());
    assert_eq!(
        grid.on_paste("ignored"),
        Outcome::Ignored,
        "no editor, nothing to paste into"
    );
    assert_eq!(grid.commit_edit(), None);

    // A rejected value keeps the session open and says why; cancel closes it.
    grid.cursor = (0, 2);
    let _ = grid.begin_edit();
    grid.on_paste("abc");
    assert_eq!(grid.commit_edit(), None);
    assert!(grid.is_editing());
    assert!(
        grid.edit_error().is_some_and(|e| e.contains("number")),
        "{:?}",
        grid.edit_error()
    );
    assert!(grid.cancel_edit());
    assert_eq!(grid.edit_error(), None);
    assert!(!grid.cancel_edit(), "nothing left to cancel");
    assert_eq!(grid.value(0, 2), &CellValue::Int(10));

    // Losing focus commits, cancels or keeps the session by policy.
    let blur = |grid: &mut DataGrid, policy: BlurAction| {
        grid.blur_action = policy;
        grid.cursor = (1, 1);
        let _ = grid.begin_edit();
        grid.on_paste("?");
        let result = one(grid.on_blur());
        (result, grid.is_editing())
    };
    let ((_, action), editing) = blur(&mut grid, BlurAction::Commit);
    assert_eq!(action, Some(GridAction::CellChanged { row: 1, col: 1 }));
    assert!(!editing);
    let ((_, action), editing) = blur(&mut grid, BlurAction::Cancel);
    assert_eq!((action, editing), (None, false));
    assert_eq!(
        grid.value(1, 1),
        &CellValue::Text("name1?".into()),
        "cancel keeps the committed text"
    );
    let (_, editing) = blur(&mut grid, BlurAction::Keep);
    assert!(editing, "keep leaves the editor open");
    grid.cancel_edit();
    assert_eq!(
        one(grid.on_blur()).0,
        Outcome::Ignored,
        "no session, nothing to resolve"
    );

    // A choice column opens a list editor; the pick commits and out-of-set
    // values are flagged.
    grid.cursor = (0, 6);
    let _ = grid.begin_edit();
    let choice: &GridChoice = grid.choice.as_ref().expect("a choice session");
    assert_eq!((choice.row, choice.col), (0, 6));
    assert!(grid.is_editing());
    paint(area, |a, b, ctx| grid.render(a, b, ctx, bg));
    assert_eq!(grid_press(&mut grid, KeyCode::Down), None);
    assert_eq!(
        grid_press(&mut grid, KeyCode::Enter),
        Some(GridAction::CellChanged { row: 0, col: 6 })
    );
    assert_eq!(grid.value(0, 6), &CellValue::Text("beta".into()));
    assert!(!grid.is_editing());
    assert_eq!(
        grid.record_cell(0, 6, CellValue::Text("zeta".into())),
        Ok(CellEditResult::Changed)
    );
    assert!(
        grid.out_of_set(0, 6),
        "a value outside the options is flagged"
    );
    assert!(!grid.out_of_set(0, 1), "a plain text column has no set");
    assert!(!grid.out_of_set(0, 99));

    // The open button inside a choice cell starts the same session by click.
    grid.cursor = (1, 6);
    let drawn = paint_hits(area, |a, b, ctx| grid.render(a, b, ctx, bg));
    let open = grid.open_id(1, 6);
    one(grid.on_click(open, area_of(&drawn, open).as_position()));
    assert!(
        grid.choice.is_some(),
        "clicking the open affordance opens the editor"
    );
    grid.cancel_edit();

    // Why a cell is read only: a locked column, a deleted row, a frozen grid.
    grid.cursor = (0, 5);
    assert!(!grid.cell_editable());
    assert!(
        grid.read_only_reason_at_cursor()
            .is_some_and(|r| r.contains("locked"))
    );
    grid.cursor = (2, 1);
    grid.toggle_delete(2).expect("an unlocked grid");
    assert!(!grid.cell_editable());
    assert!(grid.read_only_reason_at_cursor().is_some());
    let mut frozen = demo_grid("scenario.grid.frozen2", 1).editable(false);
    frozen.read_only_reason = Some("Archived".into());
    assert_eq!(
        frozen.read_only_reason_at_cursor().as_deref(),
        Some("Archived")
    );
    assert!(!frozen.cell_editable());
}

fn table_columns() -> Vec<Column> {
    vec![
        Column::new("name", Constraint::Length(10)).editable(),
        Column::new("qty", Constraint::Length(6)).right().editable(),
        Column::new("kind", Constraint::Length(8)).editor(ColumnEditor::Select(Options::new(
            ["alpha", "beta"].map(OptionItem::new),
        ))),
        Column::new("note", Constraint::Fill(1)),
    ]
}

fn table_rows(n: usize) -> Vec<Vec<Cell>> {
    (0..n)
        .map(|i| {
            vec![
                Cell::new(format!("name{i}")),
                Cell::new(["10", "9", "100", "1000", "5"][i % 5]),
                Cell::new("alpha"),
                Cell::new(format!("note{i}")),
            ]
        })
        .collect()
}

fn demo_table(name: &'static str, n: usize) -> DataTable {
    DataTable::try_new(WidgetId::of(name), table_columns(), table_rows(n))
        .expect("rectangular rows")
}

fn tact(resp: Response<TableAction>) -> (Outcome, Option<TableAction>) {
    resp.try_into_single().expect("at most one action")
}

fn table_press(table: &mut DataTable, code: KeyCode) -> Option<TableAction> {
    tact(table.on_key(&key(code))).1
}

fn table_model_and_selection() {
    // Column and cell builders.
    let name = Column::new("name", Constraint::Length(10)).editable();
    assert_eq!(
        (name.min_width(), name.align, name.editable),
        (10, Align::Left, true)
    );
    assert_eq!(Column::new("note", Constraint::Fill(1)).min_width(), 6);
    assert!(!Column::new("note", Constraint::Fill(1)).editable);
    let qty = Column::new("qty", Constraint::Length(6)).right();
    assert_eq!(qty.align, Align::Right);
    assert!(
        !qty.editable,
        "right alignment alone does not make a column editable"
    );
    let kind = Column::new("kind", Constraint::Min(8)).editor(ColumnEditor::Select(Options::new(
        ["a"].map(OptionItem::new),
    )));
    assert!(kind.editable && matches!(kind.editor, ColumnEditor::Select(_)));
    assert_eq!(kind.min_width(), 8);
    let cell = Cell::new("x").tone(Tone::Success);
    let borrowed = cell.borrowed();
    assert_eq!(
        (borrowed.text, borrowed.tone, borrowed.error),
        ("x", Tone::Success, None)
    );

    // A multi-select table: select all skips disabled rows, the label counts.
    let mut table = demo_table("scenario.table.model", 5)
        .selection_mode(SelectionMode::Multi)
        .selection_column(true)
        .esc_clears(true)
        .disabled_rows(&[4])
        .toggle_mods(DataTable::TOGGLE_CTRL_CMD);
    assert_eq!((table.len(), table.is_empty()), (5, false));
    assert!(table.is_disabled(4) && !table.is_disabled(0));
    assert_eq!(table.selection_state(), TriState::None);
    assert!(table.select_all());
    assert_eq!(
        table.selected_rows(),
        vec![0, 1, 2, 3],
        "the disabled row stays out"
    );
    assert_eq!(table.selection_label(), "4 selected");
    assert_eq!(table.selection_state(), TriState::All);
    assert!(table.toggle_row(1), "a toggle removes a selected row");
    assert_eq!(table.selection_state(), TriState::Mixed);
    assert!(!table.toggle_row(4), "a disabled row cannot be toggled");
    assert!(table.clear_selection());
    assert!(!table.clear_selection(), "already clear");
    assert_eq!(table.selection_label(), "");
    assert!(
        table.context_selection(2),
        "a context click selects its row"
    );
    assert_eq!(table.selected_rows(), vec![2]);
    table.set_row_disabled(2, true);
    assert!(
        table.selected_rows().is_empty(),
        "disabling a row deselects it"
    );
    table.set_row_disabled(2, false);
    assert!(!table.is_disabled(2));
    table.set_row_disabled(0, true);
    table.remove_rows(&[1]);
    assert_eq!(table.len(), 4);
    assert!(
        table.is_disabled(0) && table.is_disabled(3),
        "disabled flags follow their rows"
    );
    assert_eq!(table.rows()[1][0].text, "name2");

    // The platform toggle chords differ by modifier set, and a table takes any of them.
    assert_eq!(
        DataTable::toggle_mods_for(ClientPlatform::MacOs),
        DataTable::TOGGLE_OPTION_CMD
    );
    assert_eq!(
        DataTable::toggle_mods_for(ClientPlatform::Other),
        DataTable::TOGGLE_CTRL_CMD
    );
    assert_ne!(DataTable::TOGGLE_WITH_ALT, DataTable::TOGGLE_CTRL_CMD);
    let mut chords = demo_table("scenario.table.chords", 3)
        .selection_mode(SelectionMode::Multi)
        .toggle_mods(DataTable::TOGGLE_WITH_ALT);
    let plain = tact(chords.on_click_cell_with(0, 0, Position::new(0, 0), KeyModifiers::NONE));
    assert_eq!(
        plain,
        (Outcome::Changed, Some(TableAction::SelectionChanged))
    );
    let added =
        tact(chords.on_click_cell_with(2, 0, Position::new(0, 0), DataTable::TOGGLE_WITH_ALT));
    assert_eq!(
        added,
        (Outcome::Changed, Some(TableAction::SelectionChanged))
    );
    assert_eq!(
        chords.selected_rows(),
        vec![0, 2],
        "the chosen chord adds without replacing"
    );
    // A modifier no platform default uses proves the builder, not the default.
    let mut hyper = demo_table("scenario.table.hyper", 3)
        .selection_mode(SelectionMode::Multi)
        .toggle_mods(KeyModifiers::HYPER);
    tact(hyper.on_click_cell_with(0, 0, Position::new(0, 0), KeyModifiers::NONE));
    tact(hyper.on_click_cell_with(2, 0, Position::new(0, 0), KeyModifiers::HYPER));
    assert_eq!(hyper.selected_rows(), vec![0, 2], "the custom chord adds");
    tact(hyper.on_click_cell_with(
        1,
        0,
        Position::new(0, 0),
        KeyModifiers::ALT | KeyModifiers::CONTROL,
    ));
    assert_eq!(
        hyper.selected_rows(),
        vec![1],
        "a chord outside the configured one replaces the selection"
    );

    // Alignment and tone show in the frame.
    let small = Rect::new(0, 0, 24, 4);
    let nine_at = |column: Column| {
        let mut t = DataTable::try_new(
            WidgetId::of("scenario.table.align"),
            vec![column],
            vec![vec![Cell::new("9")]],
        )
        .expect("rectangular rows");
        let buf = paint(small, |a, b, ctx| {
            t.render(a, b, ctx, Theme::for_level(ColorLevel::TrueColor).surface)
        });
        (0..small.height)
            .flat_map(|y| (0..small.width).map(move |x| (x, y)))
            .find(|&(x, y)| buf[(x, y)].symbol() == "9")
            .map(|(x, _)| x)
            .expect("the cell text is drawn")
    };
    assert!(
        nine_at(Column::new("qty", Constraint::Length(8)).right())
            > nine_at(Column::new("qty", Constraint::Length(8))),
        "a right-aligned column ends at its right edge"
    );
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let color_of_o = |tone: Tone| {
        let mut t = DataTable::try_new(
            WidgetId::of("scenario.table.tone"),
            vec![Column::new("c", Constraint::Length(8))],
            vec![vec![Cell::new("ok").tone(tone)]],
        )
        .expect("rectangular rows");
        let buf = paint(small, |a, b, ctx| t.render(a, b, ctx, theme.surface));
        (0..small.height)
            .flat_map(|y| (0..small.width).map(move |x| (x, y)))
            .find(|&(x, y)| buf[(x, y)].symbol() == "o")
            .map(|(x, y)| buf[(x, y)].fg)
            .expect("the cell text is drawn")
    };
    assert_eq!(color_of_o(Tone::Success), theme.tone(Tone::Success));
    assert_ne!(color_of_o(Tone::Normal), theme.tone(Tone::Success));

    // Sorting cycles ascending, descending, none; numeric columns compare as numbers.
    let mut sorted = demo_table("scenario.table.sort", 3);
    assert_eq!(sorted.sort_by(0), Outcome::Changed);
    assert_eq!(sorted.sort, Some((0, SortDir::Asc)));
    assert_eq!(sorted.source_row(0), 0);
    assert_eq!(sorted.sort_by(0), Outcome::Changed);
    assert_eq!(sorted.sort, Some((0, SortDir::Desc)));
    assert_eq!(
        sorted.source_row(0),
        2,
        "descending puts the last row first"
    );
    assert_eq!(sorted.sort_by(0), Outcome::Changed);
    assert_eq!(sorted.sort, None);
    assert_eq!(sorted.sort_by(9), Outcome::Consumed, "no such column");
    let mut texts = demo_table("scenario.table.text", 3);
    texts.sort_by(1);
    let by_text: Vec<usize> = (0..3).map(|d| texts.source_row(d)).collect();
    let mut numbers = demo_table("scenario.table.numeric", 3).numeric(&[1]);
    numbers.sort_by(1);
    let by_number: Vec<usize> = (0..3).map(|d| numbers.source_row(d)).collect();
    assert_ne!(
        by_text, by_number,
        "quantities 10, 9, 100 sort differently as text"
    );
    assert_eq!(by_number, vec![1, 0, 2]);
    assert!(numbers.numeric[1] && !numbers.numeric[0]);
}

fn table_keys_and_edits() {
    let area = Rect::new(0, 0, 100, 12);
    let bg = Theme::for_level(ColorLevel::TrueColor).surface;

    // Cell navigation, sorting and the remappable command keys.
    let mut table = demo_table("scenario.table.keys", 4).cell_nav(true);
    paint(area, |a, b, ctx| table.render(a, b, ctx, bg));
    assert_eq!(table_press(&mut table, KeyCode::Down), None);
    assert_eq!(table_press(&mut table, KeyCode::Right), None);
    assert_eq!((table.cursor_row, table.cursor_col), (1, 1));
    table.cursor_col = 0;
    assert_eq!(
        tact(table.on_key(&key(KeyCode::Char('s')))).0,
        Outcome::Changed
    );
    assert_eq!(table.sort, Some((0, SortDir::Asc)), "s is SortByColumn");
    let mut remapped = demo_table("scenario.table.remap", 3)
        .keymap(vec![(KeyBinding::char('o'), TableCmd::SortByColumn)])
        .nav(NavKeys::VIM);
    table_press(&mut remapped, KeyCode::Char('s'));
    assert_eq!(remapped.sort, None, "the default key is unbound");
    table_press(&mut remapped, KeyCode::Char('o'));
    assert_eq!(remapped.sort, Some((0, SortDir::Asc)));
    table_press(&mut remapped, KeyCode::Char('j'));
    assert_eq!(remapped.cursor_row, 1, "vim j moves down");
    let mut typed = demo_table("scenario.table.typeahead", 4)
        .type_ahead(true)
        .type_ahead_column(Some(0))
        .type_ahead_precedence(TypeAheadPrecedence::TypeAhead)
        .type_ahead_timeout(std::time::Duration::from_millis(400));
    for c in "name2".chars() {
        table_press(&mut typed, KeyCode::Char(c));
    }
    assert_eq!(
        typed.cursor_row, 2,
        "typing a prefix jumps to the matching row"
    );
    table_press(&mut typed, KeyCode::Char('s'));
    assert_eq!(
        typed.sort, None,
        "with type-ahead first, s is a search letter"
    );
    let first_s = |precedence: TypeAheadPrecedence| {
        let mut fresh = demo_table("scenario.table.precedence", 4)
            .type_ahead(true)
            .type_ahead_column(Some(0))
            .type_ahead_precedence(precedence);
        route_key(&key(KeyCode::Char('s')), || {
            tact(fresh.on_key(&key(KeyCode::Char('s')))).0
        });
        fresh.sort
    };
    assert_eq!(first_s(TypeAheadPrecedence::TypeAhead), None);
    assert_eq!(
        first_s(TypeAheadPrecedence::Shortcuts),
        Some((0, SortDir::Asc)),
        "the precedence decides whether s sorts or starts a search"
    );
    let mut by_qty = demo_table("scenario.table.typeahead-qty", 4)
        .type_ahead(true)
        .type_ahead_column(Some(1))
        .type_ahead_precedence(TypeAheadPrecedence::TypeAhead);
    table_press(&mut by_qty, KeyCode::Char('9'));
    assert_eq!(
        by_qty.cursor_row, 1,
        "type-ahead searches the chosen column, not the first one"
    );
    // A zero timeout forgets the prefix before every letter.
    let mut forgetful = demo_table("scenario.table.forgetful", 4)
        .type_ahead(true)
        .type_ahead_column(Some(0))
        .type_ahead_precedence(TypeAheadPrecedence::TypeAhead)
        .type_ahead_timeout(std::time::Duration::ZERO);
    for c in "name2".chars() {
        std::thread::sleep(std::time::Duration::from_millis(2));
        table_press(&mut forgetful, KeyCode::Char(c));
    }
    assert_eq!(
        forgetful.cursor_row, 0,
        "each letter starts a new search, so the prefix never matches"
    );

    // Selection by keyboard: space toggles, shift extends, select all, Esc clears.
    let mut multi = demo_table("scenario.table.multi", 4)
        .selection_mode(SelectionMode::Multi)
        .esc_clears(true);
    assert_eq!(
        table_press(&mut multi, KeyCode::Char(' ')),
        Some(TableAction::SelectionChanged)
    );
    assert_eq!(multi.selected_rows(), vec![0]);
    let shift_down = Key {
        code: KeyCode::Down,
        mods: KeyModifiers::SHIFT,
    };
    assert_eq!(
        tact(multi.on_key(&shift_down)).1,
        Some(TableAction::SelectionChanged)
    );
    assert_eq!(multi.selected_rows(), vec![0, 1]);
    let select_all = Key {
        code: KeyCode::Char('a'),
        mods: KeyModifiers::CONTROL,
    };
    assert_eq!(
        tact(multi.on_key(&select_all)).1,
        Some(TableAction::SelectionChanged)
    );
    assert_eq!(multi.selected_rows().len(), 4);
    assert_eq!(
        table_press(&mut multi, KeyCode::Esc),
        Some(TableAction::SelectionChanged)
    );
    assert!(multi.selected_rows().is_empty());
    let mut mapped = demo_table("scenario.table.select-all", 4)
        .selection_mode(SelectionMode::Multi)
        .keymap(vec![(KeyBinding::char('A'), TableCmd::SelectAll)]);
    assert_eq!(
        table_press(&mut mapped, KeyCode::Char('A')),
        Some(TableAction::SelectionChanged)
    );
    assert_eq!(mapped.selected_rows().len(), 4);
    let mut quiet = demo_table("scenario.table.quiet", 4)
        .selection_mode(SelectionMode::Multi)
        .esc_clears(false);
    quiet.select_all();
    assert_eq!(
        table_press(&mut quiet, KeyCode::Esc),
        None,
        "Esc keeps the selection unless asked"
    );
    assert_eq!(quiet.selected_rows().len(), 4);

    // Activation: Enter reports the row; single mode remembers it, none does not.
    let mut single = demo_table("scenario.table.single", 3);
    table_press(&mut single, KeyCode::Down);
    assert_eq!(
        table_press(&mut single, KeyCode::Enter),
        Some(TableAction::Activated(1))
    );
    assert_eq!(single.selected_rows(), vec![1]);
    let mut nothing = demo_table("scenario.table.nothing", 3).selection_mode(SelectionMode::None);
    assert_eq!(
        table_press(&mut nothing, KeyCode::Enter),
        Some(TableAction::Activated(0))
    );
    assert!(nothing.selected_rows().is_empty());

    // Editing: open, type, commit; cancel; a validator keeps a bad edit open.
    let mut edit = demo_table("scenario.table.edit", 3)
        .cell_nav(true)
        .editor_config(EditorConfig {
            blur: BlurAction::Commit,
            ..EditorConfig::default()
        })
        .validator(|col, text| {
            (col == 1 && text.parse::<u32>().is_err()).then(|| "digits only".to_owned())
        });
    assert!(!edit.is_editing());
    assert_eq!(table_press(&mut edit, KeyCode::Enter), None);
    assert!(edit.is_editing());
    let session: &termrock::widgets::table::EditState = edit.edit.as_ref().expect("a text session");
    assert_eq!((session.row, session.col), (0, 0));
    assert_eq!(edit.on_paste("!"), Outcome::Changed);
    assert_eq!(
        table_press(&mut edit, KeyCode::Enter),
        Some(TableAction::Committed { row: 0, col: 0 })
    );
    assert_eq!(edit.rows()[0][0].text, "name0!");
    assert_eq!(
        edit.on_paste("ignored"),
        Outcome::Ignored,
        "no editor to paste into"
    );
    edit.cursor_row = 1;
    assert_eq!(tact(edit.begin_edit()), (Outcome::Changed, None));
    assert_eq!(
        table_press(&mut edit, KeyCode::Esc),
        Some(TableAction::Cancelled)
    );
    assert_eq!(tact(edit.start_edit()), (Outcome::Changed, None));
    assert_eq!(edit.cancel_edit(), Some(TableAction::Cancelled));
    assert_eq!(edit.cancel_edit(), None, "nothing left to cancel");

    edit.cursor_col = 1;
    let _ = edit.begin_edit();
    edit.on_paste("x");
    assert_eq!(edit.commit_edit(), None);
    assert!(edit.is_editing(), "a rejected value keeps the editor open");
    assert_eq!(edit.edit_error(), Some("digits only"));
    edit.cancel_edit();
    assert_eq!(edit.edit_error(), None);
    let _ = edit.begin_edit();
    edit.on_paste("5");
    assert_eq!(
        edit.commit_edit(),
        Some(TableAction::Committed { row: 1, col: 1 })
    );
    assert_eq!(edit.rows()[1][1].text, "95");

    // Losing focus follows the table's blur policy.
    let blur = |table: &mut DataTable, policy: BlurAction| {
        table.blur_action = policy;
        table.cursor_row = 2;
        table.cursor_col = 0;
        let _ = table.begin_edit();
        table.on_paste("?");
        let result = tact(table.on_blur());
        (result.1, table.is_editing())
    };
    let (action, editing) = blur(&mut edit, BlurAction::Commit);
    assert_eq!(
        (action, editing),
        (Some(TableAction::Committed { row: 2, col: 0 }), false)
    );
    let (action, editing) = blur(&mut edit, BlurAction::Cancel);
    assert_eq!((action, editing), (Some(TableAction::Cancelled), false));
    let (action, editing) = blur(&mut edit, BlurAction::Keep);
    assert_eq!((action, editing), (None, true));
    edit.cancel_edit();
    assert_eq!(tact(edit.on_blur()).0, Outcome::Ignored);

    // A choice column opens a list editor; the pick commits; strays are flagged.
    edit.cursor_row = 0;
    edit.cursor_col = 2;
    let _ = edit.begin_edit();
    let choice: &ChoiceEdit = edit.choice.as_ref().expect("a choice session");
    assert_eq!((choice.row, choice.col), (0, 2));
    assert_eq!(
        edit.choice_id(),
        WidgetId::of("scenario.table.edit").sub("choice")
    );
    assert!(edit.is_editing());
    paint(area, |a, b, ctx| edit.render(a, b, ctx, bg));
    table_press(&mut edit, KeyCode::Down);
    assert_eq!(
        table_press(&mut edit, KeyCode::Enter),
        Some(TableAction::Committed { row: 0, col: 2 })
    );
    assert_eq!(edit.rows()[0][2].text, "beta");
    assert!(edit.update_cell(1, 2, |c| c.text = "zeta".into()));
    assert!(edit.out_of_set(1, 2) && !edit.out_of_set(0, 0));
    assert_eq!(edit.out_of_set_cells(), vec![(1, 2)]);

    // Tab walks to the next editable cell and reports where it leaves the table.
    let mut two = DataTable::try_new(
        WidgetId::of("scenario.table.tabs"),
        vec![
            Column::new("a", Constraint::Length(8)).editable(),
            Column::new("b", Constraint::Length(8)).editable(),
        ],
        vec![vec![Cell::new("1"), Cell::new("2")]],
    )
    .expect("rectangular rows")
    .cell_nav(true);
    two.cursor_col = 1;
    let _ = two.begin_edit();
    assert_eq!(
        table_press(&mut two, KeyCode::Tab),
        Some(TableAction::LeaveForward)
    );
    two.cursor_col = 0;
    let _ = two.begin_edit();
    assert_eq!(
        table_press(&mut two, KeyCode::BackTab),
        Some(TableAction::LeaveBackward)
    );
}

fn table_pointer_and_chrome() {
    let area = Rect::new(0, 0, 100, 14);
    let bg = Theme::for_level(ColorLevel::TrueColor).surface;
    let mut table = demo_table("scenario.table.pointer", 4)
        .selection_mode(SelectionMode::Multi)
        .selection_column(true)
        .resizable_columns(true)
        .seam_hit_w(3)
        .reveal(Reveal::OFF);
    let painted = paint_hits(area, |a, b, ctx| table.render(a, b, ctx, bg));
    let text = text_of(&painted.buf);
    assert!(text.contains("name1") && text.contains("qty"), "{text}");
    let origin = |id: WidgetId| area_of(&painted, id).as_position();
    let origin_in = |frame: &Painted, id: WidgetId| area_of(frame, id).as_position();

    // The frame registers every addressable part under the id the table reports.
    for id in [
        table.header_id(1),
        table.row_id(1),
        table.cell_id(1, 1),
        table.check_id(1),
        table.check_all_id(),
        table.seam_id(1),
    ] {
        area_of(&painted, id);
    }

    // Check boxes toggle rows; the header box toggles them all.
    assert_eq!(
        table.on_click_check(1),
        (Outcome::Changed, Some(TableAction::SelectionChanged))
    );
    assert_eq!(table.selected_rows(), vec![1]);
    assert_eq!(
        table.on_click_check(9),
        (Outcome::Consumed, None),
        "no such row"
    );
    assert_eq!(
        table.on_click_check_all(),
        (Outcome::Changed, Some(TableAction::SelectionChanged))
    );
    assert_eq!(table.selection_state(), TriState::All);
    assert_eq!(
        table.on_click_check_all(),
        (Outcome::Changed, Some(TableAction::SelectionChanged))
    );
    assert_eq!(table.selection_state(), TriState::None);

    // Cell clicks select, a double click activates; the header click sorts.
    assert_eq!(
        tact(table.on_click_cell(2, 0, origin(table.cell_id(2, 0)))),
        (Outcome::Changed, Some(TableAction::SelectionChanged))
    );
    assert_eq!(table.selected_rows(), vec![2]);
    assert_eq!(
        tact(table.on_click_cell_count(2, 0, origin(table.cell_id(2, 0)), KeyModifiers::NONE, 2)).1,
        Some(TableAction::Activated(2))
    );
    assert_eq!(tact(table.on_click_header(0)), (Outcome::Changed, None));
    assert_eq!(table.sort, Some((0, SortDir::Asc)));

    // Seams resize columns; the header menu lists and applies what a column offers.
    let seam = origin(table.seam_id(1));
    let before = area_of(&painted, table.header_id(2)).x;
    assert_eq!(table.on_press(table.seam_id(1), seam, 1), Outcome::Changed);
    assert_eq!(
        table.on_drag(table.seam_id(1), Position::new(seam.x + 5, seam.y)),
        Outcome::Changed
    );
    let wider = paint_hits(area, |a, b, ctx| table.render(a, b, ctx, bg));
    let x_wider = area_of(&wider, table.header_id(2)).x;
    assert!(x_wider > before, "the next column moved right");
    assert_eq!(
        area_of(&painted, table.seam_id(1)).width,
        3,
        "the seam hit area is as wide as configured"
    );
    table.autofit(1);
    let fitted = paint_hits(area, |a, b, ctx| table.render(a, b, ctx, bg));
    assert!(
        area_of(&fitted, table.header_id(2)).x < x_wider,
        "autofit pulls the dragged column back to its content"
    );
    let seam = origin_in(&fitted, table.seam_id(1));
    assert_eq!(table.on_press(table.seam_id(1), seam, 1), Outcome::Changed);
    table.on_drag(table.seam_id(1), Position::new(seam.x + 5, seam.y));
    let seam = origin_in(
        &paint_hits(area, |a, b, ctx| table.render(a, b, ctx, bg)),
        table.seam_id(1),
    );
    assert_eq!(
        table.on_press(table.seam_id(1), seam, 2),
        Outcome::Changed,
        "a double press fits"
    );
    let refit = paint_hits(area, |a, b, ctx| table.render(a, b, ctx, bg));
    assert_eq!(
        area_of(&refit, table.header_id(2)).x,
        area_of(&fitted, table.header_id(2)).x,
        "a double press on the seam fits the column again"
    );
    let mut locked = demo_table("scenario.table.fixed-widths", 3).resizable_columns(false);
    let still = paint_hits(area, |a, b, ctx| locked.render(a, b, ctx, bg));
    assert!(
        !still
            .hits
            .regions()
            .iter()
            .any(|r| r.id == locked.seam_id(1)),
        "fixed widths register no seam"
    );
    assert!(
        !locked
            .header_menu(1)
            .iter()
            .any(|(a, _)| *a == HeaderAction::Autofit),
        "fixed widths offer no autofit"
    );
    assert_eq!(
        table.on_press(WidgetId::of("elsewhere"), seam, 1),
        Outcome::Ignored
    );
    let offered: Vec<HeaderAction> = table.header_menu(1).iter().map(|(a, _)| *a).collect();
    assert!(offered.contains(&HeaderAction::SortAsc) && offered.contains(&HeaderAction::Autofit));
    let header = origin(table.header_id(1));
    let ev = PointerEvent::new(
        &painted.hits,
        Phase::Click,
        table.header_id(1),
        header,
        1,
        KeyModifiers::NONE,
    );
    assert_eq!(table.header_menu_at(&ev), Some(1));
    assert_eq!(
        table.on_header_action(1, HeaderAction::SortDesc),
        Outcome::Changed
    );
    assert_eq!(table.sort, Some((1, SortDir::Desc)));
    assert_eq!(
        table.on_header_action(1, HeaderAction::ClearSort),
        Outcome::Changed
    );
    assert_eq!(table.sort, None);
    assert_eq!(
        table.on_header_action(1, HeaderAction::Autofit),
        Outcome::Changed
    );
    assert_eq!(
        table.on_header_action(1, HeaderAction::ResetWidths),
        Outcome::Changed
    );
    let reset = paint_hits(area, |a, b, ctx| table.render(a, b, ctx, bg));
    assert_eq!(
        area_of(&reset, table.header_id(2)).x,
        before,
        "reset restores the widths"
    );

    // The open button of a choice cell starts the editor by click.
    let mut cells = demo_table("scenario.table.open", 3)
        .cell_nav(true)
        .editor_config(EditorConfig {
            affordance: Affordance::Always,
            ..EditorConfig::default()
        });
    let shown = paint_hits(area, |a, b, ctx| cells.render(a, b, ctx, bg));
    let open = cells.open_id(1, 2);
    let at = area_of(&shown, open).as_position();
    assert!(cells.on_click_editor(WidgetId::of("elsewhere")).is_none());
    let opened = cells
        .on_click_editor(open)
        .expect("the open button is ours");
    assert_eq!(tact(opened), (Outcome::Changed, None));
    assert!(
        cells.choice.is_some(),
        "clicking the open affordance opens the editor {at:?}"
    );

    // A focused table offers the full text of a cut cell, unless reveal is off.
    let peek_of = |reveal: Option<Reveal>| {
        let theme = Theme::for_level(ColorLevel::TrueColor);
        let mut t = DataTable::try_new(
            WidgetId::of("scenario.table.reveal"),
            vec![Column::new("n", Constraint::Length(6))],
            vec![vec![Cell::new("a much longer cell")]],
        )
        .expect("rectangular rows");
        if let Some(reveal) = reveal {
            t = t.reveal(reveal);
        }
        let small = Rect::new(0, 0, 20, 4);
        let mut buf = Buffer::empty(small);
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let interaction = Interaction {
            focus: Some(t.id),
            ..Interaction::default()
        };
        let mut ctx = RenderCtx::new(&theme, interaction, &mut hits, &mut ring);
        t.render(small, &mut buf, &mut ctx, theme.surface);
        ctx.peek.clone()
    };
    assert_eq!(peek_of(None).as_deref(), Some("a much longer cell"));
    assert_eq!(
        peek_of(Some(Reveal::OFF)),
        None,
        "reveal off offers nothing"
    );

    // A navigation table activates on the first click.
    let mut nav = demo_table("scenario.table.nav", 3).click(ClickPolicy::NAVIGATION);
    paint(area, |a, b, ctx| nav.render(a, b, ctx, bg));
    assert_eq!(
        tact(nav.on_click_cell(1, 0, Position::new(2, 3))).1,
        Some(TableAction::Activated(1))
    );

    // Chrome knobs: overflow marks, horizontal scroll policy, track clicks, empty state.
    let narrow = Rect::new(0, 0, 24, 8);
    let marked = paint(narrow, |a, b, ctx| {
        demo_table("scenario.table.marks", 3).render(a, b, ctx, bg)
    });
    let bare = paint(narrow, |a, b, ctx| {
        demo_table("scenario.table.nomarks", 3)
            .overflow_marks(false)
            .render(a, b, ctx, bg)
    });
    assert_ne!(
        text_of(&marked),
        text_of(&bare),
        "overflow marks are drawn only on request"
    );
    let fixed = paint(narrow, |a, b, ctx| {
        demo_table("scenario.table.fixed", 3)
            .horizontal(HScroll::off())
            .render(a, b, ctx, bg)
    });
    assert_ne!(
        text_of(&marked),
        text_of(&fixed),
        "horizontal scrolling off changes the frame"
    );
    let offset_after_track_click = |policy: TrackClick| {
        let mut tall = DataTable::try_new(
            WidgetId::of("scenario.table.tall"),
            vec![Column::new("n", Constraint::Length(6))],
            (0..200).map(|i| vec![Cell::new(i.to_string())]).collect(),
        )
        .expect("rectangular rows")
        .track_click(policy);
        let small = Rect::new(0, 0, 30, 12);
        let drawn = paint_hits(small, |a, b, ctx| tall.render(a, b, ctx, bg));
        let bar = drawn
            .hits
            .regions()
            .iter()
            .find(|r| scrollbar::is_bar_of(r.id, tall.id))
            .map(|r| (r.id, r.area))
            .expect("a vertical scrollbar");
        let at = Position::new(bar.1.x, bar.1.bottom() - 1);
        let press = PointerEvent::new(&drawn.hits, Phase::Press, bar.0, at, 1, KeyModifiers::NONE);
        let _ = MouseTarget::pointer(&mut tall, &press);
        tall.scroll.offset
    };
    assert!(
        offset_after_track_click(TrackClick::Jump) > offset_after_track_click(TrackClick::Page),
        "jump lands under the click, page advances one screen"
    );
    let mut empty = DataTable::try_new(
        WidgetId::of("scenario.table.empty"),
        table_columns(),
        vec![],
    )
    .expect("an empty table")
    .empty_text("No rows to show");
    assert!(empty.is_empty());
    let blank = paint(area, |a, b, ctx| empty.render(a, b, ctx, bg));
    assert!(text_of(&blank).contains("No rows to show"));
    let mut custom = DataTable::try_new(
        WidgetId::of("scenario.table.custom"),
        table_columns(),
        vec![],
    )
    .expect("an empty table")
    .empty(EmptyState::new("Nothing queued"));
    let blank = paint(area, |a, b, ctx| custom.render(a, b, ctx, bg));
    assert!(text_of(&blank).contains("Nothing queued"));
}
