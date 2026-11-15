//! Keyed collection state: identity survives reorder and filter, and is
//! pruned or cancelled deterministically when a record disappears.

use std::collections::HashSet;

use junie_tui::core::id::ItemKey;
use junie_tui::core::keyed::{KeyedRecord, KeyedSourceError, KeyedState, KeyedVec, Reconcile};
use junie_tui::core::list::{KeyedRowSource, RowSource, SourceStamp};
use junie_tui::core::work::Work;

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

#[test]
fn keyed_resize_cancels_capture_without_source_scan() {
    let (mut state, source) = bound(&[1, 2, 3]);
    assert!(state.capture_press(k(2), 5));
    let (cancelled, work) = Work::measure(|| {
        let a = state.invalidate_layout(5);
        let b = state.invalidate_layout(6);
        let unchanged = state.reconcile(&source).expect("valid");
        (a, b, unchanged)
    });
    assert_eq!((cancelled.0, cancelled.1), (false, true));
    assert_eq!(cancelled.2, Reconcile::default());
    assert_eq!(work.source_rows, 0, "no enumeration: {work:?}");
    assert!(!state.release_press(k(2), 5));
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
fn keyed_unchanged_stamp_does_not_enumerate_the_source() {
    let mut store =
        KeyedVec::try_from_rows((1..=500).map(|n| (k(n), Row::new("r")))).expect("valid");
    let mut state = KeyedState::new();
    let (_, first) = Work::measure(|| state.reconcile(&store).expect("valid"));
    assert_eq!(
        first.source_rows, 1_000,
        "authoritative plus visible, once: {first:?}"
    );
    let (out, again) = Work::measure(|| state.reconcile(&store).expect("valid"));
    assert_eq!((out, again.source_rows), (Reconcile::default(), 0));

    store.update(k(7), |row| row.off = true);
    let (out, work) = Work::measure(|| state.reconcile(&store).expect("valid"));
    assert!(out.data_changed);
    assert!(work.source_rows <= 1_000, "{work:?}");
    assert!(!state.is_enabled(k(7)));
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
    assert_eq!(store.replace_if_changed(same).expect("valid"), false);
    assert_eq!(store.stamp(), before);
    assert_eq!(
        store
            .replace_if_changed([(k(9), Row::new("n"))])
            .expect("valid"),
        true
    );
    assert_eq!(store.stamp().instance, before.instance);
    assert_eq!(store.len(), 1);
}

mod widgets {
    use super::*;
    use crate::common::{chord, key, row};
    use junie_tui::core::focus::FocusRing;
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
        list.on_key_source(&key(KeyCode::Down), &store)
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
        list.on_key_source(&key(KeyCode::Down), &store)
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
        list.on_key_source(&key(KeyCode::Down), &store)
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
        list.on_pointer_source(&frame.event(Phase::Press, id_a, at), &store)
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

    #[test]
    fn keyed_list_click_activates_the_clicked_key() {
        let store = items(&[(1, "alpha"), (2, "bravo")]);
        let mut list =
            list(SelectMode::Single).click(junie_tui::core::selection::ClickPolicy::NAVIGATION);
        let frame = draw_list(&mut list, &store);
        let at = Position::new(6, 1);
        let id = frame.id_at(list.id, Part::ROW, at);
        let press = frame.event(Phase::Press, id, at);
        list.on_pointer_source(&press, &store).expect("valid");
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
            t.on_key_edit_source(&key(KeyCode::Char(c)), store)
                .expect("valid");
        }
    }

    #[test]
    fn borrowed_table_commits_by_key() {
        let mut store = table_store();
        let mut t = table();
        draw_table(&mut t, &mut store);
        t.on_key_edit_source(&key(KeyCode::Down), &mut store)
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
    fn borrowed_table_cancels_the_edit_when_its_row_is_removed() {
        let mut store = table_store();
        let mut t = table();
        draw_table(&mut t, &mut store);
        t.on_key_edit_source(&key(KeyCode::Enter), &mut store)
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
        t.on_key_edit_source(&key(KeyCode::Enter), &mut store)
            .expect("valid");
        for c in "zz".chars() {
            t.on_key_edit_source(&key(KeyCode::Char(c)), &mut store)
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
    fn keyed_table_press_cannot_retarget_after_reorder() {
        let mut store = table_store();
        let mut t = table();
        let frame = draw_table(&mut t, &mut store);
        let at = Position::new(8, 1);
        let id = frame.id_at(t.id, Part::CELL, at);
        t.on_pointer_edit_source(&frame.event(Phase::Press, id, at), &mut store)
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
