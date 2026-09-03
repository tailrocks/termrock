// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Roving focus group — active descendant inside a focused collection.
//!
//! External keyboard ownership stays on [`super::FocusGraph`] (one roving
//! surface id). This type owns **which child is active** (cursor / aria-activedescendant).
//!
//! Behavioral reference: Radix RovingFocusGroup, adapted to terminal intents
//! and immediate-mode entry lists (including virtualized windows).
use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    interaction::{NavigationMove, UiIntent},
    keymap::{KeyBinding, KeyChord, Keymap, Visibility},
};

/// Axis for arrow-key roving.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum RovingOrientation {
    /// Up/Down (and j/k via list intents) move the active item.
    #[default]
    Vertical,
    /// Left/Right move the active item.
    Horizontal,
    /// Both axes map to next/previous (toolbars that accept either).
    Both,
}

/// One item in a roving group (frame projection; may be a virtualized window).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RovingEntry<Id> {
    /// Stable identity (not index — survives reordering when ids stable).
    pub id: Id,
    /// Disabled entries are skipped by movement and typeahead.
    pub enabled: bool,
    /// Typeahead / a11y label (empty skips typeahead match for this row).
    pub label: String,
}

impl<Id> RovingEntry<Id> {
    /// Enabled entry with label.
    #[must_use]
    pub fn new(id: Id, label: impl Into<String>) -> Self {
        Self {
            id,
            enabled: true,
            label: label.into(),
        }
    }

    /// Disabled flag.
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// Outcome of a roving movement (not activation).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RovingOutcome<Id> {
    /// No change.
    Ignored,
    /// Active descendant moved or was reconciled.
    ActiveChanged {
        /// Previous active id.
        from: Option<Id>,
        /// New active id.
        to: Option<Id>,
    },
}

impl<Id: PartialEq> RovingOutcome<Id> {
    /// Whether the active id changed.
    #[must_use]
    pub fn changed(&self) -> bool {
        matches!(self, Self::ActiveChanged { .. })
    }
}

/// Active-descendant cursor for menus, radios, tabs, toolbars, collections.
///
/// Does **not** own external focus — pair with `FocusNode::roving_collection`
/// on [`super::FocusGraph`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RovingFocusGroup<Id> {
    active: Option<Id>,
    orientation: RovingOrientation,
    wrap: bool,
    typeahead: String,
}

impl<Id> Default for RovingFocusGroup<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id> RovingFocusGroup<Id> {
    /// Empty active, vertical, wrapping.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            active: None,
            orientation: RovingOrientation::Vertical,
            wrap: true,
            typeahead: String::new(),
        }
    }

    /// Sets orientation.
    #[must_use]
    pub const fn orientation(mut self, orientation: RovingOrientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Whether movement wraps at ends (default true).
    #[must_use]
    pub const fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }

    /// Current orientation.
    #[must_use]
    pub const fn orientation_mode(&self) -> RovingOrientation {
        self.orientation
    }

    /// Whether wrap is enabled.
    #[must_use]
    pub const fn wraps(&self) -> bool {
        self.wrap
    }

    /// Active descendant id.
    #[must_use]
    pub const fn active(&self) -> Option<&Id> {
        self.active.as_ref()
    }

    /// Typeahead buffer (for tests / Studio).
    #[must_use]
    pub fn typeahead_buffer(&self) -> &str {
        &self.typeahead
    }

    /// Clears typeahead buffer.
    pub fn clear_typeahead(&mut self) {
        self.typeahead.clear();
    }
}

impl<Id: Clone + PartialEq> RovingFocusGroup<Id> {
    /// Sets active id without validating against entries (host may validate via reconcile).
    pub fn set_active(&mut self, id: Option<Id>) {
        self.active = id;
        self.typeahead.clear();
    }

