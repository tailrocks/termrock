// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Generic block chrome helper (paint tokens). Product recipes live in
//! [`crate::patterns`].
use crate::style::DesignSystem;

/// Marker type for block chrome that needs tokens (paint lives in consumer/story).
#[derive(Debug, Clone, Copy)]
pub struct BlockChrome<'a> {
    /// Design tokens.
    pub tokens: &'a DesignSystem,
}

impl<'a> BlockChrome<'a> {
    /// Tokens.
    #[must_use]
    pub const fn new(tokens: &'a DesignSystem) -> Self {
        Self { tokens }
    }
}
