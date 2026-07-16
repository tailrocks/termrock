use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    text::Line,
    widgets::StatefulWidget,
};

use crate::{
    input::{KeyCode, KeyEvent},
    interaction::HitRegion,
    scroll::max_offset,
    style::{Role, Theme},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowRole {
    Item,
    Separator,
}

#[derive(Debug, Clone)]
pub struct ListRow<'a, Id> {
    pub id: Id,
    pub label: Line<'a>,
    pub role: RowRole,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListOutcome<Id> {
    Ignored,
    Changed,
    Activated(Id),
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ListState<Id> {
    pub selected: Option<Id>,
    pub hovered: Option<Id>,
    pub focused: bool,
    pub offset: usize,
    pub viewport_height: usize,
    pub regions: Vec<HitRegion<Id>>,
}

impl<Id: Clone + PartialEq> ListState<Id> {
    #[must_use]
    pub const fn new(selected: Option<Id>) -> Self {
        Self {
            selected,
            hovered: None,
            focused: true,
            offset: 0,
            viewport_height: 0,
            regions: Vec::new(),
        }
    }

    pub fn handle_key(&mut self, rows: &[ListRow<'_, Id>], key: KeyEvent) -> ListOutcome<Id> {
        match key.code {
            KeyCode::Up | KeyCode::Char('k' | 'K') => self.select_relative(rows, -1),
            KeyCode::Down | KeyCode::Char('j' | 'J') => self.select_relative(rows, 1),
            KeyCode::Home => self.select_edge(rows, false),
            KeyCode::End => self.select_edge(rows, true),
            KeyCode::PageUp => self.select_page(rows, -1),
            KeyCode::PageDown => self.select_page(rows, 1),
            KeyCode::Enter => self.activate(rows),
            KeyCode::Esc => ListOutcome::Cancelled,
            _ => ListOutcome::Ignored,
        }
    }

    pub fn select_next(&mut self, rows: &[ListRow<'_, Id>]) -> ListOutcome<Id> {
        self.select_relative(rows, 1)
    }

    pub fn select_previous(&mut self, rows: &[ListRow<'_, Id>]) -> ListOutcome<Id> {
        self.select_relative(rows, -1)
    }

    fn select_relative(&mut self, rows: &[ListRow<'_, Id>], direction: isize) -> ListOutcome<Id> {
        let selectable = selectable_indices(rows);
        if selectable.is_empty() {
            self.selected = None;
            return ListOutcome::Ignored;
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
        ListOutcome::Changed
    }

    fn select_edge(&mut self, rows: &[ListRow<'_, Id>], end: bool) -> ListOutcome<Id> {
        let selectable = selectable_indices(rows);
        let index = if end {
            selectable.last().copied()
        } else {
            selectable.first().copied()
        };
        let Some(index) = index else {
            self.selected = None;
            return ListOutcome::Ignored;
        };
        self.selected = Some(rows[index].id.clone());
        ListOutcome::Changed
    }

    fn select_page(&mut self, rows: &[ListRow<'_, Id>], direction: isize) -> ListOutcome<Id> {
        let selectable = selectable_indices(rows);
        if selectable.is_empty() {
            self.selected = None;
            return ListOutcome::Ignored;
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
        ListOutcome::Changed
    }

    #[must_use]
    pub fn activate(&self, rows: &[ListRow<'_, Id>]) -> ListOutcome<Id> {
        self.selected
            .as_ref()
            .and_then(|selected| {
                rows.iter()
                    .find(|row| row.enabled && row.role == RowRole::Item && &row.id == selected)
            })
            .map_or(ListOutcome::Ignored, |row| {
                ListOutcome::Activated(row.id.clone())
            })
    }

    pub fn hover(&mut self, position: Position) -> Option<&Id> {
        self.hovered = self
            .regions
            .iter()
            .find(|region| region.area.contains(position))
            .map(|region| region.id.clone());
        self.hovered.as_ref()
    }

    #[must_use]
    pub fn click(&mut self, position: Position) -> ListOutcome<Id> {
        let Some(region) = self
            .regions
            .iter()
            .find(|region| region.area.contains(position))
        else {
            return ListOutcome::Ignored;
        };
        self.selected = Some(region.id.clone());
        ListOutcome::Activated(region.id.clone())
    }

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

#[derive(Debug, Clone, Copy)]
pub struct List<'a, Id> {
    pub rows: &'a [ListRow<'a, Id>],
    pub theme: &'a Theme,
}

impl<Id: Clone + PartialEq> StatefulWidget for &List<'_, Id> {
    type State = ListState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        state.regions.clear();
        state.viewport_height = usize::from(area.height);
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
                area.width,
                1,
            );
            let selected = state.selected.as_ref() == Some(&row.id);
            let hovered = state.hovered.as_ref() == Some(&row.id);
            let style = if !row.enabled {
                self.theme.style(Role::TextDisabled)
            } else if selected && state.focused {
                self.theme.style(Role::Selection)
            } else if hovered {
                self.theme.style(Role::LinkHover)
            } else {
                self.theme.style(Role::Text)
            };
            buffer.set_style(rect, style);
            if row.role == RowRole::Separator {
                buffer.set_stringn(rect.x, rect.y, "─", usize::from(rect.width), style);
                if rect.width > 2 {
                    buffer.set_line(rect.x.saturating_add(2), rect.y, &row.label, rect.width - 2);
                }
            } else {
                let marker = if selected { "▸ " } else { "  " };
                buffer.set_stringn(rect.x, rect.y, marker, usize::from(rect.width), style);
                if rect.width > 2 {
                    buffer.set_line(rect.x.saturating_add(2), rect.y, &row.label, rect.width - 2);
                }
            }
            if row.enabled && row.role == RowRole::Item && !rect.is_empty() {
                state.regions.push(HitRegion {
                    id: row.id.clone(),
                    area: rect,
                });
            }
        }
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
                role: RowRole::Separator,
                enabled: true,
            },
            ListRow {
                id: "disabled",
                label: Line::from("Disabled"),
                role: RowRole::Item,
                enabled: false,
            },
            ListRow {
                id: "first",
                label: Line::from("First"),
                role: RowRole::Item,
                enabled: true,
            },
            ListRow {
                id: "second",
                label: Line::from("Second"),
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
            ListOutcome::Changed
        );
        assert_eq!(state.selected, Some("first"));
        assert_eq!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            ListOutcome::Changed
        );
        assert_eq!(state.selected, Some("second"));
        assert_eq!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            ListOutcome::Activated("second")
        );
        assert_eq!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            ListOutcome::Cancelled
        );
    }

    #[test]
    fn render_reveals_selection_and_mouse_uses_painted_regions() {
        let rows = rows();
        let theme = Theme::default();
        let mut state = ListState::new(Some("second"));
        let area = Rect::new(4, 3, 12, 1);
        let mut buffer = Buffer::empty(area);
        (&List {
            rows: &rows,
            theme: &theme,
        })
            .render(area, &mut buffer, &mut state);
        assert_eq!(state.offset, 3);
        assert_eq!(state.regions.len(), 1);
        let position = Position::new(area.x, area.y);
        assert_eq!(state.hover(position), Some(&"second"));
        assert_eq!(state.click(position), ListOutcome::Activated("second"));
        assert_eq!(buffer[(area.x, area.y)].symbol(), "▸");
    }
}
