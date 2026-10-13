use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::{Modifier, Style},
    widgets::StatefulWidget,
};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind},
    osc::HyperlinkRegion,
    scroll::{DialogScroll, Measured, UNCACHED_REVISION, effective_offset},
    style::{Role, Theme},
};

const SELECTED_MARKER: &str = "▸ ";
const NORMAL_MARKER: &str = "  ";
const SEPARATOR: &str = " : ";
const COPY_AFFORDANCE: &str = "  ⧉";
const COPIED_AFFORDANCE: &str = "  ✓";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Optional activation capabilities exposed by a detail row.
pub enum DetailCapability {
    /// The row supports none.
    None,
    /// The row supports copy.
    Copy,
    /// The row supports link.
    Link,
    /// The row supports copy and link.
    CopyAndLink,
}

impl DetailCapability {
    #[must_use]
    /// Marks the detail row as copyable.
    pub const fn copyable(self) -> bool {
        matches!(self, Self::Copy | Self::CopyAndLink)
    }

    #[must_use]
    /// Associates the detail row with an activatable hyperlink.
    pub const fn linkable(self) -> bool {
        matches!(self, Self::Link | Self::CopyAndLink)
    }
}

#[derive(Debug, Clone)]
/// A stable key/value row with optional activation capabilities.
pub struct DetailRow<'a, Id> {
    /// Stable identity used for selection and activation.
    pub id: Id,
    /// Caller-visible label.
    pub label: &'a str,
    /// Caller-owned value displayed by this item.
    pub value: &'a str,
    /// Optional hyperlink target associated with the value.
    pub href: Option<&'a str>,
    /// Optional semantic activation supported by the row.
    pub capability: DetailCapability,
    /// Whether rendering should emphasize the row value.
    pub emphasis: bool,
    /// Ratatui style applied while rendering this item.
    pub style: Option<Style>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
/// Semantic results produced by detail-table interaction.
pub enum DetailTableOutcome<Id> {
    /// Reports ignored.
    Ignored,
    /// Reports selected.
    Selected(Id),
    /// Reports copy.
    Copy(Id),
    /// Reports activate link.
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
/// Runtime state for `DetailTable`.
pub struct DetailTableState<Id> {
    /// Whether this item is selected.
    pub selected: Option<Id>,
    /// Whether this item is hovered.
    pub hovered: Option<Id>,
    /// Stable identity most recently copied, for confirmation feedback.
    pub copied: Option<Id>,
    /// Two-axis scroll offsets and measured bounds.
    pub scroll: DialogScroll,
    /// Hit regions produced by the most recent render.
    pub regions: Vec<DetailRegion<Id>>,
    /// Content width in terminal cells.
    pub content_width: usize,
    /// Content height in terminal rows.
    pub content_height: usize,
    /// Painted body rectangle from the most recent render.
    pub viewport: Rect,
    /// Revision-keyed cached content dimensions.
    pub(crate) measurement: Measured,
    /// Visual-row start offset for each projected row.
    pub(crate) row_offsets: Vec<usize>,
    /// Cached visual range occupied by the selected row.
    pub(crate) selected_range: Option<(usize, usize)>,
    /// Selection identity used to validate `selected_range`.
    pub(crate) measured_selected: Option<Id>,
    /// Label width used by the cached row geometry.
    pub(crate) measured_label_width: usize,
    /// Viewport width used by the cached wrapping geometry.
    pub(crate) measured_area_width: u16,
    /// Explicit label-width setting used by the cached geometry.
    pub(crate) measured_label_setting: u16,
    /// Wrap setting used by the cached geometry.
    pub(crate) measured_wrap: bool,
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
            measurement: Measured::default(),
            row_offsets: Vec::new(),
            selected_range: None,
            measured_selected: None,
            measured_label_width: 0,
            measured_area_width: 0,
            measured_label_setting: 0,
            measured_wrap: false,
        }
    }
}

impl<Id: Clone + PartialEq> DetailTableState<Id> {
    /// Handles the `handle_key` interaction.
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

    /// Moves selection to the next enabled item, wrapping at the end.
    pub fn select_next(&mut self, rows: &[DetailRow<'_, Id>]) -> DetailTableOutcome<Id> {
        self.select_relative(rows, 1)
    }

    /// Moves selection to the previous enabled item, wrapping at the start.
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

    /// Updates hover state from the current pointer position and painted hit regions.
    pub fn hover(&mut self, position: Position) -> Option<&Id> {
        self.hovered = self
            .regions
            .iter()
            .find(|region| region.action_area.contains(position))
            .map(|region| region.id.clone());
        self.hovered.as_ref()
    }

    #[must_use]
    /// Maps a pointer position to the semantic outcome of the painted hit region.
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
    /// Returns hyperlink activation only for a painted link region.
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

    /// Marks the copied row so rendering can expose confirmation.
    pub fn mark_copied(&mut self, id: Option<Id>) {
        self.copied = id;
    }

    /// Clamps table scrolling after rows or viewport geometry change.
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
    /// Returns the semantic outcome for the currently selected item.
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
/// A selectable key/value table with typed row activation.
pub struct DetailTable<'a, Id> {
    rows: &'a [DetailRow<'a, Id>],
    /// Zero derives the label width from the borrowed rows.
    label_width: u16,
    /// Wrap values into aligned continuation rows instead of scrolling horizontally.
    wrap: bool,
    theme: &'a Theme,
    content_revision: u64,
}

