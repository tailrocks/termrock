use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    widgets::StatefulWidget,
};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind},
    interaction::Outcome,
    style::{Role, Theme},
    text::take_display_cols,
};

use super::{List, ListRow, ListState, RowRole, TextInput, TextInputOutcome, TextInputState};

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
/// Semantic results produced by picker interaction.
pub enum PickerOutcome<Id> {
    /// The input produced no picker action.
    Ignored,
    /// Query text or its cursor changed; the caller should rebuild its projection.
    QueryChanged,
    /// The selected visible identity changed.
    SelectionChanged,
    /// The selected visible identity was activated.
    Activated(Id),
    /// Escape was pressed while the query was already empty.
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Query and stable-selection state for [`Picker`].
///
/// Callers own matching, scoring, ordering, candidate lifecycle, and labels.
/// Rebuild the visible [`ListRow`] projection after [`PickerOutcome::QueryChanged`],
/// then call [`Self::reconcile`] before rendering or handling another key.
pub struct PickerState<Id> {
    query: TextInputState,
    list: ListState<Id>,
    previous_visible: Vec<Id>,
}

impl<Id> PickerState<Id> {
    /// Creates empty query state with an optional stable selection.
    #[must_use]
    pub fn new(selected: Option<Id>) -> Self {
        Self {
            query: TextInputState::new("").with_allow_empty(true),
            list: ListState::new(selected),
            previous_visible: Vec::new(),
        }
    }

    /// Returns the query used by the caller-owned projection.
    #[must_use]
    pub fn query_text(&self) -> &str {
        self.query.value()
    }

    /// Returns the text-input state for cursor and validation inspection.
    #[must_use]
    pub const fn query(&self) -> &TextInputState {
        &self.query
    }

    /// Returns mutable query state for consumer-specific constraints.
    pub const fn query_mut(&mut self) -> &mut TextInputState {
        &mut self.query
    }

    /// Returns the list state for selection and painted-geometry inspection.
    #[must_use]
    pub const fn list(&self) -> &ListState<Id> {
        &self.list
    }

