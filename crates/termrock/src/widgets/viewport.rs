use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Style,
    text::Line,
    widgets::StatefulWidget,
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::{
    interaction::Outcome,
    scroll::{DialogScroll, UNCACHED_REVISION, max_line_width},
    style::{DesignSystem, Role},
};

use super::{PanelChrome, Surface, SurfaceFill, SurfaceRecipe};

/// Logical line and display-column position in a viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CellPos {
    /// Zero-based logical line.
    pub line: usize,
    /// Zero-based terminal display column.
    pub col: usize,
}

/// Events emitted by viewport interaction handlers.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ViewportEvent {
    /// The host should put the selected text on its clipboard.
    Copy(String),
    /// The text selection changed.
    SelectionChanged,
}

#[derive(Debug, Clone)]
struct ViewportCell {
    symbol: String,
    width: usize,
    style: Style,
}

/// Persistent interaction and layout state for [`Viewport`].
///
/// `Viewport` is a borrowed render configuration and is commonly rebuilt on
/// every frame. Selection, drag state, and scroll therefore live here rather
/// than in the widget value.
#[derive(Debug, Clone, Default)]
pub struct ViewportState {
    /// Shared two-axis scroll offsets.
    pub scroll: DialogScroll,
    area: Rect,
    selection: Option<(CellPos, CellPos)>,
    drag_anchor: Option<CellPos>,
    cells: Vec<Vec<ViewportCell>>,
    cached_len: usize,
    cached_revision: u64,
    cached_base_style: Style,
    cache_valid: bool,
}

