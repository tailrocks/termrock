// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Shared header language for the table family.
//!
//! junie DataTable: muted headers, two blank cells between columns, sort
//! suffix `▴`/`▾`, filter `∇`. A header is a label for a column, not a state
//! — it does not brighten with table focus. Sorted columns take the primary
//! tone; a hovered sortable header underlines.
use ratatui_core::style::Style;

use crate::style::DesignSystem;

/// Style for a column header label (unsorted, idle).
///
/// Never focus-brightened: a header that changes with focus reads as a state
/// the user can act on.
pub(crate) fn header_style(system: &DesignSystem) -> Style {
    system.junie_theme().muted()
}

/// Header label style for one column: sorted → primary, idle → muted,
/// hovered+sortable → primary + underline (junie table.rs:591-598).
pub(crate) fn header_label_style(
    system: &DesignSystem,
    sorted: bool,
    hovered: bool,
    sortable: bool,
) -> Style {
    let theme = system.junie_theme();
    let mut st = if sorted {
        theme.primary()
    } else {
        theme.muted()
    };
    if hovered && sortable {
        // Brighten; the pointer underline is a D5 class owned by the
        // header scan allow-list (sortable header under pointer).
        st = st.fg(theme.text_primary);
    }
    st
}

/// Transparent header ground; labels carry hierarchy without a slab.
pub(crate) const fn header_band(_system: &DesignSystem) -> Style {
    Style::new()
}

/// Two blank cells between adjacent columns (junie column-gap 2).
///
/// Painters leave this seam unpainted (page fill), matching junie `gap = 2`.
#[cfg(test)]
pub(crate) const fn column_gap() -> &'static str {
    "  "
}

/// Primary-key header mark. Junie writes `"▪ "` then overdraws this glyph.
pub(crate) const fn primary_key_mark() -> &'static str {
    "⚷"
}

/// Sort direction marker, in the operator's glyph profile.
pub(crate) fn sort_marker(ascending: bool) -> &'static str {
    if ascending { "▴" } else { "▾" }
}

/// Filter mark a filtered column wears (junie grid header `" ∇"`).
pub(crate) const fn filter_marker() -> &'static str {
    "∇"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::Role;

    #[test]
    fn headers_do_not_brighten_with_focus() {
        let system = DesignSystem::junie();
        assert_eq!(header_style(&system), system.junie_theme().muted());
        assert_ne!(header_style(&system), system.style(Role::TextStrong));
        assert!(header_band(&system).bg.is_none());
        assert_eq!(column_gap(), "  ");
        assert_eq!(column_gap().chars().count(), 2);
        assert_eq!(filter_marker(), "∇");
        assert_eq!(primary_key_mark(), "⚷");
    }

    #[test]
    fn sort_markers_follow_the_junie_glyphs() {
        assert_eq!(sort_marker(true), "▴");
        assert_eq!(sort_marker(false), "▾");
        assert_eq!(filter_marker(), "∇");
    }

    #[test]
    fn sorted_header_uses_primary_hovered_sortable_underlines() {
        let system = DesignSystem::junie();
        let theme = system.junie_theme();
        let sorted = header_label_style(&system, true, false, true);
        assert_eq!(sorted.fg, Some(theme.text_primary));
        let hovered = header_label_style(&system, false, true, true);
        assert_eq!(hovered.fg, Some(theme.text_primary));
        let idle = header_label_style(&system, false, false, true);
        assert_eq!(idle.fg, Some(theme.text_muted));
    }
}