    /// Returns mutable list state for focus, scrolling, and pointer integration.
    pub const fn list_mut(&mut self) -> &mut ListState<Id> {
        &mut self.list
    }
}

impl<Id> Default for PickerState<Id> {
    fn default() -> Self {
        Self::new(None)
    }
}

impl<Id: Clone + PartialEq> PickerState<Id> {
    /// Keeps a surviving stable identity selected, otherwise falls back to the
    /// same selectable index clamped into the new projection.
    pub fn reconcile(&mut self, visible: &[ListRow<'_, Id>]) {
        let selectable_count = visible
            .iter()
            .filter(|row| row.enabled && row.role == RowRole::Item)
            .count();
        let unchanged = self.previous_visible.len() == selectable_count
            && self.previous_visible.iter().eq(visible
                .iter()
                .filter(|row| row.enabled && row.role == RowRole::Item)
                .map(|row| &row.id));
        let fallback = self
            .list
            .selected()
            .and_then(|selected| self.previous_visible.iter().position(|id| id == selected))
            .unwrap_or(0);
        let selected_survives = self.list.selected().is_some_and(|selected| {
            visible
                .iter()
                .any(|row| row.enabled && row.role == RowRole::Item && &row.id == selected)
        });
        if unchanged && selected_survives {
            return;
        }
        if !unchanged {
            let mut index = 0;
            for row in visible
                .iter()
                .filter(|row| row.enabled && row.role == RowRole::Item)
            {
                if let Some(existing) = self.previous_visible.get_mut(index) {
                    existing.clone_from(&row.id);
                } else {
                    self.previous_visible.push(row.id.clone());
                }
                index += 1;
            }
            self.previous_visible.truncate(index);
        }
        if self.previous_visible.is_empty() {
            self.list.select(None);
            return;
        }
        if selected_survives {
            return;
        }
        let fallback = fallback.min(self.previous_visible.len() - 1);
        self.list
            .select(Some(self.previous_visible[fallback].clone()));
    }

    /// Routes text editing to the query and navigation/activation to the list.
    pub fn handle_key(&mut self, visible: &[ListRow<'_, Id>], key: KeyEvent) -> PickerOutcome<Id> {
        if key.kind == KeyEventKind::Release {
            return PickerOutcome::Ignored;
        }
        if !key.modifiers.is_empty() && !matches!(key.code, KeyCode::Char(_)) {
            return PickerOutcome::Ignored;
        }
        match key.code {
            KeyCode::Up | KeyCode::Down | KeyCode::PageUp | KeyCode::PageDown
                if key.modifiers.is_empty() =>
            {
                match self.list.handle_key(visible, key) {
                    Outcome::Changed => PickerOutcome::SelectionChanged,
                    _ => PickerOutcome::Ignored,
                }
            }
            KeyCode::Enter if key.modifiers.is_empty() => match self.list.activate(visible) {
                Outcome::Activated(id) => PickerOutcome::Activated(id),
                _ => PickerOutcome::Ignored,
            },
            KeyCode::Esc if key.modifiers.is_empty() && !self.query.value().is_empty() => {
                self.query.clear();
                PickerOutcome::QueryChanged
            }
            KeyCode::Esc if key.modifiers.is_empty() => PickerOutcome::Cancelled,
            _ => match self.query.handle_key(key) {
                TextInputOutcome::Changed => PickerOutcome::QueryChanged,
                TextInputOutcome::Cancelled => PickerOutcome::Cancelled,
                TextInputOutcome::Submitted(_) | TextInputOutcome::Ignored => {
                    PickerOutcome::Ignored
                }
            },
        }
    }

    /// Updates list hover from geometry painted by the latest picker render.
    pub fn hover(&mut self, position: Position) -> Option<&Id> {
        self.list.hover(position)
    }

    /// Activates a list row from geometry painted by the latest picker render.
    pub fn click(&mut self, position: Position) -> PickerOutcome<Id> {
        match self.list.click(position) {
            Outcome::Activated(id) => PickerOutcome::Activated(id),
            _ => PickerOutcome::Ignored,
        }
    }

    /// Scrolls the result list and clamps it to the supplied projection length.
    pub fn scroll_by(&mut self, delta: isize, visible_len: usize) -> bool {
        self.list.scroll_by(delta, visible_len)
    }
}

#[derive(Debug, Clone, Copy)]
/// Strongly defaulted query-plus-list composition over caller-filtered rows.
///
/// The first row is a [`TextInput`]; remaining rows render a [`List`] or the
/// product-neutral empty cue. Picker owns no overlay, matching, or async policy.
pub struct Picker<'a, Id> {
    rows: &'a [ListRow<'a, Id>],
    theme: &'a Theme,
    label: &'a str,
    placeholder: &'a str,
    empty_message: &'a str,
}

impl<'a, Id> Picker<'a, Id> {
    /// Creates a picker with `Filter`, `Type to filter`, and `No matches` defaults.
    #[must_use]
    pub const fn new(rows: &'a [ListRow<'a, Id>], theme: &'a Theme) -> Self {
        Self {
            rows,
            theme,
            label: "Filter",
            placeholder: "Type to filter",
            empty_message: "No matches",
        }
    }

    /// Replaces the semantic query label.
    #[must_use]
    pub const fn label(mut self, label: &'a str) -> Self {
        self.label = label;
        self
    }

