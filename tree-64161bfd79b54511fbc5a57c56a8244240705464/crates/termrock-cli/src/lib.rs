// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Offline registry planner and installer (Plan 047).
//!
//! **Trust:** registry content is untrusted. Destination workspace is user-owned.
//! No shell/template execution. No network in the core resolver.
//!
//! **Path law:** every destination must resolve under the canonical workspace
//! root. Symlink components that escape the workspace are refused. Intermediate
//! symlinks are followed only when the resolved real path stays inside the root.

#![allow(clippy::collapsible_if)] // path walk prefers explicit nested symlink gates

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Schema version for registry entries.
pub const SCHEMA_VERSION: u32 = 1;

/// Backup suffix used when `--force` overwrites a dirty destination.
pub const FORCE_BACKUP_SUFFIX: &str = ".termrock.bak";

/// A single file to install.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryFile {
    /// Path relative to registry entry root.
    pub source: String,
    /// Destination relative to consumer project root.
    pub dest: String,
    /// Hex sha256 of source bytes.
    pub sha256: String,
}

/// Registry entry metadata + files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryEntry {
    /// Schema version.
    pub schema: u32,
    /// Stable name (`ops-dashboard`).
    pub name: String,
    /// Semver-like version string.
    pub version: String,
    /// Human description.
    pub description: String,
    /// SPDX license.
    pub license: String,
    /// Required kernel crate version constraint (informational).
    pub kernel: String,
    /// Files to copy.
    pub files: Vec<RegistryFile>,
}

/// Installed file record in consumer manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledFile {
    /// Destination relative path.
    pub dest: String,
    /// Upstream digest at install time.
    pub upstream_sha256: String,
}

/// Consumer install manifest fragment for one entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallRecord {
    /// Entry name.
    pub name: String,
    /// Installed version.
    pub version: String,
    /// Files.
    pub files: Vec<InstalledFile>,
}

/// Full consumer manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct InstallManifest {
    /// Schema.
    pub schema: u32,
    /// Records by name.
    pub installed: BTreeMap<String, InstallRecord>,
}

/// One planned file action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanAction {
    /// Create new file (or force-replace with backup).
    Create {
        /// Dest relative to workspace.
        dest: PathBuf,
        /// Source absolute path (registry entry file).
        source: PathBuf,
        /// Content digest.
        sha256: String,
        /// Whether an existing different file will be backed up then replaced.
        force_overwrite: bool,
    },
    /// Conflict: dest exists and differs from upstream (dirty or foreign).
    Conflict {
        /// Dest.
        dest: PathBuf,
        /// Reason.
        reason: String,
    },
    /// Skip identical.
    Unchanged {
        /// Dest.
        dest: PathBuf,
    },
}

/// Install plan (no mutation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallPlan {
    /// Entry name.
    pub name: String,
    /// Actions.
    pub actions: Vec<PlanAction>,
    /// Fatal errors (path escape, bad hash, schema).
    pub errors: Vec<String>,
}

/// Errors from registry operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// I/O.
    Io(String),
    /// Invalid path.
    Path(String),
    /// Schema/parse.
    Schema(String),
    /// Hash mismatch.
    Hash(String),
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(s) | Self::Path(s) | Self::Schema(s) | Self::Hash(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for RegistryError {}

/// Hex sha256 of bytes.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}

/// Reject absolute paths, `..`, empty, and null components (string form only).
pub fn validate_relative_path(path: &str) -> Result<PathBuf, RegistryError> {
    if path.is_empty() {
        return Err(RegistryError::Path("empty path".into()));
    }
    if path.starts_with('/') || path.starts_with('\\') {
        return Err(RegistryError::Path(format!(
            "absolute path refused: {path}"
        )));
    }
    let p = Path::new(path);
    for c in p.components() {
        match c {
            Component::Normal(s) => {
                let s = s.to_string_lossy();
                if s == ".." || s.contains('\0') {
                    return Err(RegistryError::Path(format!("illegal component in {path}")));
                }
            }
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(RegistryError::Path(format!("parent dir refused: {path}")));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(RegistryError::Path(format!(
                    "absolute path refused: {path}"
                )));
            }
        }
    }
    Ok(p.to_path_buf())
}