    /// Enabled entries only, in list order.
    fn enabled_indices(entries: &[RovingEntry<Id>]) -> Vec<usize> {
        entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.enabled)
            .map(|(i, _)| i)
            .collect()
    }

    fn outcome(&self, from: Option<Id>) -> RovingOutcome<Id> {
        if from == self.active {
            RovingOutcome::Ignored
        } else {
            RovingOutcome::ActiveChanged {
                from,
                to: self.active.clone(),
            }
        }
    }

    fn reconciliation_outcome(&mut self, from: Option<Id>) -> RovingOutcome<Id> {
        let out = self.outcome(from);
        if out.changed() {
            self.typeahead.clear();
        }
        out
    }

    /// Ensures `active` is an enabled entry; otherwise nearest enabled (or None).
    ///
    /// Call after virtual window changes, insert/remove, or disabled flips.
    pub fn reconcile(&mut self, entries: &[RovingEntry<Id>]) -> RovingOutcome<Id> {
        let from = self.active.clone();
        let enabled = Self::enabled_indices(entries);
        if enabled.is_empty() {
            self.active = None;
            return self.reconciliation_outcome(from);
        }
        if let Some(cur) = &self.active {
            if let Some(pos) = entries.iter().position(|e| &e.id == cur) {
                if entries[pos].enabled {
                    return RovingOutcome::Ignored;
                }
                // Disabled: pick next enabled after position, else previous, else first.
                let next = enabled
                    .iter()
                    .copied()
                    .find(|&i| i > pos)
                    .or_else(|| enabled.iter().copied().rev().find(|&i| i < pos))
                    .unwrap_or(enabled[0]);
                self.active = Some(entries[next].id.clone());
                return self.reconciliation_outcome(from);
            }
            // Missing id: try same index if we can find nothing — fall through to first.
        }
        // Prefer first enabled.
        self.active = Some(entries[enabled[0]].id.clone());
        self.reconciliation_outcome(from)
    }

    /// Index of active among all entries, if present.
    fn active_index(&self, entries: &[RovingEntry<Id>]) -> Option<usize> {
        self.active
            .as_ref()
            .and_then(|id| entries.iter().position(|e| &e.id == id))
    }

    /// Moves by signed steps among enabled entries.
    pub fn move_by(&mut self, entries: &[RovingEntry<Id>], steps: isize) -> RovingOutcome<Id> {
        let from = self.active.clone();
        let enabled = Self::enabled_indices(entries);
        if enabled.is_empty() || steps == 0 {
            return self.reconcile(entries);
        }
        if let Some(disabled_index) = self
            .active
            .as_ref()
            .and_then(|active| entries.iter().position(|entry| &entry.id == active))
            .filter(|&index| !entries[index].enabled)
        {
            let insertion = enabled
                .iter()
                .position(|&index| index > disabled_index)
                .unwrap_or(enabled.len());
            let magnitude = steps.unsigned_abs();
            let target = if steps > 0 {
                let start = if insertion < enabled.len() {
                    insertion
                } else if self.wrap {
                    0
                } else {
                    enabled.len() - 1
                };
                if self.wrap {
                    (start + (magnitude.saturating_sub(1) % enabled.len())) % enabled.len()
                } else {
                    start
                        .saturating_add(magnitude.saturating_sub(1))
                        .min(enabled.len() - 1)
                }
            } else {
                let start = insertion
                    .checked_sub(1)
                    .unwrap_or_else(|| if self.wrap { enabled.len() - 1 } else { 0 });
                if self.wrap {
                    let back = magnitude.saturating_sub(1) % enabled.len();
                    if back <= start {
                        start - back
                    } else {
                        enabled.len() - (back - start)
                    }
                } else {
                    start.saturating_sub(magnitude.saturating_sub(1))
                }
            };
            self.active = Some(entries[enabled[target]].id.clone());
            self.typeahead.clear();
            return self.outcome(from);
        }
        let missing_active = self
            .active
            .as_ref()
            .is_some_and(|active| !entries.iter().any(|entry| &entry.id == active));
        let _ = self.reconcile(entries);
        if missing_active {
            return self.outcome(from);
        }
        let from = self.active.clone().or(from);
        let enabled = Self::enabled_indices(entries);
        let cur = self
            .active
            .as_ref()
            .and_then(|id| enabled.iter().position(|&i| &entries[i].id == id))
            .unwrap_or(0);
        let len = enabled.len() as isize;
        let next = if self.wrap {
            (cur as isize + steps).rem_euclid(len) as usize
        } else {
            (cur as isize + steps).clamp(0, len - 1) as usize
        };
        self.active = Some(entries[enabled[next]].id.clone());
        self.typeahead.clear();
        self.outcome(from)
    }

    /// Next enabled item.
    pub fn move_next(&mut self, entries: &[RovingEntry<Id>]) -> RovingOutcome<Id> {
        self.move_by(entries, 1)
    }

    /// Previous enabled item.
    pub fn move_previous(&mut self, entries: &[RovingEntry<Id>]) -> RovingOutcome<Id> {
        self.move_by(entries, -1)
    }

    /// First enabled item.
    pub fn move_first(&mut self, entries: &[RovingEntry<Id>]) -> RovingOutcome<Id> {
        let from = self.active.clone();
        let enabled = Self::enabled_indices(entries);
        if enabled.is_empty() {
            self.active = None;
            return self.outcome(from);
        }
        self.active = Some(entries[enabled[0]].id.clone());
        self.typeahead.clear();
        self.outcome(from)
    }

    /// Last enabled item.
    pub fn move_last(&mut self, entries: &[RovingEntry<Id>]) -> RovingOutcome<Id> {
        let from = self.active.clone();
        let enabled = Self::enabled_indices(entries);
        if enabled.is_empty() {
            self.active = None;
            return self.outcome(from);
        }
        self.active = Some(entries[*enabled.last().expect("non-empty")].id.clone());
        self.typeahead.clear();
        self.outcome(from)
    }

    /// Semantic intents: Move / First / Last only.
    pub fn handle_intent(
        &mut self,
        intent: UiIntent,
        entries: &[RovingEntry<Id>],
    ) -> RovingOutcome<Id> {
        match intent {
            UiIntent::Move(NavigationMove::Next | NavigationMove::Down | NavigationMove::Right) => {
                self.move_next(entries)
            }
            UiIntent::Move(
                NavigationMove::Previous | NavigationMove::Up | NavigationMove::Left,
            ) => self.move_previous(entries),
            UiIntent::Move(NavigationMove::First) => self.move_first(entries),
            UiIntent::Move(NavigationMove::Last) => self.move_last(entries),
            _ => RovingOutcome::Ignored,
        }
    }

    /// Whether a key is a navigation key for this orientation (before typeahead).
    fn orientation_accepts_arrow(&self, code: KeyCode) -> bool {
        match self.orientation {
            RovingOrientation::Vertical => matches!(code, KeyCode::Up | KeyCode::Down),
            RovingOrientation::Horizontal => matches!(code, KeyCode::Left | KeyCode::Right),
            RovingOrientation::Both => {
                matches!(
                    code,
                    KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right
                )
            }
        }
    }

    /// Key routing: Home/End, orientation arrows, printable typeahead.
    ///
    /// Does not Activate — host maps Enter/Space after consulting [`Self::active`].
    pub fn handle_key(&mut self, key: KeyEvent, entries: &[RovingEntry<Id>]) -> RovingOutcome<Id> {
        if key.is_release() || entries.is_empty() {
            return RovingOutcome::Ignored;
        }
        if !key.modifiers.is_empty()
            && key.modifiers != KeyModifiers::SHIFT
            && !matches!(key.code, KeyCode::Char(_))
        {
            return RovingOutcome::Ignored;
        }
        match key.code {
            KeyCode::Home => self.move_first(entries),
            KeyCode::End => self.move_last(entries),
            code if self.orientation_accepts_arrow(code) => match code {
                KeyCode::Down | KeyCode::Right => self.move_next(entries),
                KeyCode::Up | KeyCode::Left => self.move_previous(entries),
                _ => RovingOutcome::Ignored,
            },
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT)
                    && !c.is_control() =>
            {
                self.typeahead_char(c, entries)
            }
            _ => RovingOutcome::Ignored,
        }
    }

    /// Append typeahead char and jump to first enabled label prefix match (case-insensitive).
    pub fn typeahead_char(&mut self, ch: char, entries: &[RovingEntry<Id>]) -> RovingOutcome<Id> {
        if ch == '\u{1b}' {
            self.typeahead.clear();
            return RovingOutcome::Ignored;
        }
        let from = self.active.clone();
        self.typeahead.push(ch);
        let needle = self.typeahead.to_lowercase();
        // Search from next after current, then wrap full list.
        let start = self.active_index(entries).map(|i| i + 1).unwrap_or(0) % entries.len().max(1);
        let n = entries.len();
        for offset in 0..n {
            let i = (start + offset) % n;
            let e = &entries[i];
            if !e.enabled || e.label.is_empty() {
                continue;
            }
            if e.label.to_lowercase().starts_with(&needle) {
                self.active = Some(e.id.clone());
                return self.outcome(from);
            }
        }
        // No match: try from start of buffer as single char restart
        if needle.chars().count() > 1 {
            self.typeahead.clear();
            self.typeahead.push(ch);
            let needle = self.typeahead.to_lowercase();
            for offset in 0..n {
                let i = (start + offset) % n;
                let e = &entries[i];
                if e.enabled && !e.label.is_empty() && e.label.to_lowercase().starts_with(&needle) {
                    self.active = Some(e.id.clone());
                    return self.outcome(from);
                }
            }
        }
        RovingOutcome::Ignored
    }

    /// Builds entries from parallel id/enabled/label slices (virtualized windows).
    #[must_use]
    pub fn entries_from_parts(
        ids: &[Id],
        enabled: &[bool],
        labels: &[&str],
    ) -> Vec<RovingEntry<Id>> {
        ids.iter()
            .enumerate()
            .map(|(i, id)| RovingEntry {
                id: id.clone(),
                enabled: enabled.get(i).copied().unwrap_or(true),
                label: labels.get(i).unwrap_or(&"").to_string(),
            })
            .collect()
    }

    /// Registers active-descendant geometry into a [`crate::interaction::SemanticScene`].
    ///
    /// Parent should be the collection surface id already registered on the semantic tree.
    pub fn register_semantic(
        &self,
        scene: &mut crate::interaction::SemanticScene<Id>,
        parent: &Id,
        entries: &[RovingEntry<Id>],
        areas: &[ratatui_core::layout::Rect],
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
    {
        for (i, e) in entries.iter().enumerate() {
            let area = areas.get(i).copied().unwrap_or_default();
            let mut node = crate::interaction::SemanticNode::control(e.id.clone(), area)
                .role(crate::interaction::SemanticRole::ListItem)
                .parent(parent.clone())
                .focusable(e.enabled)
                .disabled(!e.enabled)
                .state(crate::interaction::SemanticState {
                    selected: self.active.as_ref() == Some(&e.id),
                    ..crate::interaction::SemanticState::default()
                });
            if !e.label.is_empty() {
                node = node.label(e.label.clone());
            }
            let _ = scene.register(node);
        }
    }
}

