// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Offline registry planner and installer (Plan 047).
//!
//! **Trust:** registry content is untrusted. Destination workspace is user-owned.
//! No shell/template execution. No network in the core resolver.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Schema version for registry entries.
pub const SCHEMA_VERSION: u32 = 1;

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
    /// Create new file.
    Create {
        /// Dest.
        dest: PathBuf,
        /// Source absolute.
        source: PathBuf,
        /// Digest.
        sha256: String,
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

/// Reject absolute paths, `..`, and empty components.
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
        let source = entry_dir.join(&source_rel);
        let dest = workspace.join(&dest_rel);
        // Ensure dest stays under workspace
        if !dest.starts_with(workspace) {
            errors.push(format!("path escape: {}", file.dest));
            continue;
        }
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
        if dest.exists() {
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
                        });
                    } else {
                        actions.push(PlanAction::Conflict {
                            dest: dest_rel.clone(),
                            reason: "destination differs from upstream (dirty or foreign); refuse silent overwrite".into(),
                        });
                    }
                }
                Err(e) => errors.push(format!("stat {}: {e}", dest.display())),
            }
        } else {
            actions.push(PlanAction::Create {
                dest: dest_rel,
                source,
                sha256: digest,
            });
        }
    }
    InstallPlan {
        name: entry.name.clone(),
        actions,
        errors,
    }
}

/// Apply plan when no errors and no conflicts (unless force already resolved).
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
            } => {
                let full = workspace.join(dest);
                if let Some(parent) = full.parent() {
                    fs::create_dir_all(parent).map_err(|e| RegistryError::Io(e.to_string()))?;
                }
                let tmp = full.with_extension("termrock.tmp");
                fs::copy(source, &tmp).map_err(|e| RegistryError::Io(e.to_string()))?;
                fs::rename(&tmp, &full).map_err(|e| RegistryError::Io(e.to_string()))?;
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
        serde_json::from_str(&raw).map_err(|e| RegistryError::Schema(e.to_string()))?
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
    fs::write(manifest_path, out).map_err(|e| RegistryError::Io(e.to_string()))?;
    Ok(())
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
    for file in &entry.files {
        let dest = workspace.join(&file.dest);
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

    fn write_fixture(root: &Path) -> PathBuf {
        let entry_dir = root.join("registry/demo-block");
        fs::create_dir_all(entry_dir.join("src")).unwrap();
        let body = b"// demo block\npub fn hello() {}\n";
        fs::write(entry_dir.join("src/lib.rs"), body).unwrap();
        let digest = sha256_hex(body);
        let entry = RegistryEntry {
            schema: 1,
            name: "demo-block".into(),
            version: "0.1.0".into(),
            description: "fixture".into(),
            license: "Apache-2.0".into(),
            kernel: "0.11.0".into(),
            files: vec![RegistryFile {
                source: "src/lib.rs".into(),
                dest: "src/ui/demo_block.rs".into(),
                sha256: digest,
            }],
        };
        let json = serde_json::to_string_pretty(&entry).unwrap();
        fs::write(entry_dir.join("entry.json"), json).unwrap();
        entry_dir
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
        let entry_dir = write_fixture(&tmp);
        let entry = load_entry(&entry_dir).unwrap();
        let workspace = tmp.join("app");
        fs::create_dir_all(&workspace).unwrap();
        let plan = plan_install(&entry, &entry_dir, &workspace, false);
        assert!(plan.errors.is_empty());
        assert!(matches!(plan.actions[0], PlanAction::Create { .. }));
        apply_plan(
            &plan,
            &workspace,
            &entry,
            &workspace.join("termrock.lock.json"),
        )
        .unwrap();
        // dirty
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
}
