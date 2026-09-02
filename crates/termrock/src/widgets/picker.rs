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

use super::{
    Hint, HintBar, List, ListRow, ListState, RowRole, Surface, SurfaceRecipe, TextInput,
    TextInputOutcome, TextInputState,
};

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

/// Centered in the upper third of `bounds` (command-palette / quick-open class).
#[must_use]
pub fn place_picker_modal(bounds: Rect, preferred: PickerSize) -> Rect {
    place_upper_third(bounds, preferred.width, preferred.height)
}

fn place_upper_third(bounds: Rect, width: u16, height: u16) -> Rect {
    if bounds.is_empty() || width == 0 || height == 0 {
        return Rect::default();
    }
    let width = width.min(bounds.width).max(1);
    let height = height.min(bounds.height).max(1);
    let x = bounds
        .x
        .saturating_add(bounds.width.saturating_sub(width) / 2);
    let y = bounds
        .y
        .saturating_add((bounds.height.saturating_sub(height) / 3).max(1));
    Rect::new(
        x,
        y.min(bounds.bottom().saturating_sub(height)),
        width,
        height,
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
    colorless: bool,
    title: &'a str,
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
            // Seeded from the system: a widget that defaults to false is
            // claiming the terminal has Unicode and colour before anyone
            // asked it. Builders below still force either way.
            colorless: system.mono(),
            title: "Picker",
        }
    }

    /// Overlay title painted above the query field.
    #[must_use]
    pub const fn title(mut self, title: &'a str) -> Self {
        self.title = title;
        self
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
    /// Reduced-color paint.
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
        let chrome = area.height >= 6 && area.width >= 16;
        let inner = if chrome {
            let recipe = if self.focused {
                SurfaceRecipe::OverlayFocused
            } else {
                SurfaceRecipe::Overlay
            };
            let inner = Surface::new(self.system)
                .recipe(recipe)
                .bordered(true)
                .content_inset()
                .paint(area, buffer);
            if area.width > 4 {
                buffer.set_stringn(
                    area.x.saturating_add(2),
                    area.y,
                    take_display_cols(self.title, usize::from(area.width.saturating_sub(4))),
                    usize::from(area.width.saturating_sub(4)),
                    self.system.style(Role::TextStrong),
                );
            }
            inner
        } else {
            area
        };
        if inner.is_empty() {
            return;
        }
        let show_hints = chrome && inner.height >= 4;
        let query_area = Rect::new(inner.x, inner.y, inner.width, 1);
        StatefulWidget::render(
            &TextInput::new(self.label, self.system).placeholder(self.placeholder),
            query_area,
            buffer,
            &mut state.query,
        );
        if inner.height < 2 {
            return;
        }
        let list_bottom = if show_hints {
            inner.bottom().saturating_sub(1)
        } else {
            inner.bottom()
        };
        let list_area = Rect::new(
            inner.x,
            inner.y.saturating_add(1),
            inner.width,
            list_bottom.saturating_sub(inner.y.saturating_add(1)),
        );
        if list_area.is_empty() {
            return;
        }
        if self.rows.is_empty() {
            let mark = { "∅ " };
            let msg = if list_area.width < 12 {
                format!("{mark}empty")
            } else {
                format!("{mark}{}", self.empty_message)
            };
            let style = self.system.style(Role::TextMuted);
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
            let _ = (self.focused, false, self.colorless);
            StatefulWidget::render(&list, list_area, buffer, &mut state.list);
        }
        if show_hints {
            ratatui_core::widgets::Widget::render(
                &HintBar::new(PICKER_HINTS, self.system),
                Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
                buffer,
            );
        }
    }
}

const PICKER_HINTS: &[Hint<'static>] = &[
    Hint {
        chord: "↑↓",
        label: "move",
        priority: 10,
        visible: true,
    },
    Hint {
        chord: "enter",
        label: "choose",
        priority: 20,
        visible: true,
    },
    Hint {
        chord: "esc",
        label: "close",
        priority: 50,
        visible: true,
    },
];

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
                status: None,
                badge: None,
                shortcut: None,
                actions: None,
                custom: None,
                role: RowRole::Item,
                enabled: true,
                loading: false,
            })
            .collect()
    }

    #[test]
    fn modal_sits_in_the_upper_third() {
        let bounds = Rect::new(0, 0, 80, 24);
        let placed = place_picker_modal(bounds, PickerSize::default());
        assert!(placed.y < bounds.height / 2, "upper third: {placed:?}");
        assert!(placed.width >= 24);
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
                status: None,
                badge: None,
                shortcut: None,
                actions: None,
                custom: None,
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
            status: None,
            badge: None,
            shortcut: None,
            actions: None,
            custom: None,
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
    fn modal_picker_sits_in_the_upper_third() {
        let bounds = Rect::new(0, 0, 120, 40);
        let placed = place_picker_modal(bounds, PickerSize::default());
        assert!(placed.y < bounds.height / 2, "upper third, not true center");
        assert_eq!(
            placed.x,
            bounds.width.saturating_sub(placed.width) / 2,
            "horizontally centered"
        );
    }

    #[test]
    fn chrome_picker_keeps_list_anatomy_and_hint_row() {
        let tokens = DesignSystem::default();
        let visible = rows(&["alpha", "beta"]);
        let mut state = PickerState::new(Some("alpha"));
        let area = Rect::new(0, 0, 40, 12);
        let mut buffer = Buffer::empty(area);
        (&Picker::new(&visible, &tokens).title("Open")).render(area, &mut buffer, &mut state);
        let mut text = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                text.push_str(buffer[(x, y)].symbol());
            }
        }
        assert!(text.contains("alpha"));
        assert!(
            text.contains("move") || text.contains("enter") || text.contains("choose"),
            "own hint row: {text:?}"
        );
        let selected = state
            .list()
            .regions()
            .iter()
            .find(|r| r.id == "alpha")
            .expect("selected row painted");
        assert_eq!(
            buffer[(selected.area.x, selected.area.y)].symbol(),
            tokens.glyphs.selection_gutter()
        );
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
