//! The session's context gate, with the paint's relocation taken out of the way.

use super::*;
use crate::search_detail::ResultRowKind;
use termrock_core::id::WidgetId;
use termrock_core::list::{KeyedRowSource, RowSource};

const ROW: u64 = 1;
const FIRST: u64 = 10;
const SECOND: u64 = 11;

/// One row of two chips.
struct Chips(SourceStamp);

impl RowSource for Chips {
    fn len(&self) -> usize {
        1
    }
    fn enabled(&self, _: usize) -> bool {
        true
    }
    fn label(&self, _: usize) -> &str {
        "git"
    }
    fn key(&self, _: usize) -> ItemKey {
        ItemKey::num(ROW)
    }
}

impl KeyedRowSource for Chips {
    fn stamp(&self) -> SourceStamp {
        self.0
    }
    fn source_len(&self) -> usize {
        3
    }
    fn source_key(&self, i: usize) -> ItemKey {
        ItemKey::num([ROW, FIRST, SECOND][i])
    }
}

impl SearchSource for Chips {
    fn row_kind(&self, _: usize) -> ResultRowKind {
        ResultRowKind::Cells
    }
    fn cell_count(&self, _: usize) -> usize {
        2
    }
    fn cell_key(&self, _: usize, cell: usize) -> ItemKey {
        ItemKey::num([FIRST, SECOND][cell])
    }
    fn cell_label(&self, _: usize, _: usize) -> &str {
        "chip"
    }
    fn cell_enabled(&self, _: usize, _: usize) -> bool {
        true
    }
}

fn session_on(chip: u64) -> SearchSession {
    let mut s = SearchSession::new(
        TextInput::new(WidgetId::of("gate.q"), "Search"),
        SearchConfig::default(),
    );
    let stamp = SourceStamp {
        instance: ItemKey::num(900),
        data: 1,
        view: 0,
    };
    let request = s
        .refresh_source(stamp)
        .into_actions()
        .into_iter()
        .find_map(|a| match a {
            SearchAction::QueryRequested(r) => Some(r),
            _ => None,
        })
        .expect("a request");
    let source = Chips(request.token.result_stamp());
    let _ = s.submit(request.token, &source).expect("accepted");
    let _ = s.select(ItemKey::num(chip));
    assert_eq!(s.selected_key(), Some(ItemKey::num(chip)));
    s
}

/// The paint that leaves a chip out also moves the selection off it, which
/// hides the `actionable()` gate from any test that paints. This leaves the
/// cursor on the unplaced chip, so only the gate can refuse.
#[test]
fn begin_context_refuses_a_chip_the_paint_left_out() {
    let chip = ItemKey::num(SECOND);

    let mut placed = session_on(SECOND);
    let _ = placed.begin_context();
    assert_eq!(
        placed.context_intent(),
        Some(chip),
        "premise: a placed chip asks"
    );

    let mut s = session_on(SECOND);
    let slots = &mut s.accepted.as_mut().expect("accepted").slots;
    let at = slots
        .iter()
        .position(|slot| slot.key == chip)
        .expect("slot");
    slots[at].placed = false;
    assert_eq!(
        s.selected_key(),
        Some(chip),
        "premise: the cursor did not move"
    );
    assert!(!s.is_selectable(chip), "premise: the chip is unplaced");

    let reply = s.begin_context();
    assert!(reply.is_consumed(), "the refused key is still consumed");
    assert_eq!(
        s.context_intent(),
        None,
        "nothing is held for an unplaced chip"
    );
}
