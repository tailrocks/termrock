// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! One selection language for collections that paint whole rows.
//!
//! Widgets that render a row as a single string used to hand-roll
//! `match system.selection { … }` and repaint the row in `Role::Focus`, which
//! erased whatever the row was *saying* — an error line stopped being red the
//! moment the cursor landed on it. [`RowChrome`] resolves the same
//! [`ListRowRecipe`] the list family uses and keeps the row's own tone:
//! selection changes the ground and the gutter, never the meaning.

use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, style::Style};

use crate::style::{DesignSystem, ListRowRecipe, ListRowVisualState};

/// Repaints a status glyph inside an already-painted row.
///
/// Status color belongs to the glyph cell and nowhere else: the words stay in
/// the body tone, so a list of five levels reads as one column of color
/// instead of five colored sentences
/// (`docs/design/termrock-design-language.md` §3).
///
/// `column` is the glyph's offset in display columns from the row's left
/// edge. The cell's existing background is kept, so a selection wash painted
/// under the row survives.
pub(crate) fn paint_status_glyph(
    buffer: &mut Buffer,
    row: Rect,
    column: u16,
    glyph: &str,
    style: Style,
) {
    let x = row.x.saturating_add(column);
    if row.width == 0 || row.height == 0 || x >= row.right() {
        return;
    }
    let cell = &mut buffer[(x, row.y)];
    let ground = cell.style().bg;
    let mut style = style;
    if style.bg.is_none() {
        if let Some(bg) = ground {
            style = style.bg(bg);
        }
    }
    cell.set_symbol(glyph);
    cell.set_style(style);
}

/// Resolved row chrome for one painted row.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RowChrome {
    recipe: ListRowRecipe,
    selected: bool,
}

impl RowChrome {
    /// Resolves the shared row recipe for this visual state.
    pub(crate) fn resolve(system: &DesignSystem, state: ListRowVisualState) -> Self {
        Self {
            recipe: system.resolve_list_row(state),
            selected: state.selected,
        }
    }

    /// Style for the row's text, keeping `base`'s meaning.
    ///
    /// `base` carries the widget's semantic role (log level, diff kind,
    /// severity). Selection adds weight and the recipe's ground; it only
    /// supplies a foreground when the row had no opinion of its own.
    pub(crate) fn label_style(&self, base: Style) -> Style {
        let mut style = base;
        if style.fg.is_none() {
            style.fg = self.recipe.label.fg;
        }
        if let Some(bg) = self.wash() {
            style = style.bg(bg);
        }
        if self.selected {
            style = style.add_modifier(Modifier::BOLD);
        }
        style
    }

    /// Background the row sits on, if this chrome washes at all.
    pub(crate) fn wash(&self) -> Option<ratatui_core::style::Color> {
        if self.recipe.use_fill {
            self.recipe.label.bg
        } else if self.recipe.use_tint {
            self.recipe.tint.bg
        } else {
            None
        }
    }

    /// Paints ground and gutter over an already-written row.
    ///
    /// Call after the row's text: cell symbols are preserved, so this only
    /// moves the ground and stamps the gutter glyph into the reserved slot.
    pub(crate) fn paint(&self, buffer: &mut Buffer, row: Rect) {
        if row.width == 0 || row.height == 0 {
            return;
        }
        if let Some(bg) = self.wash() {
            for y in row.top()..row.bottom() {
                for x in row.left()..row.right() {
                    let cell = &mut buffer[(x, y)];
                    let style = cell.style().bg(bg);
                    cell.set_style(style);
                }
            }
        }
        if let Some((glyph, gutter_style)) = self.recipe.gutter {
            let cell = &mut buffer[(row.x, row.y)];
            let mut style = gutter_style;
            if let Some(bg) = self.wash() {
                style = style.bg(bg);
            }
            cell.set_symbol(glyph);
            cell.set_style(style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::Role;

    fn state(selected: bool, focused: bool) -> ListRowVisualState {
        ListRowVisualState {
            selected,
            focused,
            enabled: true,
            ..Default::default()
        }
    }

    #[test]
    fn selection_keeps_the_rows_own_meaning() {
        let system = DesignSystem::phosphor();
        let danger = system.style(Role::Danger);
        let chrome = RowChrome::resolve(&system, state(true, true));
        let style = chrome.label_style(danger);
        assert_eq!(
            style.fg, danger.fg,
            "a selected error line is still an error line"
        );
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn unopinionated_rows_take_the_recipe_tone() {
        let system = DesignSystem::phosphor();
        let chrome = RowChrome::resolve(&system, state(true, true));
        let style = chrome.label_style(Style::new());
        assert_eq!(style.fg, system.style(Role::TextStrong).fg);
    }

    #[test]
    fn gutter_lands_in_the_reserved_slot_without_eating_text() {
        let system = DesignSystem::phosphor();
        let row = Rect::new(0, 0, 8, 1);
        let mut buffer = Buffer::empty(row);
        buffer.set_stringn(0, 0, " payload", 8, Style::new());

        let chrome = RowChrome::resolve(&system, state(true, true));
        chrome.paint(&mut buffer, row);

        assert_eq!(buffer[(0, 0)].symbol(), system.glyphs.selection_gutter());
        assert_eq!(buffer[(1, 0)].symbol(), "p");
    }

    #[test]
    fn tint_chrome_washes_the_ground_and_keeps_symbols() {
        let system = DesignSystem::phosphor().selection(crate::style::SelectionChrome::Tint);
        let row = Rect::new(0, 0, 6, 1);
        let mut buffer = Buffer::empty(row);
        buffer.set_stringn(0, 0, " abcde", 6, Style::new());

        let chrome = RowChrome::resolve(&system, state(true, true));
        chrome.paint(&mut buffer, row);

        let tint = system.style(Role::SelectionTint).bg;
        assert_eq!(buffer[(3, 0)].bg, tint.expect("tint carries a background"));
        assert_eq!(buffer[(3, 0)].symbol(), "c");
    }

    #[test]
    fn unselected_rows_are_left_alone() {
        let system = DesignSystem::phosphor();
        let row = Rect::new(0, 0, 5, 1);
        let mut buffer = Buffer::empty(row);
        buffer.set_stringn(0, 0, "abcde", 5, Style::new());
        let before = buffer.clone();

        RowChrome::resolve(&system, state(false, true)).paint(&mut buffer, row);
        assert_eq!(buffer, before);
    }
}
