//! Keyed collection state: identity survives reorder and filter, and is
//! pruned or cancelled deterministically when a record disappears.

use std::collections::HashSet;

use junie_tui::core::id::ItemKey;
use junie_tui::core::keyed::{KeyedRecord, KeyedSourceError, KeyedState, KeyedVec, Reconcile};
use junie_tui::core::list::{KeyedRowSource, RowSource, SourceStamp};

pub fn k(n: u64) -> ItemKey {
    ItemKey::num(n)
}

/// A provider with an authoritative set, a visible projection and disabled keys.
#[derive(Clone)]
pub struct Fake {
    pub instance: u64,
    pub data: u64,
    pub view: u64,
    pub all: Vec<ItemKey>,
    pub shown: Vec<ItemKey>,
    pub disabled: HashSet<ItemKey>,
    pub names: Vec<String>,
}

impl Fake {
    pub fn new(keys: &[u64]) -> Self {
        let all: Vec<ItemKey> = keys.iter().map(|n| k(*n)).collect();
        Self {
            instance: 1_000,
            data: 0,
            view: 0,
            names: keys.iter().map(|n| format!("row {n}")).collect(),
            shown: all.clone(),
            all,
            disabled: HashSet::new(),
        }
    }

    pub fn show(&mut self, keys: &[u64]) {
        self.shown = keys.iter().map(|n| k(*n)).collect();
        self.view += 1;
    }

    pub fn remove(&mut self, key: u64) {
        self.all.retain(|x| *x != k(key));
        self.shown.retain(|x| *x != k(key));
        self.data += 1;
        self.view += 1;
    }

    pub fn disable(&mut self, key: u64) {
        self.disabled.insert(k(key));
        self.data += 1;
    }

    fn name_of(&self, key: ItemKey) -> &str {
        let at = self.all.iter().position(|x| *x == key).expect("known");
        &self.names[at]
    }
}

impl RowSource for Fake {
    fn len(&self) -> usize {
        self.shown.len()
    }
    fn enabled(&self, i: usize) -> bool {
        !self.disabled.contains(&self.shown[i])
    }
    fn label(&self, i: usize) -> &str {
        self.name_of(self.shown[i])
    }
    fn key(&self, i: usize) -> ItemKey {
        self.shown[i]
    }
}

impl KeyedRowSource for Fake {
    fn stamp(&self) -> SourceStamp {
        SourceStamp {
            instance: ItemKey::num(self.instance),
            data: self.data,
            view: self.view,
        }
    }
    fn source_len(&self) -> usize {
        self.all.len()
    }
    fn source_key(&self, i: usize) -> ItemKey {
        self.all[i]
    }
}

fn bound(keys: &[u64]) -> (KeyedState, Fake) {
    let source = Fake::new(keys);
    let mut state = KeyedState::new();
    state.reconcile(&source).expect("valid source");
    (state, source)
}

#[test]
fn keyed_reconcile_is_atomic_on_invalid_source() {
    let (mut state, mut source) = bound(&[1, 2, 3]);
    assert!(state.set_cursor(k(2)));
    assert!(state.set_selected(k(3), true));
    let before_stamp = state.stamp();

    let mut dup = source.clone();
    dup.all.push(k(1));
    dup.data += 1;
    assert_eq!(
        state.reconcile(&dup),
        Err(KeyedSourceError::DuplicateAuthoritative { key: k(1) })
    );
    let mut positional = source.clone();
    positional.all[1] = ItemKey::index(9);
    positional.data += 1;
    assert_eq!(
        state.reconcile(&positional),
        Err(KeyedSourceError::PositionalKey {
            index: 1,
            authoritative: true
        })
    );
    let mut dup_visible = source.clone();
    dup_visible.shown = vec![k(1), k(1)];
    dup_visible.view += 1;
    assert_eq!(
        state.reconcile(&dup_visible),
        Err(KeyedSourceError::DuplicateVisible { key: k(1) })
    );
    let mut unknown = source.clone();
    unknown.shown = vec![k(1), k(77)];
    unknown.view += 1;
    assert_eq!(
        state.reconcile(&unknown),
        Err(KeyedSourceError::UnknownVisible { key: k(77) })
    );
    let mut visible_index = source.clone();
    visible_index.shown = vec![ItemKey::index(0)];
    visible_index.view += 1;
    assert_eq!(
        state.reconcile(&visible_index),
        Err(KeyedSourceError::PositionalKey {
            index: 0,
            authoritative: false
        })
    );

    assert_eq!(state.stamp(), before_stamp);
    assert_eq!(state.cursor(), Some(k(2)));
    assert_eq!(state.selected().collect::<Vec<_>>(), vec![k(3)]);
    assert_eq!(state.visible_index(k(3)), Some(2));
    source.show(&[3, 2, 1]);
    assert!(state.reconcile(&source).expect("valid").view_changed);
    assert_eq!(state.visible_index(k(3)), Some(0));
}

#[test]
fn keyed_filter_preserves_hidden_selection() {
    let (mut state, mut source) = bound(&[1, 2, 3, 4]);
    assert!(state.set_selected(k(2), true));
    assert!(state.set_expanded(k(2), true));
    assert!(state.set_anchor(Some(k(2))));
    source.show(&[1, 3]);
    let out = state.reconcile(&source).expect("filter is valid");
    assert!(out.view_changed && !out.data_changed);
    assert_eq!(state.visible_index(k(2)), None);
    assert!(state.is_selected(k(2)) && state.is_expanded(k(2)));
    assert_eq!(state.anchor(), Some(k(2)));

    source.show(&[1, 2, 3, 4]);
    state.reconcile(&source).expect("filter cleared");
    assert!(state.is_selected(k(2)) && state.is_expanded(k(2)));
    assert_eq!(state.visible_index(k(2)), Some(1));

    source.show(&[1, 3]);
    source.remove(2);
    state.reconcile(&source).expect("deleted");
    assert!(!state.is_selected(k(2)) && !state.is_expanded(k(2)));
    assert_eq!(state.anchor(), None);
    assert!(
        !state.set_selected(k(2), true),
        "deleted keys cannot be selected"
    );
}

#[test]
fn keyed_cursor_fallback_is_deterministic() {
    let (mut state, mut source) = bound(&[1, 2, 3, 4, 5]);
    assert!(state.set_cursor(k(3)));
    source.remove(3);
    let out = state.reconcile(&source).expect("valid");
    assert!(out.cursor_changed);
    assert_eq!(state.cursor(), Some(k(4)), "the row that took its place");

    source.disable(4);
    source.disable(5);
    state.reconcile(&source).expect("valid");
    assert_eq!(
        state.cursor(),
        Some(k(2)),
        "predecessor when no successor is enabled"
    );

    for key in [1, 2] {
        source.disable(key);
    }
    state.reconcile(&source).expect("valid");
    assert_eq!(state.cursor(), None);
    assert!(
        !state.set_cursor(k(1)),
        "disabled rows cannot take the cursor"
    );
    assert!(
        !state.set_cursor(k(99)),
        "absent rows cannot take the cursor"
    );

    source.disabled.clear();
    source.data += 1;
    state.reconcile(&source).expect("valid");
    assert_eq!(
        state.cursor(),
        Some(k(1)),
        "a fresh cursor starts on the first enabled row"
    );
}

#[test]
fn keyed_press_cannot_retarget_after_reorder() {
    let (mut state, mut source) = bound(&[1, 2, 3]);
    assert!(state.capture_press(k(1), 7));
    source.show(&[2, 1, 3]);
    let out = state.reconcile(&source).expect("valid");
    assert!(out.pointer_cancelled);
    assert!(!state.release_press(k(1), 7));
    assert!(
        !state.release_press(k(2), 7),
        "the row now at the old position is not the target"
    );

    assert!(state.capture_press(k(2), 7));
    source.remove(2);
    assert!(state.reconcile(&source).expect("valid").pointer_cancelled);
    assert!(!state.release_press(k(2), 7));
}

#[test]
fn keyed_edit_cancels_on_changed_source() {
    let (mut state, mut source) = bound(&[1, 2, 3]);
    assert!(state.begin_edit(k(2)));
    source.show(&[2, 3, 1]);
    let out = state.reconcile(&source).expect("valid");
    assert!(!out.edit_cancelled, "reordering keeps the edited record");
    assert_eq!(state.edit_key(), Some(k(2)));

    source.data += 1;
    assert!(
        state.reconcile(&source).expect("valid").edit_cancelled,
        "payload revision moved"
    );
    assert_eq!(state.edit_key(), None);

    assert!(state.begin_edit(k(3)));
    source.show(&[2, 1]);
    assert!(
        state.reconcile(&source).expect("valid").edit_cancelled,
        "hidden"
    );

    source.show(&[2, 3, 1]);
    state.reconcile(&source).expect("valid");
    assert!(state.begin_edit(k(3)));
    source.remove(3);
    assert!(
        state.reconcile(&source).expect("valid").edit_cancelled,
        "deleted"
    );

    assert!(state.begin_edit(k(1)));
    source.disable(1);
    assert!(
        state.reconcile(&source).expect("valid").edit_cancelled,
        "disabled"
    );
    assert!(!state.begin_edit(k(1)));
}

