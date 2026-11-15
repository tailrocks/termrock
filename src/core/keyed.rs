//! Stable identity for collection state.
//!
//! [`KeyedState`] holds the cursor, anchor, selection, expansion, edit and
//! pointer-press targets of a collection by [`ItemKey`], and reconciles them
//! against a [`KeyedRowSource`] whenever the source revision changes. Indices
//! stay transient view positions; they never identify a record.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::core::id::ItemKey;
use crate::core::list::{KeyedRowSource, RowSource, SourceStamp};
#[cfg(test)]
use crate::core::work::{Counter, add};

/// What one reconciliation changed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reconcile {
    pub cursor_changed: bool,
    pub edit_cancelled: bool,
    pub pointer_cancelled: bool,
    pub data_changed: bool,
    pub view_changed: bool,
}

/// Why a source was rejected. The previous state is left untouched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyedSourceError {
    PositionalKey { index: usize, authoritative: bool },
    DuplicateAuthoritative { key: ItemKey },
    DuplicateVisible { key: ItemKey },
    UnknownVisible { key: ItemKey },
    MissingNode { key: ItemKey },
    Cycle { key: ItemKey },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PressTarget {
    key: ItemKey,
    visible_index: usize,
    layout_generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct EditTarget {
    key: ItemKey,
    data: u64,
}

#[derive(Clone, Debug, Default)]
pub struct KeyedState {
    stamp: Option<SourceStamp>,
    authoritative: HashSet<ItemKey>,
    visible: Vec<ItemKey>,
    positions: HashMap<ItemKey, usize>,
    enabled: HashSet<ItemKey>,
    cursor: Option<ItemKey>,
    anchor: Option<ItemKey>,
    selected: BTreeSet<ItemKey>,
    expanded: BTreeSet<ItemKey>,
    expanded_revision: u64,
    edit: Option<EditTarget>,
    press: Option<PressTarget>,
    /// A capture was dropped by reconciliation or a layout change and its
    /// click has not arrived yet.
    press_lost: bool,
}

struct Projection {
    authoritative: Option<HashSet<ItemKey>>,
    visible: Vec<ItemKey>,
    positions: HashMap<ItemKey, usize>,
    enabled: HashSet<ItemKey>,
}

impl KeyedState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Revision of the source this state last accepted.
    pub fn stamp(&self) -> Option<SourceStamp> {
        self.stamp
    }

    pub fn visible_len(&self) -> usize {
        self.visible.len()
    }

    pub fn visible_key(&self, index: usize) -> Option<ItemKey> {
        self.visible.get(index).copied()
    }

    pub fn visible_index(&self, key: ItemKey) -> Option<usize> {
        self.positions.get(&key).copied()
    }

    pub fn is_authoritative(&self, key: ItemKey) -> bool {
        self.authoritative.contains(&key)
    }

    pub fn is_enabled(&self, key: ItemKey) -> bool {
        self.enabled.contains(&key)
    }

    /// Brings this state in line with `source`.
    ///
    /// An identical stamp returns without enumerating the source. An invalid
    /// source returns its error and leaves every field as it was.
    pub fn reconcile<S: KeyedRowSource + ?Sized>(
        &mut self,
        source: &S,
    ) -> Result<Reconcile, KeyedSourceError> {
        let stamp = source.stamp();
        if self.stamp == Some(stamp) {
            return Ok(Reconcile::default());
        }
        let same_instance = self.stamp.is_some_and(|old| old.instance == stamp.instance);
        let reuse_authoritative =
            same_instance && self.stamp.is_some_and(|old| old.data == stamp.data);
        let next = Self::project(source, reuse_authoritative.then_some(&self.authoritative))?;

        let old_stamp = self.stamp;
        let mut out = Reconcile {
            data_changed: old_stamp
                .is_none_or(|old| old.instance != stamp.instance || old.data != stamp.data),
            view_changed: old_stamp
                .is_none_or(|old| old.view != stamp.view || old.instance != stamp.instance),
            ..Reconcile::default()
        };
        let old_cursor = self.cursor;
        let old_position = old_cursor
            .filter(|_| same_instance)
            .and_then(|key| self.positions.get(&key).copied());

        if !same_instance {
            self.cursor = None;
            self.anchor = None;
            self.selected.clear();
            self.expanded.clear();
            self.expanded_revision += 1;
            out.edit_cancelled = self.edit.take().is_some();
            out.pointer_cancelled = self.press.take().is_some();
            self.press_lost |= out.pointer_cancelled;
        }
        if let Some(authoritative) = next.authoritative {
            self.authoritative = authoritative;
        }
        let authoritative = &self.authoritative;
        self.selected.retain(|key| authoritative.contains(key));
        let expanded = self.expanded.len();
        self.expanded.retain(|key| authoritative.contains(key));
        if self.expanded.len() != expanded {
            self.expanded_revision += 1;
        }
        if self.anchor.is_some_and(|key| !authoritative.contains(&key)) {
            self.anchor = None;
        }
        self.visible = next.visible;
        self.positions = next.positions;
        self.enabled = next.enabled;
        self.stamp = Some(stamp);

        let keep = self
            .cursor
            .filter(|key| self.enabled.contains(key) && self.positions.contains_key(key));
        self.cursor = keep.or_else(|| self.nearest_enabled(old_position.unwrap_or(0)));
        out.cursor_changed = self.cursor != old_cursor;

        if let Some(edit) = self.edit {
            let alive = self.enabled.contains(&edit.key) && edit.data == stamp.data;
            if !alive {
                self.edit = None;
                out.edit_cancelled = true;
            }
        }
        if let Some(press) = self.press {
            let alive = self.enabled.contains(&press.key)
                && self.positions.get(&press.key) == Some(&press.visible_index);
            if !alive {
                self.press = None;
                self.press_lost = true;
                out.pointer_cancelled = true;
            }
        }
        Ok(out)
    }

    fn project<S: KeyedRowSource + ?Sized>(
        source: &S,
        reuse: Option<&HashSet<ItemKey>>,
    ) -> Result<Projection, KeyedSourceError> {
        let authoritative = match reuse {
            Some(_) => None,
            None => {
                let n = source.source_len();
                #[cfg(test)]
                add(Counter::SourceRows, n);
                let mut set = HashSet::with_capacity(n);
                for index in 0..n {
                    let key = source.source_key(index);
                    if matches!(key, ItemKey::Index(_)) {
                        return Err(KeyedSourceError::PositionalKey {
                            index,
                            authoritative: true,
                        });
                    }
                    if !set.insert(key) {
                        return Err(KeyedSourceError::DuplicateAuthoritative { key });
                    }
                }
                Some(set)
            }
        };
        let known = authoritative.as_ref().or(reuse).expect("one set exists");
        let n = source.len();
        #[cfg(test)]
        add(Counter::SourceRows, n);
        let mut visible = Vec::with_capacity(n);
        let mut positions = HashMap::with_capacity(n);
        let mut enabled = HashSet::new();
        for index in 0..n {
            let key = source.key(index);
            if matches!(key, ItemKey::Index(_)) {
                return Err(KeyedSourceError::PositionalKey {
                    index,
                    authoritative: false,
                });
            }
            if positions.insert(key, index).is_some() {
                return Err(KeyedSourceError::DuplicateVisible { key });
            }
            if !known.contains(&key) {
                return Err(KeyedSourceError::UnknownVisible { key });
            }
            if source.enabled(index) {
                enabled.insert(key);
            }
            visible.push(key);
        }
        Ok(Projection {
            authoritative,
            visible,
            positions,
            enabled,
        })
    }

    /// First enabled row at or after `from`, else the last enabled row before.
    fn nearest_enabled(&self, from: usize) -> Option<ItemKey> {
        let is_enabled = |key: &&ItemKey| self.enabled.contains(key);
        self.visible
            .iter()
            .skip(from)
            .find(is_enabled)
            .or_else(|| self.visible.iter().take(from).rev().find(is_enabled))
            .copied()
    }

    pub fn cursor(&self) -> Option<ItemKey> {
        self.cursor
    }

    /// Moves the cursor to a visible, enabled key. Returns whether it moved.
    pub fn set_cursor(&mut self, key: ItemKey) -> bool {
        if self.cursor == Some(key) || !self.enabled.contains(&key) {
            return false;
        }
        self.cursor = Some(key);
        true
    }

    pub fn clear_cursor(&mut self) -> bool {
        self.cursor.take().is_some()
    }

    pub fn anchor(&self) -> Option<ItemKey> {
        self.anchor
    }

    pub fn set_anchor(&mut self, key: Option<ItemKey>) -> bool {
        if key.is_some_and(|key| !self.authoritative.contains(&key)) || self.anchor == key {
            return false;
        }
        self.anchor = key;
        true
    }

    pub fn selected(&self) -> impl Iterator<Item = ItemKey> + '_ {
        self.selected.iter().copied()
    }

    pub fn selected_len(&self) -> usize {
        self.selected.len()
    }

    pub fn is_selected(&self, key: ItemKey) -> bool {
        self.selected.contains(&key)
    }

    /// Selects or deselects an authoritative key. Hidden keys stay selected.
    /// A visible disabled key cannot be newly selected.
    pub fn set_selected(&mut self, key: ItemKey, selected: bool) -> bool {
        if !self.authoritative.contains(&key) {
            return false;
        }
        if !selected {
            return self.selected.remove(&key);
        }
        if self.positions.contains_key(&key) && !self.enabled.contains(&key) {
            return false;
        }
        self.selected.insert(key)
    }

    /// Replaces the selection by `keys`, ignoring unknown and disabled ones.
    /// Returns whether the selection changed.
    pub fn replace_selection(&mut self, keys: impl IntoIterator<Item = ItemKey>) -> bool {
        let next: BTreeSet<ItemKey> = keys
            .into_iter()
            .filter(|key| {
                self.authoritative.contains(key)
                    && (!self.positions.contains_key(key) || self.enabled.contains(key))
            })
            .collect();
        let changed = next != self.selected;
        self.selected = next;
        changed
    }

    pub fn expanded(&self) -> impl Iterator<Item = ItemKey> + '_ {
        self.expanded.iter().copied()
    }

    pub fn is_expanded(&self, key: ItemKey) -> bool {
        self.expanded.contains(&key)
    }

    pub fn set_expanded(&mut self, key: ItemKey, expanded: bool) -> bool {
        if !self.authoritative.contains(&key) {
            return false;
        }
        let changed = if expanded {
            self.expanded.insert(key)
        } else {
            self.expanded.remove(&key)
        };
        self.expanded_revision += u64::from(changed);
        changed
    }

    /// Advances whenever the expanded key set changes, so a projection built
    /// from it can tell when to rebuild.
    pub fn expanded_revision(&self) -> u64 {
        self.expanded_revision
    }

    pub fn edit_key(&self) -> Option<ItemKey> {
        self.edit.map(|edit| edit.key)
    }

    /// Starts an edit of a visible, enabled row. It is cancelled, never
    /// transferred, when that row is deleted, hidden, disabled or its payload
    /// revision changes before the edit resolves.
    pub fn begin_edit(&mut self, key: ItemKey) -> bool {
        let Some(stamp) = self.stamp else {
            return false;
        };
        if !self.enabled.contains(&key) || self.edit_key() == Some(key) {
            return false;
        }
        self.edit = Some(EditTarget {
            key,
            data: stamp.data,
        });
        true
    }

    pub fn cancel_edit(&mut self) -> bool {
        self.edit.take().is_some()
    }

    /// Records that the edit was committed through the provider, which moved
    /// its `data` revision on purpose, and ends it.
    pub fn finish_edit(&mut self) -> Option<ItemKey> {
        self.edit.take().map(|edit| edit.key)
    }

    pub fn capture_press(&mut self, key: ItemKey, layout_generation: u64) -> bool {
        self.press_lost = false;
        let Some(&visible_index) = self.positions.get(&key) else {
            return false;
        };
        if !self.enabled.contains(&key) {
            return false;
        }
        self.press = Some(PressTarget {
            key,
            visible_index,
            layout_generation,
        });
        true
    }

    pub fn pressed(&self) -> Option<ItemKey> {
        self.press.map(|press| press.key)
    }

    /// Consumes the capture on every attempt. True only when `key` and
    /// `layout_generation` match the capture and the row is still in place.
    pub fn release_press(&mut self, key: ItemKey, layout_generation: u64) -> bool {
        let Some(press) = self.press.take() else {
            return false;
        };
        press.key == key
            && press.layout_generation == layout_generation
            && self.enabled.contains(&key)
            && self.positions.get(&key) == Some(&press.visible_index)
    }

    /// Decides a click. A click that follows a press must land on the pressed
    /// row, so it is refused when that press was dropped meanwhile (the source
    /// was reordered, replaced or the layout moved). A click with no press at
    /// all, such as one that is not pointer-driven, is accepted.
    pub(crate) fn release_click(&mut self, key: ItemKey, layout_generation: u64) -> bool {
        if self.press.is_some() {
            return self.release_press(key, layout_generation);
        }
        !std::mem::take(&mut self.press_lost)
    }

    /// Cancels a capture made under another layout generation. Resizing moves
    /// hit regions without changing the source, so `reconcile` cannot see it.
    pub fn invalidate_layout(&mut self, generation: u64) -> bool {
        if self
            .press
            .is_some_and(|press| press.layout_generation != generation)
        {
            self.press = None;
            self.press_lost = true;
            return true;
        }
        false
    }
}

