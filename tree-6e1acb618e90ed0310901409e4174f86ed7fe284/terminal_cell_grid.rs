// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Borrowed terminal-cell projection into a Ratatui buffer.

use ratatui_core::{
    buffer::{Buffer, CellDiffOption},
    layout::Rect,
    style::Style,
    widgets::Widget,
};

/// One projected terminal cell.
///
/// Hosts convert emulator-specific cells into this neutral view. Wide-glyph
/// continuation cells should be returned as blank ordinary cells.
#[derive(Debug, Clone, Copy)]
pub struct TerminalCell<'a> {
    /// Grapheme painted at this cell; an empty value paints one blank.
    pub symbol: &'a str,
    /// Exact terminal style, including foreground, background, and modifiers.
    pub style: Style,
    /// Ratatui diff behavior for forced-width or always-redrawn cells.
    pub diff: CellDiffOption,
}

impl<'a> TerminalCell<'a> {
    /// Creates an ordinary projected cell.
    #[must_use]
    pub const fn new(symbol: &'a str, style: Style) -> Self {
        Self {
            symbol,
            style,
            diff: CellDiffOption::None,
        }
    }

    /// Supplies Ratatui's terminal diff behavior.
    #[must_use]
    pub const fn diff(mut self, diff: CellDiffOption) -> Self {
        self.diff = diff;
        self
    }
}

/// Borrowed row/column access to an emulator or terminal-screen snapshot.
pub trait TerminalCellSource {
    /// Returns `(rows, columns)` in terminal cells.
    fn size(&self) -> (u16, u16);

    /// Projects one cell, or `None` when the source has no cell there.
    fn cell(&self, row: u16, column: u16) -> Option<TerminalCell<'_>>;
}

/// Ratatui widget that clips and blits a borrowed terminal-cell source.
///
/// The widget clears every destination cell not supplied by the source, so a
/// shrinking terminal snapshot cannot leak stale content from an older frame.
#[derive(Clone, Copy)]
pub struct TerminalCellGrid<'a> {
    source: &'a dyn TerminalCellSource,
}

impl core::fmt::Debug for TerminalCellGrid<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("TerminalCellGrid")
            .finish_non_exhaustive()
    }
}

impl<'a> TerminalCellGrid<'a> {
    /// Borrows a terminal-cell snapshot for this frame.
    #[must_use]
    pub const fn new(source: &'a dyn TerminalCellSource) -> Self {
        Self { source }
    }
}

impl Widget for TerminalCellGrid<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let (rows, columns) = self.source.size();
        for row in 0..area.height {
            for column in 0..area.width {
                let destination = &mut buffer[(area.x + column, area.y + row)];
                let Some(cell) = (row < rows && column < columns)
                    .then(|| self.source.cell(row, column))
                    .flatten()
                else {
                    destination.reset();
                    continue;
                };

                destination.reset();
                destination.set_symbol(if cell.symbol.is_empty() {
                    " "
                } else {
                    cell.symbol
                });
                destination.set_style(cell.style);
                let diff = match cell.diff {
                    // Skip would preserve stale destination content and violate
                    // this widget's one-frame projection contract.
                    CellDiffOption::Skip => CellDiffOption::None,
                    CellDiffOption::ForcedWidth(width)
                        if column.saturating_add(width.get()) > area.width =>
                    {
                        CellDiffOption::None
                    }
                    other => other,
                };
                destination.set_diff_option(diff);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use core::num::NonZeroU16;

    use ratatui_core::{
        buffer::CellDiffOption,
        style::{Color, Modifier},
    };

    use super::*;

    struct Grid(Vec<Vec<TerminalCell<'static>>>);

    impl TerminalCellSource for Grid {
        fn size(&self) -> (u16, u16) {
            (
                self.0.len() as u16,
                self.0.first().map_or(0, |row| row.len()) as u16,
            )
        }

        fn cell(&self, row: u16, column: u16) -> Option<TerminalCell<'_>> {
            self.0.get(row as usize)?.get(column as usize).copied()
        }
    }

    #[test]
    fn paints_symbols_colors_and_modifiers() {
        let grid = Grid(vec![vec![
            TerminalCell::new("A", Style::new().fg(Color::Green).bold()),
            TerminalCell::new("β", Style::new().bg(Color::Blue).italic()),
        ]]);
        let area = Rect::new(0, 0, 2, 1);
        let mut buffer = Buffer::empty(area);
        TerminalCellGrid::new(&grid).render(area, &mut buffer);
        assert_eq!(buffer[(0, 0)].symbol(), "A");
        assert_eq!(buffer[(0, 0)].fg, Color::Green);
        assert!(buffer[(0, 0)].modifier.contains(Modifier::BOLD));
        assert_eq!(buffer[(1, 0)].symbol(), "β");
        assert_eq!(buffer[(1, 0)].bg, Color::Blue);
        assert!(buffer[(1, 0)].modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn clears_destination_outside_source() {
        let grid = Grid(vec![vec![TerminalCell::new("x", Style::new())]]);
        let area = Rect::new(0, 0, 3, 2);
        let mut buffer = Buffer::with_lines(["old", "old"]);
        TerminalCellGrid::new(&grid).render(area, &mut buffer);
        assert_eq!(buffer[(0, 0)].symbol(), "x");
        assert_eq!(buffer[(1, 0)].symbol(), " ");
        assert_eq!(buffer[(2, 1)].symbol(), " ");
    }

    #[test]
    fn narrow_clip_drops_invalid_forced_width_and_skip() {
        let width = NonZeroU16::new(2).unwrap();
        let grid = Grid(vec![vec![
            TerminalCell::new("界", Style::new()).diff(CellDiffOption::ForcedWidth(width)),
            TerminalCell::new("x", Style::new()).diff(CellDiffOption::Skip),
        ]]);
        let area = Rect::new(0, 0, 1, 1);
        let mut buffer = Buffer::empty(area);
        TerminalCellGrid::new(&grid).render(area, &mut buffer);
        assert_eq!(buffer[(0, 0)].symbol(), "界");
        assert_eq!(buffer[(0, 0)].diff_option, CellDiffOption::None);

        let area = Rect::new(0, 0, 2, 1);
        let mut buffer = Buffer::empty(area);
        TerminalCellGrid::new(&grid).render(area, &mut buffer);
        assert_eq!(buffer[(1, 0)].diff_option, CellDiffOption::None);
    }

    #[test]
    fn ascii_reset_cell_remains_unstyled() {
        let grid = Grid(vec![vec![TerminalCell::new("#", Style::reset())]]);
        let area = Rect::new(0, 0, 1, 1);
        let mut buffer = Buffer::empty(area);
        TerminalCellGrid::new(&grid).render(area, &mut buffer);
        assert_eq!(buffer[(0, 0)].symbol(), "#");
        assert_eq!(buffer[(0, 0)].fg, Color::Reset);
        assert_eq!(buffer[(0, 0)].bg, Color::Reset);
    }
}