#[test]
fn keyed_new_provider_with_equal_counters_resets_state() {
    let (mut state, source) = bound(&[1, 2, 3]);
    assert!(state.set_cursor(k(2)));
    assert!(state.set_selected(k(3), true));
    assert!(state.set_expanded(k(1), true));
    assert!(state.begin_edit(k(2)));

    let mut other = source.clone();
    other.instance += 1;
    let out = state.reconcile(&other).expect("valid");
    assert!(out.data_changed && out.edit_cancelled);
    assert_eq!(state.selected().count(), 0);
    assert_eq!(state.expanded().count(), 0);
    assert_eq!(state.cursor(), Some(k(1)));

    let a = KeyedVec::<Row>::new();
    let b = KeyedVec::<Row>::new();
    assert_ne!(a.stamp().instance, b.stamp().instance);
    let mut c = KeyedVec::try_from_rows([(k(1), Row::new("x"))]).expect("valid");
    let d = c.clone();
    assert_ne!(c.stamp().instance, d.stamp().instance);
    let before = c.stamp();
    c.update(k(1), |row| row.name = "y".into());
    assert_eq!(c.stamp().instance, before.instance);
    assert!(c.stamp().data > before.data);
}

#[test]
fn keyed_failed_release_consumes_capture() {
    let (mut state, _) = bound(&[1, 2]);
    assert!(state.capture_press(k(1), 3));
    assert!(!state.release_press(k(2), 3), "wrong key");
    assert!(
        !state.release_press(k(1), 3),
        "the failed attempt consumed the capture"
    );

    assert!(state.capture_press(k(1), 3));
    assert!(!state.release_press(k(1), 4), "wrong layout generation");
    assert!(!state.release_press(k(1), 3));

    assert!(state.capture_press(k(1), 3));
    assert!(state.release_press(k(1), 3));
    assert!(
        !state.capture_press(k(99), 3),
        "absent keys cannot be captured"
    );
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub name: String,
    pub off: bool,
}

impl Row {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            off: false,
        }
    }
}

impl KeyedRecord for Row {
    fn label(&self) -> &str {
        &self.name
    }
    fn enabled(&self) -> bool {
        !self.off
    }
}

#[test]
fn keyed_vec_serves_keyed_records_and_a_state_reconciles_them() {
    fn label_of<T: KeyedRecord>(record: &T) -> &str {
        record.label()
    }
    let mut store = KeyedVec::<Row>::new();
    assert!(store.is_empty());
    store
        .replace([(k(1), Row::new("a")), (k(2), Row::new("b"))])
        .expect("unique keys");
    assert!(!store.is_empty());
    assert_eq!(store.get(k(2)).map(label_of), Some("b"));
    assert_eq!(store.value_at(0).map(label_of), Some("a"));
    assert!(store.get(k(9)).is_none() && store.value_at(2).is_none());
    assert_eq!(
        store.replace([(k(3), Row::new("c")), (k(3), Row::new("dup"))]),
        Err(KeyedSourceError::DuplicateAuthoritative { key: k(3) })
    );
    assert_eq!(store.len(), 2, "a refused replace keeps the rows");

    let mut state = KeyedState::new();
    assert_eq!(
        state.visible_len(),
        0,
        "nothing is visible before a reconcile"
    );
    let first: Reconcile = state.reconcile(&store).expect("valid source");
    assert!(first.data_changed && first.view_changed, "{first:?}");
    assert_eq!(state.visible_len(), 2);
    let quiet = state.reconcile(&store).expect("valid source");
    assert_eq!(
        quiet,
        Reconcile::default(),
        "an unchanged stamp reports nothing"
    );

    assert!(state.begin_edit(k(1)));
    assert!(state.cancel_edit(), "an open edit is cancelled");
    assert!(
        !state.cancel_edit(),
        "a second cancel has nothing to cancel"
    );
    assert_eq!(state.edit_key(), None);
}

#[test]
fn row_selection_replace_swaps_the_rows_and_the_anchor_in_one_step() {
    use junie_tui::core::selection::RowSelection;
    let mut selection = RowSelection::new();
    selection.replace([4, 2], Some(2));
    assert_eq!(selection.len(), 2);
    assert!(selection.contains(4) && selection.contains(2));
    assert_eq!(selection.anchor(), Some(2));
    selection.replace([7], None);
    assert_eq!(selection.to_vec(), [7], "the earlier rows are gone");
    assert_eq!(selection.anchor(), None);
}

#[test]
fn keyed_vec_owns_its_revisions() {
    let mut store = KeyedVec::<Row>::new();
    let s0 = store.stamp();
    store.push(k(1), Row::new("a")).expect("new");
    store.push(k(2), Row::new("b")).expect("new");
    assert_eq!(
        store.push(k(2), Row::new("dup")),
        Err(KeyedSourceError::DuplicateAuthoritative { key: k(2) })
    );
    assert_eq!(
        store.push(ItemKey::index(3), Row::new("idx")),
        Err(KeyedSourceError::PositionalKey {
            index: 2,
            authoritative: true
        })
    );
    assert_eq!(store.len(), 2);
    let s1 = store.stamp();
    assert!(s1.data > s0.data && s1.view > s0.view);

    store.insert(0, k(3), Row::new("c")).expect("new");
    assert_eq!(store.position(k(3)), Some(0));
    assert_eq!(store.position(k(1)), Some(1));
    assert!(store.move_to(k(3), 2));
    assert_eq!(store.key_at(0), Some(k(1)));
    assert_eq!(store.key_at(2), Some(k(3)));
    assert_eq!(store.remove(k(1)).map(|r| r.name), Some("a".to_owned()));
    assert_eq!(store.position(k(3)), Some(1));

    let before = store.stamp();
    let same = store
        .iter()
        .map(|(key, row)| (key, row.clone()))
        .collect::<Vec<_>>();
    assert!(!store.replace_if_changed(same).expect("valid"));
    assert_eq!(store.stamp(), before);
    assert!(
        store
            .replace_if_changed([(k(9), Row::new("n"))])
            .expect("valid")
    );
    assert_eq!(store.stamp().instance, before.instance);
    assert_eq!(store.len(), 1);
}