static NEXT_INSTANCE: AtomicU64 = AtomicU64::new(1);

pub(crate) fn fresh_instance() -> ItemKey {
    let id = NEXT_INSTANCE
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
        .expect("provider instance ids exhausted");
    ItemKey::num(id)
}

/// A record a [`KeyedVec`] can show as a row.
pub trait KeyedRecord {
    fn label(&self) -> &str;

    fn enabled(&self) -> bool {
        true
    }
}

/// An owned, ordered store of keyed records that advances its own revisions.
///
/// Mutation goes through methods so applications never maintain counters.
/// `Clone` copies the data but allocates a fresh instance: a clone is an
/// independent store, not another handle to this one.
#[derive(Debug)]
pub struct KeyedVec<T> {
    instance: ItemKey,
    data: u64,
    view: u64,
    rows: Vec<(ItemKey, T)>,
    index: HashMap<ItemKey, usize>,
}

impl<T: Clone> Clone for KeyedVec<T> {
    fn clone(&self) -> Self {
        Self {
            instance: fresh_instance(),
            data: self.data,
            view: self.view,
            rows: self.rows.clone(),
            index: self.index.clone(),
        }
    }
}

impl<T> Default for KeyedVec<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> KeyedVec<T> {
    pub fn new() -> Self {
        Self {
            instance: fresh_instance(),
            data: 0,
            view: 0,
            rows: Vec::new(),
            index: HashMap::new(),
        }
    }

