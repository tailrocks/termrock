// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Contract validation (design lint + structural CI).

use super::contract::{ComponentContract, ContractFileRole, RegistryItemKind, CONTRACT_SCHEMA};

/// Severity of a validation issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ContractIssueLevel {
    /// Hard failure for CI.
    Error,
    /// Soft warning.
    Warning,
}

/// One validation finding.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContractIssue {
    /// Level.
    pub level: ContractIssueLevel,
    /// Contract id (if known).
    pub contract_id: String,
    /// Machine code.
    pub code: String,
    /// Human message.
    pub message: String,
}

/// Full validation report.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ValidationReport {
    /// Issues found.
    pub issues: Vec<ContractIssue>,
}

impl ValidationReport {
    /// True when no errors (warnings allowed).
    #[must_use]
    pub fn ok(&self) -> bool {
        !self
            .issues
            .iter()
            .any(|i| matches!(i.level, ContractIssueLevel::Error))
    }

    /// Error count.
    #[must_use]
    pub fn error_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| matches!(i.level, ContractIssueLevel::Error))
            .count()
    }

    fn push(
        &mut self,
        level: ContractIssueLevel,
        contract_id: &str,
        code: &str,
        message: impl Into<String>,
    ) {
        self.issues.push(ContractIssue {
            level,
            contract_id: contract_id.into(),
            code: code.into(),
            message: message.into(),
        });
    }
}

fn is_safe_relative(path: &str) -> bool {
    if path.is_empty() || path.starts_with('/') || path.starts_with('\\') {
        return false;
    }
    if path.contains('\0') {
        return false;
    }
    for part in path.split(['/', '\\']) {
        if part == ".." || part == "." && path == "." {
            return false;
        }
        if part == ".." {
            return false;
        }
    }
    true
}

fn looks_like_sha256(hash: &str) -> bool {
    let hex = hash
        .strip_prefix("sha256:")
        .or_else(|| hash.strip_prefix("sha256-"))
        .unwrap_or(hash);
    hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit())
}