/// Canonicalize workspace root (must exist as a directory).
pub fn canonicalize_workspace(workspace: &Path) -> Result<PathBuf, RegistryError> {
    let meta = fs::symlink_metadata(workspace)
        .map_err(|e| RegistryError::Path(format!("workspace {}: {e}", workspace.display())))?;
    if meta.file_type().is_symlink() {
        // Allow workspace itself to be a symlink only if it resolves to a dir.
    }
    let canon = fs::canonicalize(workspace).map_err(|e| {
        RegistryError::Path(format!(
            "canonicalize workspace {}: {e}",
            workspace.display()
        ))
    })?;
    if !canon.is_dir() {
        return Err(RegistryError::Path(format!(
            "workspace is not a directory: {}",
            canon.display()
        )));
    }
    Ok(canon)
}

/// Resolve `workspace / rel` ensuring the final real path stays under `workspace_root`.
///
/// Walks components; if a component exists as a symlink, its target must canonicalize
/// under the workspace. Non-existent trailing components are appended lexically after
/// verifying the deepest existing ancestor is still under the root.
pub fn resolve_dest_under_workspace(
    workspace_root: &Path,
    rel: &Path,
) -> Result<PathBuf, RegistryError> {
    let mut cur = workspace_root.to_path_buf();
    for c in rel.components() {
        let Component::Normal(name) = c else {
            return Err(RegistryError::Path(format!(
                "illegal dest component in {}",
                rel.display()
            )));
        };
        let next = cur.join(name);
        if next.exists() || next.symlink_metadata().is_ok() {
            // Exists as file, dir, or symlink — canonicalize this step.
            let meta = fs::symlink_metadata(&next)
                .map_err(|e| RegistryError::Path(format!("stat {}: {e}", next.display())))?;
            if meta.file_type().is_symlink() {
                let target = fs::canonicalize(&next).map_err(|e| {
                    RegistryError::Path(format!(
                        "symlink escape or broken link {}: {e}",
                        next.display()
                    ))
                })?;
                if !target.starts_with(workspace_root) {
                    return Err(RegistryError::Path(format!(
                        "symlink escapes workspace: {} -> {}",
                        next.display(),
                        target.display()
                    )));
                }
                cur = target;
            } else {
                // Regular path — still canonicalize to collapse .. if any slipped in.
                let target = fs::canonicalize(&next).map_err(|e| {
                    RegistryError::Path(format!("canonicalize {}: {e}", next.display()))
                })?;
                if !target.starts_with(workspace_root) {
                    return Err(RegistryError::Path(format!(
                        "path escapes workspace: {}",
                        next.display()
                    )));
                }
                cur = target;
            }
        } else {
            // Path does not exist yet — remaining components must be Normal-only
            // (already validated). Append lexically under verified `cur`.
            if !cur.starts_with(workspace_root) {
                return Err(RegistryError::Path(format!(
                    "path escapes workspace at {}",
                    cur.display()
                )));
            }
            cur = next;
            // Append rest without further existence checks, but refuse if the
            // joined path ever leaves root via string prefix (defense).
            // Continue loop for remaining components as pure joins.
        }
    }
    // Final check: if path exists, full canonicalize; else ensure parent chain under root.
    if cur.exists() || cur.symlink_metadata().is_ok() {
        let real = fs::canonicalize(&cur).map_err(|e| {
            RegistryError::Path(format!("final canonicalize {}: {e}", cur.display()))
        })?;
        if !real.starts_with(workspace_root) {
            return Err(RegistryError::Path(format!(
                "final path escapes workspace: {}",
                real.display()
            )));
        }
        return Ok(real);
    }
    // Non-existent: ensure deepest existing ancestor is under root, then return
    // workspace_root-joined lexical path (not outside).
    let mut ancestor = cur.as_path();
    loop {
        if ancestor == workspace_root || ancestor.starts_with(workspace_root) {
            if let Ok(meta) = fs::symlink_metadata(ancestor) {
                if meta.file_type().is_symlink() {
                    let real = fs::canonicalize(ancestor).map_err(|e| {
                        RegistryError::Path(format!("ancestor symlink {}: {e}", ancestor.display()))
                    })?;
                    if !real.starts_with(workspace_root) {
                        return Err(RegistryError::Path(format!(
                            "ancestor symlink escapes workspace: {}",
                            ancestor.display()
                        )));
                    }
                }
            }
        }
        if ancestor == workspace_root {
            break;
        }
        match ancestor.parent() {
            Some(p) if p != ancestor => ancestor = p,
            _ => break,
        }
    }
    // Build lexical dest from root + rel to avoid relative `cur` that snuck out.
    let lexical = workspace_root.join(rel);
    if !lexical.starts_with(workspace_root) {
        return Err(RegistryError::Path(format!(
            "lexical path escapes workspace: {}",
            rel.display()
        )));
    }
    // Reject if any prefix of lexical is a symlink leaving the workspace.
    let mut check = workspace_root.to_path_buf();
    for c in rel.components() {
        let Component::Normal(name) = c else {
            return Err(RegistryError::Path("illegal component".into()));
        };
        check = check.join(name);
        if let Ok(meta) = fs::symlink_metadata(&check) {
            if meta.file_type().is_symlink() {
                let real = fs::canonicalize(&check).map_err(|e| {
                    RegistryError::Path(format!("symlink {}: {e}", check.display()))
                })?;
                if !real.starts_with(workspace_root) {
                    return Err(RegistryError::Path(format!(
                        "symlink component escapes workspace: {}",
                        check.display()
                    )));
                }
            }
        }
    }
    Ok(lexical)
}

