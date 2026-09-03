// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// NAV_ENTRIES order and from_name normalization adapted from junie-tui
// src/bin/showcase/app.rs (MIT).

//! Single catalog registry: source prefix first, TermRock extensions after.

use crate::profile::ProductIdentity;

/// Which catalog data set the shell mounts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogProfile {
    /// TermRock-branded catalog including extensions and Applications.
    TermRock,
    /// Literal source identity and source-only navigation for capture.
    JunieReference,
}

impl CatalogProfile {
    /// Product strings for this profile.
    #[must_use]
    pub const fn identity(self) -> ProductIdentity {
        match self {
            Self::TermRock => ProductIdentity::termrock(),
            Self::JunieReference => ProductIdentity::junie(),
        }
    }
}

/// Stable page identity. Values 0..19 are the source prefix; later values are
/// TermRock extensions. Never reorder the source prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PageId(pub u16);

impl PageId {
    pub const OVERVIEW: Self = Self(0);
    pub const BUTTONS: Self = Self(1);
    pub const INPUTS: Self = Self(2);
    pub const TEXT_AREAS: Self = Self(3);
    pub const FORMS: Self = Self(4);
    pub const LISTS: Self = Self(5);
    pub const TREES: Self = Self(6);
    pub const TABLES: Self = Self(7);
    pub const EDITABLE: Self = Self(8);
    pub const PANELS: Self = Self(9);
    pub const SIDEBARS: Self = Self(10);
    pub const DIALOGS: Self = Self(11);
    pub const PROGRESS: Self = Self(12);
    pub const SCROLLING: Self = Self(13);
    pub const EDITOR: Self = Self(14);
    pub const GRID: Self = Self(15);
    pub const CHIPS: Self = Self(16);
    pub const PICKERS: Self = Self(17);
    pub const SETTINGS: Self = Self(18);
    pub const TASK_RUNNER: Self = Self(19);
    /// First TermRock-only slot (Applications / extra components).
    pub const TABLEPRO: Self = Self(20);
    /// TermRock-only: alerts, toasts, badges, loading.
    pub const FEEDBACK: Self = Self(21);
    /// TermRock-only: drawer, popover, menus.
    pub const OVERLAYS: Self = Self(22);
    /// TermRock-only: charts and meters.
    pub const CHARTS: Self = Self(23);
    /// TermRock-only: layout, inspectors, remaining widgets.
    pub const STRUCTURE: Self = Self(24);
}

/// One sidebar row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NavEntry {
    pub id: PageId,
    pub label: &'static str,
    pub section: &'static str,
}

/// Canonical navigation storage. Keep the source-defined prefix first and do
/// not reorder it; TermRock extensions follow it.
pub static CATALOG_NAV: [NavEntry; 25] = [
    NavEntry {
        id: PageId::OVERVIEW,
        label: "Overview",
        section: "Foundations",
    },
    NavEntry {
        id: PageId::BUTTONS,
        label: "Buttons",
        section: "Components",
    },
    NavEntry {
        id: PageId::INPUTS,
        label: "Inputs",
        section: "Components",
    },
    NavEntry {
        id: PageId::TEXT_AREAS,
        label: "Text areas",
        section: "Components",
    },
    NavEntry {
        id: PageId::FORMS,
        label: "Forms",
        section: "Components",
    },
    NavEntry {
        id: PageId::LISTS,
        label: "Lists",
        section: "Components",
    },
    NavEntry {
        id: PageId::TREES,
        label: "Trees",
        section: "Components",
    },
    NavEntry {
        id: PageId::TABLES,
        label: "Tables",
        section: "Components",
    },
    NavEntry {
        id: PageId::EDITABLE,
        label: "Editable tables",
        section: "Components",
    },
    NavEntry {
        id: PageId::PANELS,
        label: "Panels",
        section: "Components",
    },
    NavEntry {
        id: PageId::SIDEBARS,
        label: "Sidebars",
        section: "Components",
    },
    NavEntry {
        id: PageId::DIALOGS,
        label: "Dialogs",
        section: "Components",
    },
    NavEntry {
        id: PageId::PROGRESS,
        label: "Progress",
        section: "Components",
    },
    NavEntry {
        id: PageId::SCROLLING,
        label: "Scrolling",
        section: "Components",
    },
    NavEntry {
        id: PageId::EDITOR,
        label: "Code editor",
        section: "Components",
    },
    NavEntry {
        id: PageId::GRID,
        label: "Data grid",
        section: "Components",
    },
    NavEntry {
        id: PageId::CHIPS,
        label: "Chips & selects",
        section: "Components",
    },
    NavEntry {
        id: PageId::PICKERS,
        label: "Pickers",
        section: "Components",
    },
    NavEntry {
        id: PageId::SETTINGS,
        label: "Settings",
        section: "Screens",
    },
    NavEntry {
        id: PageId::TASK_RUNNER,
        label: "Task runner",
        section: "Screens",
    },
    NavEntry {
        id: PageId::FEEDBACK,
        label: "Feedback",
        section: "Library",
    },
    NavEntry {
        id: PageId::OVERLAYS,
        label: "Overlays",
        section: "Library",
    },
    NavEntry {
        id: PageId::CHARTS,
        label: "Charts",
        section: "Library",
    },
    NavEntry {
        id: PageId::STRUCTURE,
        label: "Structure",
        section: "Library",
    },
    NavEntry {
        id: PageId::TABLEPRO,
        label: "TablePro",
        section: "Applications",
    },
];

