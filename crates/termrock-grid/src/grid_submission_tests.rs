use std::cell::Cell;
use std::rc::Rc;

use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use super::*;
use crate::grid::FinishEdit;
use crate::grid::{
    CellEditResult, CellKind, CellValue, ColumnSpec, CommittedRow, GridAction, GridCmd,
    GridDataError, GridEditError, GridRows, PendingChanges, RowTotal, default_validator,
};
use termrock_core::event::{Key, Outcome};
use termrock_core::id::WidgetId;

fn text(s: &str) -> CellValue {
    CellValue::Text(s.into())
}

fn page(n: usize) -> GridRows {
    GridRows {
        rows: (0..n)
            .map(|i| vec![text(&format!("name{i}")), text(&format!("note{i}"))])
            .collect(),
        total: RowTotal::Exact(n),
        more: false,
    }
}

fn grid(n: usize) -> DataGrid {
    let mut g = DataGrid::new(
        WidgetId::of("issue11"),
        vec![
            ColumnSpec::new("name", CellKind::Text),
            ColumnSpec::new("note", CellKind::Text),
        ],
    );
    g.try_set_rows(page(n)).unwrap();
    g
}

fn edited(n: usize) -> DataGrid {
    let mut g = grid(n);
    g.record_cell(0, 0, text("edited")).unwrap();
    g
}

fn press(g: &mut DataGrid, code: KeyCode, mods: KeyModifiers) -> (Outcome, Option<GridAction>) {
    let (outcome, actions) = g.on_key(&Key { code, mods }).into_parts();
    (outcome, actions.into_iter().next())
}

fn read_only(action: Option<GridAction>) -> bool {
    matches!(action, Some(GridAction::ReadOnly { .. }))
}

fn lock(g: &mut DataGrid) -> (GridChangeSet, GridCommitToken) {
    let snapshot = g.snapshot_changes();
    let token = g.begin_submission(snapshot.clone()).unwrap();
    (snapshot, token)
}

fn state(g: &DataGrid) -> (Vec<Vec<CellValue>>, PendingChanges, GridRevision, bool) {
    (
        g.rows().to_vec(),
        g.pending().clone(),
        g.revision(),
        g.submission_pending(),
    )
}

#[test]
fn issue11_cross_grid_snapshot_token_and_clone_rejected() {
    let mut a = edited(2);
    let mut b = edited(2);
    assert_eq!(a.revision(), b.revision(), "identical revisions");
    assert_ne!(a.instance_id(), b.instance_id());

    let snapshot_a = a.snapshot_changes();
    let before_b = state(&b);
    assert_eq!(
        b.begin_submission(snapshot_a.clone()),
        Err(GridCommitError::WrongOrigin)
    );
    assert_eq!(state(&b), before_b, "a refused admission changes nothing");

    let token_a = a.begin_submission(snapshot_a.clone()).unwrap();
    let (snapshot_b, token_b) = lock(&mut b);
    assert_eq!(token_a.revision(), token_b.revision());
    assert_ne!(token_a.origin(), token_b.origin());

    let locked_a = state(&a);
    assert_eq!(
        b.complete_submission(token_a, Ok(snapshot_a.identity_receipt())),
        Err(GridCommitError::WrongOrigin)
    );
    assert_eq!(
        a.complete_submission(token_b, Ok(snapshot_b.identity_receipt())),
        Err(GridCommitError::WrongOrigin)
    );
    assert_eq!(
        a.complete_submission(token_a, Ok(snapshot_b.identity_receipt())),
        Err(GridCommitError::WrongOrigin),
        "a receipt for another grid is refused even with the right token"
    );
    assert_eq!(
        a.cancel_submission(token_b),
        Err(GridCommitError::WrongOrigin)
    );
    assert_eq!(state(&a), locked_a, "nothing moved");
    assert!(b.submission_pending(), "b kept its own lock");

    let mut clone = a.clone();
    assert!(!clone.submission_pending(), "a clone starts unlocked");
    assert_ne!(clone.instance_id(), a.instance_id());
    assert_eq!(
        clone.cancel_submission(token_a),
        Err(GridCommitError::WrongOrigin)
    );
    assert_eq!(
        clone.begin_submission(snapshot_a.clone()),
        Err(GridCommitError::WrongOrigin)
    );
    assert_eq!(
        clone.complete_submission(token_a, Ok(snapshot_a.identity_receipt())),
        Err(GridCommitError::WrongOrigin)
    );
    let (own, own_token) = lock(&mut clone);
    assert_eq!(own_token.revision(), token_a.revision());
    assert_eq!(
        a.cancel_submission(own_token),
        Err(GridCommitError::WrongOrigin),
        "the clone's token cannot release the original"
    );
    assert!(a.submission_pending());
    clone
        .complete_submission(own_token, Ok(own.identity_receipt()))
        .unwrap();
    assert!(a.submission_pending(), "the clone's save did not touch a");
    assert_eq!(a.pending().total(), 1);
}

