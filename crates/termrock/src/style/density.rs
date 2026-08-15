// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Layout density tokens for product-neutral chrome.
//!
//! Motion policy lives in [`super::motion`].

/// Spacing scale shared by chrome and composite layouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
#[non_exhaustive]
pub enum Density {
    /// Agent chat and form comfort (default).
    #[default]
    Comfortable,
    /// Ops tools and multi-panel workspaces.
    Compact,
    /// Maximum information density (dashboard class).
    Dashboard,
}

impl Density {
    /// Stable id, for inspectors and story metadata.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Comfortable => "comfortable",
            Self::Dashboard => "dashboard",
        }
    }

    /// Horizontal padding cells around content regions.
    #[must_use]
    pub const fn padding_x(self) -> u16 {
        match self {
            Self::Comfortable => 2,
            Self::Compact => 1,
            Self::Dashboard => 0,
        }
    }

    /// Vertical padding cells around content regions.
    #[must_use]
    pub const fn padding_y(self) -> u16 {
        match self {
            Self::Comfortable => 1,
            Self::Compact => 0,
            Self::Dashboard => 0,
        }
    }

    /// Gap between sibling chrome regions.
    #[must_use]
    pub const fn gap(self) -> u16 {
        match self {
            Self::Comfortable => 1,
            Self::Compact | Self::Dashboard => 0,
        }
    }

    /// Preferred hint-bar row budget.
    #[must_use]
    pub const fn hint_rows(self) -> u16 {
        match self {
            Self::Comfortable => 2,
            Self::Compact | Self::Dashboard => 1,
        }
    }

    /// Cells of indent per hierarchy depth for tree rows.
    #[must_use]
    pub const fn tree_indent(self) -> u16 {
        match self {
            Self::Comfortable => 2,
            Self::Compact | Self::Dashboard => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn density_scales_monotonically_toward_dashboard() {
        assert!(Density::Comfortable.padding_x() >= Density::Compact.padding_x());
        assert!(Density::Compact.padding_x() >= Density::Dashboard.padding_x());
        assert!(Density::Comfortable.gap() >= Density::Dashboard.gap());
    }
}