    pub fn try_from_rows(
        rows: impl IntoIterator<Item = (ItemKey, T)>,
    ) -> Result<Self, KeyedSourceError> {
        let mut out = Self::new();
        out.replace(rows)?;
        Ok(out)
    }

    fn index_rows(rows: &[(ItemKey, T)]) -> Result<HashMap<ItemKey, usize>, KeyedSourceError> {
        let mut index = HashMap::with_capacity(rows.len());
        for (position, (key, _)) in rows.iter().enumerate() {
            if matches!(key, ItemKey::Index(_)) {
                return Err(KeyedSourceError::PositionalKey {
                    index: position,
                    authoritative: true,
                });
            }
            if index.insert(*key, position).is_some() {
                return Err(KeyedSourceError::DuplicateAuthoritative { key: *key });
            }
        }
        Ok(index)
    }

    fn bump(&mut self) {
        self.data += 1;
        self.view += 1;
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn get(&self, key: ItemKey) -> Option<&T> {
        self.index.get(&key).map(|&at| &self.rows[at].1)
    }

    pub fn position(&self, key: ItemKey) -> Option<usize> {
        self.index.get(&key).copied()
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (ItemKey, &T)> + '_ {
        self.rows.iter().map(|(key, value)| (*key, value))
    }

    pub fn key_at(&self, position: usize) -> Option<ItemKey> {
        self.rows.get(position).map(|(key, _)| *key)
    }

    pub fn value_at(&self, position: usize) -> Option<&T> {
        self.rows.get(position).map(|(_, value)| value)
    }

    /// Replaces every row, keeping this store's instance.
    pub fn replace(
        &mut self,
        rows: impl IntoIterator<Item = (ItemKey, T)>,
    ) -> Result<(), KeyedSourceError> {
        let rows: Vec<(ItemKey, T)> = rows.into_iter().collect();
        let index = Self::index_rows(&rows)?;
        self.rows = rows;
        self.index = index;
        self.bump();
        Ok(())
    }

    /// Inserts a new record before `position` (or at the end when beyond it).
    pub fn insert(
        &mut self,
        position: usize,
        key: ItemKey,
        value: T,
    ) -> Result<(), KeyedSourceError> {
        if matches!(key, ItemKey::Index(_)) {
            return Err(KeyedSourceError::PositionalKey {
                index: position.min(self.rows.len()),
                authoritative: true,
            });
        }
        if self.index.contains_key(&key) {
            return Err(KeyedSourceError::DuplicateAuthoritative { key });
        }
        let position = position.min(self.rows.len());
        self.rows.insert(position, (key, value));
        for (at, (key, _)) in self.rows.iter().enumerate().skip(position) {
            self.index.insert(*key, at);
        }
        self.bump();
        Ok(())
    }

    pub fn push(&mut self, key: ItemKey, value: T) -> Result<(), KeyedSourceError> {
        self.insert(self.rows.len(), key, value)
    }

    pub fn remove(&mut self, key: ItemKey) -> Option<T> {
        let at = self.index.remove(&key)?;
        let (_, value) = self.rows.remove(at);
        for (position, (key, _)) in self.rows.iter().enumerate().skip(at) {
            self.index.insert(*key, position);
        }
        self.bump();
        Some(value)
    }

    /// Edits one record. The data revision advances after the closure, because
    /// the store cannot tell whether it changed the payload.
    pub fn update<R>(&mut self, key: ItemKey, edit: impl FnOnce(&mut T) -> R) -> Option<R> {
        let at = *self.index.get(&key)?;
        let out = edit(&mut self.rows[at].1);
        self.data += 1;
        Some(out)
    }

    /// Moves a record so it sits at `position`, advancing the view revision.
    pub fn move_to(&mut self, key: ItemKey, position: usize) -> bool {
        let Some(&from) = self.index.get(&key) else {
            return false;
        };
        let to = position.min(self.rows.len() - 1);
        if from == to {
            return false;
        }
        let row = self.rows.remove(from);
        self.rows.insert(to, row);
        for (at, (key, _)) in self
            .rows
            .iter()
            .enumerate()
            .take(from.max(to) + 1)
            .skip(from.min(to))
        {
            self.index.insert(*key, at);
        }
        self.view += 1;
        true
    }

    /// Replaces every row unless the new rows equal the current ones.
    pub fn replace_if_changed(
        &mut self,
        rows: impl IntoIterator<Item = (ItemKey, T)>,
    ) -> Result<bool, KeyedSourceError>
    where
        T: PartialEq,
    {
        let rows: Vec<(ItemKey, T)> = rows.into_iter().collect();
        if rows == self.rows {
            return Ok(false);
        }
        self.replace(rows)?;
        Ok(true)
    }

    pub fn stamp(&self) -> SourceStamp {
        SourceStamp {
            instance: self.instance,
            data: self.data,
            view: self.view,
        }
    }
}

impl<T: KeyedRecord> RowSource for KeyedVec<T> {
    fn len(&self) -> usize {
        self.rows.len()
    }

