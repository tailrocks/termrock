use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::{Modifier, Style},
    widgets::StatefulWidget,
};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind},
    osc::HyperlinkRegion,
    scroll::{DialogScroll, effective_offset},
    style::{Role, Theme},
};

const SELECTED_MARKER: &str = "▸ ";
const NORMAL_MARKER: &str = "  ";
const SEPARATOR: &str = " : ";
const COPY_AFFORDANCE: &str = "  ⧉";
const COPIED_AFFORDANCE: &str = "  ✓";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailCapability {
    None,
    Copy,
    Link,
    CopyAndLink,
}

impl DetailCapability {
    #[must_use]
    pub const fn copyable(self) -> bool {
        matches!(self, Self::Copy | Self::CopyAndLink)
    }

    #[must_use]
    pub const fn linkable(self) -> bool {
        matches!(self, Self::Link | Self::CopyAndLink)
    }
}

#[derive(Debug, Clone)]
pub struct DetailRow<'a, Id> {
    pub id: Id,
    pub label: &'a str,
    pub value: &'a str,
    pub href: Option<&'a str>,
    pub capability: DetailCapability,
    pub emphasis: bool,
    pub style: Option<Style>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetailTableOutcome<Id> {
    Ignored,
    Selected(Id),
    Copy(Id),
    ActivateLink(Id),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetailRegion<Id> {
    pub id: Id,
    pub row_area: Rect,
    pub action_area: Rect,
    pub value_area: Rect,
    pub capability: DetailCapability,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetailTableState<Id> {
    pub selected: Option<Id>,
    pub hovered: Option<Id>,
    pub copied: Option<Id>,
    pub scroll: DialogScroll,
    pub regions: Vec<DetailRegion<Id>>,
    pub content_width: usize,
    pub content_height: usize,
    pub viewport: Rect,
}

impl<Id> Default for DetailTableState<Id> {
    fn default() -> Self {
        Self {
            selected: None,
            hovered: None,
            copied: None,
            scroll: DialogScroll::default(),
            regions: Vec::new(),
            content_width: 0,
            content_height: 0,
            viewport: Rect::default(),
        }
    }
}

impl<Id: Clone + PartialEq> DetailTableState<Id> {
    pub fn handle_key(
        &mut self,
        rows: &[DetailRow<'_, Id>],
        key: KeyEvent,
    ) -> DetailTableOutcome<Id> {
        if key.kind == KeyEventKind::Release {
            return DetailTableOutcome::Ignored;
        }
        match key.code {
            KeyCode::Up | KeyCode::Char('k' | 'K') => self.select_previous(rows),
            KeyCode::Down | KeyCode::Char('j' | 'J') => self.select_next(rows),
            KeyCode::Enter => self.activate_selected(rows),
            _ => DetailTableOutcome::Ignored,
        }
    }

    pub fn select_next(&mut self, rows: &[DetailRow<'_, Id>]) -> DetailTableOutcome<Id> {
        self.select_relative(rows, 1)
    }

    pub fn select_previous(&mut self, rows: &[DetailRow<'_, Id>]) -> DetailTableOutcome<Id> {
        self.select_relative(rows, -1)
    }

    fn select_relative(
        &mut self,
        rows: &[DetailRow<'_, Id>],
        direction: isize,
    ) -> DetailTableOutcome<Id> {
        if rows.is_empty() {
            self.selected = None;
            return DetailTableOutcome::Ignored;
        }
        let current = self
            .selected
            .as_ref()
            .and_then(|selected| rows.iter().position(|row| &row.id == selected));
        let next = match (current, direction.is_negative()) {
            (Some(0), true) | (None, true) => rows.len() - 1,
            (Some(index), true) => index - 1,
            (Some(index), false) => (index + 1) % rows.len(),
            (None, false) => 0,
        };
        let id = rows[next].id.clone();
        self.selected = Some(id.clone());
        DetailTableOutcome::Selected(id)
    }

    pub fn hover(&mut self, position: Position) -> Option<&Id> {
        self.hovered = self
            .regions
            .iter()
            .find(|region| region.action_area.contains(position))
            .map(|region| region.id.clone());
        self.hovered.as_ref()
    }

    #[must_use]
    pub fn click(&mut self, position: Position) -> DetailTableOutcome<Id> {
        let Some(region) = self
            .regions
            .iter()
            .find(|region| region.row_area.contains(position))
        else {
            return DetailTableOutcome::Ignored;
        };
        let id = region.id.clone();
        self.selected = Some(id.clone());
        if region.action_area.contains(position) && region.capability.copyable() {
            DetailTableOutcome::Copy(id)
        } else if region.value_area.contains(position) && region.capability.linkable() {
            DetailTableOutcome::ActivateLink(id)
        } else {
            DetailTableOutcome::Selected(id)
        }
    }

    #[must_use]
    pub fn click_link(&mut self, position: Position) -> DetailTableOutcome<Id> {
        let Some(region) = self
            .regions
            .iter()
            .find(|region| region.value_area.contains(position) && region.capability.linkable())
        else {
            return DetailTableOutcome::Ignored;
        };
        let id = region.id.clone();
        self.selected = Some(id.clone());
        DetailTableOutcome::ActivateLink(id)
    }

    pub fn mark_copied(&mut self, id: Option<Id>) {
        self.copied = id;
    }

    pub fn clamp_scroll(&mut self) {
        self.scroll.scroll_x = effective_offset(
            self.content_width,
            usize::from(self.viewport.width),
            self.scroll.scroll_x,
        );
        self.scroll.scroll_y = effective_offset(
            self.content_height,
            usize::from(self.viewport.height),
            self.scroll.scroll_y,
        );
    }

    #[must_use]
    pub fn activate_selected(&self, rows: &[DetailRow<'_, Id>]) -> DetailTableOutcome<Id> {
        let Some(selected) = self.selected.as_ref() else {
            return DetailTableOutcome::Ignored;
        };
        let Some(row) = rows.iter().find(|row| &row.id == selected) else {
            return DetailTableOutcome::Ignored;
        };
        if row.capability.copyable() {
            DetailTableOutcome::Copy(selected.clone())
        } else if row.capability.linkable() {
            DetailTableOutcome::ActivateLink(selected.clone())
        } else {
            DetailTableOutcome::Selected(selected.clone())
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DetailTable<'a, Id> {
    pub rows: &'a [DetailRow<'a, Id>],
    /// Zero derives the label width from the borrowed rows.
    pub label_width: u16,
    /// Wrap values into aligned continuation rows instead of scrolling horizontally.
    pub wrap: bool,
    pub theme: &'a Theme,
}

impl<Id: Clone + PartialEq> DetailTable<'_, Id> {
    #[must_use]
    pub fn hyperlink_regions<'a>(
        &'a self,
        state: &'a DetailTableState<Id>,
    ) -> Vec<HyperlinkRegion<'a, Id>> {
        state
            .regions
            .iter()
            .filter_map(|region| {
                let row = self.rows.iter().find(|row| row.id == region.id)?;
                let url = row.href.filter(|_| row.capability.linkable())?;
                Some(HyperlinkRegion {
                    id: row.id.clone(),
                    area: region.value_area,
                    url,
                })
            })
            .collect()
    }

    fn label_width(&self) -> usize {
        if self.label_width == 0 {
            self.rows
                .iter()
                .map(|row| crate::display_cols(row.label))
                .max()
                .unwrap_or(0)
        } else {
            usize::from(self.label_width)
        }
    }

    fn row_width(&self, row: &DetailRow<'_, Id>, label_width: usize) -> usize {
        crate::display_cols(SELECTED_MARKER)
            + label_width
            + crate::display_cols(SEPARATOR)
            + crate::display_cols(row.value)
            + affordance_width(row, false)
    }

    fn value_width(&self, area: Rect, label_width: usize) -> usize {
        usize::from(area.width).saturating_sub(
            crate::display_cols(SELECTED_MARKER) + label_width + crate::display_cols(SEPARATOR),
        )
    }

    fn row_height(&self, row: &DetailRow<'_, Id>, value_width: usize) -> usize {
        if !self.wrap {
            return 1;
        }
        let width = crate::display_cols(row.value).saturating_add(affordance_width(row, false));
        width.max(1).div_ceil(value_width.max(1))
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &DetailTable<'_, Id> {
    type State = DetailTableState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        state.regions.clear();
        state.viewport = area;
        let label_width = self.label_width();
        let value_width = self.value_width(area, label_width);
        state.content_width = self
            .rows
            .iter()
            .map(|row| self.row_width(row, label_width))
            .max()
            .unwrap_or(0);
        let mut selected_range = None;
        let mut measured_height = 0usize;
        for row in self.rows {
            let row_height = self.row_height(row, value_width);
            if state.selected.as_ref() == Some(&row.id) {
                selected_range =
                    Some((measured_height, measured_height.saturating_add(row_height)));
            }
            measured_height = measured_height.saturating_add(row_height);
        }
        state.content_height = measured_height;
        if self.wrap {
            state.scroll.scroll_x = 0;
        }
        if let Some((start, end)) = selected_range {
            let viewport_height = usize::from(area.height);
            let current = usize::from(state.scroll.scroll_y);
            if start < current {
                state.scroll.scroll_y = u16::try_from(start).unwrap_or(u16::MAX);
            } else if end > current.saturating_add(viewport_height) {
                state.scroll.scroll_y =
                    u16::try_from(end.saturating_sub(viewport_height)).unwrap_or(u16::MAX);
            }
        }
        state.clamp_scroll();
        if area.is_empty() {
            return;
        }

        let scroll_x = usize::from(state.scroll.scroll_x);
        let scroll_y = usize::from(state.scroll.scroll_y);
        let mut visual_row = 0usize;
        for row in self.rows {
            let row_height = self.row_height(row, value_width);
            for continuation in 0..row_height {
                if visual_row >= scroll_y
                    && visual_row < scroll_y.saturating_add(usize::from(area.height))
                {
                    let y = area.y.saturating_add(
                        u16::try_from(visual_row.saturating_sub(scroll_y)).unwrap_or(u16::MAX),
                    );
                    render_row(
                        self,
                        row,
                        continuation,
                        label_width,
                        value_width,
                        scroll_x,
                        Rect::new(area.x, y, area.width, 1),
                        buffer,
                        state,
                    );
                }
                visual_row = visual_row.saturating_add(1);
            }
        }
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "row painting keeps the measured geometry and state mutation explicit"
)]
fn render_row<Id: Clone + PartialEq>(
    table: &DetailTable<'_, Id>,
    row: &DetailRow<'_, Id>,
    continuation: usize,
    label_width: usize,
    value_width: usize,
    scroll_x: usize,
    area: Rect,
    buffer: &mut Buffer,
    state: &mut DetailTableState<Id>,
) {
    let selected = state.selected.as_ref() == Some(&row.id);
    let hovered = state.hovered.as_ref() == Some(&row.id);
    let copied = state.copied.as_ref() == Some(&row.id);
    let marker = if selected {
        SELECTED_MARKER
    } else {
        NORMAL_MARKER
    };
    let marker_width = crate::display_cols(marker);
    let separator_width = crate::display_cols(SEPARATOR);
    let value_col = marker_width + label_width + separator_width;
    let value_style = row.style.unwrap_or_else(|| {
        if hovered && (row.capability.copyable() || row.capability.linkable()) {
            table.theme.style(Role::LinkHover)
        } else if row.capability.copyable() || row.capability.linkable() {
            table.theme.style(Role::Link)
        } else if row.emphasis {
            table.theme.style(Role::Accent)
        } else {
            table.theme.style(Role::Text)
        }
    });
    let value_style = if row.emphasis || row.capability != DetailCapability::None {
        value_style.add_modifier(Modifier::BOLD)
    } else {
        value_style
    };

    if continuation == 0 {
        paint_segment(
            buffer,
            area,
            marker,
            0,
            scroll_x,
            table.theme.style(Role::Focus),
        );
        paint_segment(
            buffer,
            area,
            row.label,
            marker_width,
            scroll_x,
            table.theme.style(Role::TextMuted),
        );
        paint_segment(
            buffer,
            area,
            SEPARATOR,
            marker_width + label_width,
            scroll_x,
            table.theme.style(Role::Border),
        );
    }

    let chunk_width = if table.wrap {
        value_width.max(1)
    } else {
        crate::display_cols(row.value).saturating_add(affordance_width(row, copied))
    };
    let chunk_start = continuation.saturating_mul(chunk_width);
    let value_and_affordance = format!("{}{}", row.value, affordance(row, copied));
    let chunk = crate::display_cols_slice(&value_and_affordance, chunk_start, chunk_width);
    let global_value_col = if table.wrap && continuation > 0 {
        value_col
    } else {
        value_col.saturating_add(chunk_start)
    };
    paint_segment(
        buffer,
        area,
        &chunk,
        global_value_col,
        scroll_x,
        value_style,
    );

    let painted_start = global_value_col.max(scroll_x);
    let painted_end = global_value_col
        .saturating_add(crate::display_cols(&chunk))
        .min(scroll_x.saturating_add(usize::from(area.width)));
    let value_end = global_value_col
        .saturating_add(crate::display_cols(row.value).saturating_sub(chunk_start))
        .min(painted_end);
    if painted_start < painted_end {
        let row_area = Rect::new(area.x, area.y, area.width, 1);
        let value_area = Rect::new(
            area.x.saturating_add(
                u16::try_from(painted_start.saturating_sub(scroll_x)).unwrap_or(u16::MAX),
            ),
            area.y,
            u16::try_from(value_end.saturating_sub(painted_start)).unwrap_or(u16::MAX),
            1,
        );
        let action_area = Rect::new(
            area.x.saturating_add(
                u16::try_from(painted_start.saturating_sub(scroll_x)).unwrap_or(u16::MAX),
            ),
            area.y,
            u16::try_from(painted_end.saturating_sub(painted_start)).unwrap_or(u16::MAX),
            1,
        );
        state.regions.push(DetailRegion {
            id: row.id.clone(),
            row_area,
            action_area,
            value_area,
            capability: row.capability,
        });
    }
}

fn paint_segment(
    buffer: &mut Buffer,
    area: Rect,
    text: &str,
    start: usize,
    scroll_x: usize,
    style: Style,
) {
    let end = start.saturating_add(crate::display_cols(text));
    let viewport_end = scroll_x.saturating_add(usize::from(area.width));
    let visible_start = start.max(scroll_x);
    let visible_end = end.min(viewport_end);
    if visible_start >= visible_end {
        return;
    }
    let visible = crate::display_cols_slice(
        text,
        visible_start.saturating_sub(start),
        visible_end.saturating_sub(visible_start),
    );
    buffer.set_stringn(
        area.x.saturating_add(
            u16::try_from(visible_start.saturating_sub(scroll_x)).unwrap_or(u16::MAX),
        ),
        area.y,
        visible,
        visible_end.saturating_sub(visible_start),
        style,
    );
}

fn affordance<Id>(row: &DetailRow<'_, Id>, copied: bool) -> &'static str {
    if !row.capability.copyable() {
        ""
    } else if copied {
        COPIED_AFFORDANCE
    } else {
        COPY_AFFORDANCE
    }
}

fn affordance_width<Id>(row: &DetailRow<'_, Id>, copied: bool) -> usize {
    crate::display_cols(affordance(row, copied))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_core::{buffer::Buffer, widgets::StatefulWidget};

    fn rows() -> [DetailRow<'static, &'static str>; 3] {
        [
            DetailRow {
                id: "run",
                label: "Run ID",
                value: "abc",
                href: None,
                capability: DetailCapability::Copy,
                emphasis: false,
                style: None,
            },
            DetailRow {
                id: "log",
                label: "Diagnostics",
                value: "/wide/🧪🔬/path",
                href: Some("file:///wide/🧪🔬/path"),
                capability: DetailCapability::CopyAndLink,
                emphasis: true,
                style: None,
            },
            DetailRow {
                id: "role",
                label: "Role",
                value: "operator",
                href: None,
                capability: DetailCapability::None,
                emphasis: false,
                style: None,
            },
        ]
    }

    #[test]
    fn stable_selection_and_typed_activation_follow_painted_regions() {
        let rows = rows();
        let theme = Theme::default();
        let table = DetailTable {
            rows: &rows,
            label_width: 0,
            wrap: false,
            theme: &theme,
        };
        let mut state = DetailTableState::default();
        assert_eq!(
            state.select_next(&rows),
            DetailTableOutcome::Selected("run")
        );
        assert_eq!(
            state.activate_selected(&rows),
            DetailTableOutcome::Copy("run")
        );
        let area = Rect::new(4, 3, 32, 3);
        let mut buffer = Buffer::empty(area);
        (&table).render(area, &mut buffer, &mut state);
        let run = state
            .regions
            .iter()
            .find(|region| region.id == "run")
            .unwrap();
        assert_eq!(
            state.click(Position::new(run.value_area.x, run.value_area.y)),
            DetailTableOutcome::Copy("run")
        );
        let log_position = state
            .regions
            .iter()
            .find(|region| region.id == "log")
            .map(|region| Position::new(region.value_area.x, region.value_area.y))
            .unwrap();
        assert_eq!(state.click(log_position), DetailTableOutcome::Copy("log"));
        assert_eq!(
            state.click_link(log_position),
            DetailTableOutcome::ActivateLink("log")
        );
    }

    #[test]
    fn wrap_and_both_axis_scroll_are_bounded_and_unicode_safe() {
        let rows = rows();
        let theme = Theme::default();
        let mut state = DetailTableState::default();
        state.scroll.scroll_x = u16::MAX;
        state.scroll.scroll_y = u16::MAX;
        let mut buffer = Buffer::empty(Rect::new(0, 0, 18, 2));
        (&DetailTable {
            rows: &rows,
            label_width: 0,
            wrap: false,
            theme: &theme,
        })
            .render(buffer.area, &mut buffer, &mut state);
        assert!(usize::from(state.scroll.scroll_x) <= state.content_width);
        assert!(usize::from(state.scroll.scroll_y) <= state.content_height);

        state.scroll.scroll_x = 9;
        (&DetailTable {
            rows: &rows,
            label_width: 0,
            wrap: true,
            theme: &theme,
        })
            .render(buffer.area, &mut buffer, &mut state);
        assert_eq!(state.scroll.scroll_x, 0);
        assert!(state.content_height > rows.len());

        state.selected = Some("role");
        state.scroll.scroll_y = 0;
        (&DetailTable {
            rows: &rows,
            label_width: 0,
            wrap: true,
            theme: &theme,
        })
            .render(buffer.area, &mut buffer, &mut state);
        assert!(state.scroll.scroll_y > 0);
    }

    #[test]
    fn hyperlink_regions_use_caller_urls_and_visible_value_geometry() {
        let rows = rows();
        let theme = Theme::default();
        let table = DetailTable {
            rows: &rows,
            label_width: 0,
            wrap: false,
            theme: &theme,
        };
        let area = Rect::new(0, 0, 40, 3);
        let mut state = DetailTableState::default();
        let mut buffer = Buffer::empty(area);
        (&table).render(area, &mut buffer, &mut state);
        let links = table.hyperlink_regions(&state);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].id, "log");
        assert_eq!(links[0].url, "file:///wide/🧪🔬/path");
        assert!(links[0].area.width > 0);
    }
}
