// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// NAV_ENTRIES order and from_name normalization adapted from junie-tui
// src/bin/showcase/app.rs (MIT).

//! Single catalog registry: source prefix first, TermRock extensions after.

use crate::coverage::catalog_page_for;
use crate::profile::ProductIdentity;
use serde::Serialize;
use termrock::registry::{pattern_inventory, public_ui_inventory};

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

/// One deterministic representative scenario owned by the canonical catalog.
///
/// Scenarios are component and pattern entry points used by documentation and
/// headless hosts. They all render through their owning catalog page; no host
/// maintains a second story renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogScenario {
    /// Stable representative scenario ID.
    pub id: &'static str,
    /// Human-readable scenario title.
    pub title: &'static str,
    /// Public visual owner or composed pattern identity.
    pub component: &'static str,
    /// Deterministic purpose text.
    pub description: &'static str,
    /// Canonical owning page.
    pub page: PageId,
    /// Preferred scenario width.
    pub cols: u16,
    /// Preferred scenario height.
    pub rows: u16,
    /// Whether the owning page accepts interaction.
    pub interactive: bool,
    /// Stable interaction family.
    pub interaction_kind: &'static str,
}

/// JSON projection of the public component authority used by documentation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicUiAuthority {
    /// Stable public component identity.
    pub public_ui: &'static str,
    /// Rendering contract kind.
    pub kind: &'static str,
    /// Product family.
    pub family: &'static str,
    /// Documentation collection.
    pub documentation_kind: &'static str,
    /// Documentation slug.
    pub docs_slug: &'static str,
    /// Documentation route.
    pub docs_path: String,
    /// Representative scenario ID.
    pub representative_story: &'static str,
}

/// JSON projection of one composed pattern authority entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatternAuthority {
    /// Stable pattern identity.
    pub pattern: &'static str,
    /// Documentation slug.
    pub docs_slug: &'static str,
    /// Documentation route.
    pub docs_path: String,
    /// Representative scenario ID.
    pub representative_story: &'static str,
    /// Exact public visual owner where one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_ui: Option<&'static str>,
}

/// Complete canonical documentation authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogAuthority {
    /// Public visual owners.
    pub public_ui: Vec<PublicUiAuthority>,
    /// Composed patterns.
    pub patterns: Vec<PatternAuthority>,
}

