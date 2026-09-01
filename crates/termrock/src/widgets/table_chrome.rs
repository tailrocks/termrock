// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Shared header language for the table family.
//!
//! `Table`, `DataTable` and `TreeTable` used to disagree about their own
//! chrome: two banded the header row and one did not, one brightened its
//! header when the table owned focus, and each spelled the sort marker its own
//! way. A header is a label for a column, not a state — it reads the same in
//! all three, and focus is expressed by the container's border.

use ratatui_core::style::Style;

use crate::style::{DesignSystem, Glyph, Role};

/// Style for a column header label.
///
/// Never focus-brightened: a header that changes with focus reads as a state
/// the user can act on.
pub(crate) fn header_style(system: &DesignSystem) -> Style {
    system.style(Role::TextMuted)
}

/// Transparent header ground; labels carry hierarchy without a slab.
pub(crate) const fn header_band(_system: &DesignSystem) -> Style {
    Style::new()
}

/// Stable one-cell separation between adjacent columns.
pub(crate) const fn column_gap() -> &'static str {
    " "
}

/// Sort direction marker, in the operator's glyph profile.
pub(crate) fn sort_marker(system: &DesignSystem, ascending: bool) -> &'static str {
    let glyph = if ascending {
        Glyph::ArrowUp
    } else {
        Glyph::ArrowDown
    };
    system.glyphs.resolve(glyph).text
}

/// Marker a sortable-but-unsorted column wears.
///
/// A column that can be sorted and never says so is a hidden affordance: the
/// operator has to click and see what happens. The neutral marker states the
/// capability faintly; the direction arrow replaces it once sorted
/// (plans/021 Step 3).
pub(crate) fn sortable_marker(system: &DesignSystem) -> &'static str {
    if system.glyphs.is_ascii() {
        "^v"
    } else {
        "⇅"
    }
}

/// Tone for the neutral sortable marker.
pub(crate) fn sortable_marker_style(system: &DesignSystem) -> Style {
    system.style(Role::TextFaint)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::GlyphSet;

    #[test]
    fn headers_do_not_brighten_with_focus() {
        let system = DesignSystem::phosphor();
        assert_eq!(header_style(&system), system.style(Role::TextMuted));
        assert_ne!(header_style(&system), system.style(Role::TextStrong));
        assert!(header_band(&system).bg.is_none());
        assert_eq!(column_gap(), " ");
    }

    #[test]
    fn sort_markers_follow_the_glyph_profile() {
        let unicode = DesignSystem::phosphor().glyphs(GlyphSet::Unicode);
        let ascii = DesignSystem::phosphor().glyphs(GlyphSet::Ascii);
        assert_eq!(sort_marker(&unicode, true), "↑");
        assert_eq!(sort_marker(&unicode, false), "↓");
        assert_eq!(sort_marker(&ascii, true), "^");
        assert_eq!(sort_marker(&ascii, false), "v");
    }
}