    /// Replaces the empty-query placeholder.
    #[must_use]
    pub const fn placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = placeholder;
        self
    }

    /// Replaces the cue rendered when the projection is empty.
    #[must_use]
    pub const fn empty_message(mut self, empty_message: &'a str) -> Self {
        self.empty_message = empty_message;
        self
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &Picker<'_, Id> {
    type State = PickerState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        state.reconcile(self.rows);
        if area.is_empty() {
            StatefulWidget::render(
                &List::new(self.rows, self.theme),
                area,
                buffer,
                &mut state.list,
            );
            return;
        }
        let query_area = Rect::new(area.x, area.y, area.width, 1);
        StatefulWidget::render(
            &TextInput::new(self.label, self.theme).placeholder(self.placeholder),
            query_area,
            buffer,
            &mut state.query,
        );
        let list_area = Rect::new(
            area.x,
            area.y.saturating_add(1),
            area.width,
            area.height.saturating_sub(1),
        );
        if list_area.is_empty() {
            StatefulWidget::render(
                &List::new(self.rows, self.theme),
                list_area,
                buffer,
                &mut state.list,
            );
            return;
        }
        if self.rows.is_empty() {
            StatefulWidget::render(
                &List::new(self.rows, self.theme),
                list_area,
                buffer,
                &mut state.list,
            );
            buffer.set_stringn(
                list_area.x,
                list_area.y,
                take_display_cols(self.empty_message, usize::from(list_area.width)),
                usize::from(list_area.width),
                self.theme.style(Role::TextMuted),
            );
        } else {
            StatefulWidget::render(
                &List::new(self.rows, self.theme),
                list_area,
                buffer,
                &mut state.list,
            );
        }
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for Picker<'_, Id> {
    type State = PickerState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        StatefulWidget::render(&self, area, buffer, state);
    }
}

#[cfg(test)]
mod tests {
    use ratatui_core::text::Line;

    use super::*;
    use crate::input::KeyModifiers;