mod widgets {
    use super::*;
    use crate::common::{chord, key, row};
    use junie_tui::core::event::Outcome;
    use junie_tui::core::focus::{BlurAction, BlurTarget, FocusRing};
    use junie_tui::core::hit::HitRegistry;
    use junie_tui::core::id::{Part, WidgetId};
    use junie_tui::core::intent::Phase;
    use junie_tui::core::pointer::PointerEvent;
    use junie_tui::core::selection::SelectionMode;
    use junie_tui::theme::{ColorLevel, Theme};
    use junie_tui::ui::ctx::{Interaction, RenderCtx};
    use junie_tui::widgets::list::{ListAction, ListBox, ListItem, SelectMode};
    use junie_tui::widgets::table::{
        Cell, Column, DataTable, KeyedTable, TableAction, TableEditSource, TableSource,
    };
    use ratatui::buffer::Buffer;
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};
    use ratatui::layout::{Constraint, Position, Rect};

    const AREA: Rect = Rect::new(0, 0, 40, 10);

    pub struct Frame {
        pub buf: Buffer,
        pub hits: HitRegistry,
    }

    impl Frame {
        pub fn text(&self) -> String {
            (0..self.buf.area.height)
                .map(|y| row(&self.buf, y).trim_end().to_owned())
                .collect::<Vec<_>>()
                .join("\n")
        }

        pub fn id_at(&self, owner: WidgetId, part: Part, pos: Position) -> WidgetId {
            self.hits
                .regions()
                .iter()
                .rev()
                .find(|r| r.owner == owner && r.part.part == part && r.area.contains(pos))
                .map(|r| r.id)
                .expect("a hit region at the position")
        }

        pub fn event(&self, phase: Phase, id: WidgetId, pos: Position) -> PointerEvent {
            PointerEvent::new(&self.hits, phase, id, pos, 1, KeyModifiers::NONE)
        }
    }

    pub fn paint(f: impl FnOnce(&mut Buffer, &mut RenderCtx)) -> Frame {
        let theme = Theme::for_level(ColorLevel::TrueColor);
        let mut buf = Buffer::empty(AREA);
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        {
            let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
            f(&mut buf, &mut ctx);
        }
        Frame { buf, hits }
    }

    fn items(names: &[(u64, &str)]) -> KeyedVec<ListItem> {
        KeyedVec::try_from_rows(names.iter().map(|(n, name)| (k(*n), ListItem::new(name))))
            .expect("unique keys")
    }

    fn list(mode: SelectMode) -> ListBox {
        ListBox::new(WidgetId::of("kl"), vec![], mode)
    }

    fn draw_list(list: &mut ListBox, source: &KeyedVec<ListItem>) -> Frame {
        paint(|buf, ctx| {
            list.render_source(AREA, source, buf, ctx, ctx.theme.canvas)
                .expect("valid source");
        })
    }

    #[test]
    fn keyed_list_selection_follows_keys_across_reorder() {
        let mut store = items(&[(1, "alpha"), (2, "bravo"), (3, "charlie")]);
        let mut list = list(SelectMode::Multi);
        draw_list(&mut list, &store);
        let _ = list
            .on_key_source(&key(KeyCode::Down), &store)
            .expect("valid");
        let out = list
            .on_key_source(&key(KeyCode::Char(' ')), &store)
            .expect("valid");
        assert!(out.actions().any(|a| *a == ListAction::SelectionChanged));
        assert_eq!(list.keyed().selected().collect::<Vec<_>>(), vec![k(2)]);

        store.move_to(k(2), 2);
        draw_list(&mut list, &store);
        assert_eq!(list.keyed().cursor(), Some(k(2)));
        assert_eq!(list.core.cursor, 2, "the view index follows the key");
        assert!(list.is_checked(2));
        assert!(!list.is_checked(1));

        store.insert(0, k(9), ListItem::new("zulu")).expect("new");
        draw_list(&mut list, &store);
        assert_eq!(list.core.cursor, 3);
        assert!(list.is_checked(3));
        store.remove(k(2));
        draw_list(&mut list, &store);
        assert_eq!(
            list.keyed().selected().count(),
            0,
            "deleted keys are pruned"
        );
        assert_eq!(list.keyed().cursor(), Some(k(3)));
    }

    #[test]
    fn keyed_list_reports_activation_by_key() {
        let store = items(&[(10, "one"), (20, "two")]);
        let mut list = list(SelectMode::Single);
        draw_list(&mut list, &store);
        let _ = list
            .on_key_source(&key(KeyCode::Down), &store)
            .expect("valid");
        let out = list
            .on_key_source(&key(KeyCode::Enter), &store)
            .expect("valid");
        assert_eq!(
            out.actions().copied().collect::<Vec<_>>(),
            vec![ListAction::Activated(k(20)), ListAction::SelectionChanged]
        );
        assert_eq!(list.keyed().selected().collect::<Vec<_>>(), vec![k(20)]);
    }

    #[test]
    fn keyed_list_rejects_invalid_sources_without_touching_state() {
        let mut store = items(&[(1, "a"), (2, "b")]);
        let mut list = list(SelectMode::Multi);
        draw_list(&mut list, &store);
        let _ = list
            .on_key_source(&key(KeyCode::Down), &store)
            .expect("valid");
        let cursor = list.keyed().cursor();

        let mut dup = Fake::new(&[1, 1]);
        dup.data = 3;
        struct Bad(Fake, Vec<ListItem>);
        impl RowSource for Bad {
            fn len(&self) -> usize {
                self.0.len()
            }
            fn label(&self, i: usize) -> &str {
                &self.1[i].label
            }
            fn key(&self, i: usize) -> ItemKey {
                self.0.key(i)
            }
        }
        impl KeyedRowSource for Bad {
            fn stamp(&self) -> SourceStamp {
                self.0.stamp()
            }
            fn source_len(&self) -> usize {
                self.0.source_len()
            }
            fn source_key(&self, i: usize) -> ItemKey {
                self.0.source_key(i)
            }
        }
        impl junie_tui::widgets::list::ListSource for Bad {
            fn item(&self, i: usize) -> junie_tui::widgets::list::ListItemRef<'_> {
                self.1[i].borrowed()
            }
        }
        let bad = Bad(dup, vec![ListItem::new("x"), ListItem::new("y")]);
        let err = list
            .on_key_source(&key(KeyCode::Down), &bad)
            .expect_err("duplicate keys");
        assert_eq!(err, KeyedSourceError::DuplicateAuthoritative { key: k(1) });
        assert_eq!(list.keyed().cursor(), cursor);
        store.remove(k(1));
        draw_list(&mut list, &store);
        assert_eq!(list.keyed().cursor(), Some(k(2)));
    }

    #[test]
    fn keyed_list_press_cannot_retarget_after_reorder() {
        let mut store = items(&[(1, "alpha"), (2, "bravo"), (3, "charlie")]);
        let mut list = list(SelectMode::Single);
        let frame = draw_list(&mut list, &store);
        let at = Position::new(6, 0);
        let id_a = frame.id_at(list.id, Part::ROW, at);
        let _ = list
            .on_pointer_source(&frame.event(Phase::Press, id_a, at), &store)
            .expect("valid");
        assert_eq!(list.keyed().pressed(), Some(k(1)));

        store.move_to(k(1), 2);
        let after = draw_list(&mut list, &store);
        let id_b = after.id_at(list.id, Part::ROW, at);
        assert_ne!(id_a, id_b, "row hits are keyed, not positional");
        let out = list
            .on_pointer_source(&after.event(Phase::Click, id_b, at), &store)
            .expect("valid");
        assert!(
            out.actions().next().is_none(),
            "no activation of the row that moved under the pointer"
        );
        assert!(list.keyed().selected().next().is_none());
        assert_eq!(
            list.keyed().pressed(),
            None,
            "the failed release consumed the capture"
        );
    }

    fn nav_list() -> ListBox {
        list(SelectMode::Single).click(junie_tui::core::selection::ClickPolicy::NAVIGATION)
    }

    fn press_at(list: &mut ListBox, source: &KeyedVec<ListItem>, at: Position) {
        let frame = draw_list(list, source);
        let id = frame.id_at(list.id, Part::ROW, at);
        let _ = list
            .on_pointer_source(&frame.event(Phase::Press, id, at), source)
            .expect("valid");
    }

    fn click_at(list: &mut ListBox, source: &KeyedVec<ListItem>, at: Position) -> usize {
        let frame = draw_list(list, source);
        let id = frame.id_at(list.id, Part::ROW, at);
        list.on_pointer_source(&frame.event(Phase::Click, id, at), source)
            .expect("valid")
            .actions()
            .count()
    }

    #[test]
    fn keyed_list_click_after_a_reorder_is_consumed() {
        let mut store = items(&[(1, "alpha"), (2, "bravo"), (3, "charlie")]);
        let mut list = nav_list();
        let at = Position::new(6, 0);
        press_at(&mut list, &store, at);
        assert_eq!(list.keyed().pressed(), Some(k(1)));

        // The pressed row moves away; another row now sits under the pointer.
        store.move_to(k(1), 2);
        assert_eq!(
            click_at(&mut list, &store, at),
            0,
            "the click belongs to the row that moved, not the one under it"
        );
        assert!(list.keyed().selected().next().is_none());
    }

    #[test]
    fn keyed_list_dropped_press_does_not_eat_a_click_after_a_release_elsewhere() {
        let first = items(&[(1, "alpha"), (2, "bravo")]);
        let mut list = nav_list();
        let at = Position::new(6, 1);
        press_at(&mut list, &first, at);

        let second = items(&[(1, "one"), (2, "two")]);
        let frame = draw_list(&mut list, &second);
        let id = frame.id_at(list.id, Part::ROW, at);
        // The router ends the gesture with a release instead of a click.
        let _ = list
            .on_pointer_source(&frame.event(Phase::Release, id, at), &second)
            .expect("valid");

        assert!(
            click_at(&mut list, &second, at) > 0,
            "a click with no press behind it is not the dropped one"
        );
    }

    #[test]
    fn keyed_list_dropped_press_is_forgotten_by_a_press_off_the_rows() {
        let first = items(&[(1, "alpha"), (2, "bravo")]);
        let mut list = nav_list();
        let at = Position::new(6, 1);
        press_at(&mut list, &first, at);

        // A press on something that is not a row (a gap, a header, a track)
        // starts a new gesture, so the dropped press is no longer awaited.
        let second = items(&[(1, "one"), (2, "two")]);
        let frame = draw_list(&mut list, &second);
        let gap = frame.event(Phase::Press, WidgetId::of("gap"), Position::new(39, 9));
        let _ = list.on_pointer_source(&gap, &second).expect("valid");

        assert!(
            click_at(&mut list, &second, at) > 0,
            "a click with no press behind it is not the dropped one"
        );
    }

    #[test]
    fn keyed_list_click_after_a_source_swap_is_consumed() {
        let first = items(&[(1, "alpha"), (2, "bravo")]);
        let mut list =
            list(SelectMode::Single).click(junie_tui::core::selection::ClickPolicy::NAVIGATION);
        let frame = draw_list(&mut list, &first);
        let at = Position::new(6, 1);
        let id = frame.id_at(list.id, Part::ROW, at);
        let _ = list
            .on_pointer_source(&frame.event(Phase::Press, id, at), &first)
            .expect("valid");
        assert_eq!(list.keyed().pressed(), Some(k(2)));

        // Another provider instance that happens to reuse the same keys.
        let second = items(&[(1, "one"), (2, "two")]);
        let after = draw_list(&mut list, &second);
        let id = after.id_at(list.id, Part::ROW, at);
        let out = list
            .on_pointer_source(&after.event(Phase::Click, id, at), &second)
            .expect("valid");
        assert!(
            out.actions().next().is_none(),
            "the press belonged to the old source"
        );
    }

    #[test]
    fn keyed_list_click_activates_the_clicked_key() {
        let store = items(&[(1, "alpha"), (2, "bravo")]);
        let mut list =
            list(SelectMode::Single).click(junie_tui::core::selection::ClickPolicy::NAVIGATION);
        let frame = draw_list(&mut list, &store);
        let at = Position::new(6, 1);
        let id = frame.id_at(list.id, Part::ROW, at);
        let press = frame.event(Phase::Press, id, at);
        let _ = list.on_pointer_source(&press, &store).expect("valid");
        let out = list
            .on_pointer_source(&frame.event(Phase::Click, id, at), &store)
            .expect("valid");
        assert!(out.actions().any(|a| *a == ListAction::Activated(k(2))));
    }

    fn owned_items(names: &[&str]) -> Vec<ListItem> {
        names.iter().map(|n| ListItem::new(n)).collect()
    }

    #[test]
    fn keyed_owned_and_borrowed_paths_conform() {
        let names = ["alpha", "bravo", "charlie", "delta", "echo"];
        let store = items(&[
            (1, "alpha"),
            (2, "bravo"),
            (3, "charlie"),
            (4, "delta"),
            (5, "echo"),
        ]);
        for mode in [SelectMode::Single, SelectMode::Multi] {
            let mut owned = ListBox::new(WidgetId::of("kl"), owned_items(&names), mode);
            let mut borrowed = list(mode);
            let script = [
                key(KeyCode::Down),
                key(KeyCode::Down),
                key(KeyCode::Char(' ')),
                key(KeyCode::Up),
                chord(KeyCode::Down, KeyModifiers::SHIFT),
                chord(KeyCode::Down, KeyModifiers::SHIFT),
                key(KeyCode::End),
                key(KeyCode::Enter),
                key(KeyCode::Home),
                key(KeyCode::Char('c')),
            ];
            let mut a = paint(|buf, ctx| owned.render(AREA, buf, ctx, ctx.theme.canvas));
            let mut b = draw_list(&mut borrowed, &store);
            assert_eq!(a.text(), b.text());
            for k in &script {
                let owned_out = owned.on_key(k);
                let borrowed_out = borrowed.on_key_source(k, &store).expect("valid");
                assert_eq!(owned_out, borrowed_out.outcome(), "{mode:?} {k:?}");
                a = paint(|buf, ctx| owned.render(AREA, buf, ctx, ctx.theme.canvas));
                b = draw_list(&mut borrowed, &store);
                assert_eq!(a.text(), b.text(), "{mode:?} after {k:?}");
                assert_eq!(owned.core.cursor, borrowed.core.cursor);
                assert_eq!(owned.chosen, borrowed.chosen);
                assert_eq!(
                    owned.core.selection.to_vec(),
                    borrowed.core.selection.to_vec()
                );
            }
        }
    }

    fn columns() -> Vec<Column> {
        vec![
            Column::new("name", Constraint::Length(12)).editable(),
            Column::new("owner", Constraint::Length(10)).editable(),
        ]
    }

    fn cells(a: &str, b: &str) -> Vec<Cell> {
        vec![Cell::new(a), Cell::new(b)]
    }

    fn table_store() -> KeyedTable {
        KeyedTable::try_new(
            columns(),
            [
                (k(1), cells("alpha", "ann")),
                (k(2), cells("bravo", "bob")),
                (k(3), cells("charlie", "cy")),
            ],
        )
        .expect("rectangular")
    }

    fn table() -> DataTable {
        DataTable::try_new(WidgetId::of("kt"), vec![], vec![])
            .expect("empty")
            .cell_nav(true)
    }

    fn draw_table(t: &mut DataTable, store: &mut KeyedTable) -> Frame {
        paint(|buf, ctx| {
            t.render_edit_source(AREA, store, buf, ctx, ctx.theme.canvas)
                .expect("valid source");
        })
    }

    fn type_text(t: &mut DataTable, store: &mut KeyedTable, text: &str) {
        for c in text.chars() {
            let _ = t
                .on_key_edit_source(&key(KeyCode::Char(c)), store)
                .expect("valid");
        }
    }

    #[test]
    fn borrowed_table_commits_by_key() {
        let mut store = table_store();
        let mut t = table();
        draw_table(&mut t, &mut store);
        let _ = t
            .on_key_edit_source(&key(KeyCode::Down), &mut store)
            .expect("valid");
        assert_eq!(t.keyed().cursor(), Some(k(2)));
        let begun = t
            .on_key_edit_source(&key(KeyCode::Enter), &mut store)
            .expect("valid");
        assert!(begun.actions().next().is_none());
        assert_eq!(t.keyed().edit_key(), Some(k(2)));

        store.move_row(k(2), 0);
        draw_table(&mut t, &mut store);
        assert_eq!(
            t.keyed().edit_key(),
            Some(k(2)),
            "reordering keeps the edit target"
        );

        type_text(&mut t, &mut store, "!");
        let out = t
            .on_key_edit_source(&key(KeyCode::Enter), &mut store)
            .expect("valid");
        assert_eq!(
            out.actions().copied().collect::<Vec<_>>(),
            vec![TableAction::Committed { row: k(2), col: 0 }]
        );
        assert_eq!(store.row(k(2)).expect("row")[0].text, "bravo!");
        assert_eq!(store.row(k(1)).expect("row")[0].text, "alpha");
        assert_eq!(store.row(k(3)).expect("row")[0].text, "charlie");
    }

    #[test]
    fn borrowed_table_tab_commit_keeps_the_next_cell_editor_open() {
        let mut store = table_store();
        let mut t = table();
        draw_table(&mut t, &mut store);
        let _ = t
            .on_key_edit_source(&key(KeyCode::Enter), &mut store)
            .expect("valid");
        type_text(&mut t, &mut store, "!");
        let out = t
            .on_key_edit_source(&key(KeyCode::Tab), &mut store)
            .expect("valid");
        assert_eq!(
            out.actions().copied().collect::<Vec<_>>(),
            vec![TableAction::Committed { row: k(1), col: 0 }]
        );
        assert_eq!(store.row(k(1)).expect("row")[0].text, "alpha!");
        assert!(t.is_editing(), "Tab opens the next cell's editor");

        draw_table(&mut t, &mut store);
        assert!(
            t.is_editing(),
            "repainting the frame keeps the editor Tab opened"
        );
        type_text(&mut t, &mut store, "?");
        let out = t
            .on_key_edit_source(&key(KeyCode::Enter), &mut store)
            .expect("valid");
        assert_eq!(
            out.actions().copied().collect::<Vec<_>>(),
            vec![TableAction::Committed { row: k(1), col: 1 }]
        );
        assert_eq!(store.row(k(1)).expect("row")[1].text, "ann?");
    }

    #[test]
    fn borrowed_table_blur_commits_like_an_owned_table() {
        let mut store = table_store();
        let mut t = table();
        draw_table(&mut t, &mut store);
        let _ = t
            .on_key_edit_source(&key(KeyCode::Enter), &mut store)
            .expect("valid");
        type_text(&mut t, &mut store, "!");
        let _ = t.on_blur_edit_source(&mut store).expect("valid");
        assert_eq!(
            store.row(k(1)).expect("row")[0].text,
            "alpha!",
            "the default blur policy commits the typed text"
        );
    }

    #[test]
    fn borrowed_table_blur_edit_source_commits_by_key() {
        let mut store = table_store();
        let mut t = table();
        draw_table(&mut t, &mut store);
        let _ = t
            .on_key_edit_source(&key(KeyCode::Down), &mut store)
            .expect("valid");
        let _ = t
            .on_key_edit_source(&key(KeyCode::Enter), &mut store)
            .expect("valid");
        type_text(&mut t, &mut store, "!");
        store.move_row(k(2), 0);
        draw_table(&mut t, &mut store);

        let out = t.on_blur_edit_source(&mut store).expect("valid");
        assert_eq!(
            out.actions().copied().collect::<Vec<_>>(),
            vec![TableAction::Committed { row: k(2), col: 0 }]
        );
        assert_eq!(store.row(k(2)).expect("row")[0].text, "bravo!");
        assert_eq!(store.row(k(1)).expect("row")[0].text, "alpha");
        assert!(!t.is_editing());
        assert_eq!(t.keyed().edit_key(), None, "no stale edit target");
        draw_table(&mut t, &mut store);
        assert!(!t.is_editing(), "a repaint after the blur stays closed");
    }

    #[test]
    fn borrowed_table_blur_with_the_cancel_policy_discards_the_edit() {
        let mut store = table_store();
        let mut t = table();
        t.blur_action = BlurAction::Cancel;
        draw_table(&mut t, &mut store);
        let _ = t
            .on_key_edit_source(&key(KeyCode::Enter), &mut store)
            .expect("valid");
        type_text(&mut t, &mut store, "!");
        let out = t.on_blur_edit_source(&mut store).expect("valid");
        assert_eq!(
            out.actions().copied().collect::<Vec<_>>(),
            vec![TableAction::Cancelled]
        );
        assert_eq!(store.row(k(1)).expect("row")[0].text, "alpha");
        assert!(!t.is_editing());
        assert_eq!(t.keyed().edit_key(), None, "no stale edit target");
    }

    #[test]
    fn borrowed_table_blur_without_a_source_leaves_the_editor_open() {
        let mut store = table_store();
        let mut t = table();
        draw_table(&mut t, &mut store);
        let _ = t
            .on_key_edit_source(&key(KeyCode::Enter), &mut store)
            .expect("valid");
        type_text(&mut t, &mut store, "!");
        assert!(
            t.on_blur().outcome() == Outcome::Ignored,
            "a keyed table cannot commit without its source"
        );
        assert_eq!(t.blur(), Outcome::Ignored);
        assert!(t.is_editing(), "the typed text is not thrown away");
        let _ = t.on_blur_edit_source(&mut store).expect("valid");
        assert_eq!(store.row(k(1)).expect("row")[0].text, "alpha!");
    }

    #[test]
    fn borrowed_table_cancels_the_edit_when_its_row_is_removed() {
        let mut store = table_store();
        let mut t = table();
        draw_table(&mut t, &mut store);
        let _ = t
            .on_key_edit_source(&key(KeyCode::Enter), &mut store)
            .expect("valid");
        assert_eq!(t.keyed().edit_key(), Some(k(1)));
        type_text(&mut t, &mut store, "x");

        store.remove(k(1));
        draw_table(&mut t, &mut store);
        assert!(!t.is_editing());
        let out = t
            .on_key_edit_source(&key(KeyCode::Enter), &mut store)
            .expect("valid");
        assert!(out.actions().next().is_none());
        assert_eq!(store.row(k(2)).expect("row")[0].text, "bravo");
        assert_eq!(store.row(k(3)).expect("row")[0].text, "charlie");
    }

    #[test]
    fn borrowed_table_keeps_a_rejected_commit_in_the_editor() {
        struct Refuses(KeyedTable);
        impl RowSource for Refuses {
            fn len(&self) -> usize {
                self.0.len()
            }
            fn label(&self, i: usize) -> &str {
                self.0.label(i)
            }
            fn key(&self, i: usize) -> ItemKey {
                self.0.key(i)
            }
        }
        impl KeyedRowSource for Refuses {
            fn stamp(&self) -> SourceStamp {
                self.0.stamp()
            }
            fn source_len(&self) -> usize {
                self.0.source_len()
            }
            fn source_key(&self, i: usize) -> ItemKey {
                self.0.source_key(i)
            }
        }
        impl TableSource for Refuses {
            fn columns(&self) -> &[Column] {
                self.0.columns()
            }
            fn cell(&self, r: usize, c: usize) -> junie_tui::widgets::table::CellRef<'_> {
                self.0.cell(r, c)
            }
        }
        impl TableEditSource for Refuses {
            fn set_cell(&mut self, _: ItemKey, _: usize, _: String) -> Result<(), String> {
                Err("locked by policy".into())
            }
        }
        let mut store = Refuses(table_store());
        let mut t = table();
        let theme = Theme::for_level(ColorLevel::TrueColor);
        let mut buf = Buffer::empty(AREA);
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        t.render_edit_source(AREA, &mut store, &mut buf, &mut ctx, theme.canvas)
            .expect("valid");
        let _ = t
            .on_key_edit_source(&key(KeyCode::Enter), &mut store)
            .expect("valid");
        for c in "zz".chars() {
            let _ = t
                .on_key_edit_source(&key(KeyCode::Char(c)), &mut store)
                .expect("valid");
        }
        let out = t
            .on_key_edit_source(&key(KeyCode::Enter), &mut store)
            .expect("valid");
        assert!(
            out.actions().next().is_none(),
            "a refused commit reports nothing"
        );
        assert!(t.is_editing());
        assert_eq!(t.edit_error(), Some("locked by policy"));
        assert_eq!(store.0.row(k(1)).expect("row")[0].text, "alpha");
    }

    #[test]
    fn keyed_table_lists_its_row_keys_and_edits_one_cell_by_key() {
        let mut store = table_store();
        assert!(!store.is_empty());
        assert_eq!(store.row_keys().collect::<Vec<_>>(), [k(1), k(2), k(3)]);
        let before = store.stamp();
        assert!(store.update_cell(k(2), 1, |cell| *cell = Cell::new("zed")));
        assert!(
            store.stamp().data > before.data,
            "an edit advances the data revision"
        );
        assert!(
            !store.update_cell(k(2), 9, |_| ()) && !store.update_cell(k(99), 0, |_| ()),
            "an unknown column or key edits nothing"
        );
        let empty = KeyedTable::try_new(columns(), []).expect("no rows");
        assert!(empty.is_empty() && empty.row_keys().next().is_none());
    }

    #[test]
    fn data_table_and_list_box_paint_borrowed_rows_through_render_source() {
        let mut store = table_store();
        let mut t = table();
        let frame = paint(|buf, ctx| {
            t.render_source(AREA, &store, buf, ctx, ctx.theme.canvas)
                .expect("valid source");
        });
        assert!(frame.text().contains("alpha") && frame.text().contains("charlie"));
        store.update_cell(k(1), 0, |cell| *cell = Cell::new("omega"));
        let frame = paint(|buf, ctx| {
            t.render_source(AREA, &store, buf, ctx, ctx.theme.canvas)
                .expect("valid source");
        });
        assert!(frame.text().contains("omega"), "a changed cell repaints");

        let names = items(&[(1, "one"), (2, "two")]);
        let mut l = list(SelectMode::Single);
        let frame = paint(|buf, ctx| {
            l.render_source(AREA, &names, buf, ctx, ctx.theme.canvas)
                .expect("valid source");
        });
        assert!(frame.text().contains("one") && frame.text().contains("two"));
    }

    #[test]
    fn keyed_table_validates_shape_and_keys() {
        use junie_tui::core::data::DataError;
        use junie_tui::widgets::table::KeyedTableError;
        let err = KeyedTable::try_new(columns(), [(k(1), vec![Cell::new("only one")])])
            .expect_err("ragged");
        assert!(matches!(err, KeyedTableError::Shape(DataError::Shape(_))));
        let err = KeyedTable::try_new(
            columns(),
            [(k(1), cells("a", "b")), (k(1), cells("c", "d"))],
        )
        .expect_err("duplicate");
        assert_eq!(
            err,
            KeyedTableError::Key(KeyedSourceError::DuplicateAuthoritative { key: k(1) })
        );
    }

    #[test]
    fn keyed_table_rejects_rows_under_an_empty_schema() {
        use junie_tui::core::data::DataError;
        use junie_tui::widgets::table::KeyedTableError;
        let refused = Err(KeyedTableError::Shape(DataError::EmptySchemaWithRows));
        assert_eq!(
            KeyedTable::try_new(vec![], [(k(1), vec![])]).map(|_| ()),
            refused
        );
        let mut empty = KeyedTable::try_new(vec![], []).expect("no columns and no rows");
        assert_eq!(empty.replace_rows([(k(1), vec![])]), refused);
        assert_eq!(empty.insert(0, k(1), vec![]), refused);
        assert_eq!(empty.len(), 0, "nothing was stored");
    }

    #[test]
    fn keyed_table_press_cannot_retarget_after_reorder() {
        let mut store = table_store();
        let mut t = table();
        let frame = draw_table(&mut t, &mut store);
        let at = Position::new(8, 1);
        let id = frame.id_at(t.id, Part::CELL, at);
        let _ = t
            .on_pointer_edit_source(&frame.event(Phase::Press, id, at), &mut store)
            .expect("valid");
        assert_eq!(t.keyed().pressed(), Some(k(1)));
        store.move_row(k(1), 2);
        let after = draw_table(&mut t, &mut store);
        let id_b = after.id_at(t.id, Part::CELL, at);
        assert_ne!(id, id_b);
        let out = t
            .on_pointer_edit_source(&after.event(Phase::Click, id_b, at), &mut store)
            .expect("valid");
        assert!(out.actions().next().is_none());
        assert_eq!(t.keyed().pressed(), None);
        assert_eq!(store.len(), 3);
    }

    #[test]
    fn keyed_table_click_after_a_source_swap_is_consumed() {
        let mut store = table_store();
        let mut t = table();
        let frame = draw_table(&mut t, &mut store);
        let at = Position::new(8, 2);
        let id = frame.id_at(t.id, Part::CELL, at);
        let _ = t
            .on_pointer_edit_source(&frame.event(Phase::Press, id, at), &mut store)
            .expect("valid");
        assert_eq!(t.keyed().pressed(), Some(k(2)));
        let cursor = t.keyed().cursor();

        let mut second = table_store();
        let after = draw_table(&mut t, &mut second);
        let id = after.id_at(t.id, Part::CELL, at);
        let out = t
            .on_pointer_edit_source(&after.event(Phase::Click, id, at), &mut second)
            .expect("valid");
        assert!(out.is_consumed() && out.actions().next().is_none());
        assert_eq!(t.keyed().cursor(), cursor, "the old press moved nothing");
    }

    #[test]
    fn keyed_owned_and_borrowed_tables_conform() {
        let rows = [("alpha", "ann"), ("bravo", "bob"), ("charlie", "cy")];
        let mut owned = DataTable::try_new(
            WidgetId::of("kt"),
            columns(),
            rows.iter().map(|(a, b)| cells(a, b)).collect(),
        )
        .expect("rectangular")
        .cell_nav(true)
        .selection_mode(SelectionMode::Multi);
        let mut store = table_store();
        let mut borrowed = table().selection_mode(SelectionMode::Multi);
        let script = [
            key(KeyCode::Down),
            key(KeyCode::Right),
            key(KeyCode::Char(' ')),
            chord(KeyCode::Down, KeyModifiers::SHIFT),
            key(KeyCode::Up),
            key(KeyCode::Left),
            key(KeyCode::End),
        ];
        let mut a = paint(|buf, ctx| owned.render(AREA, buf, ctx, ctx.theme.canvas));
        let mut b = draw_table(&mut borrowed, &mut store);
        assert_eq!(a.text(), b.text());
        for k in &script {
            let owned_out = owned.on_key(k);
            let borrowed_out = borrowed.on_key_source(k, &store).expect("valid");
            assert_eq!(owned_out.outcome(), borrowed_out.outcome(), "{k:?}");
            a = paint(|buf, ctx| owned.render(AREA, buf, ctx, ctx.theme.canvas));
            b = draw_table(&mut borrowed, &mut store);
            assert_eq!(a.text(), b.text(), "after {k:?}");
            assert_eq!(
                (owned.cursor_row, owned.cursor_col),
                (borrowed.cursor_row, borrowed.cursor_col)
            );
            assert_eq!(owned.selected_rows(), borrowed.selected_rows());
        }
    }
}