impl ViewportState {
    /// Creates empty viewport interaction state.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            scroll: DialogScroll::new(),
            area: Rect::ZERO,
            selection: None,
            drag_anchor: None,
            cells: Vec::new(),
            cached_len: 0,
            cached_revision: UNCACHED_REVISION,
            cached_base_style: Style::new(),
            cache_valid: false,
        }
    }

    /// Rebases logical line positions after a host removes a prefix.
    pub(crate) fn rebase_lines(&mut self, removed: usize) {
        if removed == 0 {
            return;
        }
        if let Some((start, end)) = self.selection.as_mut() {
            start.line = start.line.saturating_sub(removed);
            end.line = end.line.saturating_sub(removed);
        }
        if let Some(anchor) = self.drag_anchor.as_mut() {
            anchor.line = anchor.line.saturating_sub(removed);
        }
    }

    fn ensure_cells(&mut self, lines: &[Line<'_>], base_style: Style, revision: u64) {
        if revision != UNCACHED_REVISION
            && self.cache_valid
            && self.cached_len == lines.len()
            && self.cached_revision == revision
            && self.cached_base_style == base_style
        {
            return;
        }

        self.cells = lines
            .iter()
            .map(|line| {
                let line_style = base_style.patch(line.style);
                line.spans
                    .iter()
                    .flat_map(move |span| {
                        let style = line_style.patch(span.style);
                        span.content
                            .as_ref()
                            .graphemes(true)
                            .flat_map(move |grapheme| {
                                if grapheme == "\t" {
                                    return (0..4)
                                        .map(|_| ViewportCell {
                                            symbol: " ".to_owned(),
                                            width: 1,
                                            style,
                                        })
                                        .collect::<Vec<_>>();
                                }
                                if grapheme.contains(char::is_control) {
                                    return Vec::new();
                                }
                                let width = UnicodeWidthStr::width(grapheme);
                                if width == 0 {
                                    Vec::new()
                                } else {
                                    vec![ViewportCell {
                                        symbol: grapheme.to_owned(),
                                        width,
                                        style,
                                    }]
                                }
                            })
                    })
                    .collect()
            })
            .collect();
        self.cached_len = lines.len();
        self.cached_revision = revision;
        self.cached_base_style = base_style;
        self.cache_valid = revision != UNCACHED_REVISION;

        let line_count = self.cells.len();
        if line_count == 0 {
            self.selection = None;
            self.drag_anchor = None;
        } else {
            if let Some((start, end)) = self.selection.as_mut() {
                start.line = start.line.min(line_count - 1);
                end.line = end.line.min(line_count - 1);
            }
            if let Some(anchor) = self.drag_anchor.as_mut() {
                anchor.line = anchor.line.min(line_count - 1);
            }
        }
    }

    fn normalized_selection(&self) -> Option<(CellPos, CellPos)> {
        let (start, end) = self.selection?;
        (start != end).then_some((start.min(end), start.max(end)))
    }

    fn line_width(&self, line: usize) -> usize {
        self.cells
            .get(line)
            .map(|cells| cells.iter().map(|cell| cell.width).sum())
            .unwrap_or(0)
    }

    fn max_line_width(&self) -> usize {
        self.cells
            .iter()
            .map(|cells| cells.iter().map(|cell| cell.width).sum())
            .max()
            .unwrap_or(0)
    }

    fn column_of(&self, line: usize, cell: usize) -> usize {
        self.cells
            .get(line)
            .map(|cells| cells.iter().take(cell).map(|c| c.width).sum())
            .unwrap_or(0)
    }

    fn cell_at(&self, line: usize, column: usize) -> usize {
        let Some(cells) = self.cells.get(line) else {
            return 0;
        };
        let mut current: usize = 0;
        for (index, cell) in cells.iter().enumerate() {
            if current.saturating_add(cell.width) > column {
                return index;
            }
            current += cell.width;
        }
        cells.len()
    }

    fn pos_at(&self, position: Position) -> Option<CellPos> {
        if self.area.is_empty() || self.cells.is_empty() || !self.area.contains(position) {
            return None;
        }
        let row = usize::from(position.y.saturating_sub(self.area.y))
            .saturating_add(usize::from(self.scroll.scroll_y))
            .min(self.cells.len() - 1);
        let raw_column = usize::from(position.x.saturating_sub(self.area.x))
            .saturating_add(usize::from(self.scroll.scroll_x))
            .min(self.line_width(row));
        let cell = self.cell_at(row, raw_column);
        let cell_start = self.column_of(row, cell);
        let cell_end = self.column_of(row, cell.saturating_add(1));
        let column = if raw_column.saturating_sub(cell_start).saturating_mul(2)
            >= cell_end.saturating_sub(cell_start)
        {
            cell_end
        } else {
            cell_start
        };
        Some(CellPos {
            line: row,
            col: column,
        })
    }

    fn max_scroll_y(&self) -> u16 {
        u16::try_from(
            self.cells
                .len()
                .saturating_sub(usize::from(self.area.height)),
        )
        .unwrap_or(u16::MAX)
    }

    fn scroll_axes(&self) -> crate::scroll::ScrollAxes {
        crate::scroll::ScrollAxes {
            vertical: crate::scroll::is_scrollable(self.cells.len(), usize::from(self.area.height)),
            horizontal: crate::scroll::is_scrollable(
                self.max_line_width(),
                usize::from(self.area.width),
            ),
        }
    }

    fn set_scroll_y(&mut self, offset: u16) {
        let next = offset.min(self.max_scroll_y());
        self.scroll.scroll_y = next;
    }

    fn scroll_by(&mut self, delta: isize) {
        let current = usize::from(self.scroll.scroll_y);
        let next = if delta.is_negative() {
            current.saturating_sub(delta.unsigned_abs())
        } else {
            current
                .saturating_add(delta.unsigned_abs())
                .min(usize::from(self.max_scroll_y()))
        };
        self.set_scroll_y(u16::try_from(next).unwrap_or(u16::MAX));
    }

    fn scrollbar_area(&self) -> Rect {
        Rect::new(self.area.right(), self.area.y, 1, self.area.height)
    }

    fn scroll_to_track(&mut self, position: Position) -> bool {
        let track = self.scrollbar_area();
        if !track.contains(position) || self.max_scroll_y() == 0 {
            return false;
        }
        let track_len = usize::from(track.height.saturating_sub(1)).max(1);
        let along = usize::from(position.y.saturating_sub(track.y)).min(track_len);
        let target = along
            .saturating_mul(usize::from(self.max_scroll_y()))
            .checked_div(track_len)
            .unwrap_or(0);
        let before = self.scroll.scroll_y;
        self.set_scroll_y(u16::try_from(target).unwrap_or(u16::MAX));
        before != self.scroll.scroll_y
    }
}