/// Load entry JSON from a directory (`entry.json` + files).
pub fn load_entry(entry_dir: &Path) -> Result<RegistryEntry, RegistryError> {
    let meta = entry_dir.join("entry.json");
    let raw = fs::read_to_string(&meta).map_err(|e| RegistryError::Io(e.to_string()))?;
    let entry: RegistryEntry =
        serde_json::from_str(&raw).map_err(|e| RegistryError::Schema(e.to_string()))?;
    if entry.schema != SCHEMA_VERSION {
        return Err(RegistryError::Schema(format!(
            "unsupported schema {}",
            entry.schema
        )));
    }
    Ok(entry)
}

/// Build a dry-run plan.
pub fn plan_install(
    entry: &RegistryEntry,
    entry_dir: &Path,
    workspace: &Path,
    force: bool,
) -> InstallPlan {
    let mut actions = Vec::new();
    let mut errors = Vec::new();

    let workspace_root = match canonicalize_workspace(workspace) {
        Ok(p) => p,
        Err(e) => {
            return InstallPlan {
                name: entry.name.clone(),
                actions,
                errors: vec![e.to_string()],
            };
        }
    };

    for file in &entry.files {
        let source_rel = match validate_relative_path(&file.source) {
            Ok(p) => p,
            Err(e) => {
                errors.push(e.to_string());
                continue;
            }
        };
        let dest_rel = match validate_relative_path(&file.dest) {
            Ok(p) => p,
            Err(e) => {
                errors.push(e.to_string());
                continue;
            }
        };

        // Source must stay under entry_dir (no symlink escape from registry either).
        let source = match resolve_source_under_entry(entry_dir, &source_rel) {
            Ok(p) => p,
            Err(e) => {
                errors.push(e.to_string());
                continue;
            }
        };

        let dest = match resolve_dest_under_workspace(&workspace_root, &dest_rel) {
            Ok(p) => p,
            Err(e) => {
                errors.push(format!("{}: {e}", file.dest));
                continue;
            }
        };

        let bytes = match fs::read(&source) {
            Ok(b) => b,
            Err(e) => {
                errors.push(format!("read {}: {e}", source.display()));
                continue;
            }
        };
        let digest = sha256_hex(&bytes);
        if digest != file.sha256 {
            errors.push(format!(
                "hash mismatch for {}: expected {}, got {}",
                file.source, file.sha256, digest
            ));
            continue;
        }

        if dest.exists()
            || dest
                .symlink_metadata()
                .map(|m| m.is_file() || m.is_symlink())
                .unwrap_or(false)
        {
            // Refuse writing through a dest that is itself a symlink out (already checked).
            match fs::read(&dest) {
                Ok(existing) => {
                    let existing_hash = sha256_hex(&existing);
                    if existing_hash == file.sha256 {
                        actions.push(PlanAction::Unchanged {
                            dest: dest_rel.clone(),
                        });
                    } else if force {
                        actions.push(PlanAction::Create {
                            dest: dest_rel.clone(),
                            source: source.clone(),
                            sha256: digest,
                            force_overwrite: true,
                        });
                    } else {
                        actions.push(PlanAction::Conflict {
                            dest: dest_rel.clone(),
                            reason: "destination differs from upstream (dirty or foreign); refuse silent overwrite".into(),
                        });
                    }
                }
                Err(e) => {
                    // Symlink to missing target, or directory — refuse.
                    errors.push(format!("cannot install over {}: {e}", dest.display()));
                }
            }
        } else {
            actions.push(PlanAction::Create {
                dest: dest_rel,
                source,
                sha256: digest,
                force_overwrite: false,
            });
        }
    }
    InstallPlan {
        name: entry.name.clone(),
        actions,
        errors,
    }
}