mod trees {
    use super::widgets::{Frame, paint};
    use super::*;
    use crate::common::key;
    use junie_tui::core::id::{Part, WidgetId};
    use junie_tui::core::intent::Phase;
    use junie_tui::core::list::SourceStamp;
    use junie_tui::widgets::tree::{
        KeyedTree, KeyedTreeNode, TreeAction, TreeNode, TreeNodeRef, TreeSource, TreeView,
    };
    use ratatui::crossterm::event::KeyCode;
    use ratatui::layout::{Position, Rect};
    use std::collections::HashMap;

    const AREA: Rect = Rect::new(0, 0, 40, 10);

    fn tree() -> KeyedTree {
        let mut t = KeyedTree::new();
        t.insert(None, 0, k(1), KeyedTreeNode::new("src"))
            .expect("root");
        t.insert(Some(k(1)), 0, k(11), KeyedTreeNode::new("main.rs"))
            .expect("child");
        t.insert(Some(k(1)), 1, k(12), KeyedTreeNode::new("lib.rs"))
            .expect("child");
        t.insert(None, 1, k(2), KeyedTreeNode::new("docs"))
            .expect("root");
        t.insert(Some(k(2)), 0, k(21), KeyedTreeNode::new("guide.md"))
            .expect("child");
        t
    }

