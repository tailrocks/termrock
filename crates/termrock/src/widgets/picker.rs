use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    widgets::StatefulWidget,
};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind},
    interaction::{
        Outcome, OverlayId, OverlayOutcome, OverlaySize, OverlaySpec, OverlayStack, place_overlay,
    },
    style::{DesignSystem, Role},
    text::take_display_cols,
};

use super::{List, ListRow, ListState, RowRole, TextInput, TextInputOutcome, TextInputState};

/// Default overlay id for a select/picker popup on an [`OverlayStack`].
pub const PICKER_OVERLAY_ID: &str = "termrock.picker";

/// Preferred picker popup size before clamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PickerSize {
    /// Preferred width in cells.
    pub width: u16,
    /// Preferred height in rows.
    pub height: u16,
}

impl Default for PickerSize {
    fn default() -> Self {
        Self {
            width: 36,
            height: 12,
        }
    }
}

impl From<PickerSize> for OverlaySize {
    fn from(value: PickerSize) -> Self {
        Self {
            width: value.width,
            height: value.height,
            min_width: 16,
            min_height: 4,
            max_width: 0,
            max_height: 0,
        }
    }
}

/// Place a picker/select popup under `anchor` (Select kind policy).
#[must_use]
pub fn place_picker(bounds: Rect, anchor: Rect, preferred: PickerSize) -> Rect {
    place_overlay(
        bounds,
        Some(anchor),
        OverlaySize::from(preferred),
        crate::interaction::OverlayPolicy::for_kind(crate::interaction::OverlayKind::Select),
    )
}

/// Open a select-kind picker popup on the overlay stack.
pub fn open_picker_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    anchor: Rect,
    preferred: PickerSize,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.open(
        bounds,
        OverlaySpec::select(
            PICKER_OVERLAY_ID,
            anchor,
            OverlaySize::from(preferred),
            opener_focus,
        ),
    )
}

/// Dismiss the default picker overlay id.
pub fn dismiss_picker_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.dismiss(&OverlayId::from_static(PICKER_OVERLAY_ID))
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
/// Semantic results produced by picker interaction.
pub enum PickerOutcome<Id> {
    /// The input produced no picker action.
    Ignored,
    /// Query text or its cursor changed; the caller should rebuild its projection.
    QueryChanged,
    /// Result-list cursor moved (not scene surface focus).
    CursorMoved,
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
///
/// Scene/overlay surface focus is host-owned via [`Self::set_accepts_input`].
pub struct PickerState<Id> {
    query: TextInputState,
    list: ListState<Id>,
    previous_visible: Vec<Id>,
    accepts_input: bool,
}

impl<Id: Clone + PartialEq> PickerState<Id> {
    /// Creates empty query state with an optional stable selection.
    #[must_use]
    pub fn new(selected: Option<Id>) -> Self {
        Self {
            query: TextInputState::new("").with_allow_empty(true),
            list: ListState::new(selected),
            previous_visible: Vec::new(),
            accepts_input: true,
        }
    }

    /// Host input gate (overlay top / scene ownership).
    pub fn set_accepts_input(&mut self, accepts: bool) {
        self.accepts_input = accepts;
    }