/// Validate a single contract.
#[must_use]
pub fn validate_contract(c: &ComponentContract) -> ValidationReport {
    let mut r = ValidationReport::default();
    let id = c.id.as_str();

    if c.schema != CONTRACT_SCHEMA {
        r.push(
            ContractIssueLevel::Error,
            id,
            "schema",
            format!("expected schema {CONTRACT_SCHEMA}, got {}", c.schema),
        );
    }
    if c.id.trim().is_empty() {
        r.push(
            ContractIssueLevel::Error,
            id,
            "id",
            "id must be non-empty",
        );
    }
    if c.id.contains('/') || c.id.contains('\\') {
        r.push(
            ContractIssueLevel::Error,
            id,
            "id",
            "id must not contain path separators (use namespace)",
        );
    }
    if c.title.trim().is_empty() {
        r.push(
            ContractIssueLevel::Error,
            id,
            "title",
            "title must be non-empty",
        );
    }
    if c.description.trim().is_empty() {
        r.push(
            ContractIssueLevel::Warning,
            id,
            "description",
            "description is empty",
        );
    }
    if c.license.trim().is_empty() {
        r.push(
            ContractIssueLevel::Error,
            id,
            "license",
            "license (SPDX) required",
        );
    }
    if c.namespace.trim().is_empty() {
        r.push(
            ContractIssueLevel::Error,
            id,
            "namespace",
            "namespace required (e.g. termrock)",
        );
    }
    if c.version.trim().is_empty() {
        r.push(
            ContractIssueLevel::Error,
            id,
            "version",
            "version required",
        );
    }
    if c.provenance.origin.trim().is_empty() || c.provenance.path.trim().is_empty() {
        r.push(
            ContractIssueLevel::Error,
            id,
            "provenance",
            "provenance.origin and path required",
        );
    }
    if c.provenance.spdx.trim().is_empty() {
        r.push(
            ContractIssueLevel::Error,
            id,
            "provenance.spdx",
            "provenance.spdx required",
        );
    }

    if c.files.is_empty() {
        r.push(
            ContractIssueLevel::Error,
            id,
            "files",
            "at least one file entry required",
        );
    }
    let mut has_primary = false;
    for f in &c.files {
        if !is_safe_relative(&f.source) {
            r.push(
                ContractIssueLevel::Error,
                id,
                "files.source",
                format!("unsafe source path {}", f.source),
            );
        }
        if let Some(t) = &f.target
            && !is_safe_relative(t)
        {
            r.push(
                ContractIssueLevel::Error,
                id,
                "files.target",
                format!("unsafe target path {t}"),
            );
        }
        if let Some(h) = &f.hash
            && !looks_like_sha256(h)
        {
            r.push(
                ContractIssueLevel::Error,
                id,
                "files.hash",
                format!("invalid sha256 for {}", f.source),
            );
        }
        if matches!(f.role, ContractFileRole::Primary) {
            has_primary = true;
        }
    }
    if !has_primary {
        r.push(
            ContractIssueLevel::Warning,
            id,
            "files.primary",
            "no primary file role declared",
        );
    }

    if let Some(h) = &c.source_hash
        && !looks_like_sha256(h)
    {
        r.push(
            ContractIssueLevel::Error,
            id,
            "source_hash",
            "source_hash must be sha256 hex (optional prefix sha256:)",
        );
    }

    // Kernel-hosted components should name a module.
    if matches!(
        c.kind,
        RegistryItemKind::Primitive | RegistryItemKind::Component | RegistryItemKind::Behavior
    ) && c.module.as_ref().is_none_or(|m| m.trim().is_empty())
    {
        r.push(
            ContractIssueLevel::Warning,
            id,
            "module",
            "kernel-style kinds should set module path",
        );
    }

    // Completeness: stories + tests for complete interactive components.
    if c.complete {
        if c.stories.is_empty() {
            r.push(
                ContractIssueLevel::Error,
                id,
                "complete.stories",
                "complete=true requires at least one story id",
            );
        }
        if c.tests.is_empty() {
            r.push(
                ContractIssueLevel::Error,
                id,
                "complete.tests",
                "complete=true requires at least one test filter",
            );
        }
        if c.anatomy.is_empty() {
            r.push(
                ContractIssueLevel::Warning,
                id,
                "complete.anatomy",
                "complete=true should list anatomy parts",
            );
        }
    }

    // Blocks should declare registry or composition deps.
    if matches!(c.kind, RegistryItemKind::Block)
        && c.dependencies.registry.is_empty()
        && c.dependencies.cargo.is_empty()
    {
        r.push(
            ContractIssueLevel::Warning,
            id,
            "block.deps",
            "block has no registry/cargo dependencies listed",
        );
    }

    r
}

/// Validate many contracts; also check unique ids within namespace.
#[must_use]
pub fn validate_contracts(contracts: &[ComponentContract]) -> ValidationReport {
    let mut r = ValidationReport::default();
    let mut seen = std::collections::BTreeSet::new();
    for c in contracts {
        let sub = validate_contract(c);
        r.issues.extend(sub.issues);
        let q = c.qualified_name();
        if !seen.insert(q.clone()) {
            r.push(
                ContractIssueLevel::Error,
                &c.id,
                "duplicate",
                format!("duplicate qualified name {q}"),
            );
        }
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::official_kernel_contracts;

    #[test]
    fn official_catalog_validates() {
        let catalog = official_kernel_contracts();
        let report = validate_contracts(&catalog);
        assert!(
            report.ok(),
            "official catalog errors: {:?}",
            report
                .issues
                .iter()
                .filter(|i| matches!(i.level, ContractIssueLevel::Error))
                .collect::<Vec<_>>()
        );
        assert!(catalog.len() >= 5);
    }

    #[test]
    fn rejects_path_escape() {
        let mut c = official_kernel_contracts().remove(0);
        c.files[0].source = "../secret".into();
        let r = validate_contract(&c);
        assert!(!r.ok());
        assert!(r.issues.iter().any(|i| i.code == "files.source"));
    }

    #[test]
    fn complete_requires_stories_and_tests() {
        let mut c = official_kernel_contracts()
            .into_iter()
            .find(|x| x.id == "Panel")
            .unwrap();
        c.complete = true;
        c.stories.clear();
        c.tests.clear();
        let r = validate_contract(&c);
        assert!(!r.ok());
    }
}