/// Default key chords advertised for a vertical roving group (hints / help).
#[must_use]
pub fn roving_hint_keymap_vertical() -> Keymap<&'static str> {
    static BINDINGS: &[KeyBinding<&'static str>] = &[
        KeyBinding::borrowed(
            &[KeyChord::plain(KeyCode::Up), KeyChord::plain(KeyCode::Down)],
            "move",
            Some("move"),
            Visibility::Shown,
            Some("↑↓"),
        ),
        KeyBinding::borrowed(
            &[
                KeyChord::plain(KeyCode::Home),
                KeyChord::plain(KeyCode::End),
            ],
            "edge",
            Some("first/last"),
            Visibility::Shown,
            Some("Home/End"),
        ),
    ];
    Keymap::from_static(BINDINGS)
}

/// Default key chords for a horizontal roving group.
#[must_use]
pub fn roving_hint_keymap_horizontal() -> Keymap<&'static str> {
    static BINDINGS: &[KeyBinding<&'static str>] = &[
        KeyBinding::borrowed(
            &[
                KeyChord::plain(KeyCode::Left),
                KeyChord::plain(KeyCode::Right),
            ],
            "move",
            Some("move"),
            Visibility::Shown,
            Some("←→"),
        ),
        KeyBinding::borrowed(
            &[
                KeyChord::plain(KeyCode::Home),
                KeyChord::plain(KeyCode::End),
            ],
            "edge",
            Some("first/last"),
            Visibility::Shown,
            Some("Home/End"),
        ),
    ];
    Keymap::from_static(BINDINGS)
}

