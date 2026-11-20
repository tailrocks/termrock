//! Guards the `[workflow].default_branch` source setting that generated CI
//! push filters and follow-up `head_branch` predicates are derived from.
//! The check is pure Rust over config text, parsed with the `toml` crate; no
//! subprocess and no network.

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
                "workflow.default_branch = \"{configured}\"; expected \"{expected}\""
            ),
            Self::Invalid(reason) => f.write_str(reason),
        }
    }
}

fn check_default_branch(config_text: &str, expected: &str) -> Result<(), DefaultBranchError> {
    if expected.is_empty() {
        return Err(DefaultBranchError::Invalid(
            "expected default branch must be nonempty".to_owned(),
        ));
    }
    let configured = workflow_default_branch(config_text)?;
    if configured == expected {
        Ok(())
    } else {
        Err(DefaultBranchError::Mismatch {
            configured,
            expected: expected.to_owned(),
        })
    }
}

/// Reads `default_branch` from the `[workflow]` table of a parsed TOML
/// document, so quoting, escapes, duplicate keys and table shapes are decided
/// by the TOML parser rather than by this test.
fn workflow_default_branch(config_text: &str) -> Result<String, DefaultBranchError> {
    let document: toml::Table = config_text.parse().map_err(|error| {
        DefaultBranchError::Invalid(format!("config is not valid TOML: {error}"))
    })?;
    let workflow = document
        .get("workflow")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| {
            DefaultBranchError::Invalid("workflow must be a [workflow] table".to_owned())
        })?;
    let value = workflow.get("default_branch").ok_or_else(|| {
        DefaultBranchError::Invalid("[workflow] is missing default_branch".to_owned())
    })?;
    value
        .as_str()
        .filter(|branch| !branch.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            DefaultBranchError::Invalid(
                "workflow.default_branch must be a nonempty string".to_owned(),
            )
        })
}

#[test]
fn default_branch_accepts_match() {
    let config = "[workflow]\ndefault_branch = \"main\"\n";
    assert_eq!(check_default_branch(config, "main"), Ok(()));
}

#[test]
fn default_branch_rejects_stale_source() {
    let config = "[workflow]\ndefault_branch = \"design/interaction-polish\"\n";
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
    let config = "[workflow]\ndefault_branch = 'trunk' # fork default\n";
    assert_eq!(check_default_branch(config, "trunk"), Ok(()));
}

#[test]
fn default_branch_reads_escapes_and_inline_tables_like_toml_does() {
    let escaped = "[workflow]\ndefault_branch = \"ma\\u0069n\"\n";
    assert_eq!(check_default_branch(escaped, "main"), Ok(()));
    let inline = "workflow = { default_branch = \"main\" }\n";
    assert_eq!(check_default_branch(inline, "main"), Ok(()));
    let dotted = "workflow.default_branch = \"main\"\n";
    assert_eq!(check_default_branch(dotted, "main"), Ok(()));
}

#[test]
fn default_branch_rejects_missing_value() {
    let config = "[workflow]\npolicy = \"consumer-v1\"\n";
    let error = check_default_branch(config, "main").expect_err("missing key must fail");
    assert!(
        matches!(&error, DefaultBranchError::Invalid(reason) if reason.contains("default_branch")),
        "{error}"
    );
}

