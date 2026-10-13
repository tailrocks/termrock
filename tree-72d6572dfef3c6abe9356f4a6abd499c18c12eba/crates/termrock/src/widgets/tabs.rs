use ratatui_core::{
    buffer::Buffer, layout::Rect, style::Modifier, text::Span, widgets::StatefulWidget,
};

use crate::{
    interaction::HitRegion,
    style::{Role, Theme},
};
use unicode_width::UnicodeWidthStr;

/// Per-tab descriptor shared by terminal tab renderers.
#[derive(Debug, Clone)]
pub struct TabCell<'a> {
    /// Documentation for `item`.
    pub label: &'a str,
    /// Documentation for `item`.
    pub active: bool,
    /// Documentation for `item`.
    pub start_col: u16,
    /// Documentation for `item`.
    pub cell_cols: u16,
}

/// Single space between adjacent tab cells.
pub const TAB_GAP: u16 = 1;

/// Build tab-cell geometry from `(label, active)` pairs.
#[must_use]
pub fn lay_out_tabs<'a>(labels: &[(&'a str, bool)], start_col: u16) -> Vec<TabCell<'a>> {
    let mut col = start_col;
    let mut out = Vec::with_capacity(labels.len());
    for &(label, active) in labels {
        let label_cols = u16::try_from(UnicodeWidthStr::width(label)).unwrap_or(u16::MAX);
        let cell_cols = label_cols.saturating_add(2);
        out.push(TabCell {
            label,
            active,
            start_col: col,
            cell_cols,
        });
        col = col.saturating_add(cell_cols).saturating_add(TAB_GAP);
    }
    out
}

