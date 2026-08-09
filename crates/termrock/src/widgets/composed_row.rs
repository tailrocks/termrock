// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Named-part row projection for priority-aware contraction.

use ratatui_core::{buffer::Buffer, layout::Rect, style::Style, text::Line};

use crate::text::display_cols;

/// Borrowed composed row anatomy (list/menu/tree/task-rail).
///
/// Drop priority under narrow pressure (lowest survival first):
/// shortcut → badge → secondary → leading → primary (last).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposedRow<'a, Id> {
    /// Stable identity.
    pub id: Id,
    /// Leading icon/check/status.
    pub leading: Option<Line<'a>>,
    /// Primary label (never dropped first under narrow pressure).
    pub primary: Line<'a>,
    /// Secondary metadata.
    pub secondary: Option<Line<'a>>,
    /// Trailing badge/value.
    pub badge: Option<Line<'a>>,
    /// Shortcut hint.
    pub shortcut: Option<&'a str>,
    /// Enabled for interaction.
    pub enabled: bool,
    /// Loading placeholder (leading becomes a busy glyph when set).
    pub loading: bool,
}

impl<'a, Id> ComposedRow<'a, Id> {
    /// Creates a primary-only row.
    #[must_use]
    pub fn primary(id: Id, primary: Line<'a>) -> Self {
        Self {
            id,
            leading: None,
            primary,
            secondary: None,
            badge: None,
            shortcut: None,
            enabled: true,
            loading: false,
        }
    }

    /// Drop priority for narrow terminals: shortcut → badge → secondary → leading → primary.
    ///
    /// Uses measured cell budgets so optional chrome is kept whenever it still
    /// fits next to a non-empty primary identity slot (min 1 cell).
    #[must_use]
    pub fn parts_for_width(&self, width: u16) -> ComposedRowParts<'a> {
        let mut parts = ComposedRowParts {
            leading: if self.loading {
                Some(Line::from("…"))
            } else {
                self.leading.clone()
            },
            primary: self.primary.clone(),
            secondary: self.secondary.clone(),
            badge: self.badge.clone(),
            shortcut: self.shortcut,
        };
        // Drop order: shortcut → badge → secondary → leading → primary (last).
        const PRIMARY_MIN: u16 = 1;
        let order: [fn(&mut ComposedRowParts<'_>); 4] = [
            |p| p.shortcut = None,
            |p| p.badge = None,
            |p| p.secondary = None,
            |p| p.leading = None,
        ];
        for drop_part in order {
            if parts.occupied_width(PRIMARY_MIN) <= width {
                break;
            }
            drop_part(&mut parts);
        }
        parts
    }
}

/// Resolved visible parts after contraction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposedRowParts<'a> {
    /// Leading.
    pub leading: Option<Line<'a>>,
    /// Primary.
    pub primary: Line<'a>,
    /// Secondary.
    pub secondary: Option<Line<'a>>,
    /// Badge.
    pub badge: Option<Line<'a>>,
    /// Shortcut.
    pub shortcut: Option<&'a str>,
}

