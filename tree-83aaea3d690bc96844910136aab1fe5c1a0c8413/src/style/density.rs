// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Layout density and motion tokens for product-neutral chrome.

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

/// Motion intensity for frame-driven animations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
#[non_exhaustive]
pub enum Motion {
    /// Full spinner and soft transitions.
    #[default]
    Full,
    /// Reduced animation (prefer static glyphs).
    Reduced,
    /// No animated frames.
    Off,
}

impl Motion {
    /// Whether indeterminate spinners should advance frames.
    #[must_use]
    pub const fn animate_spinners(self) -> bool {
        matches!(self, Self::Full)
    }

    /// Frame step divisor for reduced motion (slower advance).
    #[must_use]
    pub const fn spinner_divisor(self) -> u64 {
        match self {
            Self::Full => 1,
            Self::Reduced => 4,
            Self::Off => u64::MAX,
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

    #[test]
    fn motion_off_disables_spinner_advance() {
        assert!(!Motion::Off.animate_spinners());
        assert!(Motion::Full.animate_spinners());
        assert_eq!(Motion::Off.spinner_divisor(), u64::MAX);
    }
}
