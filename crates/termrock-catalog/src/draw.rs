// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/ui/ctx.rs fill() (MIT).

//! Buffer fill used by catalog chrome.

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;

/// Fill an area with a background colour without touching symbols.
pub fn fill(buf: &mut Buffer, area: Rect, style: Style) {
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if let Some(cell) = buf.cell_mut(Position::new(x, y)) {
                cell.set_symbol(" ");
                cell.set_style(style);
            }
        }
    }
}