impl<'a, Id> DetailTable<'a, Id> {
    #[must_use]
    /// Creates a detail table over borrowed rows and mutable table state.
    pub const fn new(rows: &'a [DetailRow<'a, Id>], theme: &'a Theme) -> Self {
        Self {
            rows,
            label_width: 0,
            wrap: false,
            theme,
            content_revision: UNCACHED_REVISION,
        }
    }

    #[must_use]
    /// Reserves a fixed label width in terminal display columns.
    pub const fn label_width(mut self, label_width: u16) -> Self {
        self.label_width = label_width;
        self
    }

    #[must_use]
    /// Sets whether long content wraps instead of scrolling horizontally.
    pub const fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }

    /// Enables measurement reuse for unchanged rows.
    ///
    /// Bump `revision` whenever row contents change. Length changes invalidate
    /// the cache automatically. Omitting this builder measures every frame.
    #[must_use]
    pub const fn content_revision(mut self, revision: u64) -> Self {
        self.content_revision = revision;
        self
    }
}

impl<Id: Clone + PartialEq> DetailTable<'_, Id> {
    #[must_use]
    /// Returns hyperlink hit regions produced by the most recent render.
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

    fn resolved_label_width(&self) -> usize {
        if self.label_width == 0 {
            self.rows
                .iter()
                .map(|row| crate::text::display_cols(row.label))
                .max()
                .unwrap_or(0)
        } else {
            usize::from(self.label_width)
        }
    }

    fn row_width(&self, row: &DetailRow<'_, Id>, label_width: usize) -> usize {
        crate::text::display_cols(SELECTED_MARKER)
            + label_width
            + crate::text::display_cols(SEPARATOR)
            + crate::text::display_cols(row.value)
            + affordance_width(row, false)
    }

    fn value_width(&self, area: Rect, label_width: usize) -> usize {
        usize::from(area.width).saturating_sub(
            crate::text::display_cols(SELECTED_MARKER)
                + label_width
                + crate::text::display_cols(SEPARATOR),
        )
    }

    fn row_height(&self, row: &DetailRow<'_, Id>, value_width: usize) -> usize {
        if !self.wrap {
            return 1;
        }
        let width =
            crate::text::display_cols(row.value).saturating_add(affordance_width(row, false));
        width.max(1).div_ceil(value_width.max(1))
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &DetailTable<'_, Id> {
    type State = DetailTableState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        state.regions.clear();
        state.viewport = area;
        let revision = self.content_revision;
        if state.measured_area_width != area.width
            || state.measured_label_setting != self.label_width
            || state.measured_wrap != self.wrap
        {
            state.measurement.invalidate();
            state.measured_area_width = area.width;
            state.measured_label_setting = self.label_width;
            state.measured_wrap = self.wrap;
        }
        if !state.measurement.is_current(self.rows.len(), revision) {
            state.measured_label_width = self.resolved_label_width();
            let value_width = self.value_width(area, state.measured_label_width);
            state.row_offsets.clear();
            state.row_offsets.reserve(self.rows.len());
            state.selected_range = None;
            let mut content_width = 0usize;
            let mut content_height = 0usize;
            for row in self.rows {
                state.row_offsets.push(content_height);
                content_width = content_width.max(self.row_width(row, state.measured_label_width));
                let row_height = self.row_height(row, value_width);
                if state.selected.as_ref() == Some(&row.id) {
                    state.selected_range =
                        Some((content_height, content_height.saturating_add(row_height)));
                }
                content_height = content_height.saturating_add(row_height);
            }
            state.measured_selected = state.selected.clone();
            state
                .measurement
                .get_or_measure(self.rows.len(), revision, || {
                    (content_width, content_height)
                });
        } else if state.measured_selected != state.selected {
            state.selected_range = state.selected.as_ref().and_then(|selected| {
                let index = self.rows.iter().position(|row| &row.id == selected)?;
                let start = state.row_offsets[index];
                let end = state
                    .row_offsets
                    .get(index + 1)
                    .copied()
                    .unwrap_or(state.measurement.height);
                Some((start, end))
            });
            state.measured_selected = state.selected.clone();
        }
        let label_width = state.measured_label_width;
        let value_width = self.value_width(area, label_width);
        state.content_width = state.measurement.width;
        state.content_height = state.measurement.height;
        if self.wrap {
            state.scroll.scroll_x = 0;
        }
        if let Some((start, end)) = state.selected_range {
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
        let window_end = scroll_y.saturating_add(usize::from(area.height));
        let first_row = state
            .row_offsets
            .partition_point(|start| *start <= scroll_y)
            .saturating_sub(1);
        let mut scratch = PaintScratch::default();
        for (index, row) in self.rows.iter().enumerate().skip(first_row) {
            let row_start = state.row_offsets[index];
            if row_start >= window_end {
                break;
            }
            let row_end = state
                .row_offsets
                .get(index + 1)
                .copied()
                .unwrap_or(state.content_height);
            let first_continuation = scroll_y.saturating_sub(row_start);
            for continuation in first_continuation..row_end.saturating_sub(row_start) {
                let visual_row = row_start.saturating_add(continuation);
                if visual_row >= window_end {
                    break;
                }
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
                    &mut scratch,
                );
            }
        }
    }
}

