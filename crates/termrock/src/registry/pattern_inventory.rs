// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Typed authority for canonical application-pattern documentation.

use std::collections::BTreeSet;

use super::inventory::{DocumentationKind, PublicUiId, public_ui_by_id, public_ui_inventory};

macro_rules! define_pattern_ids {
    ($($id:ident),+ $(,)?) => {
        /// Stable canonical pattern identity.
        #[allow(missing_docs)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[non_exhaustive]
        pub enum PatternId {
            $($id),+
        }

        impl PatternId {
            /// Every registered pattern identity, in stable lexical order.
            pub const ALL: &'static [Self] = &[$(Self::$id),+];

            /// Stable PascalCase pattern identity.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$id => stringify!($id)),+
                }
            }

            /// Resolve only registered canonical patterns.
            #[must_use]
            pub fn parse(id: &str) -> Option<Self> {
                pattern_by_id(id).map(|pattern| pattern.id)
            }
        }
    };
}

define_pattern_ids![
    ActivityShelf,
    AgentShell,
    AgentStatusHeader,
    AgentWorkbench,
    AppDashboard,
    AppShell,
    ApprovalQueue,
    AuthEntry,
    BackgroundTaskPanel,
    ConnectionManager,
    DatabaseWorkbench,
    ErrorRecovery,
    FileManager,
    GitWorkbench,
    HelpCenter,
    IntegrationStatus,
    MetricsDashboard,
    ObservabilityDashboard,
    OpsDashboard,
    PlanReview,
    ProcessTable,
    ProjectLauncher,
    PromptQueue,
    QueryEditor,
    ResourceBrowser,
    ResultGrid,
    SchemaBrowser,
    SessionPicker,
    SettingsScreen,
    SetupWizard,
    StudioShell,
    SubagentCard,
    TaskRail,
    TerminalRunCard,
    WorkingStateCard,
];

impl std::fmt::Display for PatternId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// One canonical pattern joined to docs, story, and optional exact public struct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PatternInventoryEntry {
    /// Stable typed pattern identity.
    pub id: PatternId,
    /// Stable unique docs slug.
    pub docs_slug: &'static str,
    /// Representative mounted story.
    pub representative_story: &'static str,
    /// Exact public `termrock::patterns` struct, when the pattern has one.
    pub public_ui: Option<PublicUiId>,
}

impl PatternInventoryEntry {
    /// Unique canonical documentation path.
    #[must_use]
    pub fn docs_path(self) -> String {
        format!("/docs/patterns/{}", self.docs_slug)
    }
}

/// Structural pattern-inventory failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PatternInventoryError {
    /// No patterns were supplied.
    Empty,
    /// Pattern id is invalid.
    InvalidId(PatternId),
    /// Docs slug is invalid.
    InvalidDocsSlug(PatternId),
    /// Representative story id is invalid.
    InvalidRepresentativeStory(PatternId),
    /// Pattern id appears more than once.
    DuplicateId(PatternId),
    /// Docs slug appears more than once.
    DuplicateDocsSlug(PatternId),
    /// Pattern ids are not in stable lexical order.
    UnsortedId(PatternId),
    /// Public UI link does not use the exact pattern identity.
    InvalidPublicUi(PatternId),
    /// Linked public UI is not documented as an exact pattern surface.
    InvalidPublicUiDocumentation(PatternId),
    /// Linked public UI and pattern route or representative story disagree.
    PublicUiMetadataMismatch(PatternId),
    /// Exact public pattern surfaces and typed pattern links disagree.
    PublicUiPatternSetMismatch,
}

impl std::fmt::Display for PatternInventoryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => formatter.write_str("pattern inventory is empty"),
            Self::InvalidId(id) => write!(formatter, "invalid pattern id {id}"),
            Self::InvalidDocsSlug(id) => write!(formatter, "invalid docs slug for {id}"),
            Self::InvalidRepresentativeStory(id) => {
                write!(formatter, "invalid representative story for {id}")
            }
            Self::DuplicateId(id) => write!(formatter, "duplicate pattern id {id}"),
            Self::DuplicateDocsSlug(id) => write!(formatter, "duplicate docs slug for {id}"),
            Self::UnsortedId(id) => write!(formatter, "pattern id is out of order at {id}"),
            Self::InvalidPublicUi(id) => write!(formatter, "invalid public UI link for {id}"),
            Self::InvalidPublicUiDocumentation(id) => {
                write!(
                    formatter,
                    "public UI link for {id} is not a pattern surface"
                )
            }
            Self::PublicUiMetadataMismatch(id) => {
                write!(formatter, "public UI route or story disagrees for {id}")
            }
            Self::PublicUiPatternSetMismatch => {
                formatter.write_str("public UI pattern surfaces and pattern links disagree")
            }
        }
    }
}