    fn draw(view: &mut TreeView, source: &dyn TreeSource) -> Frame {
        paint(|buf, ctx| {
            view.render_source(AREA, &Dyn(source), buf, ctx, ctx.theme.canvas)
                .expect("valid source");
        })
    }

    struct Dyn<'a>(&'a dyn TreeSource);
    impl TreeSource for Dyn<'_> {
        fn stamp(&self) -> SourceStamp {
            self.0.stamp()
        }
        fn root_count(&self) -> usize {
            self.0.root_count()
        }
        fn root_key(&self, i: usize) -> ItemKey {
            self.0.root_key(i)
        }
        fn node(&self, key: ItemKey) -> Option<TreeNodeRef<'_>> {
            self.0.node(key)
        }
    }

    fn labels(f: &Frame) -> Vec<String> {
        f.text()
            .lines()
            .map(|l| l.trim().to_owned())
            .filter(|l| !l.is_empty())
            .collect()
    }

    #[test]
    fn keyed_tree_keeps_expansion_after_insert() {
        let mut store = tree();
        let mut view = TreeView::new(WidgetId::of("kt"), vec![]);
        let f = draw(&mut view, &store);
        assert!(
            view.keyed().is_expanded(k(1)) && view.keyed().is_expanded(k(2)),
            "roots open by default"
        );
        assert!(labels(&f).iter().any(|l| l.contains("main.rs")));

        let _ = view
            .on_key_source(&key(KeyCode::Down), &store)
            .expect("valid");
        let _ = view
            .on_key_source(&key(KeyCode::Down), &store)
            .expect("valid");
        assert_eq!(view.keyed().cursor(), Some(k(12)));
        let out = view
            .on_key_source(&key(KeyCode::Enter), &store)
            .expect("valid");
        assert_eq!(
            out.actions().cloned().collect::<Vec<_>>(),
            vec![TreeAction::Activate(k(12))]
        );
        let _ = view
            .on_key_source(&key(KeyCode::Up), &store)
            .expect("valid");
        let _ = view
            .on_key_source(&key(KeyCode::Up), &store)
            .expect("valid");
        assert_eq!(view.keyed().cursor(), Some(k(1)));

        store
            .insert(None, 0, k(9), KeyedTreeNode::new("aaa"))
            .expect("earlier sibling");
        store
            .insert(Some(k(1)), 0, k(10), KeyedTreeNode::new("a.rs"))
            .expect("earlier child");
        let after = draw(&mut view, &store);
        assert!(view.keyed().is_expanded(k(1)), "src stays expanded");
        assert!(view.keyed().is_selected(k(12)));
        assert_eq!(view.keyed().cursor(), Some(k(1)));
        assert_eq!(view.cursor, 1, "the view index follows the key");
        let l = labels(&after);
        assert!(
            l.iter().any(|x| x.contains("a.rs")) && l.iter().any(|x| x.contains("lib.rs")),
            "{l:?}"
        );

        store.remove(k(1));
        let _ = draw(&mut view, &store);
        assert!(!view.keyed().is_expanded(k(1)) && !view.keyed().is_selected(k(12)));
    }

    #[test]
    fn keyed_tree_cursor_moves_by_key_and_expand_all_opens_every_branch() {
        let mut store = tree();
        let mut view = TreeView::new(WidgetId::of("kt"), vec![]);
        view.expand_all_source(&store).expect("valid");
        assert!(view.keyed().is_expanded(k(1)) && view.keyed().is_expanded(k(2)));

        assert!(view.set_cursor_source(k(21), &store).expect("valid"));
        store
            .insert(None, 0, k(9), KeyedTreeNode::new("aaa"))
            .expect("earlier sibling");
        let _ = draw(&mut view, &store);
        assert_eq!(view.keyed().cursor(), Some(k(21)));
        assert_eq!(view.cursor, 5, "the view index follows the key");

        assert!(
            !view.set_cursor_source(k(99), &store).expect("valid"),
            "an unknown key is refused"
        );
        assert_eq!(view.keyed().cursor(), Some(k(21)));
    }

    #[test]
    fn keyed_tree_collapse_and_lazy_expand_report_keys() {
        let mut store = tree();
        store
            .insert(Some(k(2)), 1, k(3), KeyedTreeNode::lazy("remote"))
            .expect("lazy");
        let mut view = TreeView::new(WidgetId::of("kt"), vec![]);
        draw(&mut view, &store);
        let out = view
            .on_key_source(&key(KeyCode::Right), &store)
            .expect("valid");
        assert!(out.actions().next().is_none(), "already open: moves in");
        let _ = view
            .on_key_source(&key(KeyCode::Left), &store)
            .expect("valid");
        let _ = view
            .on_key_source(&key(KeyCode::Left), &store)
            .expect("valid");
        assert!(
            !view.keyed().is_expanded(k(1)),
            "left on an open branch collapses it"
        );
        draw(&mut view, &store);
        for _ in 0..2 {
            let _ = view
                .on_key_source(&key(KeyCode::Down), &store)
                .expect("valid");
        }
        while view.keyed().cursor() != Some(k(3)) {
            let _ = view
                .on_key_source(&key(KeyCode::Down), &store)
                .expect("valid");
        }
        let out = view
            .on_key_source(&key(KeyCode::Right), &store)
            .expect("valid");
        assert_eq!(
            out.actions().cloned().collect::<Vec<_>>(),
            vec![TreeAction::Expand(k(3))]
        );
    }

    struct Raw {
        nodes: HashMap<ItemKey, (String, Vec<ItemKey>)>,
        roots: Vec<ItemKey>,
        data: u64,
        notes: Vec<ItemKey>,
    }

    impl Raw {
        fn new(roots: &[u64], nodes: &[(u64, &[u64])]) -> Self {
            Self {
                roots: roots.iter().map(|n| k(*n)).collect(),
                nodes: nodes
                    .iter()
                    .map(|(n, c)| (k(*n), (format!("n{n}"), c.iter().map(|x| k(*x)).collect())))
                    .collect(),
                data: 1,
                notes: vec![],
            }
        }
    }

    impl TreeSource for Raw {
        fn stamp(&self) -> SourceStamp {
            SourceStamp {
                instance: ItemKey::num(77),
                data: self.data,
                view: 0,
            }
        }
        fn root_count(&self) -> usize {
            self.roots.len()
        }
        fn root_key(&self, i: usize) -> ItemKey {
            self.roots[i]
        }
        fn node(&self, key: ItemKey) -> Option<TreeNodeRef<'_>> {
            let (label, children) = self.nodes.get(&key)?;
            Some(TreeNodeRef {
                label,
                meta: None,
                glyph: None,
                glyph_tone: junie_tui::theme::Tone::Muted,
                label_tone: None,
                children,
                lazy: false,
                busy: false,
                note: self.notes.contains(&key),
            })
        }
    }

    #[test]
    fn keyed_tree_rejects_cycles_duplicates_and_missing_nodes_atomically() {
        let good = Raw::new(&[1], &[(1, &[2]), (2, &[])]);
        let mut view = TreeView::new(WidgetId::of("kt"), vec![]);
        draw(&mut view, &good);
        let _ = view
            .on_key_source(&key(KeyCode::Down), &good)
            .expect("valid");
        let cursor = view.keyed().cursor();

        let mut cycle = Raw::new(&[1], &[(1, &[2]), (2, &[1])]);
        cycle.data = 2;
        let err = view
            .on_key_source(&key(KeyCode::Down), &cycle)
            .expect_err("cycle");
        assert_eq!(err, KeyedSourceError::Cycle { key: k(1) });
        let mut shared = Raw::new(&[1], &[(1, &[2, 3]), (2, &[4]), (3, &[4]), (4, &[])]);
        shared.data = 3;
        assert_eq!(
            view.on_key_source(&key(KeyCode::Down), &shared)
                .expect_err("two parents"),
            KeyedSourceError::DuplicateAuthoritative { key: k(4) }
        );
        let mut missing = Raw::new(&[1], &[(1, &[9])]);
        missing.data = 4;
        assert_eq!(
            view.on_key_source(&key(KeyCode::Down), &missing)
                .expect_err("missing"),
            KeyedSourceError::MissingNode { key: k(9) }
        );
        let mut positional = Raw::new(&[1], &[(1, &[])]);
        positional.roots = vec![ItemKey::index(0)];
        positional.data = 5;
        assert!(matches!(
            view.on_key_source(&key(KeyCode::Down), &positional),
            Err(KeyedSourceError::PositionalKey {
                authoritative: true,
                ..
            })
        ));
        assert_eq!(
            view.keyed().cursor(),
            cursor,
            "state untouched by rejected sources"
        );
        let _ = draw(&mut view, &good);
    }

    #[test]
    fn keyed_tree_filter_shows_matching_subtrees() {
        let store = tree();
        let mut view = TreeView::new(WidgetId::of("kt"), vec![]);
        draw(&mut view, &store);
        view.set_filter(Some("guide"));
        let f = draw(&mut view, &store);
        let l = labels(&f);
        assert!(
            l.iter().any(|x| x.contains("docs")) && l.iter().any(|x| x.contains("guide.md")),
            "{l:?}"
        );
        assert!(
            !l.iter().any(|x| x.contains("main.rs") || x.contains("src")),
            "{l:?}"
        );
        view.set_filter(None);
        let f = draw(&mut view, &store);
        assert!(labels(&f).iter().any(|x| x.contains("main.rs")));
    }

    #[test]
    fn keyed_tree_press_cannot_retarget_after_reorder() {
        let mut store = tree();
        let mut view = TreeView::new(WidgetId::of("kt"), vec![]);
        let frame = draw(&mut view, &store);
        let at = Position::new(8, 0);
        let id_a = frame.id_at(view.id, Part::ROW, at);
        let _ = view
            .on_pointer_source(&frame.event(Phase::Press, id_a, at), &store)
            .expect("valid");
        assert_eq!(view.keyed().pressed(), Some(k(1)));
        store
            .insert(None, 0, k(9), KeyedTreeNode::new("zzz"))
            .expect("new first row");
        let after = draw(&mut view, &store);
        let id_b = after.id_at(view.id, Part::ROW, at);
        assert_ne!(id_a, id_b);
        let out = view
            .on_pointer_source(&after.event(Phase::Click, id_b, at), &store)
            .expect("valid");
        assert!(out.actions().next().is_none());
        assert_eq!(view.keyed().pressed(), None);
    }

    #[test]
    fn keyed_tree_click_after_a_source_swap_is_consumed() {
        let first = tree();
        let mut view = TreeView::new(WidgetId::of("kt"), vec![]);
        let frame = draw(&mut view, &first);
        let at = Position::new(8, 3);
        let id = frame.id_at(view.id, Part::ROW, at);
        let _ = view
            .on_pointer_source(&frame.event(Phase::Press, id, at), &first)
            .expect("valid");
        assert_eq!(view.keyed().pressed(), Some(k(2)));
        let cursor = view.keyed().cursor();

        // Another provider instance that happens to reuse the same keys.
        let second = tree();
        let after = draw(&mut view, &second);
        let id = after.id_at(view.id, Part::ROW, at);
        let _ = view
            .on_pointer_source(&after.event(Phase::Click, id, at), &second)
            .expect("valid");
        assert_eq!(view.keyed().cursor(), cursor, "the old press moved nothing");
        assert!(view.keyed().selected().next().is_none());
    }

    #[test]
    fn keyed_tree_nodes_carry_meta_and_glyph_and_serve_a_tree_view() {
        let mut store = KeyedTree::new();
        assert!(store.is_empty());
        store
            .insert(
                None,
                0,
                k(1),
                KeyedTreeNode::new("src").meta("2 files").glyph("*"),
            )
            .expect("root");
        store
            .insert(Some(k(1)), 0, k(11), KeyedTreeNode::new("main.rs"))
            .expect("child");
        assert!(!store.is_empty());
        let node = store.get(k(1)).expect("the root node");
        assert_eq!(node.children(), [k(11)]);
        assert_eq!(node.meta.as_deref(), Some("2 files"));
        assert_eq!(node.glyph.as_deref(), Some("*"));
        assert!(store.get(k(99)).is_none());

        fn serves<S: TreeSource + ?Sized>(source: &S, key: ItemKey) -> Option<TreeNodeRef<'_>> {
            source.node(key)
        }
        let served: TreeNodeRef<'_> = serves(&store, k(1)).expect("served as a TreeSource node");
        assert_eq!((served.meta, served.glyph), (Some("2 files"), Some("*")));
        assert_eq!(served.children, [k(11)]);

        let mut view = TreeView::new(WidgetId::of("kt"), vec![]).show_selection(false);
        let frame = draw(&mut view, &store);
        assert!(
            frame.text().contains("src") && frame.text().contains("2 files"),
            "{}",
            frame.text()
        );
    }

    #[test]
    fn select_follows_cursor_moves_the_selection_with_the_cursor() {
        let store = tree();
        let mut following = TreeView::new(WidgetId::of("kt"), vec![]).select_follows_cursor(true);
        draw(&mut following, &store);
        let _ = following
            .on_key_source(&key(KeyCode::Down), &store)
            .expect("valid");
        assert_eq!(
            following.keyed().selected().collect::<Vec<_>>(),
            following.keyed().cursor().into_iter().collect::<Vec<_>>(),
            "the selection is the cursor's node"
        );

        let mut apart = TreeView::new(WidgetId::of("kt"), vec![]);
        draw(&mut apart, &store);
        let _ = apart
            .on_key_source(&key(KeyCode::Down), &store)
            .expect("valid");
        assert_eq!(
            apart.keyed().selected().count(),
            0,
            "without it the cursor moves and nothing is selected"
        );
    }

    #[test]
    fn keyed_tree_select_source_refuses_an_unknown_key_without_mutation() {
        let store = tree();
        let mut view = TreeView::new(WidgetId::of("kt"), vec![]);
        draw(&mut view, &store);
        assert_eq!(view.select_source(k(2), &store), Ok(true));
        assert_eq!(view.select_source(k(99), &store), Ok(false));
        assert_eq!(
            view.keyed().selected().collect::<Vec<_>>(),
            vec![k(2)],
            "a refused key leaves the selection alone"
        );
    }

    #[test]
    fn keyed_tree_select_source_refuses_note_nodes_hidden_or_visible() {
        let mut raw = Raw::new(&[1, 5], &[(1, &[2]), (2, &[]), (5, &[])]);
        raw.notes = vec![k(2)];
        let mut view = TreeView::new(WidgetId::of("kt"), vec![]);
        draw(&mut view, &raw);
        assert_eq!(view.select_source(k(5), &raw), Ok(true));

        assert_eq!(view.select_source(k(2), &raw), Ok(false), "visible note");
        assert_eq!(view.keyed().selected().collect::<Vec<_>>(), vec![k(5)]);

        assert_eq!(view.set_cursor_source(k(1), &raw), Ok(true));
        let _ = view
            .on_key_source(&key(KeyCode::Left), &raw)
            .expect("valid");
        draw(&mut view, &raw);
        assert!(view.keyed().visible_index(k(2)).is_none(), "note is hidden");
        assert_eq!(view.select_source(k(2), &raw), Ok(false), "hidden note");
        assert_eq!(
            view.keyed().selected().collect::<Vec<_>>(),
            vec![k(5)],
            "a refused note leaves the selection alone"
        );
    }

    #[test]
    fn keyed_owned_and_borrowed_trees_conform() {
        let owned_nodes = vec![
            TreeNode::dir(
                "src",
                vec![TreeNode::leaf("main.rs"), TreeNode::leaf("lib.rs")],
            ),
            TreeNode::dir("docs", vec![TreeNode::leaf("guide.md")]),
        ];
        let mut owned = TreeView::new(WidgetId::of("kt"), owned_nodes);
        let store = tree();
        let mut borrowed = TreeView::new(WidgetId::of("kt"), vec![]);
        let script = [
            KeyCode::Down,
            KeyCode::Down,
            KeyCode::Down,
            KeyCode::Left,
            KeyCode::Left,
            KeyCode::Down,
            KeyCode::Right,
            KeyCode::End,
            KeyCode::Home,
            KeyCode::Right,
            KeyCode::Right,
            KeyCode::Char('g'),
        ];
        let mut a = paint(|buf, ctx| owned.render(AREA, buf, ctx, ctx.theme.canvas));
        let mut b = draw(&mut borrowed, &store);
        assert_eq!(a.text(), b.text());
        for code in script {
            let o = owned.on_key(&key(code));
            let r = borrowed.on_key_source(&key(code), &store).expect("valid");
            assert_eq!(o.outcome(), r.outcome(), "{code:?}");
            a = paint(|buf, ctx| owned.render(AREA, buf, ctx, ctx.theme.canvas));
            b = draw(&mut borrowed, &store);
            assert_eq!(a.text(), b.text(), "after {code:?}");
            assert_eq!(owned.cursor, borrowed.cursor, "{code:?}");
        }
    }
}