/// Hint keymap for the group's orientation.
#[must_use]
pub fn roving_hint_keymap(orientation: RovingOrientation) -> Keymap<&'static str> {
    match orientation {
        RovingOrientation::Horizontal => roving_hint_keymap_horizontal(),
        RovingOrientation::Vertical | RovingOrientation::Both => roving_hint_keymap_vertical(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(specs: &[(&'static str, bool)]) -> Vec<RovingEntry<&'static str>> {
        specs
            .iter()
            .map(|(id, en)| RovingEntry {
                id: *id,
                enabled: *en,
                label: (*id).to_string(),
            })
            .collect()
    }

    #[test]
    fn skips_disabled_and_wraps() {
        let e = entries(&[("a", true), ("b", false), ("c", true)]);
        let mut g = RovingFocusGroup::new();
        let _ = g.reconcile(&e);
        assert_eq!(g.active(), Some(&"a"));
        assert!(g.move_next(&e).changed());
        assert_eq!(g.active(), Some(&"c"));
        assert!(g.move_next(&e).changed());
        assert_eq!(g.active(), Some(&"a"));
    }

    #[test]
    fn no_wrap_clamps() {
        let e = entries(&[("a", true), ("b", true)]);
        let mut g = RovingFocusGroup::new().wrap(false);
        let _ = g.reconcile(&e);
        let _ = g.move_last(&e);
        assert_eq!(g.active(), Some(&"b"));
        assert!(!g.move_next(&e).changed() || g.active() == Some(&"b"));
        assert_eq!(g.active(), Some(&"b"));
    }

    #[test]
    fn reconcile_after_disable_active() {
        let mut e = entries(&[("a", true), ("b", true), ("c", true)]);
        let mut g = RovingFocusGroup::new();
        g.set_active(Some("b"));
        let _ = g.typeahead_char('z', &e);
        assert_eq!(g.typeahead_buffer(), "z");
        e[1].enabled = false;
        assert!(g.reconcile(&e).changed());
        assert_eq!(g.active(), Some(&"c"));
        assert!(g.typeahead_buffer().is_empty());
    }

    #[test]
    fn movement_from_disabled_active_does_not_skip_repaired_neighbor() {
        let e = entries(&[("a", true), ("b", false), ("c", true)]);
        let mut g = RovingFocusGroup::new();
        g.set_active(Some("b"));

        assert_eq!(
            g.move_next(&e),
            RovingOutcome::ActiveChanged {
                from: Some("b"),
                to: Some("c"),
            }
        );
        assert_eq!(g.active(), Some(&"c"));

        g.set_active(Some("b"));
        assert_eq!(
            g.move_previous(&e),
            RovingOutcome::ActiveChanged {
                from: Some("b"),
                to: Some("a"),
            }
        );
        assert_eq!(g.active(), Some(&"a"));
    }

    #[test]
    fn movement_from_missing_active_selects_first_enabled_entry() {
        let e = entries(&[("a", true), ("b", true)]);
        let mut g = RovingFocusGroup::new();
        g.set_active(Some("gone"));

        assert_eq!(
            g.move_next(&e),
            RovingOutcome::ActiveChanged {
                from: Some("gone"),
                to: Some("a"),
            }
        );
        assert_eq!(g.active(), Some(&"a"));
    }

    #[test]
    fn reconcile_after_removal() {
        let mut g = RovingFocusGroup::new();
        g.set_active(Some("gone"));
        let e = entries(&[("a", true), ("b", true)]);
        assert!(g.reconcile(&e).changed());
        assert_eq!(g.active(), Some(&"a"));
    }

    #[test]
    fn home_end_and_intent() {
        let e = entries(&[("a", true), ("b", true), ("c", true)]);
        let mut g = RovingFocusGroup::new();
        let _ = g.reconcile(&e);
        assert!(
            g.handle_intent(UiIntent::Move(NavigationMove::Last), &e)
                .changed()
        );
        assert_eq!(g.active(), Some(&"c"));
        assert!(
            g.handle_intent(UiIntent::Move(NavigationMove::First), &e)
                .changed()
        );
        assert_eq!(g.active(), Some(&"a"));
    }

    #[test]
    fn typeahead_jumps() {
        let e = vec![
            RovingEntry::new("1", "Apple"),
            RovingEntry::new("2", "Apricot"),
            RovingEntry::new("3", "Banana"),
        ];
        let mut g = RovingFocusGroup::new();
        let _ = g.reconcile(&e);
        assert!(g.typeahead_char('b', &e).changed());
        assert_eq!(g.active(), Some(&"3"));
        g.clear_typeahead();
        assert!(g.typeahead_char('a', &e).changed());
        assert!(matches!(g.active(), Some(&"1") | Some(&"2")));
        assert!(g.typeahead_char('p', &e).changed() || g.active().is_some());
    }

    #[test]
    fn typeahead_matches_unicode_case_insensitively() {
        let e = vec![
            RovingEntry::new("other", "Other"),
            RovingEntry::new("accented", "Éclair"),
        ];
        let mut g = RovingFocusGroup::new();
        let _ = g.reconcile(&e);

        assert!(g.typeahead_char('é', &e).changed());
        assert_eq!(g.active(), Some(&"accented"));
    }

    #[test]
    fn repeated_typeahead_retries_from_next_active() {
        let e = vec![
            RovingEntry::new("apple", "Apple"),
            RovingEntry::new("apricot", "Apricot"),
            RovingEntry::new("avocado", "Avocado"),
        ];
        let mut g = RovingFocusGroup::new();
        let _ = g.reconcile(&e);

        let _ = g.typeahead_char('a', &e);
        assert_eq!(g.active(), Some(&"apricot"));
        let _ = g.typeahead_char('a', &e);
        assert_eq!(g.active(), Some(&"avocado"));
        let _ = g.typeahead_char('a', &e);
        assert_eq!(g.active(), Some(&"apple"));
    }

    #[test]
    fn horizontal_ignores_vertical_arrows() {
        let e = entries(&[("a", true), ("b", true)]);
        let mut g = RovingFocusGroup::new().orientation(RovingOrientation::Horizontal);
        let _ = g.reconcile(&e);
        let key = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(g.handle_key(key, &e), RovingOutcome::Ignored);
        let key = KeyEvent::new(KeyCode::Right, KeyModifiers::NONE);
        assert!(g.handle_key(key, &e).changed());
        assert_eq!(g.active(), Some(&"b"));
    }

    #[test]
    fn property_random_disable_and_window_changes() {
        // Deterministic property-style suite (no proptest dep): many mutations
        // never leave active on a disabled or missing id when enabled items exist.
        let labels = ["a", "b", "c", "d", "e", "f", "g", "h"];
        let mut g = RovingFocusGroup::new();
        for seed in 0..64u32 {
            let mut specs: Vec<(&str, bool)> = labels
                .iter()
                .enumerate()
                .map(|(i, l)| {
                    let en = ((seed as usize).wrapping_mul(3) + i * 7) % 5 != 0;
                    (*l, en)
                })
                .collect();
            // virtual window: drop ends based on seed
            let start = (seed as usize) % 3;
            let end = (start + 3 + (seed as usize % 3)).min(specs.len());
            let window = &specs[start..end];
            let e = entries(window);
            if e.iter().all(|x| !x.enabled) {
                // force one enabled
                specs[start].1 = true;
            }
            let e = entries(&specs[start..end]);
            let _ = g.reconcile(&e);
            for step in 0..8 {
                match step % 4 {
                    0 => {
                        let _ = g.move_next(&e);
                    }
                    1 => {
                        let _ = g.move_previous(&e);
                    }
                    2 => {
                        let _ = g.move_first(&e);
                    }
                    _ => {
                        let _ = g.move_last(&e);
                    }
                }
                if let Some(id) = g.active() {
                    let entry = e.iter().find(|x| &x.id == id);
                    assert!(entry.is_some(), "active missing from window");
                    assert!(entry.unwrap().enabled, "active disabled");
                } else {
                    assert!(e.iter().all(|x| !x.enabled) || e.is_empty());
                }
            }
            // flip disable on active
            if let Some(id) = g.active().copied() {
                let mut e2 = e.clone();
                if let Some(row) = e2.iter_mut().find(|x| x.id == id) {
                    row.enabled = false;
                }
                let _ = g.reconcile(&e2);
                if let Some(id2) = g.active() {
                    assert!(e2.iter().find(|x| &x.id == id2).unwrap().enabled);
                }
            }
        }
    }

    #[test]
    fn hint_keymap_matches_orientation() {
        let v = roving_hint_keymap(RovingOrientation::Vertical);
        assert!(!v.hint_spans().is_empty());
        let h = roving_hint_keymap(RovingOrientation::Horizontal);
        assert!(!h.hint_spans().is_empty());
    }
}
