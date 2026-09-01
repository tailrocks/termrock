// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! One selection language for collections that paint whole rows.
//!
//! Widgets that render a row as a single string used to hand-roll
//! `match system.selection { … }` and repaint the row in `Role::Focus`, which
//! erased whatever the row was *saying*. [`RowChrome`] resolves the same
//! [`ListRowRecipe`] the list family uses. Selected row copy takes the
//! recipe's contrast-safe label tone; semantic glyphs and words retain the
//! meaning without making color the only signal.

use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
};

use crate::style::{DesignSystem, ListRowRecipe, ListRowVisualState, Role};

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
    selected_foreground: Option<Color>,
    gutter: Option<(&'static str, Style)>,
    colorless: bool,
}

impl RowChrome {
    /// Resolves the shared row recipe for this visual state.
    pub(crate) fn resolve(system: &DesignSystem, state: ListRowVisualState) -> Self {
        // Collection state has one grammar regardless of a theme's historical
        // `SelectionChrome` setting: selected rows reserve a stable gutter and
        // use the quiet tint when color is available. Loud fills and marker
        // substitutions made sibling collections drift apart.
        let gutter = state.selected.then(|| {
            let role = if state.focused {
                Role::Accent
            } else {
                Role::TextMuted
            };
            (system.glyphs.selection_gutter(), system.style(role))
        });
        Self {
            recipe: system.resolve_list_row(state),
            selected: state.selected,
            // RowChrome canonicalizes every historical selection setting to
            // gutter+tint, so its foreground must come from that same
            // canonical grammar rather than a Fill/Marker recipe.
            selected_foreground: system.style(Role::TextStrong).fg,
            gutter,
            colorless: system.mono(),
        }
    }

    /// Suppresses chromatic washes while retaining glyph and weight cues.
    pub(crate) const fn colorless(mut self, colorless: bool) -> Self {
        self.colorless |= colorless;
        self
    }

    /// Style for the row's primary text.
    ///
    /// A selected row owns its foreground and ground as one contrast pair.
    /// Preserving an arbitrary body foreground while adding the selection
    /// ground can make both resolve to the same terminal color. Semantic
    /// state belongs on a glyph or word painted after this base style.
    pub(crate) fn label_style(&self, base: Style) -> Style {
        let mut style = base;
        if self.selected {
            style.fg = self.selected_foreground;
            style = style.remove_modifier(Modifier::DIM);
        } else if style.fg.is_none() {
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

    /// Style for secondary/numeric metadata inside the same row ground.
    pub(crate) fn secondary_style(&self, base: Style) -> Style {
        let foreground = if self.selected {
            self.selected_foreground
        } else {
            self.recipe.secondary.fg
        };
        let mut style = foreground.map_or(base, |fg| base.fg(fg));
        if self.selected {
            // Selected metadata shares the contrast-safe foreground but stays
            // quieter through weight, which also survives monochrome.
            style = style.remove_modifier(Modifier::BOLD | Modifier::DIM);
        }
        style
    }

    /// Background the row sits on, if this chrome washes at all.
    pub(crate) fn wash(&self) -> Option<ratatui_core::style::Color> {
        if self.colorless {
            None
        } else if self.selected {
            self.recipe.tint.bg
        } else if self.recipe.hover_fill {
            self.recipe.hover_wash.bg
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
        self.paint_wash(buffer, row);
        self.paint_gutter(buffer, row);
    }

    /// Paints only the tint/hover ground, useful for wrapped continuations.
    pub(crate) fn paint_wash(&self, buffer: &mut Buffer, row: Rect) {
        if let Some(bg) = self.wash() {
            for y in row.top()..row.bottom() {
                for x in row.left()..row.right() {
                    let cell = &mut buffer[(x, y)];
                    let style = cell.style().bg(bg);
                    cell.set_style(style);
                }
            }
        }
    }

    /// Paints only the leading selection gutter.
    pub(crate) fn paint_gutter(&self, buffer: &mut Buffer, row: Rect) {
        if row.width == 0 || row.height == 0 {
            return;
        }
        if let Some((glyph, gutter_style)) = self.gutter {
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
    fn selected_body_tone_uses_the_recipe_contrast_pair() {
        let system = DesignSystem::phosphor();
        let chrome = RowChrome::resolve(&system, state(true, true));
        let style = chrome.label_style(system.style(Role::Text));
        assert_eq!(style.fg, system.style(Role::TextStrong).fg);
        assert_ne!(style.fg, chrome.wash());
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn selected_secondary_tone_cannot_collapse_into_the_wash() {
        let system = DesignSystem::phosphor();
        let chrome = RowChrome::resolve(&system, state(true, true));
        let style = chrome.secondary_style(system.style(Role::TextMuted));

        assert_eq!(style.fg, system.style(Role::TextStrong).fg);
        assert_ne!(style.fg, chrome.wash());
        assert!(!style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn unselected_semantic_tone_is_unchanged() {
        let system = DesignSystem::phosphor();
        let danger = system.style(Role::Danger);
        let chrome = RowChrome::resolve(&system, state(false, true));

        assert_eq!(chrome.label_style(danger).fg, danger.fg);
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
    fn configured_fill_cannot_replace_collection_gutter_and_tint() {
        let system = DesignSystem::phosphor().selection(crate::style::SelectionChrome::Fill);
        let row = Rect::new(0, 0, 6, 1);
        let mut buffer = Buffer::empty(row);
        buffer.set_stringn(0, 0, " abcde", 6, Style::new());

        let chrome = RowChrome::resolve(&system, state(true, true));
        chrome.paint(&mut buffer, row);

        assert_eq!(buffer[(0, 0)].symbol(), system.glyphs.selection_gutter());
        assert_eq!(
            Some(buffer[(3, 0)].bg),
            system.style(Role::SelectionTint).bg
        );
        assert_ne!(Some(buffer[(3, 0)].bg), system.style(Role::Selection).bg);
        assert_eq!(
            chrome.label_style(system.style(Role::Text)).fg,
            system.style(Role::TextStrong).fg
        );
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