impl std::error::Error for PatternInventoryError {}

macro_rules! pattern {
    ($id:ident, $slug:literal, $story:literal) => {
        PatternInventoryEntry {
            id: PatternId::$id,
            docs_slug: $slug,
            representative_story: $story,
            public_ui: None,
        }
    };
    ($id:ident, $slug:literal, $story:literal, public) => {
        PatternInventoryEntry {
            id: PatternId::$id,
            docs_slug: $slug,
            representative_story: $story,
            public_ui: Some(PublicUiId::$id),
        }
    };
}

/// Canonical pattern inventory: 18 exact public structs plus 17 composed concepts.
pub static PUBLIC_PATTERN_INVENTORY: &[PatternInventoryEntry] = &[
    pattern!(
        ActivityShelf,
        "activity-shelf",
        "activity-shelf/statuses",
        public
    ),
    pattern!(AgentShell, "agent-shell", "agent-shell/basic"),
    pattern!(
        AgentStatusHeader,
        "agent-status-header",
        "agent-status-header/basic",
        public
    ),
    pattern!(AgentWorkbench, "agent-workbench", "agent-workbench/basic"),
    pattern!(AppDashboard, "app-dashboard", "app-dashboard/basic"),
    pattern!(AppShell, "app-shell", "app-shell/workbench"),
    pattern!(
        ApprovalQueue,
        "approval-queue",
        "approval-queue/basic",
        public
    ),
    pattern!(AuthEntry, "auth-entry", "auth-entry/basic"),
    pattern!(
        BackgroundTaskPanel,
        "background-task-panel",
        "background-tasks/mixed-statuses",
        public
    ),
    pattern!(
        ConnectionManager,
        "connection-manager",
        "connection-manager/full",
        public
    ),
    pattern!(
        DatabaseWorkbench,
        "database-workbench",
        "database-workbench/basic"
    ),
    pattern!(ErrorRecovery, "error-recovery", "error-recovery/basic"),
    pattern!(FileManager, "file-manager", "file-manager/basic"),
    pattern!(GitWorkbench, "git-workbench", "git-workbench/basic"),
    pattern!(HelpCenter, "help-center", "help-center/basic"),
    pattern!(
        IntegrationStatus,
        "integration-status",
        "integration-status/list",
        public
    ),
    pattern!(
        MetricsDashboard,
        "metrics-dashboard",
        "metrics-dashboard/basic",
        public
    ),
    pattern!(
        ObservabilityDashboard,
        "observability-dashboard",
        "observability-dashboard/basic"
    ),
    pattern!(OpsDashboard, "ops-dashboard", "ops-dashboard/basic"),
    pattern!(PlanReview, "plan-review", "plan-review/basic", public),
    pattern!(ProcessTable, "process-table", "process-table/basic", public),
    pattern!(
        ProjectLauncher,
        "project-launcher",
        "project-launcher/basic"
    ),
    pattern!(PromptQueue, "prompt-queue", "prompt-queue/compact", public),
    pattern!(QueryEditor, "query-editor", "query-editor/basic", public),
    pattern!(
        ResourceBrowser,
        "resource-browser",
        "resource-browser/basic"
    ),
    pattern!(ResultGrid, "result-grid", "result-grid/basic", public),
    pattern!(
        SchemaBrowser,
        "schema-browser",
        "schema-browser/basic",
        public
    ),
    pattern!(
        SessionPicker,
        "session-picker",
        "session-picker/basic",
        public
    ),
    pattern!(SettingsScreen, "settings-screen", "settings-screen/basic"),
    pattern!(SetupWizard, "setup-wizard", "setup-wizard/welcome"),
    pattern!(StudioShell, "studio-shell", "studio-shell/basic"),
    pattern!(
        SubagentCard,
        "subagent-card",
        "subagent-card/running",
        public
    ),
    pattern!(TaskRail, "task-rail", "task-rail/basic", public),
    pattern!(
        TerminalRunCard,
        "terminal-run-card",
        "terminal-run-card/running",
        public
    ),
    pattern!(
        WorkingStateCard,
        "working-state-card",
        "working-state-card/basic",
        public
    ),
];

/// Borrow the canonical pattern inventory.
#[must_use]
pub const fn pattern_inventory() -> &'static [PatternInventoryEntry] {
    PUBLIC_PATTERN_INVENTORY
}

