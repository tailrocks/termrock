//! Guards the `default_branch` source setting of `.velnor/rust-ci.toml`, which
//! the generated Rust CI derives its push filter and default-branch cache
//! rules from. velnor-gen defaults it to `main` when the key is absent, and
//! the helper fails the run when it differs from the repository's default
//! branch. The check is pure Rust over config text, parsed with the `toml`
//! crate; no subprocess and no network.
//!
//! The same file pins the velnor-gen identity: `.velnor/rust-ci.lock` records the
//! release that `velnor-gen pin-helper` resolved. `velnor-gen check` compares the
//! `[velnor-gen]` digest with the running binary on Linux, but it never compares the
//! lock with `mise.toml`, so the tests below tie `version` and `commit` to the mise pins.

use std::fmt;

#[derive(Debug, PartialEq, Eq)]
enum DefaultBranchError {
    Mismatch {
        configured: String,
        expected: String,
    },
    Invalid(String),
}

impl fmt::Display for DefaultBranchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mismatch {
                configured,
                expected,
            } => write!(
                f,
                "default_branch = \"{configured}\"; expected \"{expected}\""
            ),
            Self::Invalid(reason) => f.write_str(reason),
        }
    }
}

const DEFAULT_BRANCH: &str = "main";

fn check_default_branch(config_text: &str, expected: &str) -> Result<(), DefaultBranchError> {
    if expected.is_empty() {
        return Err(DefaultBranchError::Invalid(
            "expected default branch must be nonempty".to_owned(),
        ));
    }
    let configured = configured_default_branch(config_text)?;
    if configured == expected {
        Ok(())
    } else {
        Err(DefaultBranchError::Mismatch {
            configured,
            expected: expected.to_owned(),
        })
    }
}

/// Reads the top-level `default_branch` of a parsed TOML document, so quoting,
/// escapes, duplicate keys and table shapes are decided by the TOML parser
/// rather than by this test. An absent key is velnor-gen's default, `main`.
fn configured_default_branch(config_text: &str) -> Result<String, DefaultBranchError> {
    let document: toml::Table = config_text.parse().map_err(|error| {
        DefaultBranchError::Invalid(format!("config is not valid TOML: {error}"))
    })?;
    let Some(value) = document.get("default_branch") else {
        return Ok(DEFAULT_BRANCH.to_owned());
    };
    value
        .as_str()
        .filter(|branch| !branch.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            DefaultBranchError::Invalid("default_branch must be a nonempty string".to_owned())
        })
}

/// The branches of the `push` trigger of the generated workflow text.
fn push_branches(workflow: &str) -> Vec<&str> {
    let mut lines = workflow.lines();
    lines.by_ref().find(|line| *line == "  push:");
    let mut branches = Vec::new();
    let mut listing = false;
    for line in lines {
        if line == "    branches:" {
            listing = true;
        } else if listing && let Some(branch) = line.strip_prefix("      - ") {
            branches.push(branch);
        } else if listing || !line.starts_with("    ") {
            break;
        }
    }
    branches
}

#[test]
fn default_branch_accepts_match() {
    let config = "default_branch = \"main\"\n";
    assert_eq!(check_default_branch(config, "main"), Ok(()));
}

#[test]
fn default_branch_defaults_to_main_when_absent() {
    assert_eq!(check_default_branch("runner = \"x\"\n", "main"), Ok(()));
    let error = check_default_branch("runner = \"x\"\n", "trunk").expect_err("main is not trunk");
    assert!(
        matches!(error, DefaultBranchError::Mismatch { .. }),
        "{error}"
    );
}

#[test]
fn default_branch_rejects_stale_source() {
    let config = "default_branch = \"design/interaction-polish\"\n";
    let error = check_default_branch(config, "main").expect_err("stale branch must fail");
    assert_eq!(
        error,
        DefaultBranchError::Mismatch {
            configured: "design/interaction-polish".to_owned(),
            expected: "main".to_owned(),
        }
    );
    let message = error.to_string();
    assert!(message.contains("design/interaction-polish"), "{message}");
    assert!(message.contains("main"), "{message}");
}

#[test]
fn default_branch_supports_fork_name() {
    let config = "default_branch = 'trunk' # fork default\n";
    assert_eq!(check_default_branch(config, "trunk"), Ok(()));
}

#[test]
fn default_branch_reads_escapes_like_toml_does() {
    let escaped = "default_branch = \"ma\\u0069n\"\n";
    assert_eq!(check_default_branch(escaped, "main"), Ok(()));
}