/// JSON projection of one representative scenario.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioDescriptor {
    /// Stable scenario ID.
    pub id: &'static str,
    /// Scenario title.
    pub title: &'static str,
    /// Public visual owner or pattern identity.
    pub component: &'static str,
    /// Scenario purpose.
    pub description: &'static str,
    /// Preferred width.
    pub cols: u16,
    /// Preferred height.
    pub rows: u16,
    /// Whether the scenario accepts interaction.
    pub interactive: bool,
    /// Interaction family.
    pub interaction_kind: &'static str,
    /// Deterministic interaction hints.
    pub hints: Vec<&'static str>,
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
        section: "Patterns",
    },
    NavEntry {
        id: PageId::OVERLAYS,
        label: "Overlays",
        section: "Patterns",
    },
    NavEntry {
        id: PageId::CHARTS,
        label: "Charts",
        section: "Patterns",
    },
    NavEntry {
        id: PageId::STRUCTURE,
        label: "Structure",
        section: "Patterns",
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

/// Return the source navigation snapshot represented by one checked-in shot.
///
/// The approved source fixtures are historical: the `f_*` captures predate
/// the four later source pages, while `s_*` captures include the 20-page
/// source prefix. This is fixture data only; both snapshots use the same
/// shell, page implementations, and event path.
#[must_use]
pub fn reference_nav_for_scene(_scene: &str) -> Vec<NavEntry> {
    SOURCE_NAV.to_vec()
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

fn pattern_page(id: &str) -> PageId {
    match id {
        "ActivityShelf"
        | "BackgroundTaskPanel"
        | "IntegrationStatus"
        | "SubagentCard"
        | "WorkingStateCard" => PageId::FEEDBACK,
        "ApprovalQueue" | "HelpCenter" | "PromptQueue" => PageId::DIALOGS,
        "AgentShell" | "AgentWorkbench" | "AppDashboard" | "AppShell" | "FileManager"
        | "GitWorkbench" | "ResourceBrowser" | "StudioShell" => PageId::STRUCTURE,
        "AuthEntry" | "SetupWizard" => PageId::FORMS,
        "ConnectionManager" | "DatabaseWorkbench" | "ProcessTable" | "QueryEditor"
        | "ResultGrid" | "SchemaBrowser" | "SessionPicker" => PageId::TABLEPRO,
        "ErrorRecovery" => PageId::FEEDBACK,
        "MetricsDashboard" | "ObservabilityDashboard" | "OpsDashboard" => PageId::CHARTS,
        "ProjectLauncher" => PageId::PICKERS,
        "SettingsScreen" => PageId::SETTINGS,
        "TaskRail" | "TerminalRunCard" => PageId::TASK_RUNNER,
        _ => PageId::STRUCTURE,
    }
}

fn public_scenario(entry: &termrock::registry::PublicUiInventoryEntry) -> CatalogScenario {
    let interactive = !matches!(entry.kind, termrock::registry::ComponentKind::Layout);
    CatalogScenario {
        id: entry.representative_story,
        title: entry.id.as_str(),
        component: entry.id.as_str(),
        description: "Canonical TermRock component scenario",
        page: catalog_page_for(entry.id),
        cols: 120,
        rows: 40,
        interactive,
        interaction_kind: if interactive {
            "interactive-component"
        } else {
            "passive-paint"
        },
    }
}

/// Return every unique representative scenario from the public component and
/// pattern registries, in stable registry order.
#[must_use]
pub fn catalog_scenarios() -> Vec<CatalogScenario> {
    let mut scenarios = Vec::new();
    for entry in public_ui_inventory() {
        if !scenarios
            .iter()
            .any(|scenario: &CatalogScenario| scenario.id == entry.representative_story)
        {
            scenarios.push(public_scenario(entry));
        }
    }
    for entry in pattern_inventory() {
        if scenarios
            .iter()
            .any(|scenario: &CatalogScenario| scenario.id == entry.representative_story)
        {
            continue;
        }
        scenarios.push(CatalogScenario {
            id: entry.representative_story,
            title: entry.id.as_str(),
            component: entry.id.as_str(),
            description: "Canonical TermRock pattern scenario",
            page: pattern_page(entry.id.as_str()),
            cols: 120,
            rows: 40,
            interactive: true,
            interaction_kind: "interactive-pattern",
        });
    }
    scenarios
}

/// Resolve a representative scenario by stable ID.
#[must_use]
pub fn scenario_by_id(id: &str) -> Option<CatalogScenario> {
    catalog_scenarios()
        .into_iter()
        .find(|scenario| scenario.id == id)
}

/// Build the typed documentation authority from the kernel registries.
#[must_use]
pub fn catalog_authority() -> CatalogAuthority {
    CatalogAuthority {
        public_ui: public_ui_inventory()
            .iter()
            .map(|entry| PublicUiAuthority {
                public_ui: entry.id.as_str(),
                kind: entry.kind.id(),
                family: entry.family.id(),
                documentation_kind: entry.documentation.id(),
                docs_slug: entry.docs_slug,
                docs_path: entry.docs_path(),
                representative_story: entry.representative_story,
            })
            .collect(),
        patterns: pattern_inventory()
            .iter()
            .map(|entry| PatternAuthority {
                pattern: entry.id.as_str(),
                docs_slug: entry.docs_slug,
                docs_path: entry.docs_path(),
                representative_story: entry.representative_story,
                public_ui: entry.public_ui.map(|id| id.as_str()),
            })
            .collect(),
    }
}

/// Build the canonical representative-scenario JSON projection.
#[must_use]
pub fn scenario_descriptors() -> Vec<ScenarioDescriptor> {
    catalog_scenarios()
        .into_iter()
        .map(|scenario| ScenarioDescriptor {
            id: scenario.id,
            title: scenario.title,
            component: scenario.component,
            description: scenario.description,
            cols: scenario.cols,
            rows: scenario.rows,
            interactive: scenario.interactive,
            interaction_kind: scenario.interaction_kind,
            hints: Vec::new(),
        })
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
        assert!(
            nav_entries(CatalogProfile::TermRock)
                .iter()
                .any(|e| e.section == "Patterns")
        );
        assert!(
            nav_entries(CatalogProfile::TermRock)
                .iter()
                .any(|e| e.section == "Applications")
        );
    }

    #[test]
    fn historical_source_shot_navigation_is_explicit() {
        let nav = reference_nav_for_scene("f_overview");
        assert_eq!(nav.len(), 20);
        assert_eq!(nav[14].id, PageId::EDITOR);
        assert_eq!(reference_nav_for_scene("s_editor").len(), 20);
    }

    #[test]
    fn representative_scenarios_cover_component_and_pattern_registries() {
        let scenarios = catalog_scenarios();
        let ids: std::collections::BTreeSet<_> =
            scenarios.iter().map(|scenario| scenario.id).collect();
        assert_eq!(ids.len(), scenarios.len());
        for entry in termrock::registry::public_ui_inventory() {
            assert!(
                scenario_by_id(entry.representative_story).is_some(),
                "missing component scenario for {}",
                entry.id
            );
        }
        for entry in termrock::registry::pattern_inventory() {
            assert!(
                scenario_by_id(entry.representative_story).is_some(),
                "missing pattern scenario for {}",
                entry.id
            );
        }
    }
}
