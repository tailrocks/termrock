// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/core/id.rs (MIT).

//! Stable widget identifiers (FNV-1a path hashes). Catalog-local IDs for
//! [`termrock::interaction::InteractionScene`]; not a second focus system.

use std::fmt;

/// Stable identity for a catalog control.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WidgetId(u64);

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0100_0000_01b3;

const fn fnv1a(mut hash: u64, bytes: &[u8]) -> u64 {
    let mut i = 0;
    while i < bytes.len() {
        hash ^= bytes[i] as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
        i += 1;
    }
    hash
}

impl WidgetId {
    /// Build an id from a static path such as `"buttons.primary"`.
    #[must_use]
    pub const fn of(path: &str) -> Self {
        Self(fnv1a(FNV_OFFSET, path.as_bytes()))
    }

    /// Derive a child id, e.g. one per table row or list item.
    #[must_use]
    pub const fn child(self, index: usize) -> Self {
        Self(fnv1a(self.0, &index.to_le_bytes()))
    }

    /// Derive a named child id.
    #[must_use]
    pub const fn sub(self, name: &str) -> Self {
        Self(fnv1a(self.0, name.as_bytes()))
    }
}

impl fmt::Debug for WidgetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WidgetId({:016x})", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_stable_and_distinct() {
        assert_eq!(WidgetId::of("a.b"), WidgetId::of("a.b"));
        assert_ne!(WidgetId::of("a.b"), WidgetId::of("a.c"));
        assert_ne!(WidgetId::of("a").child(0), WidgetId::of("a").child(1));
        assert_ne!(WidgetId::of("a").child(0), WidgetId::of("a"));
        assert_ne!(WidgetId::of("a").sub("x"), WidgetId::of("a").sub("y"));
    }
}