    /// Whether host granted input.
    #[must_use]
    pub const fn accepts_input(&self) -> bool {
        self.accepts_input
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

impl<Id: Clone + PartialEq> Default for PickerState<Id> {
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

    /// Routes navigation/activation through list intents; printable keys edit
    /// the query. Esc clears query first, then cancels.
    pub fn handle_key(&mut self, visible: &[ListRow<'_, Id>], key: KeyEvent) -> PickerOutcome<Id> {
        if !self.accepts_input || key.kind == KeyEventKind::Release {
            return PickerOutcome::Ignored;
        }
        if !key.modifiers.is_empty() && !matches!(key.code, KeyCode::Char(_)) {
            return PickerOutcome::Ignored;
        }
        // Collection intents target the results list when unshifted navigation.
        if key.modifiers.is_empty()
            && let Some(intent) = crate::interaction::default_list_intent(key)
        {
            return self.handle_intent(visible, intent);
        }
        match self.query.handle_key(key) {
            TextInputOutcome::Changed | TextInputOutcome::Cleared => PickerOutcome::QueryChanged,
            TextInputOutcome::Cancelled => PickerOutcome::Cancelled,
            TextInputOutcome::Submitted(_)
            | TextInputOutcome::Ignored
            | TextInputOutcome::ClipboardCopy { .. }
            | TextInputOutcome::ClipboardCut { .. }
            | TextInputOutcome::ClipboardPasteRequest => PickerOutcome::Ignored,
        }
    }

    /// Applies a semantic intent to the results list (query is separate).
    pub fn handle_intent(
        &mut self,
        visible: &[ListRow<'_, Id>],
        intent: crate::interaction::UiIntent,
    ) -> PickerOutcome<Id> {
        if !self.accepts_input {
            return PickerOutcome::Ignored;
        }
        use crate::interaction::UiIntent;
        match intent {
            UiIntent::Cancel | UiIntent::Close => {
                if !self.query.value().is_empty() {
                    self.query.clear();
                    PickerOutcome::QueryChanged
                } else {
                    PickerOutcome::Cancelled
                }
            }
            UiIntent::Activate | UiIntent::Open | UiIntent::Submit => {
                match self.list.handle_intent(visible, UiIntent::Activate) {
                    Outcome::Activated(id) => PickerOutcome::Activated(id),
                    _ => PickerOutcome::Ignored,
                }
            }
            UiIntent::Move(_) | UiIntent::Page(_) | UiIntent::Toggle => {
                match self.list.handle_intent(visible, intent) {
                    Outcome::Changed | Outcome::CheckToggled(_) => PickerOutcome::CursorMoved,
                    Outcome::Activated(id) => PickerOutcome::Activated(id),
                    _ => PickerOutcome::Ignored,
                }
            }
            UiIntent::Expand | UiIntent::Collapse => PickerOutcome::Ignored,
            _ => PickerOutcome::Ignored,
        }
    }

    /// Key path with [`crate::interaction::EventResult`] envelope.
    pub fn handle_key_result(
        &mut self,
        visible: &[ListRow<'_, Id>],
        key: KeyEvent,
    ) -> crate::interaction::EventResult<PickerOutcome<Id>> {
        match self.handle_key(visible, key) {
            PickerOutcome::Ignored => crate::interaction::EventResult::ignored(),
            other => crate::interaction::EventResult::emit(other),
        }
    }

    /// Intent path with [`crate::interaction::EventResult`] envelope.
    pub fn handle_intent_result(
        &mut self,
        visible: &[ListRow<'_, Id>],
        intent: crate::interaction::UiIntent,
    ) -> crate::interaction::EventResult<PickerOutcome<Id>> {
        match self.handle_intent(visible, intent) {
            PickerOutcome::Ignored => crate::interaction::EventResult::ignored(),
            other => crate::interaction::EventResult::emit(other),
        }
    }

    /// Updates list hover from geometry painted by the latest picker render.
    pub fn hover(&mut self, position: Position) -> Option<&Id> {
        if !self.accepts_input {
            return None;
        }
        self.list.hover(position)
    }

    /// Activates a list row from geometry painted by the latest picker render.
    pub fn click(&mut self, position: Position) -> PickerOutcome<Id> {
        if !self.accepts_input {
            return PickerOutcome::Ignored;
        }
        match self.list.click(position) {
            Outcome::Activated(id) => PickerOutcome::Activated(id),
            _ => PickerOutcome::Ignored,
        }
    }

    /// Scrolls the result list and clamps it to the supplied projection length.
    pub fn scroll_by(&mut self, delta: isize, visible_len: usize) -> bool {
        if !self.accepts_input {
            return false;
        }
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
    system: &'a DesignSystem,
    label: &'a str,
    placeholder: &'a str,
    empty_message: &'a str,
    focused: bool,
    ascii: bool,
    colorless: bool,
}

impl<'a, Id> Picker<'a, Id> {
    /// Creates a picker with `Filter`, `Type to filter`, and `No matches` defaults.
    #[must_use]
    pub const fn new(rows: &'a [ListRow<'a, Id>], system: &'a DesignSystem) -> Self {
        Self {
            rows,
            system,
            label: "Filter",
            placeholder: "Type to filter",
            empty_message: "No matches",
            focused: true,
            ascii: false,
            colorless: false,
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

    /// Scene surface focus chrome (list selection still uses list cursor).
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// ASCII empty / list recipes.
    #[must_use]
    pub const fn ascii(mut self, ascii: bool) -> Self {
        self.ascii = ascii;
        self
    }

    /// Reduced-color paint.
    #[must_use]
    pub const fn colorless(mut self, colorless: bool) -> Self {
        self.colorless = colorless;
        self
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &Picker<'_, Id> {
    type State = PickerState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        state.reconcile(self.rows);
        if area.is_empty() {
            StatefulWidget::render(
                &List::new(self.rows, self.system),
                area,
                buffer,
                &mut state.list,
            );
            return;
        }
        let tiny = area.height < 2;
        let query_area = Rect::new(area.x, area.y, area.width, 1);
        StatefulWidget::render(
            &TextInput::new(self.label, self.system).placeholder(self.placeholder),
            query_area,
            buffer,
            &mut state.query,
        );
        if tiny {
            return;
        }
        let list_area = Rect::new(
            area.x,
            area.y.saturating_add(1),
            area.width,
            area.height.saturating_sub(1),
        );
        if list_area.is_empty() {
            return;
        }
        if self.rows.is_empty() {
            let mark = if self.ascii { "[ ] " } else { "∅ " };
            let msg = if list_area.width < 12 {
                format!("{mark}empty")
            } else {
                format!("{mark}{}", self.empty_message)
            };
            let style = if self.colorless || !self.focused {
                self.system.style(Role::TextMuted)
            } else {
                self.system.style(Role::TextMuted)
            };
            buffer.set_stringn(
                list_area.x,
                list_area.y,
                take_display_cols(&msg, usize::from(list_area.width)),
                usize::from(list_area.width),
                style,
            );
            // Keep list geometry empty for clicks.
            StatefulWidget::render(
                &List::new(&[], self.system),
                list_area,
                buffer,
                &mut state.list,
            );
        } else {
            let list = List::new(self.rows, self.system);
            // List focused chrome is selection-based; surface focus is host.
            let _ = (self.focused, self.ascii, self.colorless);
            StatefulWidget::render(&list, list_area, buffer, &mut state.list);
        }
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for Picker<'_, Id> {
    type State = PickerState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
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
                leading: None,
                secondary: None,
                badge: None,
                shortcut: None,
                trailing: None,
                role: RowRole::Item,
                enabled: true,
                loading: false,
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
                leading: None,
                secondary: None,
                badge: None,
                shortcut: None,
                trailing: None,
                role: RowRole::Separator,
                enabled: true,
                loading: false,
            },
        );
        visible.push(ListRow {
            id: "disabled",
            label: Line::from("Disabled"),
            leading: None,
            secondary: None,
            badge: None,
            shortcut: None,
            trailing: None,
            role: RowRole::Item,
            enabled: false,
            loading: false,
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
            PickerOutcome::CursorMoved
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
    fn accepts_input_gate() {
        let visible = rows(&["alpha"]);
        let mut state = PickerState::new(Some("alpha"));
        state.set_accepts_input(false);
        assert_eq!(
            state.handle_key(&visible, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            PickerOutcome::Ignored
        );
    }

    #[test]
    fn cursor_moved_not_selection_changed_surface() {
        let src = include_str!("picker.rs");
        let head = src
            .split("#[cfg(test)]")
            .next()
            .unwrap_or(src)
            .lines()
            .filter(|l| {
                let t = l.trim_start();
                !t.starts_with("//") && !t.starts_with("//!")
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!head.contains("SelectionChanged"));
        assert!(head.contains("CursorMoved"));
    }

    #[test]
    fn empty_and_tiny_rendering_are_safe_and_clear_pointer_geometry() {
        let tokens = DesignSystem::default();
        let _theme = tokens.palette.clone();
        let visible = rows(&["alpha"]);
        let mut state = PickerState::new(Some("alpha"));
        let mut buffer = Buffer::empty(Rect::new(0, 0, 8, 2));
        (&Picker::new(&visible, &tokens)).render(Rect::new(0, 0, 8, 2), &mut buffer, &mut state);
        assert_eq!(
            state.click(Position::new(2, 1)),
            PickerOutcome::Activated("alpha")
        );
        (&Picker::new(&[], &tokens)).render(Rect::new(0, 0, 8, 2), &mut buffer, &mut state);
        let empty_cell = buffer[(0, 1)].symbol();
        assert!(
            empty_cell == "∅" || empty_cell == "[" || empty_cell == "N",
            "empty cue: {empty_cell:?}"
        );
        (&Picker::new(&[], &tokens)).render(Rect::new(0, 0, 0, 0), &mut buffer, &mut state);
        assert_eq!(state.click(Position::new(2, 1)), PickerOutcome::Ignored);
    }

    #[test]
    fn mouse_activation_delegates_to_painted_list_geometry() {
        let tokens = DesignSystem::default();
        let visible = rows(&["alpha"]);
        let mut state = PickerState::new(Some("alpha"));
        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 3));
        (&Picker::new(&visible, &tokens)).render(Rect::new(0, 0, 20, 3), &mut buffer, &mut state);
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
        let tokens = DesignSystem::default();
        let initial = rows(&["alpha", "beta"]);
        let reordered = rows(&["beta", "alpha"]);
        let filtered = rows(&["alpha"]);
        let mut state = PickerState::new(Some("alpha"));
        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 4));
        (&Picker::new(&initial, &tokens)).render(Rect::new(0, 0, 20, 4), &mut buffer, &mut state);
        assert_eq!(state.hover(Position::new(2, 2)), Some(&"beta"));
        (&Picker::new(&reordered, &tokens)).render(Rect::new(0, 0, 20, 4), &mut buffer, &mut state);
        assert_eq!(state.list().hovered(), Some(&"alpha"));
        (&Picker::new(&filtered, &tokens)).render(Rect::new(0, 0, 20, 4), &mut buffer, &mut state);
        assert_eq!(state.list().hovered(), None);
    }

    #[test]
    fn picker_overlay_helpers_open_and_dismiss() {
        use crate::interaction::OverlayKind;
        let bounds = Rect::new(0, 0, 80, 24);
        let anchor = Rect::new(10, 5, 8, 1);
        let mut stack = crate::interaction::OverlayStack::<&'static str>::new();
        let out = open_picker_overlay(
            &mut stack,
            bounds,
            anchor,
            PickerSize::default(),
            Some("trigger"),
        );
        assert!(matches!(out, OverlayOutcome::Opened { .. }));
        assert_eq!(stack.top().unwrap().kind, OverlayKind::Select);
        let placed = place_picker(bounds, anchor, PickerSize::default());
        assert_eq!(stack.top().unwrap().rect, placed);
        assert!(matches!(
            dismiss_picker_overlay(&mut stack),
            OverlayOutcome::Dismissed {
                focus: Some("trigger"),
                ..
            }
        ));
    }
}
