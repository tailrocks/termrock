// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Named-part row projection for priority-aware contraction.

use ratatui_core::text::Line;

/// Borrowed composed row anatomy (list/menu/task-rail).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposedRow<'a, Id> {
    /// Stable identity.
    pub id: Id,
    /// Leading icon/check.
    pub leading: Option<Line<'a>>,
    /// Primary label (never drop first under narrow pressure).
    pub primary: Line<'a>,
    /// Secondary metadata.
    pub secondary: Option<Line<'a>>,
    /// Trailing badge.
    pub badge: Option<Line<'a>>,
    /// Shortcut hint.
    pub shortcut: Option<&'a str>,
    /// Enabled for interaction.
    pub enabled: bool,
    /// Loading placeholder.
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
    #[must_use]
    pub fn parts_for_width(&self, width: u16) -> ComposedRowParts<'a> {
        // Heuristic budgets (cells).
        let mut parts = ComposedRowParts {
            leading: self.leading.clone(),
            primary: self.primary.clone(),
            secondary: self.secondary.clone(),
            badge: self.badge.clone(),
            shortcut: self.shortcut,
        };
        if width < 40 {
            parts.shortcut = None;
        }
        if width < 32 {
            parts.badge = None;
        }
        if width < 24 {
            parts.secondary = None;
        }
        if width < 12 {
            parts.leading = None;
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

#[cfg(test)]
mod tests {
    use super::*;

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
        let parts = row.parts_for_width(20);
        assert!(parts.shortcut.is_none());
        assert!(parts.badge.is_none());
        assert!(parts.secondary.is_none());
        assert_eq!(parts.primary, Line::from("Primary"));
    }
}