    fn enabled(&self, i: usize) -> bool {
        self.rows[i].1.enabled()
    }

    fn label(&self, i: usize) -> &str {
        self.rows[i].1.label()
    }

    fn key(&self, i: usize) -> ItemKey {
        self.rows[i].0
    }
}

impl<T: KeyedRecord> KeyedRowSource for KeyedVec<T> {
    fn stamp(&self) -> SourceStamp {
        KeyedVec::stamp(self)
    }

    fn source_len(&self) -> usize {
        self.rows.len()
    }

    fn source_key(&self, source_index: usize) -> ItemKey {
        self.rows[source_index].0
    }
}

/// Index-space views. A widget whose engine addresses rows by view position
/// mirrors this state into indices for one call, then adopts the result.
impl KeyedState {
    pub fn cursor_view(&self) -> Option<usize> {
        self.cursor.and_then(|key| self.visible_index(key))
    }

    /// Moves the cursor to the key shown at `index`. Disabled and out-of-range
    /// rows are refused, like [`KeyedState::set_cursor`].
    pub fn adopt_cursor_view(&mut self, index: usize) -> bool {
        self.visible_key(index)
            .is_some_and(|key| self.set_cursor(key))
    }

    /// Visible selected rows as view positions, ascending.
    pub fn selected_view(&self) -> Vec<usize> {
        let mut rows: Vec<usize> = self
            .selected
            .iter()
            .filter_map(|key| self.positions.get(key).copied())
            .collect();
        rows.sort_unstable();
        rows
    }

    pub fn anchor_view(&self) -> Option<usize> {
        self.anchor.and_then(|key| self.visible_index(key))
    }

    /// Replaces the visible part of the selection with the rows at `rows`,
    /// keeping selected keys a filter currently hides. Disabled rows are not
    /// selected. Returns whether the selection changed.
    pub fn adopt_selection_view(
        &mut self,
        rows: impl IntoIterator<Item = usize>,
        anchor: Option<usize>,
    ) -> bool {
        let mut next: BTreeSet<ItemKey> = self
            .selected
            .iter()
            .copied()
            .filter(|key| !self.positions.contains_key(key))
            .collect();
        next.extend(
            rows.into_iter()
                .filter_map(|index| self.visible_key(index))
                .filter(|key| self.enabled.contains(key)),
        );
        let changed = next != self.selected;
        self.selected = next;
        let hidden_anchor = self.anchor.filter(|key| !self.positions.contains_key(key));
        self.anchor = anchor
            .and_then(|index| self.visible_key(index))
            .or(hidden_anchor);
        changed
    }
}
