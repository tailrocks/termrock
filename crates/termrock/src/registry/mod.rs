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
mod validate;

pub use catalog::{official_contract, official_ids, official_kernel_contracts};
pub use contract::{
    AnatomyPartRef, CONTRACT_SCHEMA, CapabilityRequirements, ComponentContract,
    ContractDependencies, ContractFile, ContractFileRole, KernelRequirement, OutcomeRef,
    Provenance, RegistryItemKind, SemanticRoleRef, VariantRef,
};
pub use validate::{
    ContractIssue, ContractIssueLevel, ValidationReport, validate_contract, validate_contracts,
};