#[test]
fn default_branch_rejects_wrong_toml_shapes() {
    let shapes = [
        ("branch integer", "default_branch = 1\n"),
        ("branch empty", "default_branch = \"\"\n"),
        ("branch unquoted", "default_branch = main\n"),
        (
            "duplicate key",
            "default_branch = \"main\"\ndefault_branch = \"main\"\n",
        ),
    ];
    for (name, body) in shapes {
        let error = check_default_branch(body, "main").expect_err(name);
        assert!(
            matches!(error, DefaultBranchError::Invalid(_)),
            "{name}: {error}"
        );
    }
}

#[test]
fn push_branches_reads_the_push_trigger() {
    let workflow = "on:\n  pull_request:\n    branches:\n      - other\n  push:\n    branches:\n      - main\n  merge_group: {}\n";
    assert_eq!(push_branches(workflow), ["main"]);
    assert!(push_branches("on:\n  merge_group: {}\n").is_empty());
}

#[test]
fn repository_config_default_branch_is_main() {
    let config = include_str!("../../../../.velnor/rust-ci.toml");
    assert_eq!(check_default_branch(config, "main"), Ok(()));
}

#[test]
fn generated_workflow_pushes_only_the_default_branch() {
    let workflow = include_str!("../../../../.github/workflows/rust-ci.yml");
    assert_eq!(push_branches(workflow), ["main"]);
}

const GEN_TAG_PREFIX: &str = "tools-v";
const GEN_RELEASE_TOOL: &str = "github:tailrocks/velnor-new";
const GEN_SOURCE_TOOL: &str = "cargo:https://github.com/tailrocks/velnor-new";

fn toml_table(text: &str, what: &str) -> toml::Table {
    text.parse()
        .unwrap_or_else(|error| panic!("{what} is not valid TOML: {error}"))
}

/// The version string of one pin of `[tools]` in `mise.toml`.
fn mise_pin_version(mise_toml: &str, tool: &str) -> Result<String, String> {
    let document = toml_table(mise_toml, "mise.toml");
    let pin = document
        .get("tools")
        .and_then(|tools| tools.get(tool))
        .ok_or_else(|| format!("mise.toml pins no `{tool}`"))?;
    pin.get("version")
        .unwrap_or(pin)
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("mise.toml pin of `{tool}` has no version string"))
}

/// What `.velnor/rust-ci.lock` must agree with: the `tools-v` tag of the Linux
/// asset that mise installs and the commit that macOS builds. The Linux digest
/// is the digest of the extracted binary (`BINARY-SHA256SUMS`), which
/// `mise.lock` does not record (it holds the archive digest), so only
/// `velnor-gen check` on Linux compares it. Returns every disagreement.
fn check_velnor_gen_pin(rust_ci_lock: &str, mise_toml: &str) -> Vec<String> {
    let lock = toml_table(rust_ci_lock, ".velnor/rust-ci.lock");
    let text = |value: Option<&toml::Value>| value.and_then(toml::Value::as_str).map(str::to_owned);
    let mut problems = Vec::new();
    if text(lock.get("velnor-gen").and_then(|t| t.get("linux-x64"))).is_none() {
        problems.push(".velnor/rust-ci.lock has no [velnor-gen].linux-x64".to_owned());
    }
    let version = text(lock.get("version"));
    match (mise_pin_version(mise_toml, GEN_RELEASE_TOOL), version) {
        (Ok(pin), Some(version)) if pin.strip_prefix(GEN_TAG_PREFIX) == Some(version.as_str()) => {}
        (Ok(pin), version) => problems.push(format!(
            "mise.toml pins {GEN_RELEASE_TOOL} at `{pin}`, but .velnor/rust-ci.lock version is {version:?} (expected `{GEN_TAG_PREFIX}<version>`)"
        )),
        (Err(why), _) => problems.push(why),
    }
    let commit = text(lock.get("commit"));
    match (mise_pin_version(mise_toml, GEN_SOURCE_TOOL), commit) {
        (Ok(pin), Some(commit)) if pin.strip_prefix("rev:") == Some(commit.as_str()) => {}
        (Ok(pin), commit) => problems.push(format!(
            "mise.toml builds {GEN_SOURCE_TOOL} at `{pin}`, but .velnor/rust-ci.lock commit is {commit:?} (expected `rev:<commit>`)"
        )),
        (Err(why), _) => problems.push(why),
    }
    problems
}

const SAMPLE_COMMIT: &str = "1f9762b239202d138ceec484da17fa5b27f2c3a3";
const SAMPLE_SUM: &str = "222f7d078b251d74677150a2abc688357fa186ec7e4a820d0cadcf7836e74b9a";