fn resolve_source_under_entry(entry_dir: &Path, rel: &Path) -> Result<PathBuf, RegistryError> {
    let entry_root = fs::canonicalize(entry_dir).map_err(|e| {
        RegistryError::Path(format!(
            "canonicalize entry dir {}: {e}",
            entry_dir.display()
        ))
    })?;
    let joined = entry_root.join(rel);
    let real = fs::canonicalize(&joined).map_err(|e| {
        RegistryError::Path(format!("canonicalize source {}: {e}", joined.display()))
    })?;
    if !real.starts_with(&entry_root) {
        return Err(RegistryError::Path(format!(
            "source escapes entry dir: {}",
            rel.display()
        )));
    }
    Ok(real)
}

/// Apply plan when no errors and no conflicts.
///
/// Force overwrites write a sibling `*.termrock.bak` of the previous content first.
pub fn apply_plan(
    plan: &InstallPlan,
    workspace: &Path,
    entry: &RegistryEntry,
    manifest_path: &Path,
) -> Result<(), RegistryError> {
    if !plan.errors.is_empty() {
        return Err(RegistryError::Schema(plan.errors.join("; ")));
    }
    if plan
        .actions
        .iter()
        .any(|a| matches!(a, PlanAction::Conflict { .. }))
    {
        return Err(RegistryError::Path(
            "conflicts present; re-run with --force only after review".into(),
        ));
    }

    let workspace_root = canonicalize_workspace(workspace)?;

    let mut record = InstallRecord {
        name: entry.name.clone(),
        version: entry.version.clone(),
        files: Vec::new(),
    };
    for action in &plan.actions {
        match action {
            PlanAction::Create {
                dest,
                source,
                sha256,
                force_overwrite,
            } => {
                let full = resolve_dest_under_workspace(&workspace_root, dest)?;
                if let Some(parent) = full.parent() {
                    // Create parents only if each step stays under workspace.
                    ensure_parents_under_workspace(&workspace_root, parent)?;
                }
                if *force_overwrite && full.exists() {
                    let bak = force_backup_path(&full);
                    fs::copy(&full, &bak).map_err(|e| {
                        RegistryError::Io(format!(
                            "backup {} -> {}: {e}",
                            full.display(),
                            bak.display()
                        ))
                    })?;
                }
                let tmp = full.with_extension("termrock.tmp");
                // Ensure tmp also under workspace
                if !tmp.starts_with(&workspace_root) && !tmp.starts_with(workspace) {
                    // lexical check — prefer resolving parent
                    if let Some(parent) = tmp.parent() {
                        if !parent.starts_with(&workspace_root) {
                            return Err(RegistryError::Path(format!(
                                "temp path escapes workspace: {}",
                                tmp.display()
                            )));
                        }
                    }
                }
                fs::copy(source, &tmp).map_err(|e| RegistryError::Io(e.to_string()))?;
                // Atomic replace within same directory when possible.
                fs::rename(&tmp, &full).map_err(|e| RegistryError::Io(e.to_string()))?;
                // Post-write: ensure file still under workspace (no TOCTOU via parent swap).
                let written = fs::canonicalize(&full).map_err(|e| {
                    RegistryError::Path(format!("post-write canonicalize {}: {e}", full.display()))
                })?;
                if !written.starts_with(&workspace_root) {
                    let _ = fs::remove_file(&written);
                    return Err(RegistryError::Path(format!(
                        "write escaped workspace: {}",
                        written.display()
                    )));
                }
                record.files.push(InstalledFile {
                    dest: dest.to_string_lossy().into_owned(),
                    upstream_sha256: sha256.clone(),
                });
            }
            PlanAction::Unchanged { dest } => {
                record.files.push(InstalledFile {
                    dest: dest.to_string_lossy().into_owned(),
                    upstream_sha256: entry
                        .files
                        .iter()
                        .find(|f| Path::new(&f.dest) == dest.as_path())
                        .map(|f| f.sha256.clone())
                        .unwrap_or_default(),
                });
            }
            PlanAction::Conflict { .. } => unreachable!(),
        }
    }
    let mut manifest = if manifest_path.exists() {
        let raw =
            fs::read_to_string(manifest_path).map_err(|e| RegistryError::Io(e.to_string()))?;
        if raw.trim().is_empty() {
            InstallManifest {
                schema: SCHEMA_VERSION,
                installed: BTreeMap::new(),
            }
        } else {
            serde_json::from_str(&raw).map_err(|e| RegistryError::Schema(e.to_string()))?
        }
    } else {
        InstallManifest {
            schema: SCHEMA_VERSION,
            installed: BTreeMap::new(),
        }
    };
    manifest.schema = SCHEMA_VERSION;
    manifest.installed.insert(entry.name.clone(), record);
    let out = serde_json::to_string_pretty(&manifest)
        .map_err(|e| RegistryError::Schema(e.to_string()))?;
    // Manifest must also land under workspace when possible.
    if let Some(man_parent) = manifest_path
        .parent()
        .and_then(|p| fs::canonicalize(p).ok())
        && !man_parent.starts_with(&workspace_root)
    {
        return Err(RegistryError::Path(
            "manifest path escapes workspace".into(),
        ));
    }
    // Atomic-ish manifest write.
    let man_tmp = manifest_path.with_extension("json.tmp");
    fs::write(&man_tmp, &out).map_err(|e| RegistryError::Io(e.to_string()))?;
    fs::rename(&man_tmp, manifest_path).map_err(|e| RegistryError::Io(e.to_string()))?;
    Ok(())
}