impl ComposedRowParts<'_> {
    fn part_width(line: &Line<'_>) -> u16 {
        u16::try_from(line.width()).unwrap_or(u16::MAX)
    }

    /// Cells required for optional chrome + a primary identity minimum.
    #[must_use]
    pub fn occupied_width(&self, primary_min: u16) -> u16 {
        let mut w = primary_min;
        let mut gaps = 0u16;
        if let Some(leading) = self.leading.as_ref() {
            w = w.saturating_add(Self::part_width(leading));
            gaps = gaps.saturating_add(1);
        }
        if let Some(secondary) = self.secondary.as_ref() {
            w = w.saturating_add(Self::part_width(secondary));
            gaps = gaps.saturating_add(1);
        }
        if let Some(badge) = self.badge.as_ref() {
            w = w.saturating_add(Self::part_width(badge));
            gaps = gaps.saturating_add(1);
        }
        if let Some(shortcut) = self.shortcut {
            w = w.saturating_add(u16::try_from(display_cols(shortcut)).unwrap_or(u16::MAX));
            gaps = gaps.saturating_add(1);
        }
        w.saturating_add(gaps)
    }

    /// Right-side reserve in cells (badge + shortcut + gaps).
    #[must_use]
    pub fn trailing_reserve(&self) -> u16 {
        let badge = self.badge.as_ref().map(Self::part_width).unwrap_or(0);
        let shortcut = self
            .shortcut
            .map(|s| u16::try_from(display_cols(s)).unwrap_or(u16::MAX))
            .unwrap_or(0);
        let gaps = u16::from(badge > 0) + u16::from(shortcut > 0 && badge > 0);
        badge.saturating_add(shortcut).saturating_add(gaps)
    }

    /// Paints surviving parts into a single-row content band.
    ///
    /// Layout: `[leading][ ][primary…][ ][secondary] … [badge][ ][shortcut]`
    /// Primary is grapheme-clipped to the remaining middle budget.
    pub fn paint(&self, buffer: &mut Buffer, area: Rect, style: Style) {
        if area.is_empty() || area.height == 0 {
            return;
        }
        let y = area.y;
        let right = area.right();
        let mut x = area.x;

        if let Some(leading) = self.leading.as_ref() {
            let w = u16::try_from(leading.width())
                .unwrap_or(u16::MAX)
                .min(right.saturating_sub(x));
            if w > 0 {
                buffer.set_line(x, y, leading, w);
                buffer.set_style(Rect::new(x, y, w, 1), style);
                x = x.saturating_add(w).saturating_add(1);
            }
        }

        let reserve = self.trailing_reserve().min(right.saturating_sub(x));
        let mid_end = right.saturating_sub(reserve);

        // Secondary sits immediately after primary when both fit.
        let secondary_w = self
            .secondary
            .as_ref()
            .map(|s| {
                u16::try_from(s.width())
                    .unwrap_or(u16::MAX)
                    .saturating_add(1) // leading gap
            })
            .unwrap_or(0);
        let primary_budget = mid_end.saturating_sub(x).saturating_sub(
            if secondary_w + 2 < mid_end.saturating_sub(x) {
                secondary_w
            } else {
                0
            },
        );

        if primary_budget > 0 && x < mid_end {
            buffer.set_line(x, y, &self.primary, primary_budget);
            buffer.set_style(
                Rect::new(x, y, primary_budget.min(mid_end.saturating_sub(x)), 1),
                style,
            );
            x = x.saturating_add(
                u16::try_from(self.primary.width())
                    .unwrap_or(u16::MAX)
                    .min(primary_budget),
            );
        }

        if let Some(secondary) = self.secondary.as_ref() {
            let avail = mid_end.saturating_sub(x);
            if avail > 2 {
                // gap
                x = x.saturating_add(1);
                let w = u16::try_from(secondary.width())
                    .unwrap_or(u16::MAX)
                    .min(mid_end.saturating_sub(x));
                if w > 0 {
                    buffer.set_line(x, y, secondary, w);
                    buffer.set_style(Rect::new(x, y, w, 1), style);
                }
            }
        }

        // Right-aligned badge then shortcut.
        let mut cursor = right;
        if let Some(shortcut) = self.shortcut {
            let w = u16::try_from(display_cols(shortcut))
                .unwrap_or(u16::MAX)
                .min(cursor.saturating_sub(area.x));
            if w > 0 {
                cursor = cursor.saturating_sub(w);
                buffer.set_stringn(cursor, y, shortcut, usize::from(w), style);
            }
        }
        if let Some(badge) = self.badge.as_ref() {
            let w = u16::try_from(badge.width())
                .unwrap_or(u16::MAX)
                .min(cursor.saturating_sub(area.x));
            if w > 0 {
                if self.shortcut.is_some() {
                    cursor = cursor.saturating_sub(1);
                }
                cursor = cursor.saturating_sub(w);
                buffer.set_line(cursor, y, badge, w);
                buffer.set_style(Rect::new(cursor, y, w, 1), style);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_core::style::Style;

    #[test]
    fn narrow_drops_shortcut_before_primary() {
        let row = ComposedRow {
            id: "a",
            leading: Some(Line::from("*")),
            primary: Line::from("Primary"),
            secondary: Some(Line::from("meta")),
            badge: Some(Line::from("3")),
            shortcut: Some("⌘K"),
            enabled: true,
            loading: false,
        };
        // Full anatomy needs more than a few cells.
        let full = row.parts_for_width(80);
        assert!(full.shortcut.is_some());
        assert!(full.badge.is_some());
        // Progressive drops keep primary.
        let tight = row.parts_for_width(8);
        assert!(tight.shortcut.is_none());
        assert_eq!(tight.primary, Line::from("Primary"));
        let tiny = row.parts_for_width(2);
        assert!(tiny.leading.is_none());
        assert!(tiny.badge.is_none());
        assert!(tiny.secondary.is_none());
        assert_eq!(tiny.primary, Line::from("Primary"));
    }

    #[test]
    fn loading_forces_ellipsis_leading() {
        let row = ComposedRow {
            id: 1,
            leading: Some(Line::from("!")),
            primary: Line::from("Job"),
            secondary: None,
            badge: None,
            shortcut: None,
            enabled: true,
            loading: true,
        };
        let parts = row.parts_for_width(40);
        assert_eq!(parts.leading, Some(Line::from("…")));
    }

    #[test]
    fn paint_keeps_primary_on_narrow_band() {
        let row = ComposedRow {
            id: "r",
            leading: Some(Line::from("*")),
            primary: Line::from("Identity"),
            secondary: Some(Line::from("meta")),
            badge: Some(Line::from("99")),
            shortcut: Some("⌘K"),
            enabled: true,
            loading: false,
        };
        let parts = row.parts_for_width(12);
        let area = Rect::new(0, 0, 12, 1);
        let mut buffer = Buffer::empty(area);
        parts.paint(&mut buffer, area, Style::default());
        let text: String = (0..12)
            .map(|x| buffer[(x, 0)].symbol().to_string())
            .collect();
        assert!(
            text.contains("Ident") || text.contains("Iden"),
            "primary identity must remain: {text:?}"
        );
        // shortcut dropped at width 12
        assert!(!text.contains('⌘'), "shortcut must drop: {text:?}");
    }
}