/// Resolve one canonical pattern by stable identity.
#[must_use]
pub fn pattern_by_id(id: &str) -> Option<&'static PatternInventoryEntry> {
    PUBLIC_PATTERN_INVENTORY
        .iter()
        .find(|pattern| pattern.id.as_str() == id)
}

/// Validate identity, routing, representative stories, public links, and uniqueness.
pub fn validate_pattern_inventory(
    inventory: &[PatternInventoryEntry],
) -> Result<(), PatternInventoryError> {
    if inventory.is_empty() {
        return Err(PatternInventoryError::Empty);
    }
    let mut ids = BTreeSet::new();
    let mut slugs = BTreeSet::new();
    let mut linked_public_ui = BTreeSet::new();
    let mut previous_id = None;
    for pattern in inventory {
        if !valid_id(pattern.id.as_str()) {
            return Err(PatternInventoryError::InvalidId(pattern.id));
        }
        if !valid_slug(pattern.docs_slug) {
            return Err(PatternInventoryError::InvalidDocsSlug(pattern.id));
        }
        if !valid_story_id(pattern.representative_story) {
            return Err(PatternInventoryError::InvalidRepresentativeStory(
                pattern.id,
            ));
        }
        if let Some(public_ui) = pattern.public_ui {
            let Some(public_entry) = public_ui_by_id(public_ui.as_str()) else {
                return Err(PatternInventoryError::InvalidPublicUi(pattern.id));
            };
            if public_ui.as_str() != pattern.id.as_str() || public_entry.id != public_ui {
                return Err(PatternInventoryError::InvalidPublicUi(pattern.id));
            }
            if public_entry.documentation != DocumentationKind::Pattern {
                return Err(PatternInventoryError::InvalidPublicUiDocumentation(
                    pattern.id,
                ));
            }
            if public_entry.docs_slug != pattern.docs_slug
                || public_entry.representative_story != pattern.representative_story
            {
                return Err(PatternInventoryError::PublicUiMetadataMismatch(pattern.id));
            }
            linked_public_ui.insert(public_ui);
        }
        if !ids.insert(pattern.id.as_str()) {
            return Err(PatternInventoryError::DuplicateId(pattern.id));
        }
        if !slugs.insert(pattern.docs_slug) {
            return Err(PatternInventoryError::DuplicateDocsSlug(pattern.id));
        }
        if previous_id.is_some_and(|previous| previous >= pattern.id.as_str()) {
            return Err(PatternInventoryError::UnsortedId(pattern.id));
        }
        previous_id = Some(pattern.id.as_str());
    }
    let documented_public_patterns: BTreeSet<_> = public_ui_inventory()
        .iter()
        .filter(|entry| entry.documentation == DocumentationKind::Pattern)
        .map(|entry| entry.id)
        .collect();
    if linked_public_ui != documented_public_patterns {
        return Err(PatternInventoryError::PublicUiPatternSetMismatch);
    }
    Ok(())
}

fn valid_id(id: &str) -> bool {
    let mut chars = id.chars();
    chars.next().is_some_and(|first| first.is_ascii_uppercase())
        && chars.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn valid_slug(slug: &str) -> bool {
    !slug.is_empty()
        && !slug.starts_with('-')
        && !slug.ends_with('-')
        && slug
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn valid_story_id(story: &str) -> bool {
    let Some((family, variant)) = story.split_once('/') else {
        return false;
    };
    valid_slug(family) && valid_slug(variant)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pattern_inventory_is_complete_unique_and_joinable() {
        assert_eq!(pattern_inventory().len(), 35);
        assert_eq!(pattern_inventory().len(), PatternId::ALL.len());
        assert_eq!(
            PatternId::ALL.iter().copied().collect::<BTreeSet<_>>(),
            pattern_inventory()
                .iter()
                .map(|pattern| pattern.id)
                .collect()
        );
        assert_eq!(validate_pattern_inventory(pattern_inventory()), Ok(()));
        assert_eq!(
            pattern_inventory()
                .iter()
                .filter(|pattern| pattern.public_ui.is_some())
                .count(),
            18
        );
        for pattern in pattern_inventory() {
            assert_eq!(PatternId::parse(pattern.id.as_str()), Some(pattern.id));
        }
    }

    #[test]
    fn linked_pattern_metadata_cannot_drift_from_public_ui() {
        let mut pattern = pattern_inventory()[0];
        pattern.docs_slug = "wrong-route";
        assert_eq!(
            validate_pattern_inventory(&[pattern]),
            Err(PatternInventoryError::PublicUiMetadataMismatch(pattern.id))
        );
    }
}