fn force_backup_path(full: &Path) -> PathBuf {
    let mut s = full.as_os_str().to_os_string();
    s.push(FORCE_BACKUP_SUFFIX);
    PathBuf::from(s)
}

fn ensure_parents_under_workspace(
    workspace_root: &Path,
    parent: &Path,
) -> Result<(), RegistryError> {
    // Create from workspace outward, refusing symlink escapes.
    if parent.starts_with(workspace_root) && !parent.exists() {
        // Build relative from root
        let rel = parent
            .strip_prefix(workspace_root)
            .map_err(|_| RegistryError::Path("parent not under workspace".into()))?;
        let mut cur = workspace_root.to_path_buf();
        for c in rel.components() {
            let Component::Normal(name) = c else {
                return Err(RegistryError::Path("illegal parent component".into()));
            };
            cur = cur.join(name);
            if cur.exists() || cur.symlink_metadata().is_ok() {
                let meta = fs::symlink_metadata(&cur)
                    .map_err(|e| RegistryError::Path(format!("stat {}: {e}", cur.display())))?;
                if meta.file_type().is_symlink() {
                    let real = fs::canonicalize(&cur).map_err(|e| {
                        RegistryError::Path(format!("symlink {}: {e}", cur.display()))
                    })?;
                    if !real.starts_with(workspace_root) {
                        return Err(RegistryError::Path(format!(
                            "parent symlink escapes workspace: {}",
                            cur.display()
                        )));
                    }
                    cur = real;
                }
            } else {
                fs::create_dir(&cur)
                    .map_err(|e| RegistryError::Io(format!("mkdir {}: {e}", cur.display())))?;
            }
        }
        return Ok(());
    }
    fs::create_dir_all(parent).map_err(|e| RegistryError::Io(e.to_string()))
}