    fn rows(ids: &[&'static str]) -> Vec<ListRow<'static, &'static str>> {
        ids.iter()
            .map(|id| ListRow {
                id: *id,
                label: Line::from(*id),
                trailing: None,
                role: RowRole::Item,
                enabled: true,
            })
            .collect()
    }

    #[test]
    fn reconciliation_is_id_sticky_then_index_fallback() {
        let cases = [
            (Some("beta"), &["alpha", "beta", "gamma"][..], Some("beta")),
            (Some("beta"), &["beta", "gamma"][..], Some("beta")),
            (Some("beta"), &["alpha", "gamma"][..], Some("gamma")),
            (Some("gamma"), &["alpha"][..], Some("alpha")),
            (Some("alpha"), &[][..], None),
        ];
        for (selected, filtered, expected) in cases {
            let mut state = PickerState::new(selected);
            state.reconcile(&rows(&["alpha", "beta", "gamma"]));
            state.reconcile(&rows(filtered));
            assert_eq!(state.list().selected().copied(), expected);
        }

        let mut reordered = PickerState::new(Some("gamma"));
        reordered.reconcile(&rows(&["alpha", "beta", "gamma"]));
        reordered.reconcile(&rows(&["gamma", "alpha"]));
        assert_eq!(reordered.list().selected(), Some(&"gamma"));
    }

    #[test]
    fn disabled_and_separator_rows_never_become_fallbacks() {
        let mut visible = rows(&["enabled"]);
        visible.insert(
            0,
            ListRow {
                id: "separator",
                label: Line::from("Group"),
                trailing: None,
                role: RowRole::Separator,
                enabled: true,
            },
        );
        visible.push(ListRow {
            id: "disabled",
            label: Line::from("Disabled"),
            trailing: None,
            role: RowRole::Item,
            enabled: false,
        });
        let mut state = PickerState::new(Some("missing"));
        state.reconcile(&visible);
        assert_eq!(state.list().selected(), Some(&"enabled"));
    }

    #[test]
    fn unicode_query_navigation_activation_and_two_stage_escape_are_disjoint() {
        let visible = rows(&["東京", "🧪"]);
        let mut state = PickerState::new(Some("東京"));
        assert_eq!(
            state.handle_key(
                &visible,
                KeyEvent::new(KeyCode::Char('東'), KeyModifiers::NONE)
            ),
            PickerOutcome::QueryChanged
        );
        assert_eq!(state.query_text(), "東");
        assert_eq!(
            state.handle_key(&visible, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            PickerOutcome::SelectionChanged
        );
        assert_eq!(
            state.handle_key(&visible, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            PickerOutcome::Activated("🧪")
        );
        assert_eq!(
            state.handle_key(&visible, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            PickerOutcome::QueryChanged
        );
        assert_eq!(
            state.handle_key(&visible, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            PickerOutcome::Cancelled
        );
    }

    #[test]
    fn release_and_modified_navigation_are_ignored() {
        let visible = rows(&["alpha", "beta"]);
        let mut state = PickerState::new(Some("alpha"));
        let mut release = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
        release.kind = KeyEventKind::Release;
        assert_eq!(state.handle_key(&visible, release), PickerOutcome::Ignored);
        assert_eq!(
            state.handle_key(
                &visible,
                KeyEvent::new(KeyCode::Down, KeyModifiers::CONTROL)
            ),
            PickerOutcome::Ignored
        );
        assert_eq!(state.list().selected(), Some(&"alpha"));
    }

    #[test]
    fn empty_and_tiny_rendering_are_safe_and_clear_pointer_geometry() {
        let theme = Theme::default();
        let visible = rows(&["alpha"]);
        let mut state = PickerState::new(Some("alpha"));
        let mut buffer = Buffer::empty(Rect::new(0, 0, 8, 2));
        (&Picker::new(&visible, &theme)).render(Rect::new(0, 0, 8, 2), &mut buffer, &mut state);
        assert_eq!(
            state.click(Position::new(2, 1)),
            PickerOutcome::Activated("alpha")
        );
        (&Picker::new(&[], &theme)).render(Rect::new(0, 0, 8, 2), &mut buffer, &mut state);
        assert_eq!(buffer[(0, 1)].symbol(), "N");
        (&Picker::new(&[], &theme)).render(Rect::new(0, 0, 0, 0), &mut buffer, &mut state);
        assert_eq!(state.click(Position::new(2, 1)), PickerOutcome::Ignored);
    }

    #[test]
    fn mouse_activation_delegates_to_painted_list_geometry() {
        let theme = Theme::default();
        let visible = rows(&["alpha"]);
        let mut state = PickerState::new(Some("alpha"));
        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 3));
        (&Picker::new(&visible, &theme)).render(Rect::new(0, 0, 20, 3), &mut buffer, &mut state);
        assert_eq!(
            state.click(Position::new(2, 1)),
            PickerOutcome::Activated("alpha")
        );
    }

    #[test]
    fn warmed_reconciliation_reuses_projection_capacity() {
        let visible = rows(&["alpha", "beta", "gamma"]);
        let mut state = PickerState::new(Some("alpha"));
        state.reconcile(&visible);
        let capacity = state.previous_visible.capacity();
        for _ in 0..100 {
            state.reconcile(&visible);
        }
        assert_eq!(state.previous_visible.capacity(), capacity);
    }

    #[test]
    fn rendering_a_filtered_projection_clears_stale_hover() {
        let theme = Theme::default();
        let initial = rows(&["alpha", "beta"]);
        let reordered = rows(&["beta", "alpha"]);
        let filtered = rows(&["alpha"]);
        let mut state = PickerState::new(Some("alpha"));
        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 4));
        (&Picker::new(&initial, &theme)).render(Rect::new(0, 0, 20, 4), &mut buffer, &mut state);
        assert_eq!(state.hover(Position::new(2, 2)), Some(&"beta"));
        (&Picker::new(&reordered, &theme)).render(Rect::new(0, 0, 20, 4), &mut buffer, &mut state);
        assert_eq!(state.list().hovered(), Some(&"alpha"));
        (&Picker::new(&filtered, &theme)).render(Rect::new(0, 0, 20, 4), &mut buffer, &mut state);
        assert_eq!(state.list().hovered(), None);
    }
}