#[test]
fn issue11_review_locks_exact_draft() {
    let mut g = edited(3);
    g.insert_row().unwrap();
    g.toggle_delete(2).unwrap();
    let (snapshot, token) = lock(&mut g);
    let locked = state(&g);

    assert_eq!(g.record_cell(1, 0, text("x")), Err(GridEditError::Busy));
    assert_eq!(g.try_edit_text(1, 0, "x"), Err(GridEditError::Busy));
    assert_eq!(g.toggle_delete(1), Err(GridEditError::Busy));
    assert_eq!(g.insert_row(), Err(GridEditError::Busy));
    assert_eq!(g.duplicate_row(), Err(GridEditError::Busy));
    assert_eq!(g.undo(), Err(GridEditError::Busy));
    assert_eq!(g.discard(), Err(GridEditError::Busy));
    assert_eq!(g.try_set_rows(page(1)), Err(GridDataError::Busy));
    assert_eq!(g.try_append_rows(page(1)), Err(GridDataError::Busy));
    assert_eq!(
        g.try_set_rows_keyed(page(1), vec![ItemKey::num(7)]),
        Err(GridDataError::Busy)
    );
    assert_eq!(
        g.try_set_model_keyed(
            vec![ColumnSpec::new("only", CellKind::Text)],
            page(1),
            vec![ItemKey::num(7)]
        ),
        Err(GridDataError::Busy)
    );
    assert_eq!(
        g.try_set_model(vec![ColumnSpec::new("only", CellKind::Text)], page(1)),
        Err(GridDataError::Busy)
    );
    assert_eq!(g.finish_edit_session(), Ok(FinishEdit::NoSession));

    for (code, mods) in [
        (KeyCode::Enter, KeyModifiers::NONE),
        (KeyCode::F(2), KeyModifiers::NONE),
        (KeyCode::Delete, KeyModifiers::NONE),
        (KeyCode::Char('-'), KeyModifiers::NONE),
        (KeyCode::Char('+'), KeyModifiers::NONE),
        (KeyCode::Char('d'), KeyModifiers::CONTROL),
        (KeyCode::Char('u'), KeyModifiers::NONE),
        (KeyCode::Char('s'), KeyModifiers::NONE),
    ] {
        let (_, action) = press(&mut g, code, mods);
        assert!(
            read_only(action.clone()),
            "{code:?} is refused with a reason, got {action:?}"
        );
    }
    assert_eq!(
        g.run(GridCmd::InsertRow).1.map(|a| read_only(Some(a))),
        Some(true)
    );
    assert!(!g.is_editing());
    assert_eq!(state(&g), locked, "no mutation reached the data");
    assert_eq!(g.sort, None, "a refused remote sort left the marker alone");
    assert_eq!(
        g.read_only_reason_at_cursor().as_deref(),
        Some("Saving: edits are locked")
    );
    assert!(!g.cell_editable());
    assert_eq!(
        g.begin_submission(snapshot.clone()),
        Err(GridCommitError::Busy),
        "one submission at a time"
    );

    let (_, moved) = press(&mut g, KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(moved, None);
    assert_eq!(g.cursor.0, 2, "navigation still works");
    let (_, copy) = press(&mut g, KeyCode::Char('y'), KeyModifiers::NONE);
    assert!(
        matches!(copy, Some(GridAction::Copy(_))),
        "copy still works"
    );

    g.cancel_submission(token).unwrap();
    assert!(!g.submission_pending());
    assert_eq!(g.pending(), &locked.1, "the draft survived");
    assert_eq!(
        g.record_cell(1, 0, text("after")),
        Ok(CellEditResult::Changed)
    );
    assert_eq!(g.snapshot_changes().counts().0, 2);
}

#[test]
fn issue278_try_set_model_refuses_while_locked() {
    let mut g = edited(2);
    let _held = lock(&mut g);
    let locked = state(&g);
    let columns = vec![ColumnSpec::new("only", CellKind::Text)];

    // page(1) has two cells per row, so the shape check alone would fail with
    // `Shape`; the lock must win before it.
    assert_eq!(
        g.try_set_model(columns.clone(), page(1)),
        Err(GridDataError::Busy)
    );
    assert_eq!(
        state(&g),
        locked,
        "a refused model swap left the grid alone"
    );
    assert_eq!(g.columns.len(), 2);
}

#[test]
fn issue11_begin_submission_checks_the_capture() {
    let mut g = grid(2);
    assert_eq!(
        g.begin_submission(g.snapshot_changes()),
        Err(GridCommitError::Empty)
    );

    g.record_cell(0, 0, text("one")).unwrap();
    let stale = g.snapshot_changes();
    g.record_cell(1, 0, text("two")).unwrap();
    assert_eq!(
        g.begin_submission(stale),
        Err(GridCommitError::StaleRevision),
        "a capture older than the grid's revision is refused"
    );
    assert!(!g.submission_pending());

    g.cursor = (0, 0);
    let (_, action) = press(&mut g, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(action, None);
    assert!(g.is_editing());
    let revision = g.revision();
    assert_eq!(
        g.begin_submission(g.snapshot_changes()),
        Err(GridCommitError::Editing),
        "an open session is never cancelled or committed by the lock"
    );
    assert!(g.is_editing(), "the session survived");
    assert_eq!(g.revision(), revision);
    assert!(g.cancel_edit());

    let mut nan = grid(1);
    nan.columns[0] = ColumnSpec::new("n", CellKind::Number);
    nan.try_set_rows(GridRows {
        rows: vec![vec![CellValue::Num(1.0), text("x")]],
        total: RowTotal::Exact(1),
        more: false,
    })
    .unwrap();
    nan.record_cell(0, 0, CellValue::Num(f64::NAN)).unwrap();
    let snapshot = nan.snapshot_changes();
    assert_ne!(snapshot, snapshot.clone(), "derived equality rejects NaN");
    nan.begin_submission(snapshot)
        .expect("the bit-exact capture comparison admits an identical NaN payload");
}

#[test]
fn issue32_structural_cancel_keeps_staged_edits() {
    let mut g = grid(2);
    g.record_cell(0, 0, text("staged")).unwrap();
    let (staged, _, _) = g.pending().cells().next().unwrap();

    g.insert_row().unwrap();
    let inserted = g.cursor.0;
    g.cursor = (0, 1);
    let (_, action) = press(&mut g, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(action, None);
    assert!(g.is_editing());

    g.toggle_delete(inserted).unwrap();
    assert!(
        !g.is_editing(),
        "the structural removal cancels the session"
    );
    assert_eq!(
        g.pending().value(staged, 0),
        Some(&text("staged")),
        "a structural cancel keeps the staged pending edit"
    );
    assert_eq!(g.value(0, 0), &text("staged"));
}

#[test]
fn issue11_save_finishes_live_editor_once() {
    let parsed = Rc::new(Cell::new(0));
    let seen = parsed.clone();
    let mut g = grid(2).validator(move |spec, text| {
        seen.set(seen.get() + 1);
        default_validator(spec, text)
    });
    g.cursor = (0, 0);
    press(&mut g, KeyCode::Enter, KeyModifiers::NONE);
    for c in "!".chars() {
        press(&mut g, KeyCode::Char(c), KeyModifiers::NONE);
    }
    assert_eq!(parsed.get(), 0, "typing alone does not reach the parser");
    assert_eq!(
        g.begin_submission(g.snapshot_changes()),
        Err(GridCommitError::Editing)
    );
    assert_eq!(parsed.get(), 0);

    let finished = g.finish_edit_session();
    assert!(matches!(
        finished,
        Ok(FinishEdit::Accepted {
            result: CellEditResult::Changed,
            ..
        })
    ));
    assert_eq!(parsed.get(), 1, "one parser call");
    let (snapshot, _) = lock(&mut g);
    assert_eq!(parsed.get(), 1, "the lock parses nothing");
    assert_eq!(
        snapshot.updated()[0].cells()[0].after(),
        &text("name0!"),
        "the accepted value is in the capture"
    );

    let mut bad = grid(1).validator(|_, text| {
        if text.ends_with('?') {
            Err("no question marks".to_owned())
        } else {
            Ok(CellValue::Text(text.to_owned()))
        }
    });
    bad.cursor = (0, 0);
    press(&mut bad, KeyCode::Enter, KeyModifiers::NONE);
    press(&mut bad, KeyCode::Char('?'), KeyModifiers::NONE);
    assert!(matches!(
        bad.finish_edit_session(),
        Err(GridEditError::Validation(_))
    ));
    assert!(bad.is_editing(), "a rejected value keeps the input");
    assert!(bad.pending().is_empty());
    assert_eq!(
        bad.begin_submission(bad.snapshot_changes()),
        Err(GridCommitError::Editing)
    );
}

fn receipt_rows(snapshot: &GridChangeSet) -> Vec<CommittedRow> {
    snapshot.identity_receipt().rows
}

fn assert_refused(
    g: &mut DataGrid,
    token: GridCommitToken,
    snapshot: &GridChangeSet,
    rows: Vec<CommittedRow>,
    expected: GridCommitError,
) {
    let before = state(g);
    let receipt = GridCommitReceipt::new(snapshot, rows);
    assert_eq!(
        g.validate_submission_receipt(token, &receipt),
        Err(expected.clone())
    );
    assert_eq!(g.complete_submission(token, Ok(receipt)), Err(expected));
    assert_eq!(state(g), before, "an invalid receipt changed nothing");
}

#[test]
fn issue11_receipt_validation_is_atomic() {
    let mut g = grid(3);
    g.record_cell(0, 0, text("edited")).unwrap();
    g.insert_row().unwrap();
    g.record_cell(3, 0, text("new-a")).unwrap();
    g.insert_row().unwrap();
    g.record_cell(4, 0, text("new-b")).unwrap();
    g.toggle_delete(2).unwrap();
    let keys: Vec<ItemKey> = (0..5).map(|i| g.row_key(i).unwrap()).collect();
    let (snapshot, token) = lock(&mut g);
    let good = receipt_rows(&snapshot);
    assert_eq!(good.len(), 3, "one updated and two inserted rows");

    assert_refused(
        &mut g,
        token,
        &snapshot,
        good[1..].to_vec(),
        GridCommitError::InvalidReceipt(ReceiptFault::Missing(keys[0])),
    );
    let mut extra = good.clone();
    extra.push(CommittedRow {
        key: keys[1],
        committed_key: keys[1],
        values: vec![text("n"), text("n")],
    });
    assert_refused(
        &mut g,
        token,
        &snapshot,
        extra,
        GridCommitError::InvalidReceipt(ReceiptFault::Unexpected(keys[1])),
    );
    let mut deleted = good.clone();
    deleted.push(CommittedRow {
        key: keys[2],
        committed_key: keys[2],
        values: vec![text("n"), text("n")],
    });
    assert_refused(
        &mut g,
        token,
        &snapshot,
        deleted,
        GridCommitError::InvalidReceipt(ReceiptFault::Unexpected(keys[2])),
    );
    let mut duplicate = good.clone();
    duplicate.push(good[0].clone());
    assert_refused(
        &mut g,
        token,
        &snapshot,
        duplicate,
        GridCommitError::InvalidReceipt(ReceiptFault::Duplicate(keys[0])),
    );
    let mut narrow = good.clone();
    narrow[1].values.pop();
    assert_refused(
        &mut g,
        token,
        &snapshot,
        narrow,
        GridCommitError::InvalidReceipt(ReceiptFault::Width {
            key: keys[3],
            expected: 2,
            actual: 1,
        }),
    );
    let mut moved = good.clone();
    moved[0].committed_key = ItemKey::num(900);
    assert_refused(
        &mut g,
        token,
        &snapshot,
        moved,
        GridCommitError::InvalidReceipt(ReceiptFault::UpdatedKeyChanged(keys[0])),
    );
    let mut collide = good.clone();
    collide[1].committed_key = keys[1];
    assert_refused(
        &mut g,
        token,
        &snapshot,
        collide,
        GridCommitError::InvalidReceipt(ReceiptFault::KeyCollision(keys[1])),
    );
    let mut same = good.clone();
    same[1].committed_key = ItemKey::num(900);
    same[2].committed_key = ItemKey::num(900);
    assert_refused(
        &mut g,
        token,
        &snapshot,
        same,
        GridCommitError::InvalidReceipt(ReceiptFault::KeyCollision(ItemKey::num(900))),
    );

    let mut other = edited(1);
    let (other_snapshot, _) = lock(&mut other);
    let before = state(&g);
    let foreign = GridCommitReceipt::new(&other_snapshot, good.clone());
    assert_eq!(
        g.complete_submission(token, Ok(foreign)),
        Err(GridCommitError::WrongOrigin)
    );
    assert_eq!(state(&g), before);

    let mut accepted = good.clone();
    accepted[1].committed_key = ItemKey::num(901);
    accepted[1].values = vec![text("new-a"), text("server-note")];
    accepted[2].committed_key = ItemKey::num(902);
    let revision = g.revision();
    g.complete_submission(token, Ok(GridCommitReceipt::new(&snapshot, accepted)))
        .unwrap();

    assert!(!g.submission_pending());
    assert!(g.pending().is_empty());
    assert_eq!(g.undo().unwrap(), None, "the history was cleared");
    assert_eq!(g.len(), 4, "the deleted row left");
    assert_eq!(g.revision().source, revision.source + 1);
    assert_eq!(g.value(0, 0), &text("edited"));
    assert_eq!(g.row_key(2), Some(ItemKey::num(901)));
    assert_eq!(g.row_key(3), Some(ItemKey::num(902)));
    assert_eq!(g.value(2, 1), &text("server-note"));
    assert_eq!(g.source_index(keys[3]), None, "the provisional key is gone");
    assert_eq!(g.source_index(keys[2]), None, "the deleted row is gone");
    assert!(g.snapshot_changes().is_empty());
    assert_eq!(
        g.record_cell(1, 0, text("next")),
        Ok(CellEditResult::Changed)
    );
}

#[test]
fn issue11_backend_failure_unlocks_and_keeps_the_draft() {
    let mut g = edited(3);
    g.record_cell(2, 1, text("second")).unwrap();
    let draft = g.pending().clone();
    let (snapshot, token) = lock(&mut g);
    let key = snapshot.updated()[1].key();

    g.complete_submission(
        token,
        Err(GridCommitFailure {
            key: Some(key),
            message: "rejected".into(),
        }),
    )
    .unwrap();
    assert!(!g.submission_pending());
    assert_eq!(g.pending(), &draft, "the draft was retained");
    assert_eq!(g.row_errors.get(&2).map(String::as_str), Some("rejected"));
    assert_eq!(g.commit_failure(), None);
    assert!(g.begin_submission(g.snapshot_changes()).is_ok());

    let mut g = edited(1);
    let (_, token) = lock(&mut g);
    g.complete_submission(
        token,
        Err(GridCommitFailure {
            key: None,
            message: "connection lost".into(),
        }),
    )
    .unwrap();
    assert_eq!(g.commit_failure(), Some("connection lost"));
    assert!(g.row_errors.is_empty());
    assert_eq!(g.pending().total(), 1);
}

#[test]
fn issue11_completed_token_is_stale() {
    let mut g = edited(2);
    let (snapshot, first) = lock(&mut g);
    g.complete_submission(first, Ok(snapshot.identity_receipt()))
        .unwrap();
    assert_eq!(
        g.complete_submission(first, Ok(snapshot.identity_receipt())),
        Err(GridCommitError::StaleToken),
        "a repeated completion has no second effect"
    );
    assert_eq!(g.cancel_submission(first), Err(GridCommitError::StaleToken));
    assert!(g.snapshot_changes().is_empty());

    g.record_cell(1, 0, text("again")).unwrap();
    let (_, second) = lock(&mut g);
    assert_ne!(first, second, "a new submission has a new token");
    assert_eq!(g.cancel_submission(first), Err(GridCommitError::StaleToken));
    assert!(g.submission_pending(), "the old token did not release it");
    g.cancel_submission(second).unwrap();
}

#[test]
fn issue11_receipt_removes_the_deleted_row_when_an_insert_reuses_its_key() {
    let mut g = grid(3);
    let keys: Vec<ItemKey> = (0..3).map(|i| g.row_key(i).unwrap()).collect();
    g.toggle_delete(1).unwrap();
    g.insert_row().unwrap();
    g.record_cell(3, 0, text("reborn")).unwrap();
    let (snapshot, token) = lock(&mut g);
    let mut rows = receipt_rows(&snapshot);
    assert_eq!(rows.len(), 1, "only the inserted row is in the receipt");
    rows[0].committed_key = keys[1];

    g.complete_submission(token, Ok(GridCommitReceipt::new(&snapshot, rows)))
        .unwrap();

    assert_eq!(g.len(), 3);
    assert_eq!(g.value(0, 0), &text("name0"));
    assert_eq!(g.value(1, 0), &text("name2"), "the deleted row left");
    assert_eq!(g.value(2, 0), &text("reborn"), "the inserted row stayed");
    assert_eq!(g.row_key(2), Some(keys[1]));
    assert_eq!(g.source_index(keys[1]), Some(2));
}

#[test]
fn a_commit_token_names_its_grid_and_an_exhausted_nonce_refuses_the_lock() {
    let mut g = grid(2);
    g.record_cell(0, 0, text("one")).unwrap();
    let id: GridInstanceId = g.instance_id();
    assert_ne!(grid(1).instance_id(), id, "every grid has its own id");

    let token: GridCommitToken = g.begin_submission(g.snapshot_changes()).expect("locks");
    assert_eq!(token.origin(), id);
    assert_eq!(token.revision(), g.revision());
    g.cancel_submission(token)
        .expect("the token unlocks its own grid");
    assert!(!g.submission_pending());

    g.submission.next_nonce = u64::MAX;
    assert_eq!(
        g.begin_submission(g.snapshot_changes()),
        Err(GridCommitError::Exhausted),
        "a nonce that cannot advance never issues a token"
    );
    assert!(
        !g.submission_pending(),
        "the refusal leaves the grid unlocked"
    );
}