mod details {
    use super::widgets::paint;
    use super::*;
    use junie_tui::core::event::Outcome;
    use junie_tui::core::focus::Focus;
    use junie_tui::core::focus::FocusRing;
    use junie_tui::core::hit::HitRegistry;
    use junie_tui::core::id::WidgetId;
    use junie_tui::core::intent::Phase;
    use junie_tui::theme::{ColorLevel, Theme};
    use junie_tui::ui::ctx::{Interaction, RenderCtx};
    use junie_tui::ui::layout::Split;
    use junie_tui::widgets::list_detail::{
        DetailPane, Event, ListDetail, ListDetailSource, ListRow, ListRowRef, MasterRow, PaneHost,
        SourceFrame,
    };
    use ratatui::buffer::Buffer;
    use ratatui::crossterm::event::KeyCode;
    use ratatui::layout::{Position, Rect};

    const AREA: Rect = Rect::new(0, 0, 40, 10);
    const MASTER: WidgetId = WidgetId::of("kd.m");
    const DETAIL: WidgetId = WidgetId::of("kd.d");

    fn store() -> KeyedVec<ListRow> {
        KeyedVec::try_from_rows(
            [(1, "alpha"), (2, "bravo"), (3, "charlie")].map(|(n, l)| (k(n), ListRow::new(l))),
        )
        .expect("valid")
    }