/// Source-defined navigation prefix. Do not reorder.
const CATALOG_NAV_PARTS: (&[NavEntry], &[NavEntry]) = CATALOG_NAV.split_at(20);

pub const SOURCE_NAV: &[NavEntry] = CATALOG_NAV_PARTS.0;

/// TermRock extensions after the frozen source prefix.
pub const TERMROCK_NAV: &[NavEntry] = CATALOG_NAV_PARTS.1;

/// Navigation for a profile. Junie-reference hides TermRock-only entries.
#[must_use]
pub fn nav_entries(profile: CatalogProfile) -> &'static [NavEntry] {
    match profile {
        CatalogProfile::JunieReference => SOURCE_NAV,
        CatalogProfile::TermRock => TERMROCK_NAV_FULL,
    }
}

/// Source prefix plus TermRock extensions (contiguous static storage).
pub static TERMROCK_NAV_FULL: &[NavEntry] = &CATALOG_NAV;

impl PageId {
    /// Index in the active navigation, or 0.
    #[must_use]
    pub fn index(self, nav: &[NavEntry]) -> usize {
        nav.iter().position(|e| e.id == self).unwrap_or(0)
    }

    /// Source `from_name`: letters and digits only, case-insensitive.
    /// `"chips & selects"`, `"chips-selects"`, `"chipsselects"` all resolve.
    #[must_use]
    pub fn from_name(name: &str, nav: &[NavEntry]) -> Option<Self> {
        let n = normalize(name);
        nav.iter().find(|e| normalize(e.label) == n).map(|e| e.id)
    }
}

/// Letters and digits, lowercased.
#[must_use]
pub fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_prefix_order() {
        let labels: Vec<_> = SOURCE_NAV.iter().map(|e| e.label).collect();
        assert_eq!(
            labels,
            [
                "Overview",
                "Buttons",
                "Inputs",
                "Text areas",
                "Forms",
                "Lists",
                "Trees",
                "Tables",
                "Editable tables",
                "Panels",
                "Sidebars",
                "Dialogs",
                "Progress",
                "Scrolling",
                "Code editor",
                "Data grid",
                "Chips & selects",
                "Pickers",
                "Settings",
                "Task runner",
            ]
        );
        assert_eq!(SOURCE_NAV[0].section, "Foundations");
        assert_eq!(SOURCE_NAV[1].section, "Components");
        assert_eq!(SOURCE_NAV[18].section, "Screens");
        assert_eq!(SOURCE_NAV[19].section, "Screens");
    }

    #[test]
    fn aliases_resolve() {
        let nav = SOURCE_NAV;
        assert_eq!(
            PageId::from_name("chips & selects", nav),
            Some(PageId::CHIPS)
        );
        assert_eq!(PageId::from_name("chips-selects", nav), Some(PageId::CHIPS));
        assert_eq!(PageId::from_name("chipsselects", nav), Some(PageId::CHIPS));
        assert_eq!(PageId::from_name("datagrid", nav), Some(PageId::GRID));
        assert_eq!(PageId::from_name("Data grid", nav), Some(PageId::GRID));
        assert_eq!(PageId::from_name("codeeditor", nav), Some(PageId::EDITOR));
        assert_eq!(PageId::from_name("nope", nav), None);
    }

    #[test]
    fn junie_reference_hides_extensions() {
        assert_eq!(nav_entries(CatalogProfile::JunieReference).len(), 20);
        assert!(
            nav_entries(CatalogProfile::TermRock)
                .iter()
                .any(|e| e.id == PageId::TABLEPRO)
        );
        assert!(
            !nav_entries(CatalogProfile::JunieReference)
                .iter()
                .any(|e| e.id == PageId::TABLEPRO)
        );
    }
}
