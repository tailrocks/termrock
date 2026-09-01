// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Machine-readable component contracts and registry metadata.
//!
//! Every TermRock public component (and every source-owned registry item) can be
//! described by a [`ComponentContract`]. Contracts enable design linting, CI
//! validation, documentation generation, Studio browsing, and private registries
//! without turning the kernel into a package manager.
//!
//! **Distribution law:** the kernel crate stays shared; registry items may be
//! copied into apps. Contracts describe **both** without forcing copy-paste of
//! focus/Esc/Unicode engines.

mod catalog;
mod contract;
mod inventory;
mod pattern_inventory;
mod validate;

pub use catalog::{official_contract, official_ids, official_kernel_contracts};
pub use contract::{
    AnatomyPartRef, CONTRACT_SCHEMA, CapabilityRequirements, ComponentContract,
    ContractDependencies, ContractFile, ContractFileRole, KernelRequirement, OutcomeRef,
    Provenance, RegistryItemKind, SemanticRoleRef, VariantRef,
};
pub use inventory::{
    ComponentFamily, ComponentKind, DocumentationKind, PUBLIC_UI_INVENTORY, PublicUiId,
    PublicUiInventoryEntry, PublicUiInventoryError, public_ui_by_id, public_ui_inventory,
    validate_public_ui_inventory,
};
pub use pattern_inventory::{
    PUBLIC_PATTERN_INVENTORY, PatternId, PatternInventoryEntry, PatternInventoryError,
    pattern_by_id, pattern_inventory, validate_pattern_inventory,
};
pub use validate::{
    ContractIssue, ContractIssueLevel, ValidationReport, validate_contract, validate_contracts,
};
