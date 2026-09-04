// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Component contract and registry item schema (kernel-owned types).
/// Current contract schema version (v3 = registry + quality identity).
pub const CONTRACT_SCHEMA: u32 = 3;

/// Kind of registry / inventory item (shadcn-inspired, terminal-shaped).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum RegistryItemKind {
    /// Domain-neutral engine widget (Panel, List, ScrollArea).
    Primitive,
    /// Styled or composed public component.
    Component,
    /// Headless behavior (DismissableLayer, FocusGraph helpers).
    Behavior,
    /// Multi-widget application block (workbench, dashboard).
    Block,
    /// Theme / role palette package.
    Theme,
    /// Keymap pack.
    Keymap,
    /// Scaffold template (not runtime).
    Template,
}

impl RegistryItemKind {
    /// Stable id for JSON / Studio.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Primitive => "primitive",
            Self::Component => "component",
            Self::Behavior => "behavior",
            Self::Block => "block",
            Self::Theme => "theme",
            Self::Keymap => "keymap",
            Self::Template => "template",
        }
    }

    /// Parse kind string.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "primitive" => Some(Self::Primitive),
            "component" => Some(Self::Component),
            "behavior" => Some(Self::Behavior),
            "block" => Some(Self::Block),
            "theme" => Some(Self::Theme),
            "keymap" | "key-map" => Some(Self::Keymap),
            "template" => Some(Self::Template),
            _ => None,
        }
    }

    /// All kinds.
    pub const ALL: [Self; 7] = [
        Self::Primitive,
        Self::Component,
        Self::Behavior,
        Self::Block,
        Self::Theme,
        Self::Keymap,
        Self::Template,
    ];
}

/// Role of a file in a contract / registry package.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum ContractFileRole {
    /// Primary implementation.
    Primary,
    /// Supporting module.
    Support,
    /// Studio / lookbook story.
    Story,
    /// Test or fixture.
    Fixture,
    /// Documentation.
    Docs,
    /// Migration note.
    Migration,
    /// Other.
    Other,
}

impl ContractFileRole {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Support => "support",
            Self::Story => "story",
            Self::Fixture => "fixture",
            Self::Docs => "docs",
            Self::Migration => "migration",
            Self::Other => "other",
        }
    }

    /// Parse.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "primary" => Some(Self::Primary),
            "support" => Some(Self::Support),
            "story" => Some(Self::Story),
            "fixture" | "test" => Some(Self::Fixture),
            "docs" | "doc" => Some(Self::Docs),
            "migration" => Some(Self::Migration),
            "other" => Some(Self::Other),
            _ => None,
        }
    }
}

/// One file belonging to a contract or registry item.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ContractFile {
    /// Source path relative to package or monorepo root.
    pub source: String,
    /// Install destination relative to consumer project (registry items).
    pub target: Option<String>,
    /// File role.
    pub role: ContractFileRole,
    /// Optional content digest (`sha256:` + hex).
    pub hash: Option<String>,
    /// Optional install.
    pub optional: bool,
}

/// Provenance for source-owned trust.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Provenance {
    /// Origin URL or monorepo label.
    pub origin: String,
    /// Path within origin.
    pub path: String,
    /// Git revision or crate version.
    pub revision: String,
    /// SPDX license id.
    pub spdx: String,
    /// Authors (optional display).
    pub authors: Vec<String>,
}

/// Kernel crate requirement for a registry item.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct KernelRequirement {
    /// Crate name (`termrock`).
    pub crate_name: String,
    /// Minimum version string (informational).
    pub min_version: String,
    /// Required Cargo features.
    pub features: Vec<String>,
}

/// Dependencies on other registry items and crates.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ContractDependencies {
    /// Kernel requirement.
    pub kernel: Option<KernelRequirement>,
    /// Other registry item ids (`termrock/scroll-area`).
    pub registry: Vec<String>,
    /// Cargo crate names (informational).
    pub cargo: Vec<String>,
}

/// Capability expectations (progressive enhancement).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CapabilityRequirements {
    /// Color ladder labels (`truecolor`, `256`, `mono`).
    pub color: Vec<String>,
    /// Glyph sets (`unicode`, `ascii`).
    pub glyphs: Vec<String>,
    /// Responsive surface name if any.
    pub responsive_surface: Option<String>,
    /// Minimum terminal width.
    pub min_width: Option<u16>,
    /// Minimum terminal height.
    pub min_height: Option<u16>,
    /// Requires mouse for full UX (keyboard fallback required anyway).
    pub mouse_enhanced: bool,
}

/// Anatomy part name for docs/Studio.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AnatomyPartRef {
    /// Part id (`title`, `body`, `scrollbar`).
    pub id: String,
    /// Human label.
    pub label: String,
}

/// Semantic role reference (`Role::Text`, scene role).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SemanticRoleRef {
    /// Role id.
    pub id: String,
}

/// Public variant name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VariantRef {
    /// Variant id.
    pub id: String,
    /// Description.
    pub description: String,
}

/// Typed outcome / result enum name for docs.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OutcomeRef {
    /// Type or variant path.
    pub id: String,
}

/// Full machine-readable contract for a TermRock inventory item.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ComponentContract {
    /// Schema version ([`CONTRACT_SCHEMA`]).
    pub schema: u32,
    /// Stable id (`Panel`, `scroll-area`, `agent-workbench`).
    pub id: String,
    /// Display title.
    pub title: String,
    /// Short description.
    pub description: String,
    /// Item kind.
    pub kind: RegistryItemKind,
    /// SPDX license.
    pub license: String,
    /// Rust module path when kernel-hosted.
    pub module: Option<String>,
    /// Namespace for private registries (`termrock`, `acme`).
    pub namespace: String,
    /// Semver-like version of the contract/package.
    pub version: String,
    /// Files owned by this item.
    pub files: Vec<ContractFile>,
    /// Dependencies.
    pub dependencies: ContractDependencies,
    /// Capability requirements / ladders.
    pub capabilities: CapabilityRequirements,
    /// Anatomy parts.
    pub anatomy: Vec<AnatomyPartRef>,
    /// Semantic roles used.
    pub semantic_roles: Vec<SemanticRoleRef>,
    /// Variants.
    pub variants: Vec<VariantRef>,
    /// Outcomes / events.
    pub outcomes: Vec<OutcomeRef>,
    /// Studio story ids.
    pub stories: Vec<String>,
    /// Test filters / paths.
    pub tests: Vec<String>,
    /// Migration doc path if any.
    pub migration: Option<String>,
    /// Provenance.
    pub provenance: Provenance,
    /// Aggregate source hash of primary files (`sha256:` + hex) when known.
    pub source_hash: Option<String>,
    /// Whether the quality contract is considered complete.
    pub complete: bool,
}

impl ComponentContract {
    /// Fully qualified registry name `namespace/id`.
    #[must_use]
    pub fn qualified_name(&self) -> String {
        format!("{}/{}", self.namespace, self.id)
    }

    /// Whether this is a kernel-hosted primitive/behavior/component (not copy-install).
    #[must_use]
    pub const fn is_kernel_hosted(&self) -> bool {
        matches!(
            self.kind,
            RegistryItemKind::Primitive | RegistryItemKind::Behavior | RegistryItemKind::Component
        ) && self.module.is_some()
    }
}