#[test]
fn default_branch_rejects_wrong_toml_shapes() {
    let shapes = [
        ("workflow scalar", "workflow = 1\n"),
        ("workflow list", "workflow = []\n"),
        ("branch integer", "[workflow]\ndefault_branch = 1\n"),
        ("branch unquoted", "[workflow]\ndefault_branch = main\n"),
        (
            "value in other table",
            "[other]\ndefault_branch = \"main\"\n",
        ),
        (
            "duplicate key",
            "[workflow]\ndefault_branch = \"main\"\ndefault_branch = \"main\"\n",
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
fn repository_config_default_branch_is_main() {
    let config = include_str!("../../../../.velnor/config.toml");
    assert_eq!(check_default_branch(config, "main"), Ok(()));
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

/// The checksum (without `sha256:`) that `mise.lock` records for the Linux
/// release asset of velnor-gen.
fn mise_lock_linux_checksum(mise_lock: &str) -> Result<String, String> {
    let document = toml_table(mise_lock, "mise.lock");
    document
        .get("tools")
        .and_then(|tools| tools.get(GEN_RELEASE_TOOL))
        .and_then(toml::Value::as_array)
        .and_then(|rows| rows.first())
        .and_then(|row| row.get("platforms.linux-x64"))
        .and_then(|platform| platform.get("checksum"))
        .and_then(toml::Value::as_str)
        .map(|sum| sum.strip_prefix("sha256:").unwrap_or(sum).to_owned())
        .ok_or_else(|| format!("mise.lock has no linux-x64 checksum for `{GEN_RELEASE_TOOL}`"))
}

/// What `.velnor/rust-ci.lock` must agree with: the Linux asset digest that
/// mise installs, the `tools-v` tag of that asset and the commit that macOS
/// builds. Returns every disagreement.
fn check_velnor_gen_pin(rust_ci_lock: &str, mise_toml: &str, mise_lock: &str) -> Vec<String> {
    let lock = toml_table(rust_ci_lock, ".velnor/rust-ci.lock");
    let text = |value: Option<&toml::Value>| value.and_then(toml::Value::as_str).map(str::to_owned);
    let mut problems = Vec::new();
    let locked_sum = text(lock.get("velnor-gen").and_then(|t| t.get("linux-x64")));
    match (mise_lock_linux_checksum(mise_lock), locked_sum) {
        (Ok(mise), Some(locked)) if mise == locked => {}
        (Ok(mise), Some(locked)) => problems.push(format!(
            ".velnor/rust-ci.lock [velnor-gen].linux-x64 is {locked}, but mise.lock installs {mise} for {GEN_RELEASE_TOOL}"
        )),
        (Ok(_), None) => problems.push(".velnor/rust-ci.lock has no [velnor-gen].linux-x64".to_owned()),
        (Err(why), _) => problems.push(why),
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

fn sample_files() -> (String, String, String) {
    (
        format!(
            "version = \"0.1.106\"\ncommit = \"{SAMPLE_COMMIT}\"\n\n[velnor-gen]\nlinux-x64 = \"{SAMPLE_SUM}\"\n"
        ),
        format!(
            "[tools]\n\"{GEN_RELEASE_TOOL}\" = {{ version = \"tools-v0.1.106\", os = [\"linux\"] }}\n\"{GEN_SOURCE_TOOL}\" = {{ version = \"rev:{SAMPLE_COMMIT}\", crate = \"velnor-gen\" }}\n"
        ),
        format!(
            "[[tools.\"{GEN_RELEASE_TOOL}\"]]\nversion = \"tools-v0.1.106\"\n\n[tools.\"{GEN_RELEASE_TOOL}\".\"platforms.linux-x64\"]\nchecksum = \"sha256:{SAMPLE_SUM}\"\n"
        ),
    )
}

#[test]
fn velnor_gen_pin_agrees_across_the_lock_and_mise() {
    let (lock, toml, mise_lock) = sample_files();
    assert_eq!(
        check_velnor_gen_pin(&lock, &toml, &mise_lock),
        Vec::<String>::new()
    );
}

#[test]
fn velnor_gen_pin_rejects_a_tampered_checksum_version_or_commit() {
    let (lock, toml, mise_lock) = sample_files();
    let flipped = SAMPLE_SUM.replace('2', "3");
    let tampered_sum = lock.replace(SAMPLE_SUM, &flipped);
    let problems = check_velnor_gen_pin(&tampered_sum, &toml, &mise_lock);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("linux-x64"), "{problems:?}");

    let tampered_version = lock.replace("version = \"0.1.106\"", "version = \"0.1.107\"");
    let problems = check_velnor_gen_pin(&tampered_version, &toml, &mise_lock);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("version"), "{problems:?}");

    let other = "0".repeat(40);
    let tampered_commit = lock.replace(SAMPLE_COMMIT, &other);
    let problems = check_velnor_gen_pin(&tampered_commit, &toml, &mise_lock);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("commit"), "{problems:?}");

    let moving_tag = toml.replace(&format!("rev:{SAMPLE_COMMIT}"), "tag:tools-v0.1.106");
    assert_eq!(
        check_velnor_gen_pin(&lock, &moving_tag, &mise_lock).len(),
        1
    );
}

#[test]
fn repository_velnor_gen_pin_agrees_across_the_lock_and_mise() {
    let problems = check_velnor_gen_pin(
        include_str!("../.velnor/rust-ci.lock"),
        include_str!("../mise.toml"),
        include_str!("../mise.lock"),
    );
    assert!(problems.is_empty(), "{problems:#?}");
}
