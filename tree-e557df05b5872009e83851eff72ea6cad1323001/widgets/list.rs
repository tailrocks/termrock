use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    text::Line,
    widgets::StatefulWidget,
};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind},
    interaction::{HitRegion, Outcome},
    scroll::max_offset,
    style::{Role, Theme},
};

use super::Selection;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
/// Semantic roles for selectable, disabled, and separator list rows.
pub enum RowRole {
    /// A selectable content row.
    Item,
    /// A non-interactive visual separator row.
    Separator,
}

#[derive(Debug, Clone)]
/// A stable row in a selectable list.
pub struct ListRow<'a, Id> {
    /// Stable identity used for selection and activation.
    pub id: Id,
    /// Caller-visible label.
    pub label: Line<'a>,
    /// Optional metadata aligned at the trailing edge.
    pub trailing: Option<Line<'a>>,
    /// Interaction role controlling selection and hit testing.
    pub role: RowRole,
    /// Whether this item is enabled.
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Runtime state for `List`.
pub struct ListState<Id> {
    /// Whether this item is selected.
    pub selected: Option<Id>,
    /// Whether this item is hovered.
    pub hovered: Option<Id>,
    /// Whether this item is focused.
    pub focused: bool,
    /// Offset in terminal cells or rows.
    pub offset: usize,
    /// Viewport height in terminal rows.
    pub viewport_height: usize,
    /// Hit regions produced by the most recent render.
    pub regions: Vec<HitRegion<Id>>,
    /// Ordered checked identities when multi-select is enabled.
    pub selection: Option<Selection<Id>>,
    /// Hit regions produced by the most recent render.
    pub check_regions: Vec<HitRegion<Id>>,
}

impl<Id> Default for ListState<Id> {
    fn default() -> Self {
        Self {
            selected: None,
            hovered: None,
            focused: false,
            offset: 0,
            viewport_height: 0,
            regions: Vec::new(),
            selection: None,
            check_regions: Vec::new(),
        }
    }
}

impl<Id: Clone + PartialEq> ListState<Id> {
    #[must_use]
    /// Creates list state with no selection, hover, checks, or scroll.
    pub const fn new(selected: Option<Id>) -> Self {
        Self {
            selected,
            hovered: None,
            focused: true,
            offset: 0,
            viewport_height: 0,
            regions: Vec::new(),
            selection: None,
            check_regions: Vec::new(),
        }
    }

    /// Replace the stable selected identity.
    pub fn select(&mut self, selected: Option<Id>) {
        self.selected = selected;
    }

    /// Enables ordered multi-selection with an empty selection.
    pub fn enable_multi_select(&mut self) {
        self.selection.get_or_insert_with(Selection::new);
    }

    /// Disables multi-selection and discards checked identities.
    pub fn disable_multi_select(&mut self) {
        self.selection = None;
    }

    #[must_use]
    /// Returns the ordered multi-selection state, if enabled.
    pub const fn selection(&self) -> Option<&Selection<Id>> {
        self.selection.as_ref()
    }

    /// Returns mutable access to ordered multi-selection state, if enabled.
    pub fn selection_mut(&mut self) -> Option<&mut Selection<Id>> {
        self.selection.as_mut()
    }