    fn draw(ld: &mut ListDetail, store: &KeyedVec<ListRow>) -> super::widgets::Frame {
        let frame = SourceFrame {
            title: "Rows",
            meta: "3",
            head: &[],
            under: None,
            detail_title: "Detail",
            detail_meta: "",
            lines: &[],
            actions: &[],
        };
        paint(|buf, ctx| {
            ld.render_source(AREA, store, &frame, buf, ctx)
                .expect("valid source");
        })
    }

    const WIDE: Rect = Rect::new(0, 0, 120, 10);

    fn paint_in(area: Rect, f: impl FnOnce(&mut Buffer, &mut RenderCtx)) -> super::widgets::Frame {
        let theme = Theme::for_level(ColorLevel::TrueColor);
        let mut buf = Buffer::empty(area);
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        {
            let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
            f(&mut buf, &mut ctx);
        }
        super::widgets::Frame { buf, hits }
    }

    fn plain_frame() -> SourceFrame<'static> {
        SourceFrame {
            title: "Rows",
            meta: "3",
            head: &[],
            under: None,
            detail_title: "Detail",
            detail_meta: "",
            lines: &[],
            actions: &[],
        }
    }

    /// A host that paints nothing and records what it was handed.
    #[derive(Default)]
    struct ScrollHost {
        offsets: Vec<usize>,
    }

    impl PaneHost for ScrollHost {
        fn row(&mut self, _row: MasterRow, _buf: &mut Buffer, _ctx: &mut RenderCtx) {}

        fn empty(&mut self, _area: Rect, _buf: &mut Buffer, _ctx: &mut RenderCtx) {}

        fn detail(&mut self, pane: DetailPane<'_>, _buf: &mut Buffer, _ctx: &mut RenderCtx) {
            self.offsets.push(pane.scroll.offset);
            pane.scroll.set_content(100);
            pane.scroll.set_viewport(5);
        }

        fn summary(&mut self, _area: Rect, _buf: &mut Buffer, _ctx: &mut RenderCtx) {}
    }

    fn scrolled_to(ld: &mut ListDetail, host: &mut ScrollHost) -> super::widgets::Frame {
        paint_in(WIDE, |buf, ctx| {
            ld.render_hosted(WIDE, 3, |i| k(i as u64 + 1), Some(1), buf, ctx, host);
        })
    }

    #[test]
    fn hosted_list_detail_hands_rows_and_the_detail_pane_to_its_pane_host() {
        /// Records what the list-detail handed the host.
        #[derive(Default)]
        struct Probe {
            rows: Vec<(usize, ItemKey, bool)>,
            details: Vec<(bool, WidgetId)>,
            summaries: usize,
        }

        impl PaneHost for Probe {
            fn row(&mut self, row: MasterRow, _buf: &mut Buffer, _ctx: &mut RenderCtx) {
                self.rows.push((row.index, row.key, row.selected));
            }

            fn empty(&mut self, _area: Rect, _buf: &mut Buffer, _ctx: &mut RenderCtx) {}

            fn detail(&mut self, pane: DetailPane<'_>, _buf: &mut Buffer, _ctx: &mut RenderCtx) {
                self.details.push((pane.drawer, pane.id));
            }

            fn summary(&mut self, _area: Rect, _buf: &mut Buffer, _ctx: &mut RenderCtx) {
                self.summaries += 1;
            }
        }

        fn hosted<H: PaneHost>(ld: &mut ListDetail, area: Rect, host: &mut H) {
            paint_in(area, |buf, ctx| {
                ld.render_hosted(area, 3, |i| k(i as u64 + 1), Some(1), buf, ctx, host);
            });
        }

        let mut ld = ListDetail::new(MASTER, DETAIL, Split::new(50, 36, 40));
        let mut host = Probe::default();
        hosted(&mut ld, WIDE, &mut host);
        assert_eq!(
            host.rows,
            [(0, k(1), false), (1, k(2), true), (2, k(3), false)],
            "every visible row arrives with its key and the frame's selection"
        );
        assert_eq!(
            host.details,
            [(false, DETAIL)],
            "a wide area splits beside the master"
        );
        assert_eq!(ld.master_offset(), 0);
        assert!(ld.master_page() >= 1);
        assert!(!ld.drawer_open());

        // A narrow area collapses to the master and a summary; opening the
        // detail turns it into a drawer the host paints over the master.
        let mut narrow = Probe::default();
        hosted(&mut ld, AREA, &mut narrow);
        assert!(narrow.details.is_empty() && narrow.summaries == 1);
        let mut focus = Focus::default();
        ld.open_detail(&mut focus);
        assert!(ld.drawer_open());
        let mut drawer = Probe::default();
        hosted(&mut ld, AREA, &mut drawer);
        assert_eq!(drawer.details.len(), 1);
        assert!(drawer.details[0].0, "the pane reports it is a drawer");
    }

    #[test]
    fn detail_keys_wheel_and_reset_drive_the_hosted_detail_scroll() {
        let mut ld = ListDetail::new(MASTER, DETAIL, Split::new(50, 36, 40));
        let mut host = ScrollHost::default();
        let frame = scrolled_to(&mut ld, &mut host);
        let mut focus = Focus::default();
        focus.focus(DETAIL);

        let response = ld.on_detail_key(&crate::common::key(KeyCode::PageDown), &mut focus);
        assert_eq!(response.outcome(), Outcome::Changed);
        let wheel = frame.event(Phase::Middle, DETAIL, Position::new(100, 3));
        assert_eq!(ld.wheel_event(&wheel, 2), Outcome::Changed);
        let mut scrolled = ScrollHost::default();
        scrolled_to(&mut ld, &mut scrolled);
        assert!(scrolled.offsets[0] > 0, "{:?}", scrolled.offsets);

        ld.reset_detail();
        let mut reset = ScrollHost::default();
        scrolled_to(&mut ld, &mut reset);
        assert_eq!(reset.offsets[0], 0, "a reset returns the detail to its top");
    }

    #[test]
    fn list_detail_source_rows_are_painted_by_a_caller_closure_without_touching_the_list() {
        let store = store();
        let mut ld = ListDetail::new(MASTER, DETAIL, Split::new(50, 36, 40));
        let frame = plain_frame();
        let mut seen: Vec<(String, ItemKey, bool)> = Vec::new();
        paint_in(WIDE, |buf, ctx| {
            ld.render_source_selected_with(
                WIDE,
                &store,
                &frame,
                Some(2),
                buf,
                ctx,
                |source, row: MasterRow, _buf, _ctx| {
                    let shown: ListRowRef<'_> = ListDetailSource::row(source, row.index);
                    seen.push((shown.label.to_owned(), row.key, row.selected));
                },
            );
        });
        assert_eq!(
            seen,
            [
                ("alpha".to_owned(), k(1), false),
                ("bravo".to_owned(), k(2), false),
                ("charlie".to_owned(), k(3), true)
            ]
        );
        assert_eq!(ld.cursor, 0);
        assert_eq!(
            ld.selected_key(),
            None,
            "the list's own keyed state is untouched"
        );
    }

    #[test]
    fn bind_source_adopts_the_moved_cursor_without_painting() {
        let mut store = store();
        let mut ld = ListDetail::new(MASTER, DETAIL, Split::new(50, 36, 40));
        ld.bind_source(&store).expect("valid source");
        assert_eq!(ld.selected_key(), Some(k(1)));
        ld.select(2);
        store.move_to(k(3), 0);
        ld.bind_source(&store).expect("valid source");
        assert_eq!(
            ld.selected_key(),
            Some(k(3)),
            "the cursor was adopted by key, then followed it"
        );
        assert_eq!(ld.cursor, 0);
    }

    #[test]
    fn keyed_list_detail_selection_follows_the_key_across_reorder() {
        let mut store = store();
        let mut ld = ListDetail::new(MASTER, DETAIL, Split::new(50, 36, 40));
        draw(&mut ld, &store);
        ld.select(1);
        draw(&mut ld, &store);
        assert_eq!(ld.selected_key(), Some(k(2)));

        store.move_to(k(2), 0);
        let after = draw(&mut ld, &store);
        assert_eq!(ld.cursor, 0, "the view index follows the key");
        assert_eq!(ld.selected_key(), Some(k(2)));
        assert!(after.text().contains("bravo"));

        store.remove(k(2));
        draw(&mut ld, &store);
        assert_eq!(
            ld.selected_key(),
            Some(k(1)),
            "successor takes over deterministically"
        );
    }

    #[test]
    fn keyed_list_detail_click_reaches_the_clicked_key() {
        let mut store = store();
        let mut ld = ListDetail::new(MASTER, DETAIL, Split::new(50, 36, 40));
        let frame = draw(&mut ld, &store);
        let id = frame
            .hits
            .regions()
            .iter()
            .rev()
            .find(|r| r.area.contains(ratatui::layout::Position::new(4, 2)) && r.id != MASTER)
            .map(|r| r.id)
            .expect("a row hit");
        store.move_to(k(2), 2);
        draw(&mut ld, &store);
        let mut focus = Focus::default();
        let (_, event) = ld.click_source(id, &mut focus);
        assert_eq!(event, Some(Event::Row(2)));
        assert_eq!(
            ld.selected_key(),
            Some(k(2)),
            "the key, not the position, was clicked"
        );
    }
}