/// Index of the tab cell whose column range contains `col`.
#[must_use]
pub fn tab_at_column(cells: &[TabCell<'_>], col: u16) -> Option<usize> {
    cells.iter().position(|cell| {
        col >= cell.start_col && col < cell.start_col.saturating_add(cell.cell_cols)
    })
}

#[derive(Debug, Clone)]
/// Data carried by `Tab`.
pub struct Tab<'a, Id> {
    /// Documentation for `item`.
    pub id: Id,
    /// Documentation for `item`.
    pub label: &'a str,
    /// Documentation for `item`.
    pub glyph: Option<Span<'a>>,
    /// Documentation for `item`.
    pub active: bool,
    /// Documentation for `item`.
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Runtime state for `Tabs`.
pub struct TabsState<Id> {
    /// Documentation for `item`.
    pub selected: Option<Id>,
    /// Documentation for `item`.
    pub hovered: Option<Id>,
    /// Documentation for `item`.
    pub focused: bool,
    /// Documentation for `item`.
    pub regions: Vec<HitRegion<Id>>,
}

impl<Id> Default for TabsState<Id> {
    fn default() -> Self {
        Self {
            selected: None,
            hovered: None,
            focused: false,
            regions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
/// Data carried by `Tabs`.
pub struct Tabs<'a, Id> {
    tabs: &'a [Tab<'a, Id>],
    gap: u16,
    theme: &'a Theme,
}

impl<'a, Id> Tabs<'a, Id> {
    #[must_use]
    /// Creates a new value with canonical defaults.
    pub const fn new(tabs: &'a [Tab<'a, Id>], theme: &'a Theme) -> Self {
        Self {
            tabs,
            gap: TAB_GAP,
            theme,
        }
    }

    #[must_use]
    /// Performs the `gap` operation.
    pub const fn gap(mut self, gap: u16) -> Self {
        self.gap = gap;
        self
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &Tabs<'_, Id> {
    type State = TabsState<Id>;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        state.regions.clear();
        let mut x = area.x;
        for tab in self.tabs {
            let label = match &tab.glyph {
                Some(glyph) => format!("{} {}", glyph.content, tab.label),
                None => tab.label.to_owned(),
            };
            let width = UnicodeWidthStr::width(label.as_str())
                .saturating_add(2)
                .min(u16::MAX as usize) as u16;
            let label_rect = Rect::new(
                x,
                area.y,
                width.min(area.right().saturating_sub(x)),
                area.height.min(1),
            );
            let selected = state.selected.as_ref() == Some(&tab.id) || tab.active;
            let hovered = state.hovered.as_ref() == Some(&tab.id);
            let role = match (selected, hovered) {
                (true, true) => Role::TabActiveHovered,
                (true, false) => Role::TabActive,
                (false, true) => Role::TabInactiveHovered,
                (false, false) => Role::TabInactive,
            };
            let mut style = self.theme.style(role);
            if selected {
                style = style.add_modifier(Modifier::BOLD);
            }
            if hovered {
                style = style.add_modifier(Modifier::UNDERLINED);
            }
            buffer.set_stringn(
                label_rect.x,
                label_rect.y,
                format!(" {label} "),
                label_rect.width as usize,
                style,
            );
            if label_rect.height > 0
                && label_rect.width > 1
                && let Some(glyph) = &tab.glyph
            {
                buffer.set_span(
                    label_rect.x.saturating_add(1),
                    label_rect.y,
                    glyph,
                    label_rect.width.saturating_sub(1),
                );
            }
            if selected && area.height > 1 {
                let underline_style = if state.focused {
                    self.theme.style(Role::TabUnderlineFocused)
                } else {
                    self.theme.style(Role::TabUnderlineUnfocused)
                };
                buffer.set_stringn(
                    label_rect.x,
                    area.y.saturating_add(1),
                    "━".repeat(usize::from(label_rect.width)),
                    label_rect.width as usize,
                    underline_style,
                );
            }
            if tab.enabled {
                state.regions.push(HitRegion {
                    id: tab.id.clone(),
                    area: Rect::new(
                        label_rect.x,
                        label_rect.y,
                        label_rect.width,
                        area.height.min(2),
                    ),
                });
            }
            x = x.saturating_add(width).saturating_add(self.gap);
            if x >= area.right() {
                break;
            }
        }
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for Tabs<'_, Id> {
    type State = TabsState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        StatefulWidget::render(&self, area, buffer, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_core::layout::Position;
    use ratatui_core::style::{Color, Style};

    #[test]
    fn selection_cue_and_hit_regions_share_two_row_geometry() {
        let tabs = [
            Tab {
                id: "overview",
                label: "Overview",
                glyph: None,
                active: true,
                enabled: true,
            },
            Tab {
                id: "disabled",
                label: "Disabled",
                glyph: None,
                active: false,
                enabled: false,
            },
        ];
        let area = Rect::new(3, 4, 30, 2);
        let mut buffer = Buffer::empty(area);
        let mut state = TabsState {
            selected: Some("overview"),
            hovered: Some("overview"),
            focused: true,
            ..TabsState::default()
        };
        let theme = Theme::default();
        (&Tabs::new(&tabs, &theme).gap(1)).render(area, &mut buffer, &mut state);

        assert_eq!(buffer[(3, 5)].symbol(), "━");
        assert_eq!(
            buffer[(3, 5)].fg,
            theme
                .style(Role::TabUnderlineFocused)
                .fg
                .expect("focused underline role has a foreground")
        );
        assert!(buffer[(3, 4)].modifier.contains(Modifier::UNDERLINED));
        assert_eq!(state.regions.len(), 1);
        assert!(state.regions[0].area.contains(Position::new(3, 5)));
    }

    #[test]
    fn glyph_span_style_overrides_the_tab_foreground_without_losing_its_fill() {
        let tabs = [Tab {
            id: "running",
            label: "Build",
            glyph: Some(Span::styled("●", Style::new().fg(Color::Yellow))),
            active: true,
            enabled: true,
        }];
        let area = Rect::new(0, 0, 20, 2);
        let mut buffer = Buffer::empty(area);
        let mut state = TabsState::default();
        let theme = Theme::default();

        (&Tabs::new(&tabs, &theme).gap(1)).render(area, &mut buffer, &mut state);

        assert_eq!(buffer[(1, 0)].symbol(), "●");
        assert_eq!(buffer[(1, 0)].fg, Color::Yellow);
        assert_eq!(
            buffer[(1, 0)].bg,
            theme
                .style(Role::TabActive)
                .bg
                .expect("active tab role has a background")
        );
    }

    #[test]
    fn tab_geometry_uses_display_columns_and_excludes_gaps() {
        let cells = lay_out_tabs(&[("界", true), ("b", false)], 5);
        assert_eq!(cells[0].cell_cols, 4);
        assert_eq!(cells[1].start_col, 10);
        assert_eq!(tab_at_column(&cells, 5), Some(0));
        assert_eq!(tab_at_column(&cells, 8), Some(0));
        assert_eq!(tab_at_column(&cells, 9), None);
        assert_eq!(tab_at_column(&cells, 10), Some(1));
        assert_eq!(tab_at_column(&cells, 13), None);
    }
}