#[derive(Default)]
struct PaintScratch {
    combined: String,
    chunk: String,
    segment: String,
}

impl<Id: Clone + PartialEq> StatefulWidget for DetailTable<'_, Id> {
    type State = DetailTableState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        StatefulWidget::render(&self, area, buffer, state);
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
    scratch: &mut PaintScratch,
) {
    let selected = state.selected.as_ref() == Some(&row.id);
    let hovered = state.hovered.as_ref() == Some(&row.id);
    let copied = state.copied.as_ref() == Some(&row.id);
    let marker = if selected {
        SELECTED_MARKER
    } else {
        NORMAL_MARKER
    };
    let marker_width = crate::text::display_cols(marker);
    let separator_width = crate::text::display_cols(SEPARATOR);
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
            &mut scratch.segment,
        );
        paint_segment(
            buffer,
            area,
            row.label,
            marker_width,
            scroll_x,
            table.theme.style(Role::TextMuted),
            &mut scratch.segment,
        );
        paint_segment(
            buffer,
            area,
            SEPARATOR,
            marker_width + label_width,
            scroll_x,
            table.theme.style(Role::Border),
            &mut scratch.segment,
        );
    }

    let chunk_width = if table.wrap {
        value_width.max(1)
    } else {
        crate::text::display_cols(row.value).saturating_add(affordance_width(row, copied))
    };
    let chunk_start = continuation.saturating_mul(chunk_width);
    scratch.combined.clear();
    scratch.combined.push_str(row.value);
    scratch.combined.push_str(affordance(row, copied));
    crate::text::display_cols_slice_into(
        &scratch.combined,
        chunk_start,
        chunk_width,
        &mut scratch.chunk,
    );
    let global_value_col = if table.wrap && continuation > 0 {
        value_col
    } else {
        value_col.saturating_add(chunk_start)
    };
    paint_segment(
        buffer,
        area,
        &scratch.chunk,
        global_value_col,
        scroll_x,
        value_style,
        &mut scratch.segment,
    );

    let painted_start = global_value_col.max(scroll_x);
    let painted_end = global_value_col
        .saturating_add(crate::text::display_cols(&scratch.chunk))
        .min(scroll_x.saturating_add(usize::from(area.width)));
    let value_end = global_value_col
        .saturating_add(crate::text::display_cols(row.value).saturating_sub(chunk_start))
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
    scratch: &mut String,
) {
    let end = start.saturating_add(crate::text::display_cols(text));
    let viewport_end = scroll_x.saturating_add(usize::from(area.width));
    let visible_start = start.max(scroll_x);
    let visible_end = end.min(viewport_end);
    if visible_start >= visible_end {
        return;
    }
    crate::text::display_cols_slice_into(
        text,
        visible_start.saturating_sub(start),
        visible_end.saturating_sub(visible_start),
        scratch,
    );
    buffer.set_stringn(
        area.x.saturating_add(
            u16::try_from(visible_start.saturating_sub(scroll_x)).unwrap_or(u16::MAX),
        ),
        area.y,
        scratch,
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
    crate::text::display_cols(affordance(row, copied))
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
        let table = DetailTable::new(&rows, &theme);
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
        (&DetailTable::new(&rows, &theme)).render(buffer.area, &mut buffer, &mut state);
        assert!(usize::from(state.scroll.scroll_x) <= state.content_width);
        assert!(usize::from(state.scroll.scroll_y) <= state.content_height);

        state.scroll.scroll_x = 9;
        (&DetailTable::new(&rows, &theme).wrap(true)).render(buffer.area, &mut buffer, &mut state);
        assert_eq!(state.scroll.scroll_x, 0);
        assert!(state.content_height > rows.len());

        state.selected = Some("role");
        state.scroll.scroll_y = 0;
        (&DetailTable::new(&rows, &theme).wrap(true)).render(buffer.area, &mut buffer, &mut state);
        assert!(state.scroll.scroll_y > 0);
    }

    #[test]
    fn hyperlink_regions_use_caller_urls_and_visible_value_geometry() {
        let rows = rows();
        let theme = Theme::default();
        let table = DetailTable::new(&rows, &theme);
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
