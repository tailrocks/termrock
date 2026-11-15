//! Keyed collection state: identity survives reorder and filter, and is
//! pruned or cancelled deterministically when a record disappears.

use std::collections::HashSet;

use junie_tui::core::id::ItemKey;
use junie_tui::core::keyed::{KeyedSourceError, KeyedState, KeyedVec, Reconcile, KeyedRecord};
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
    assert!(!state.set_selected(k(2), true), "deleted keys cannot be selected");
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
    assert_eq!(state.cursor(), Some(k(2)), "predecessor when no successor is enabled");

    for key in [1, 2] {
        source.disable(key);
    }
    state.reconcile(&source).expect("valid");
    assert_eq!(state.cursor(), None);
    assert!(!state.set_cursor(k(1)), "disabled rows cannot take the cursor");
    assert!(!state.set_cursor(k(99)), "absent rows cannot take the cursor");

    source.disabled.clear();
    source.data += 1;
    state.reconcile(&source).expect("valid");
    assert_eq!(state.cursor(), Some(k(1)), "a fresh cursor starts on the first enabled row");
}

#[test]
fn keyed_press_cannot_retarget_after_reorder() {
    let (mut state, mut source) = bound(&[1, 2, 3]);
    assert!(state.capture_press(k(1), 7));
    source.show(&[2, 1, 3]);
    let out = state.reconcile(&source).expect("valid");
    assert!(out.pointer_cancelled);
    assert!(!state.release_press(k(1), 7));
    assert!(!state.release_press(k(2), 7), "the row now at the old position is not the target");

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
    assert!(state.reconcile(&source).expect("valid").edit_cancelled, "payload revision moved");
    assert_eq!(state.edit_key(), None);

    assert!(state.begin_edit(k(3)));
    source.show(&[2, 1]);
    assert!(state.reconcile(&source).expect("valid").edit_cancelled, "hidden");

    source.show(&[2, 3, 1]);
    state.reconcile(&source).expect("valid");
    assert!(state.begin_edit(k(3)));
    source.remove(3);
    assert!(state.reconcile(&source).expect("valid").edit_cancelled, "deleted");

    assert!(state.begin_edit(k(1)));
    source.disable(1);
    assert!(state.reconcile(&source).expect("valid").edit_cancelled, "disabled");
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
    assert!(!state.release_press(k(1), 3), "the failed attempt consumed the capture");

    assert!(state.capture_press(k(1), 3));
    assert!(!state.release_press(k(1), 4), "wrong layout generation");
    assert!(!state.release_press(k(1), 3));

    assert!(state.capture_press(k(1), 3));
    assert!(state.release_press(k(1), 3));
    assert!(!state.capture_press(k(99), 3), "absent keys cannot be captured");
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
    let mut store = KeyedVec::try_from_rows((1..=500).map(|n| (k(n), Row::new("r")))).expect("valid");
    let mut state = KeyedState::new();
    let (_, first) = Work::measure(|| state.reconcile(&store).expect("valid"));
    assert_eq!(first.source_rows, 1_000, "authoritative plus visible, once: {first:?}");
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
        Err(KeyedSourceError::PositionalKey { index: 2, authoritative: true })
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
    let same = store.iter().map(|(key, row)| (key, row.clone())).collect::<Vec<_>>();
    assert_eq!(store.replace_if_changed(same).expect("valid"), false);
    assert_eq!(store.stamp(), before);
    assert_eq!(
        store.replace_if_changed([(k(9), Row::new("n"))]).expect("valid"),
        true
    );
    assert_eq!(store.stamp().instance, before.instance);
    assert_eq!(store.len(), 1);
}