    /// Handles the `handle_key` interaction.
    pub fn handle_key(&mut self, rows: &[ListRow<'_, Id>], key: KeyEvent) -> Outcome<Id> {
        if key.kind == KeyEventKind::Release {
            return Outcome::Ignored;
        }
        match key.code {
            KeyCode::Up | KeyCode::Char('k' | 'K') => self.select_relative(rows, -1),
            KeyCode::Down | KeyCode::Char('j' | 'J') => self.select_relative(rows, 1),
            KeyCode::Home => self.select_edge(rows, false),
            KeyCode::End => self.select_edge(rows, true),
            KeyCode::PageUp => self.select_page(rows, -1),
            KeyCode::PageDown => self.select_page(rows, 1),
            KeyCode::Enter => self.activate(rows),
            KeyCode::Char(' ') => self.toggle_selected(rows),
            KeyCode::Esc => Outcome::Cancelled,
            _ => Outcome::Ignored,
        }
    }

    fn toggle_selected(&mut self, rows: &[ListRow<'_, Id>]) -> Outcome<Id> {
        let Some(selection) = self.selection.as_mut() else {
            return Outcome::Ignored;
        };
        let Some(row) = self.selected.as_ref().and_then(|selected| {
            rows.iter()
                .find(|row| row.enabled && row.role == RowRole::Item && &row.id == selected)
        }) else {
            return Outcome::Ignored;
        };
        selection.toggle(&row.id);
        Outcome::Changed
    }

    /// Moves selection to the next enabled item, wrapping at the end.
    pub fn select_next(&mut self, rows: &[ListRow<'_, Id>]) -> Outcome<Id> {
        self.select_relative(rows, 1)
    }

    /// Moves selection to the previous enabled item, wrapping at the start.
    pub fn select_previous(&mut self, rows: &[ListRow<'_, Id>]) -> Outcome<Id> {
        self.select_relative(rows, -1)
    }

    fn select_relative(&mut self, rows: &[ListRow<'_, Id>], direction: isize) -> Outcome<Id> {
        let selectable = selectable_indices(rows);
        if selectable.is_empty() {
            self.selected = None;
            return Outcome::Ignored;
        }
        let current = self.selected.as_ref().and_then(|selected| {
            selectable
                .iter()
                .position(|index| &rows[*index].id == selected)
        });
        let next = match (current, direction.is_negative()) {
            (Some(0), true) | (None, true) => selectable.len() - 1,
            (Some(index), true) => index - 1,
            (Some(index), false) => (index + 1) % selectable.len(),
            (None, false) => 0,
        };
        self.selected = Some(rows[selectable[next]].id.clone());
        Outcome::Changed
    }

    fn select_edge(&mut self, rows: &[ListRow<'_, Id>], end: bool) -> Outcome<Id> {
        let selectable = selectable_indices(rows);
        let index = if end {
            selectable.last().copied()
        } else {
            selectable.first().copied()
        };
        let Some(index) = index else {
            self.selected = None;
            return Outcome::Ignored;
        };
        self.selected = Some(rows[index].id.clone());
        Outcome::Changed
    }

    fn select_page(&mut self, rows: &[ListRow<'_, Id>], direction: isize) -> Outcome<Id> {
        let selectable = selectable_indices(rows);
        if selectable.is_empty() {
            self.selected = None;
            return Outcome::Ignored;
        }
        let current = self
            .selected
            .as_ref()
            .and_then(|selected| {
                selectable
                    .iter()
                    .position(|index| &rows[*index].id == selected)
            })
            .unwrap_or(0);
        let page = self.viewport_height.max(1);
        let next = if direction.is_negative() {
            current.saturating_sub(page)
        } else {
            current.saturating_add(page).min(selectable.len() - 1)
        };
        self.selected = Some(rows[selectable[next]].id.clone());
        Outcome::Changed
    }

    #[must_use]
    /// Returns the semantic action associated with the supplied stable identity.
    pub fn activate(&self, rows: &[ListRow<'_, Id>]) -> Outcome<Id> {
        self.selected
            .as_ref()
            .and_then(|selected| {
                rows.iter()
                    .find(|row| row.enabled && row.role == RowRole::Item && &row.id == selected)
            })
            .map_or(Outcome::Ignored, |row| Outcome::Activated(row.id.clone()))
    }

    /// Updates hover state from the current pointer position and painted hit regions.
    pub fn hover(&mut self, position: Position) -> Option<&Id> {
        self.hovered = self
            .regions
            .iter()
            .find(|region| region.area.contains(position))
            .map(|region| region.id.clone());
        self.hovered.as_ref()
    }

    #[must_use]
    /// Maps a pointer position to the semantic outcome of the painted hit region.
    pub fn click(&mut self, position: Position) -> Outcome<Id> {
        if let Some(id) = self
            .check_regions
            .iter()
            .find(|region| region.area.contains(position))
            .map(|region| region.id.clone())
        {
            self.selected = Some(id.clone());
            if let Some(selection) = self.selection.as_mut() {
                selection.toggle(&id);
                return Outcome::Changed;
            }
        }
        let Some(region) = self
            .regions
            .iter()
            .find(|region| region.area.contains(position))
        else {
            return Outcome::Ignored;
        };
        self.selected = Some(region.id.clone());
        Outcome::Activated(region.id.clone())
    }

    /// Moves the scroll position by a signed delta and clamps it to valid content.
    pub fn scroll_by(&mut self, delta: isize, rows_len: usize) -> bool {
        let before = self.offset;
        let max = max_offset(rows_len, self.viewport_height);
        self.offset = if delta.is_negative() {
            self.offset.saturating_sub(delta.unsigned_abs())
        } else {
            self.offset.saturating_add(delta.unsigned_abs()).min(max)
        };
        before != self.offset
    }

    /// Scrolls toward a pointer position within the painted viewport.
    pub fn scroll_to_position(&mut self, position: Position, rows_len: usize) -> bool {
        if self.viewport_height == 0 || self.regions.is_empty() {
            return false;
        }
        let first = self.regions[0].area;
        if position.y < first.y {
            return self.scroll_by(-1, rows_len);
        }
        let bottom = first.y.saturating_add(
            u16::try_from(self.viewport_height.saturating_sub(1)).unwrap_or(u16::MAX),
        );
        if position.y > bottom {
            return self.scroll_by(1, rows_len);
        }
        false
    }
}

impl ListState<usize> {
    /// Create index-addressed list state with the first item selected.
    #[must_use]
    pub const fn for_count(count: usize) -> Self {
        Self::new(if count == 0 { None } else { Some(0) })
    }

    /// Reconcile an index selection after the backing collection changes.
    pub fn reconcile_count(&mut self, count: usize) {
        self.selected = match (self.selected, count) {
            (_, 0) => None,
            (Some(index), _) => Some(if index < count { index } else { count - 1 }),
            (None, _) => Some(0),
        };
    }

    /// Move an index selection by one item, wrapping at either edge.
    pub fn cycle_index(&mut self, count: usize, direction: isize) -> bool {
        if count == 0 {
            self.selected = None;
            return false;
        }
        let current = self.selected.unwrap_or(0).min(count - 1);
        let next = if direction.is_negative() {
            if current == 0 { count - 1 } else { current - 1 }
        } else if current + 1 >= count {
            0
        } else {
            current + 1
        };
        self.selected = Some(next);
        next != current
    }

    /// Move an index selection by a gesture delta without wrapping.
    pub fn move_index(&mut self, count: usize, delta: isize) -> bool {
        if count == 0 {
            self.selected = None;
            return false;
        }
        let current = self.selected.unwrap_or(0).min(count - 1);
        let next = if delta.is_negative() {
            current.saturating_sub(delta.unsigned_abs())
        } else {
            current.saturating_add(delta.unsigned_abs()).min(count - 1)
        };
        self.selected = Some(next);
        next != current
    }

    /// Borrow the selected item from an index-addressed collection.
    #[must_use]
    pub fn selected_item<'a, T>(&self, items: &'a [T]) -> Option<&'a T> {
        self.selected.and_then(|index| items.get(index))
    }
}

#[derive(Debug, Clone, Copy)]
/// Stable-ID list widget rendered with [`ListState`].
///
/// See the `list/selection` lookbook story for selection, metadata, and narrow
/// terminal behavior.
///
/// # Examples
///
/// ```
/// use ratatui_core::text::Line;
/// use termrock::{
///     Theme,
///     input::{KeyCode, KeyEvent, KeyModifiers},
///     interaction::Outcome,
///     widgets::{List, ListRow, ListState, RowRole},
/// };
///
/// let rows = [
///     ListRow { id: "a", label: Line::from("Alpha"), trailing: None, role: RowRole::Item, enabled: true },
///     ListRow { id: "b", label: Line::from("Beta"), trailing: None, role: RowRole::Item, enabled: true },
/// ];
/// let theme = Theme::default();
/// let _widget = List::new(&rows, &theme);
/// let mut state = ListState::new(Some("a"));
/// let outcome = state.handle_key(&rows, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
/// assert!(matches!(outcome, Outcome::Changed));
/// assert_eq!(state.selected, Some("b"));
/// ```
pub struct List<'a, Id> {
    rows: &'a [ListRow<'a, Id>],
    theme: &'a Theme,
}

impl<'a, Id> List<'a, Id> {
    #[must_use]
    /// Creates a list over borrowed rows and mutable list state.
    pub const fn new(rows: &'a [ListRow<'a, Id>], theme: &'a Theme) -> Self {
        Self { rows, theme }
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &List<'_, Id> {
    type State = ListState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        state.regions.clear();
        state.check_regions.clear();
        state.viewport_height = usize::from(area.height);
        let scrollable = crate::scroll::is_scrollable(self.rows.len(), state.viewport_height);
        let content_width = area.width.saturating_sub(u16::from(scrollable));
        let trailing_width = self
            .rows
            .iter()
            .filter_map(|row| row.trailing.as_ref())
            .map(Line::width)
            .max()
            .and_then(|width| u16::try_from(width).ok())
            .unwrap_or(0)
            .min(content_width);
        state.offset = state
            .offset
            .min(max_offset(self.rows.len(), state.viewport_height));
        if let Some(selected) = state.selected.as_ref()
            && let Some(index) = self.rows.iter().position(|row| &row.id == selected)
        {
            if index < state.offset {
                state.offset = index;
            } else if index >= state.offset.saturating_add(state.viewport_height) {
                state.offset = index
                    .saturating_add(1)
                    .saturating_sub(state.viewport_height);
            }
        }
        for (visible, row) in self
            .rows
            .iter()
            .skip(state.offset)
            .take(state.viewport_height)
            .enumerate()
        {
            let rect = Rect::new(
                area.x,
                area.y
                    .saturating_add(u16::try_from(visible).unwrap_or(u16::MAX)),
                content_width,
                1,
            );
            let selected = state.selected.as_ref() == Some(&row.id);
            let hovered = state.hovered.as_ref() == Some(&row.id);
            let checked = state
                .selection
                .as_ref()
                .is_some_and(|selection| selection.is_checked(&row.id));
            let style = if !row.enabled {
                self.theme.style(Role::TextDisabled)
            } else if selected && state.focused {
                self.theme.style(Role::Selection)
            } else if hovered {
                self.theme.style(Role::LinkHover)
            } else if checked {
                self.theme.style(Role::Accent)
            } else {
                self.theme.style(Role::Text)
            };
            buffer.set_style(rect, style);
            let trailing_x = rect.right().saturating_sub(trailing_width);
            if row.role == RowRole::Separator {
                buffer.set_stringn(rect.x, rect.y, "─", usize::from(rect.width), style);
                if rect.width > 2 {
                    let label_x = rect
                        .x
                        .saturating_add(2)
                        .saturating_add(u16::from(state.selection.is_some()) * 4);
                    buffer.set_line(
                        label_x,
                        rect.y,
                        &row.label,
                        label_width(label_x, trailing_x, trailing_width),
                    );
                }
            } else {
                let marker = if selected { "▸ " } else { "  " };
                buffer.set_stringn(rect.x, rect.y, marker, usize::from(rect.width), style);
                let check_x = rect.x.saturating_add(2);
                render_check_cell(buffer, state, row, rect, check_x, checked, style);
                if rect.width > 2 {
                    let label_x = check_x.saturating_add(u16::from(state.selection.is_some()) * 4);
                    buffer.set_line(
                        label_x,
                        rect.y,
                        &row.label,
                        label_width(label_x, trailing_x, trailing_width),
                    );
                }
            }
            if let Some(trailing) = row.trailing.as_ref()
                && trailing_width > 0
            {
                let width = u16::try_from(trailing.width())
                    .unwrap_or(u16::MAX)
                    .min(trailing_width);
                buffer.set_line(rect.right().saturating_sub(width), rect.y, trailing, width);
            }
            if row.enabled && row.role == RowRole::Item && !rect.is_empty() {
                state.regions.push(HitRegion {
                    id: row.id.clone(),
                    area: rect,
                });
            }
        }
        if scrollable {
            crate::scroll::render_scrollbar(
                buffer,
                Rect::new(area.right().saturating_sub(1), area.y, 1, area.height),
                crate::scroll::ScrollbarSpec::new(
                    crate::scroll::ScrollAxis::Vertical,
                    crate::scroll::ScrollbarGeometry::new(
                        self.rows.len(),
                        state.viewport_height,
                        u16::try_from(state.offset).unwrap_or(u16::MAX),
                    ),
                ),
                self.theme,
            );
        }
    }
}

fn render_check_cell<Id: Clone>(
    buffer: &mut Buffer,
    state: &mut ListState<Id>,
    row: &ListRow<'_, Id>,
    rect: Rect,
    check_x: u16,
    checked: bool,
    style: ratatui_core::style::Style,
) {
    if state.selection.is_none() || check_x >= rect.right() {
        return;
    }

    let marker = if checked { "[x] " } else { "[ ] " };
    let available = rect.right().saturating_sub(check_x);
    buffer.set_stringn(
        check_x,
        rect.y,
        marker,
        usize::from(available.min(4)),
        style,
    );
    if row.enabled && available >= 3 {
        state.check_regions.push(HitRegion {
            id: row.id.clone(),
            area: Rect::new(check_x, rect.y, 3, 1),
        });
    }
}

fn label_width(label_x: u16, trailing_x: u16, trailing_width: u16) -> u16 {
    trailing_x
        .saturating_sub(label_x)
        .saturating_sub(u16::from(trailing_width > 0))
}

impl<Id: Clone + PartialEq> StatefulWidget for List<'_, Id> {
    type State = ListState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        StatefulWidget::render(&self, area, buffer, state);
    }
}

fn selectable_indices<Id>(rows: &[ListRow<'_, Id>]) -> Vec<usize> {
    rows.iter()
        .enumerate()
        .filter_map(|(index, row)| (row.enabled && row.role == RowRole::Item).then_some(index))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyModifiers;

    fn rows() -> [ListRow<'static, &'static str>; 4] {
        [
            ListRow {
                id: "section",
                label: Line::from("Section"),
                trailing: None,
                role: RowRole::Separator,
                enabled: true,
            },
            ListRow {
                id: "disabled",
                label: Line::from("Disabled"),
                trailing: None,
                role: RowRole::Item,
                enabled: false,
            },
            ListRow {
                id: "first",
                label: Line::from("First"),
                trailing: None,
                role: RowRole::Item,
                enabled: true,
            },
            ListRow {
                id: "second",
                label: Line::from("Second"),
                trailing: None,
                role: RowRole::Item,
                enabled: true,
            },
        ]
    }

    #[test]
    fn keyboard_skips_non_items_and_returns_stable_ids() {
        let rows = rows();
        let mut state = ListState::new(None);
        assert_eq!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            Outcome::Changed
        );
        assert_eq!(state.selected, Some("first"));
        assert_eq!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            Outcome::Changed
        );
        assert_eq!(state.selected, Some("second"));
        assert_eq!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Outcome::Activated("second")
        );
        assert_eq!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            Outcome::Cancelled
        );
    }

    #[test]
    fn render_reveals_selection_and_mouse_uses_painted_regions() {
        let rows = rows();
        let theme = Theme::default();
        let mut state = ListState::new(Some("second"));
        let area = Rect::new(4, 3, 12, 1);
        let mut buffer = Buffer::empty(area);
        (&List::new(&rows, &theme)).render(area, &mut buffer, &mut state);
        assert_eq!(state.offset, 3);
        assert_eq!(state.regions.len(), 1);
        let position = Position::new(area.x, area.y);
        assert_eq!(state.hover(position), Some(&"second"));
        assert_eq!(state.click(position), Outcome::Activated("second"));
        assert_eq!(buffer[(area.x, area.y)].symbol(), "▸");
    }

    #[test]
    fn trailing_cells_align_right_and_wide_labels_truncate_first() {
        let rows = [
            ListRow {
                id: "wide",
                label: Line::from("🧪🧪label"),
                trailing: Some(Line::from("9 KiB")),
                role: RowRole::Item,
                enabled: true,
            },
            ListRow {
                id: "short",
                label: Line::from("short"),
                trailing: Some(Line::from("1 B")),
                role: RowRole::Item,
                enabled: true,
            },
        ];
        let theme = Theme::default();
        let mut state = ListState::new(None);
        let area = Rect::new(0, 0, 11, 2);
        let mut buffer = Buffer::empty(area);

        (&List::new(&rows, &theme)).render(area, &mut buffer, &mut state);

        assert_eq!(buffer[(6, 0)].symbol(), "9");
        assert_eq!(buffer[(8, 1)].symbol(), "1");
        assert_eq!(buffer[(10, 0)].symbol(), "B");
        assert_eq!(buffer[(10, 1)].symbol(), "B");
        assert_eq!(buffer[(2, 0)].symbol(), "🧪");
        assert_ne!(buffer[(4, 0)].symbol(), "🧪");
    }

    #[test]
    fn narrow_trailing_cell_clips_only_at_grapheme_boundaries() {
        let rows = [ListRow {
            id: "wide-trailing",
            label: Line::from("hidden"),
            trailing: Some(Line::from("🧪Z")),
            role: RowRole::Item,
            enabled: true,
        }];
        let theme = Theme::default();
        let mut state = ListState::new(None);
        let area = Rect::new(0, 0, 2, 1);
        let mut buffer = Buffer::empty(area);

        (&List::new(&rows, &theme)).render(area, &mut buffer, &mut state);

        assert_eq!(buffer[(0, 0)].symbol(), "🧪");
        assert_eq!(buffer[(1, 0)].symbol(), " ");
        assert!(!buffer.content().iter().any(|cell| cell.symbol() == "Z"));
    }

    #[test]
    fn multi_select_toggles_by_space_and_painted_checkbox() {
        let rows = rows();
        let theme = Theme::default();
        let mut state = ListState::new(Some("first"));
        state.enable_multi_select();

        assert_eq!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)),
            Outcome::Changed
        );
        assert!(state.selection().unwrap().is_checked(&"first"));

        let area = Rect::new(0, 0, 20, 4);
        let mut buffer = Buffer::empty(area);
        (&List::new(&rows, &theme)).render(area, &mut buffer, &mut state);
        assert_eq!(buffer[(2, 2)].symbol(), "[");
        assert_eq!(buffer[(3, 2)].symbol(), "x");
        assert_eq!(state.click(Position::new(2, 3)), Outcome::Changed);
        assert_eq!(state.selection().unwrap().checked(), ["first", "second"]);

        state.selection_mut().unwrap().clear();
        assert!(state.selection().unwrap().checked().is_empty());
        state.disable_multi_select();
        assert_eq!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)),
            Outcome::Ignored
        );
    }

    #[test]
    fn indexed_picker_navigation_wraps_keys_and_bounds_gestures() {
        let mut state = ListState::for_count(3);
        assert_eq!(state.selected, Some(0));
        assert!(state.cycle_index(3, -1));
        assert_eq!(state.selected, Some(2));
        assert!(state.cycle_index(3, 1));
        assert_eq!(state.selected, Some(0));
        assert!(state.move_index(3, 9));
        assert_eq!(state.selected, Some(2));
        assert!(!state.move_index(3, 9));
        assert_eq!(state.selected_item(&["a", "b", "c"]), Some(&"c"));

        state.reconcile_count(1);
        assert_eq!(state.selected, Some(0));
        state.reconcile_count(0);
        assert_eq!(state.selected, None);
    }
}