fn sample_files() -> (String, String) {
    (
        format!(
            "version = \"0.1.106\"\ncommit = \"{SAMPLE_COMMIT}\"\n\n[velnor-gen]\nlinux-x64 = \"{SAMPLE_SUM}\"\n"
        ),
        format!(
            "[tools]\n\"{GEN_RELEASE_TOOL}\" = {{ version = \"tools-v0.1.106\", os = [\"linux\"] }}\n\"{GEN_SOURCE_TOOL}\" = {{ version = \"rev:{SAMPLE_COMMIT}\", crate = \"velnor-gen\" }}\n"
        ),
    )
}

#[test]
fn velnor_gen_pin_agrees_across_the_lock_and_mise() {
    let (lock, toml) = sample_files();
    assert_eq!(check_velnor_gen_pin(&lock, &toml), Vec::<String>::new());
}

#[test]
fn velnor_gen_pin_rejects_a_missing_digest_or_a_tampered_version_or_commit() {
    let (lock, toml) = sample_files();
    let no_digest = lock.replace("[velnor-gen]", "[other]");
    let problems = check_velnor_gen_pin(&no_digest, &toml);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("linux-x64"), "{problems:?}");

    let tampered_version = lock.replace("version = \"0.1.106\"", "version = \"0.1.107\"");
    let problems = check_velnor_gen_pin(&tampered_version, &toml);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("version"), "{problems:?}");

    let other = "0".repeat(40);
    let tampered_commit = lock.replace(SAMPLE_COMMIT, &other);
    let problems = check_velnor_gen_pin(&tampered_commit, &toml);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("commit"), "{problems:?}");

    let moving_tag = toml.replace(&format!("rev:{SAMPLE_COMMIT}"), "tag:tools-v0.1.106");
    assert_eq!(check_velnor_gen_pin(&lock, &moving_tag).len(), 1);
}

#[test]
fn repository_velnor_gen_pin_agrees_across_the_lock_and_mise() {
    let problems = check_velnor_gen_pin(
        include_str!("../../../../.velnor/rust-ci.lock"),
        include_str!("../../../../mise.toml"),
    );
    assert!(problems.is_empty(), "{problems:#?}");
}

/// `velnor-gen`'s `Check <crate> alone` steps are default-feature `cargo check`
/// runs that allow warnings, so the per-crate `--all-features` build of the
/// old CI lives in the `crates-alone` lint (mise task `check-crates-alone`).
/// It must stay configured, must deny warnings for both feature shapes and must
/// be one of the jobs that `Required` waits for.
#[test]
fn the_crates_alone_lint_is_configured_and_required() {
    let config = toml_table(
        include_str!("../../../../.velnor/rust-ci.toml"),
        ".velnor/rust-ci.toml",
    );
    let lint = config
        .get("lint")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .find(|lint| lint.get("id").and_then(toml::Value::as_str) == Some("crates-alone"))
        .expect("`.velnor/rust-ci.toml` has no `[[lint]]` with id = \"crates-alone\"");
    assert_eq!(
        lint.get("task").and_then(toml::Value::as_str),
        Some("check-crates-alone")
    );

    let mise = toml_table(include_str!("../../../../mise.toml"), "mise.toml");
    let run = mise
        .get("tasks")
        .and_then(|tasks| tasks.get("check-crates-alone"))
        .and_then(|task| task.get("run"))
        .and_then(toml::Value::as_str)
        .expect("mise.toml has no task `check-crates-alone`");
    assert!(run.contains("--package"), "{run}");
    assert!(
        run.contains("--all-targets --all-features -- -D warnings"),
        "{run}"
    );
    assert!(
        run.lines()
            .any(|line| line.contains("--all-targets -- -D warnings")),
        "the default-feature pass is missing: {run}"
    );

    for package in ["termrock", "junie-tui"] {
        assert!(
            run.contains(&format!("for package in {package} "))
                || run.contains(&format!(" {package}; do")),
            "the terminal-without-cli pass does not name `{package}`: {run}"
        );
    }
    assert!(
        run.lines().any(|line| line
            .contains("--all-targets --no-default-features --features terminal -- -D warnings")),
        "the terminal-without-cli (former `product`) pass is missing: {run}"
    );

    let workflow = include_str!("../../../../.github/workflows/rust-ci.yml");
    assert!(workflow.contains("\n  lint-crates-alone:\n"));
    assert!(
        workflow.contains("\n      - lint-crates-alone\n"),
        "`Required` does not wait for lint-crates-alone"
    );
}