/// Diff installed files vs upstream entry digests.
pub fn diff_installed(
    entry: &RegistryEntry,
    workspace: &Path,
    manifest: &InstallManifest,
) -> Vec<String> {
    let mut report = Vec::new();
    let Some(rec) = manifest.installed.get(&entry.name) else {
        report.push(format!("{}: not installed", entry.name));
        return report;
    };
    let Ok(workspace_root) = canonicalize_workspace(workspace) else {
        report.push(format!("{}: workspace unreadable", entry.name));
        return report;
    };
    for file in &entry.files {
        let Ok(dest_rel) = validate_relative_path(&file.dest) else {
            report.push(format!("{}: invalid dest", file.dest));
            continue;
        };
        let dest = match resolve_dest_under_workspace(&workspace_root, &dest_rel) {
            Ok(p) => p,
            Err(e) => {
                report.push(format!("{}: {e}", file.dest));
                continue;
            }
        };
        if !dest.exists() {
            report.push(format!("{}: missing", file.dest));
            continue;
        }
        let Ok(bytes) = fs::read(&dest) else {
            report.push(format!("{}: unreadable", file.dest));
            continue;
        };
        let hash = sha256_hex(&bytes);
        if hash != file.sha256 {
            let upstream = rec
                .files
                .iter()
                .find(|f| f.dest == file.dest)
                .map(|f| f.upstream_sha256.as_str())
                .unwrap_or("?");
            report.push(format!(
                "{}: local dirty (local={hash}, upstream_install={upstream}, registry={})",
                file.dest, file.sha256
            ));
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::fs::symlink;

    fn write_fixture(root: &Path, name: &str, dest: &str, body: &[u8]) -> PathBuf {
        let entry_dir = root.join("registry").join(name);
        fs::create_dir_all(entry_dir.join("src")).unwrap();
        fs::write(entry_dir.join("src/lib.rs"), body).unwrap();
        let digest = sha256_hex(body);
        let entry = RegistryEntry {
            schema: 1,
            name: name.into(),
            version: "0.1.0".into(),
            description: "fixture".into(),
            license: "Apache-2.0".into(),
            kernel: "0.11.0".into(),
            files: vec![RegistryFile {
                source: "src/lib.rs".into(),
                dest: dest.into(),
                sha256: digest,
            }],
        };
        let json = serde_json::to_string_pretty(&entry).unwrap();
        fs::write(entry_dir.join("entry.json"), json).unwrap();
        entry_dir
    }

    fn tempfile_dir() -> PathBuf {
        let mut d = std::env::temp_dir();
        d.push(format!(
            "termrock-cli-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn rejects_path_escape() {
        assert!(validate_relative_path("../etc/passwd").is_err());
        assert!(validate_relative_path("/abs").is_err());
        assert!(validate_relative_path("ok/file.rs").is_ok());
    }

    #[test]
    fn plan_add_and_dirty_conflict() {
        let tmp = tempfile_dir();
        let entry_dir = write_fixture(
            &tmp,
            "demo-block",
            "src/ui/demo_block.rs",
            b"// demo block\npub fn hello() {}\n",
        );
        let entry = load_entry(&entry_dir).unwrap();
        let workspace = tmp.join("app");
        fs::create_dir_all(&workspace).unwrap();
        let plan = plan_install(&entry, &entry_dir, &workspace, false);
        assert!(plan.errors.is_empty(), "errors: {:?}", plan.errors);
        assert!(matches!(
            plan.actions[0],
            PlanAction::Create {
                force_overwrite: false,
                ..
            }
        ));
        apply_plan(
            &plan,
            &workspace,
            &entry,
            &workspace.join("termrock.lock.json"),
        )
        .unwrap();
        let mut f = fs::OpenOptions::new()
            .append(true)
            .open(workspace.join("src/ui/demo_block.rs"))
            .unwrap();
        writeln!(f, "// local edit").unwrap();
        let plan2 = plan_install(&entry, &entry_dir, &workspace, false);
        assert!(matches!(plan2.actions[0], PlanAction::Conflict { .. }));
        let manifest: InstallManifest = serde_json::from_str(
            &fs::read_to_string(workspace.join("termrock.lock.json")).unwrap(),
        )
        .unwrap();
        let report = diff_installed(&entry, &workspace, &manifest);
        assert!(report.iter().any(|l| l.contains("dirty")));
    }

    #[test]
    fn force_overwrite_writes_backup() {
        let tmp = tempfile_dir();
        let entry_dir = write_fixture(
            &tmp,
            "demo-block",
            "src/ui/demo_block.rs",
            b"// upstream v1\n",
        );
        let entry = load_entry(&entry_dir).unwrap();
        let workspace = tmp.join("app");
        fs::create_dir_all(&workspace).unwrap();
        let plan = plan_install(&entry, &entry_dir, &workspace, false);
        apply_plan(
            &plan,
            &workspace,
            &entry,
            &workspace.join("termrock.lock.json"),
        )
        .unwrap();
        // Change upstream package content + hash in a new entry version.
        let body2 = b"// upstream v2\n";
        fs::write(entry_dir.join("src/lib.rs"), body2).unwrap();
        let mut entry2 = entry.clone();
        entry2.files[0].sha256 = sha256_hex(body2);
        // Dirty local already matches old - make local dirty differently
        fs::write(workspace.join("src/ui/demo_block.rs"), b"// local dirty\n").unwrap();
        let plan_force = plan_install(&entry2, &entry_dir, &workspace, true);
        assert!(plan_force.errors.is_empty(), "{:?}", plan_force.errors);
        assert!(matches!(
            plan_force.actions[0],
            PlanAction::Create {
                force_overwrite: true,
                ..
            }
        ));
        apply_plan(
            &plan_force,
            &workspace,
            &entry2,
            &workspace.join("termrock.lock.json"),
        )
        .unwrap();
        let bak = workspace.join(format!("src/ui/demo_block.rs{FORCE_BACKUP_SUFFIX}"));
        assert!(bak.exists(), "backup missing at {}", bak.display());
        assert_eq!(fs::read(&bak).unwrap(), b"// local dirty\n");
        assert_eq!(
            fs::read(workspace.join("src/ui/demo_block.rs")).unwrap(),
            body2
        );
    }

    #[test]
    fn refuses_symlink_escape_dest() {
        let tmp = tempfile_dir();
        let outside = tmp.join("outside");
        fs::create_dir_all(&outside).unwrap();
        let workspace = tmp.join("app");
        fs::create_dir_all(&workspace).unwrap();
        // Symlink inside workspace pointing outside.
        let link = workspace.join("link");
        symlink(&outside, &link).unwrap();

        let entry_dir = write_fixture(&tmp, "evil", "link/out.rs", b"// should not land outside\n");
        let entry = load_entry(&entry_dir).unwrap();
        let plan = plan_install(&entry, &entry_dir, &workspace, false);
        assert!(
            !plan.errors.is_empty(),
            "expected symlink escape errors, actions={:?}",
            plan.actions
        );
        assert!(
            plan.errors
                .iter()
                .any(|e| e.contains("symlink") || e.contains("escape")),
            "errors={:?}",
            plan.errors
        );
        // Outside must stay empty of out.rs
        assert!(!outside.join("out.rs").exists());
    }

    #[test]
    fn two_offline_entries_install() {
        let tmp = tempfile_dir();
        let a = write_fixture(
            &tmp,
            "demo-block",
            "src/ui/demo_block.rs",
            b"pub fn a() {}\n",
        );
        let b = write_fixture(
            &tmp,
            "tiny-component",
            "src/ui/tiny_component.rs",
            b"pub fn b() {}\n",
        );
        let workspace = tmp.join("app");
        fs::create_dir_all(&workspace).unwrap();
        for entry_dir in [a, b] {
            let entry = load_entry(&entry_dir).unwrap();
            let plan = plan_install(&entry, &entry_dir, &workspace, false);
            assert!(plan.errors.is_empty(), "{:?}", plan.errors);
            apply_plan(
                &plan,
                &workspace,
                &entry,
                &workspace.join("termrock.lock.json"),
            )
            .unwrap();
        }
        assert!(workspace.join("src/ui/demo_block.rs").exists());
        assert!(workspace.join("src/ui/tiny_component.rs").exists());
        let man: InstallManifest = serde_json::from_str(
            &fs::read_to_string(workspace.join("termrock.lock.json")).unwrap(),
        )
        .unwrap();
        assert!(man.installed.contains_key("demo-block"));
        assert!(man.installed.contains_key("tiny-component"));
    }
}