#[derive(Debug, Clone)]
/// A scrollable view over borrowed terminal lines.
pub struct Viewport<'a> {
    lines: &'a [Line<'a>],
    title: Option<&'a str>,
    emphasis: PanelChrome,
    system: &'a DesignSystem,
    content_style: Option<Style>,
    content_revision: u64,
    padded_content: bool,
}

impl<'a> Viewport<'a> {
    #[must_use]
    /// Creates a viewport over borrowed lines with zero scroll offset.
    pub const fn new(lines: &'a [Line<'a>], system: &'a DesignSystem) -> Self {
        Self {
            lines,
            title: None,
            emphasis: PanelChrome::Normal,
            system,
            content_style: None,
            content_revision: UNCACHED_REVISION,
            padded_content: false,
        }
    }

    #[must_use]
    /// Sets the optional visible title.
    pub const fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }

    #[must_use]
    /// Selects the border emphasis for the active interaction owner.
    pub const fn emphasis(mut self, emphasis: PanelChrome) -> Self {
        self.emphasis = emphasis;
        self
    }

    #[must_use]
    /// Sets the style applied to dialog content.
    pub const fn content_style(mut self, content_style: Style) -> Self {
        self.content_style = Some(content_style);
        self
    }

    /// Insets content horizontally by the density's `pad_x`.
    ///
    /// A viewport owns no rhythm rows, so the inset is horizontal only:
    /// content stays flush with the border on Y while its X column matches
    /// the body column a [`super::Panel`] would give the same frame. Hosts
    /// migrating framed bodies from `Panel` to `Viewport` opt in here to
    /// keep their content column stable.
    #[must_use]
    pub const fn padded_content(mut self) -> Self {
        self.padded_content = true;
        self
    }

    /// Enables measurement reuse for unchanged content.
    ///
    /// Bump `revision` whenever line contents change. Length changes invalidate
    /// the cache automatically. Omitting this builder measures every frame.
    #[must_use]
    pub const fn content_revision(mut self, revision: u64) -> Self {
        self.content_revision = revision;
        self
    }

    fn base_content_style(&self) -> Style {
        self.content_style
            .unwrap_or_else(|| self.system.style(Role::Text))
    }

    fn ensure_interaction_layout(&self, state: &mut ViewportState) {
        state.ensure_cells(self.lines, self.base_content_style(), self.content_revision);
    }

    /// Lays out the selectable body for pointer/key routing.
    ///
    /// Rendering calls this automatically. Hosts that route an event before
    /// the first paint can call it explicitly with the same body rectangle.
    pub fn set_area(&self, area: Rect, state: &mut ViewportState) {
        self.ensure_interaction_layout(state);
        state.area = area;
        state.scroll.clamp(
            self.lines.len(),
            usize::from(area.height),
            state.max_line_width(),
            usize::from(area.width),
        );
    }

    /// Returns the logical position under a terminal cell.
    #[must_use]
    pub fn pos_at(&self, state: &ViewportState, position: Position) -> Option<CellPos> {
        state.pos_at(position)
    }

    /// Returns the normalized logical selection, if it spans at least one cell.
    #[must_use]
    pub fn selection(&self, state: &ViewportState) -> Option<(CellPos, CellPos)> {
        state.normalized_selection()
    }
    /// Returns whether text is selected.
    #[must_use]
    pub fn has_selection(&self, state: &ViewportState) -> bool {
        self.selection(state).is_some()
    }

    /// Clears text selection. The drag anchor is retained like the reference.
    pub fn clear_selection(&self, state: &mut ViewportState) -> Outcome<()> {
        if state.selection.take().is_some() {
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    /// Returns the selected text, preserving line breaks and terminal spaces.
    #[must_use]
    pub fn selected_text(&self, state: &ViewportState) -> Option<String> {
        let (start, end) = state.normalized_selection()?;
        let mut text = String::new();
        for line in start.line..=end.line {
            let cells = state.cells.get(line)?;
            let from = if line == start.line {
                state.cell_at(line, start.col)
            } else {
                0
            };
            let to = if line == end.line {
                state.cell_at(line, end.col)
            } else {
                cells.len()
            };
            let line_text: String = cells[from.min(cells.len())..to.min(cells.len())]
                .iter()
                .map(|cell| cell.symbol.as_str())
                .collect();
            text.push_str(&line_text);
            if line != end.line {
                text.push('\n');
            }
        }
        Some(text)
    }

    /// Returns a fresh copy payload for the current selection.
    #[must_use]
    pub fn copy_selection(&self, state: &ViewportState) -> Option<String> {
        self.selected_text(state)
    }

    /// Mouse down: anchor a selection drag.
    pub fn on_click(&self, state: &mut ViewportState, position: Position) -> Outcome<()> {
        self.ensure_interaction_layout(state);
        if !state.area.contains(position) {
            return Outcome::Ignored;
        }
        let had_selection = state.selection.is_some();
        state.selection = None;
        state.drag_anchor = state.pos_at(position);
        if state.drag_anchor.is_some() || had_selection {
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    /// Drag from the current anchor, auto-scrolling at vertical edges.
    pub fn on_drag(&self, state: &mut ViewportState, position: Position) -> Outcome<()> {
        self.ensure_interaction_layout(state);
        let Some(anchor) = state.drag_anchor else {
            return Outcome::Ignored;
        };
        let before_scroll_y = state.scroll.scroll_y;
        let before_selection = state.selection;
        if position.y < state.area.y {
            state.scroll_by(-1);
        } else if position.y >= state.area.bottom() {
            state.scroll_by(1);
        }
        let clamped = Position::new(
            position
                .x
                .clamp(state.area.x, state.area.right().saturating_sub(1)),
            position
                .y
                .clamp(state.area.y, state.area.bottom().saturating_sub(1)),
        );
        let Some(head) = state.pos_at(clamped) else {
            return if state.scroll.scroll_y != before_scroll_y {
                Outcome::Changed
            } else {
                Outcome::Ignored
            };
        };
        state.selection = Some((anchor, head));
        if state.scroll.scroll_y != before_scroll_y || state.selection != before_selection {
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    /// Double-click: select the word under the pointer.
    pub fn select_word_at(&self, state: &mut ViewportState, position: Position) -> Outcome<()> {
        self.ensure_interaction_layout(state);
        let Some(position) = state.pos_at(position) else {
            return Outcome::Ignored;
        };
        let Some(cells) = state.cells.get(position.line) else {
            return Outcome::Ignored;
        };
        if cells.is_empty() {
            return Outcome::Ignored;
        }
        let index = state
            .cell_at(position.line, position.col)
            .min(cells.len().saturating_sub(1));
        let is_word = |cell: &ViewportCell| {
            cell.symbol.chars().all(|character| {
                character.is_alphanumeric() || matches!(character, '_' | '-' | '/' | '.')
            })
        };
        if !is_word(&cells[index]) {
            return self.clear_selection(state);
        }
        let mut start = index;
        while start > 0 && is_word(&cells[start - 1]) {
            start -= 1;
        }
        let mut end = index + 1;
        while end < cells.len() && is_word(&cells[end]) {
            end += 1;
        }
        state.selection = Some((
            CellPos {
                line: position.line,
                col: state.column_of(position.line, start),
            },
            CellPos {
                line: position.line,
                col: state.column_of(position.line, end),
            },
        ));
        state.drag_anchor = None;
        Outcome::Changed
    }

    /// Routes pointer scrolling and left-button selection gestures.
    pub fn on_mouse(
        &self,
        state: &mut ViewportState,
        event: crate::input::MouseEvent,
    ) -> Outcome<()> {
        match event.kind {
            crate::input::MouseEventKind::ScrollUp
            | crate::input::MouseEventKind::ScrollDown
            | crate::input::MouseEventKind::ScrollLeft
            | crate::input::MouseEventKind::ScrollRight => {
                self.ensure_interaction_layout(state);
                if !state.area.contains(event.position)
                    && !state.scrollbar_area().contains(event.position)
                {
                    return Outcome::Ignored;
                }
                let axes = state.scroll_axes();
                let before = state.scroll.clone();
                let handled = state.scroll.handle_mouse(event.kind, event.modifiers, axes);
                state.scroll.clamp(
                    self.lines.len(),
                    usize::from(state.area.height),
                    state.max_line_width(),
                    usize::from(state.area.width),
                );
                if handled && state.scroll != before {
                    Outcome::Changed
                } else {
                    Outcome::Ignored
                }
            }
            crate::input::MouseEventKind::Down(crate::input::MouseButton::Left) => {
                if state.scroll_to_track(event.position) {
                    return Outcome::Changed;
                }
                self.on_click(state, event.position)
            }
            crate::input::MouseEventKind::Drag(crate::input::MouseButton::Left) => {
                if state.scrollbar_area().contains(event.position) {
                    return if state.scroll_to_track(event.position) {
                        Outcome::Changed
                    } else {
                        Outcome::Ignored
                    };
                }
                self.on_drag(state, event.position)
            }
            crate::input::MouseEventKind::Up(crate::input::MouseButton::Left) => {
                if state.drag_anchor.take().is_some() {
                    Outcome::Changed
                } else {
                    Outcome::Ignored
                }
            }
            _ => Outcome::Ignored,
        }
    }

    /// Handles viewport navigation, copy (`y`), and selection clearing (`Esc`).
    pub fn on_key(
        &self,
        state: &mut ViewportState,
        key: crate::input::KeyEvent,
    ) -> (Outcome<()>, Option<ViewportEvent>) {
        use crate::input::KeyCode;

        if key.is_release() {
            return (Outcome::Ignored, None);
        }
        self.ensure_interaction_layout(state);
        if key.is_press() && matches!(key.code, KeyCode::Char('y')) && key.modifiers.is_empty() {
            return match self.selected_text(state) {
                Some(text) => (Outcome::Changed, Some(ViewportEvent::Copy(text))),
                None => (Outcome::Ignored, None),
            };
        }
        if key.code == KeyCode::Esc {
            return match self.clear_selection(state) {
                Outcome::Changed => (Outcome::Changed, Some(ViewportEvent::SelectionChanged)),
                Outcome::Ignored => (Outcome::Ignored, None),
                _ => unreachable!("clear_selection only returns Changed or Ignored"),
            };
        }

        let axes = state.scroll_axes();
        let before = state.scroll.clone();
        let handled = state.scroll.handle_key_for_axes(
            key,
            self.lines.len(),
            usize::from(state.area.height),
            state.max_line_width(),
            usize::from(state.area.width),
            axes,
        );
        if handled && state.scroll != before {
            (Outcome::Changed, None)
        } else {
            (Outcome::Ignored, None)
        }
    }

    /// Paint without consuming the reusable interaction state.
    pub fn paint(
        &self,
        area: Rect,
        buffer: &mut ratatui_core::buffer::Buffer,
        state: &mut ViewportState,
    ) {
        <&Self as StatefulWidget>::render(self, area, buffer, state);
    }
}

impl StatefulWidget for &Viewport<'_> {
    type State = ViewportState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        let pad_x = if self.padded_content {
            self.system.spacing.card_inset
        } else {
            0
        };
        // Surface owns the structural ring; the viewport owns its asymmetric
        // content inset so the scrollbar keeps the trailing border column.
        let surface_recipe = match self.emphasis {
            PanelChrome::Focused => SurfaceRecipe::Focused,
            PanelChrome::Danger => SurfaceRecipe::Destructive,
            PanelChrome::Normal => SurfaceRecipe::Interactive,
        };
        let surface_content = Surface::new(self.system)
            .recipe(surface_recipe)
            .bordered(true)
            .fill(SurfaceFill::Transparent)
            .padding(0, 0)
            .paint(area, buffer);
        let content = Rect::new(
            surface_content.x.saturating_add(pad_x),
            surface_content.y,
            surface_content.width.saturating_sub(pad_x),
            surface_content.height,
        );
        let viewport_width = usize::from(content.width);
        let viewport_height = usize::from(content.height);
        let (content_width, _) = state.scroll.measurement.get_or_measure(
            self.lines.len(),
            self.content_revision,
            || (max_line_width(self.lines), self.lines.len()),
        );
        state.scroll.clamp(
            self.lines.len(),
            viewport_height,
            content_width,
            viewport_width,
        );
        self.set_area(content, state);
        if let Some(title) = self.title {
            let budget = usize::from(area.width.saturating_sub(2));
            let clipped = crate::text::truncate_cols(
                title.trim(),
                budget.saturating_sub(2),
                self.system.glyphs.ellipsis(),
            );
            let label = format!(" {clipped} ");
            buffer.set_stringn(
                area.x.saturating_add(1),
                area.y,
                label,
                budget,
                self.system.style(Role::TextStrong),
            );
        }
        // Paint only visible logical lines and cells. This keeps the hot path
        // proportional to the viewport while allowing selection backgrounds.
        self.ensure_interaction_layout(state);
        let selection = state.normalized_selection();
        let start = usize::from(state.scroll.scroll_y).min(self.lines.len());
        for (row, line) in state.cells[start..]
            .iter()
            .take(viewport_height)
            .enumerate()
        {
            let line_index = start + row;
            let mut column: usize = 0;
            for cell in line {
                let cell_end = column.saturating_add(cell.width);
                if cell_end <= usize::from(state.scroll.scroll_x) {
                    column = cell_end;
                    continue;
                }
                let visible_column = column.saturating_sub(usize::from(state.scroll.scroll_x));
                let x = content
                    .x
                    .saturating_add(u16::try_from(visible_column).unwrap_or(u16::MAX));
                if x >= content.right() {
                    break;
                }
                if column < usize::from(state.scroll.scroll_x) {
                    column = cell_end;
                    continue;
                }
                let selected = selection.is_some_and(|(a, b)| {
                    let position = CellPos {
                        line: line_index,
                        col: column,
                    };
                    position >= a && position < b
                });
                let style = if selected {
                    self.system
                        .style(Role::Selection)
                        .add_modifier(cell.style.add_modifier)
                } else {
                    cell.style
                };
                buffer.set_stringn(
                    x,
                    content
                        .y
                        .saturating_add(u16::try_from(row).unwrap_or(u16::MAX)),
                    &cell.symbol,
                    usize::from(content.right().saturating_sub(x)),
                    style,
                );
                column = cell_end;
            }
            if let Some((a, b)) = selection
                && line_index >= a.line
                && line_index < b.line
            {
                let tail = state
                    .line_width(line_index)
                    .saturating_sub(usize::from(state.scroll.scroll_x));
                let tail_x = content
                    .x
                    .saturating_add(u16::try_from(tail).unwrap_or(u16::MAX));
                if tail_x < content.right() {
                    buffer.set_style(
                        Rect::new(
                            tail_x,
                            content
                                .y
                                .saturating_add(u16::try_from(row).unwrap_or(u16::MAX)),
                            1,
                            1,
                        ),
                        self.system.style(Role::Selection),
                    );
                }
            }
        }
        // The scrollbar belongs to the reserved gutter, never to content.
        crate::scroll::paint_overflow_scrollbar(
            buffer,
            crate::scroll::gutter_column(Rect::new(
                area.x,
                area.y.saturating_add(1),
                area.width,
                area.height.saturating_sub(2),
            )),
            self.lines.len(),
            viewport_height,
            state.scroll.scroll_y,
            false,
            self.system,
        );
    }
}

impl StatefulWidget for Viewport<'_> {
    type State = ViewportState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines() -> [Line<'static>; 3] {
        [Line::from("alpha"), Line::from("beta"), Line::from("gamma")]
    }

    #[test]
    fn content_is_flush_with_the_border_by_default() {
        let lines = lines();
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 20, 5);
        let mut buffer = Buffer::empty(area);
        let mut scroll = ViewportState::default();
        (&Viewport::new(&lines, &system)).render(area, &mut buffer, &mut scroll);
        assert_eq!(
            buffer[(1, 1)].symbol(),
            "a",
            "content starts at the inner column"
        );
    }

    #[test]
    fn padded_content_insets_x_but_stays_flush_on_y() {
        let lines = lines();
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 20, 5);
        let mut buffer = Buffer::empty(area);
        let mut scroll = ViewportState::default();
        (&Viewport::new(&lines, &system).padded_content()).render(area, &mut buffer, &mut scroll);
        let pad_x = crate::style::SpacingScale::junie().card_inset;
        assert_eq!(buffer[(1, 1)].symbol(), " ", "the pad column stays empty");
        assert_eq!(
            buffer[(1 + pad_x, 1)].symbol(),
            "a",
            "content aligns with the Panel body column"
        );
        assert_eq!(
            buffer[(1 + pad_x, 2)].symbol(),
            "b",
            "rows stay flush on Y — no rhythm row is inserted"
        );
    }
}
