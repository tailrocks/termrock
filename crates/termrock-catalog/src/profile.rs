// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Product identity for default TermRock vs literal junie-reference capture.

/// Strings painted in the header and too-small state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductIdentity {
    /// Mark glyph before the product name (`▪`).
    pub mark: &'static str,
    /// Product name (`TermRock` or `Junie`).
    pub name: &'static str,
    /// Subtitle (`Design system`).
    pub product: &'static str,
}

impl ProductIdentity {
    /// Default TermRock-branded catalog.
    #[must_use]
    pub const fn termrock() -> Self {
        Self {
            mark: "▪",
            name: "TermRock",
            product: "Design system",
        }
    }

    /// Literal source identity for `junie-reference` capture.
    #[must_use]
    pub const fn junie() -> Self {
        Self {
            mark: "▪",
            name: "Junie",
            product: "Design system",
        }
    }

    /// Too-small first line (`{name} {product}`).
    #[must_use]
    pub fn too_small_title(self) -> String {
        format!("{} {}", self.name, self.product)
    }
}
