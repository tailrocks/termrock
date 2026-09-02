// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/core/event.rs Outcome (MIT).

//! Application-level event routing result (not a widget outcome).

/// Result of offering an event to the catalog shell or a page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Route {
    /// Not interested; keep propagating.
    #[default]
    Ignored,
    /// Consumed, nothing visible changed.
    Consumed,
    /// Consumed and the UI must be redrawn.
    Changed,
}

impl Route {
    /// Whether the event was handled.
    #[must_use]
    pub fn consumed(self) -> bool {
        !matches!(self, Self::Ignored)
    }

    /// Combine with a later outcome (`Changed` dominates).
    #[must_use]
    pub fn or(self, other: Self) -> Self {
        match (self, other) {
            (Self::Changed, _) | (_, Self::Changed) => Self::Changed,
            (Self::Consumed, _) | (_, Self::Consumed) => Self::Consumed,
            _ => Self::Ignored,
        }
    }
}
