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
/// Available `RowRole` choices.
pub enum RowRole {
    /// Selects the `Item` behavior.
    Item,
    /// Selects the `Separator` behavior.
    Separator,
}

#[derive(Debug, Clone)]
/// Data carried by `ListRow`.
pub struct ListRow<'a, Id> {
    /// Documentation for `item`.
    pub id: Id,
    /// Documentation for `item`.
    pub label: Line<'a>,
    /// Documentation for `item`.
    pub trailing: Option<Line<'a>>,
    /// Documentation for `item`.
    pub role: RowRole,
    /// Documentation for `item`.
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Runtime state for `List`.
pub struct ListState<Id> {
    /// Documentation for `item`.
    pub selected: Option<Id>,
    /// Documentation for `item`.
    pub hovered: Option<Id>,
    /// Documentation for `item`.
    pub focused: bool,
    /// Documentation for `item`.
    pub offset: usize,
    /// Documentation for `item`.
    pub viewport_height: usize,
    /// Documentation for `item`.
    pub regions: Vec<HitRegion<Id>>,
    /// Documentation for `item`.
    pub selection: Option<Selection<Id>>,
    /// Documentation for `item`.
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
    /// Creates a new value with canonical defaults.
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

    /// Performs the `enable_multi_select` operation.
    pub fn enable_multi_select(&mut self) {
        self.selection.get_or_insert_with(Selection::new);
    }

    /// Performs the `disable_multi_select` operation.
    pub fn disable_multi_select(&mut self) {
        self.selection = None;
    }

    #[must_use]
    /// Performs the `selection` operation.
    pub const fn selection(&self) -> Option<&Selection<Id>> {
        self.selection.as_ref()
    }

    /// Performs the `selection_mut` operation.
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

    /// Performs the `select_next` operation.
    pub fn select_next(&mut self, rows: &[ListRow<'_, Id>]) -> Outcome<Id> {
        self.select_relative(rows, 1)
    }

    /// Performs the `select_previous` operation.
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
    /// Performs the `activate` operation.
    pub fn activate(&self, rows: &[ListRow<'_, Id>]) -> Outcome<Id> {
        self.selected
            .as_ref()
            .and_then(|selected| {
                rows.iter()
                    .find(|row| row.enabled && row.role == RowRole::Item && &row.id == selected)
            })
            .map_or(Outcome::Ignored, |row| Outcome::Activated(row.id.clone()))
    }

    /// Performs the `hover` operation.
    pub fn hover(&mut self, position: Position) -> Option<&Id> {
        self.hovered = self
            .regions
            .iter()
            .find(|region| region.area.contains(position))
            .map(|region| region.id.clone());
        self.hovered.as_ref()
    }

    #[must_use]
    /// Performs the `click` operation.
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

    /// Performs the `scroll_by` operation.
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

    /// Performs the `scroll_to_position` operation.
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
    /// Creates a new value with canonical defaults.
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
                if state.selection.is_some() && check_x < rect.right() {
                    buffer.set_stringn(
                        check_x,
                        rect.y,
                        if checked { "[x] " } else { "[ ] " },
                        usize::from(rect.right().saturating_sub(check_x).min(4)),
                        style,
                    );
                    if row.enabled && rect.right().saturating_sub(check_x) >= 3 {
                        state.check_regions.push(HitRegion {
                            id: row.id.clone(),
                            area: Rect::new(check_x, rect.y, 3, 1),
                        });
                    }
                }
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
            crate::scroll::render_vertical_scrollbar_to_buffer(
                buffer,
                Rect::new(area.right().saturating_sub(1), area.y, 1, area.height),
                self.rows.len(),
                state.viewport_height,
                u16::try_from(state.offset).unwrap_or(u16::MAX),
                crate::scroll::ScrollbarStyle::Line,
            );
        }
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