#[test]
fn keyed_state_reports_view_positions_and_ends_edits_and_presses() {
    let (mut state, mut source) = bound(&[1, 2, 3, 4]);
    assert!(state.is_authoritative(k(2)) && !state.is_authoritative(k(9)));
    assert_eq!(state.visible_key(1), Some(k(2)));
    assert_eq!(state.visible_key(9), None);

    assert!(state.adopt_cursor_view(2));
    assert_eq!(state.cursor_view(), Some(2));
    assert!(state.clear_cursor());
    assert_eq!(state.cursor_view(), None);
    assert!(
        !state.clear_cursor(),
        "a cleared cursor has nothing to clear"
    );

    assert!(state.adopt_selection_view([3, 1], Some(1)));
    assert_eq!(state.selected_view(), [1, 3]);
    assert_eq!(state.anchor_view(), Some(1));
    assert_eq!(state.selected_len(), 2);
    assert!(state.replace_selection([k(4)]));
    assert_eq!(state.selected_view(), [3]);
    assert_eq!(state.selected_len(), 1);

    let revision = state.expanded_revision();
    assert!(state.set_expanded(k(1), true));
    assert_eq!(state.expanded_revision(), revision + 1);
    assert!(!state.set_expanded(k(1), true));
    assert_eq!(state.expanded_revision(), revision + 1);

    assert!(state.begin_edit(k(2)));
    assert_eq!(state.finish_edit(), Some(k(2)));
    assert_eq!(state.finish_edit(), None);

    assert!(state.capture_press(k(3), 7));
    assert_eq!(state.pressed(), Some(k(3)));
    state.cancel_press();
    assert_eq!(state.pressed(), None);
    assert!(
        !state.release_press(k(3), 7),
        "a cancelled press releases nothing"
    );

    source.show(&[4, 3]);
    state.reconcile(&source).expect("valid");
    assert_eq!(state.visible_key(0), Some(k(4)));
    assert_eq!(state.selected_view(), [0]);
}
