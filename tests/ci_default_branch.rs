//! Guards the `[workflow].default_branch` source setting that generated CI
//! push filters and follow-up `head_branch` predicates are derived from.
//! The check is pure Rust over config text; no subprocess and no network.

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

/// Reads `default_branch` from the `[workflow]` table. Only the keys this
/// check needs are read; other TOML syntax is skipped, not validated.
fn workflow_default_branch(config_text: &str) -> Result<String, DefaultBranchError> {
    let mut in_workflow = false;
    let mut found_table = false;
    let mut configured: Option<String> = None;

    for raw in config_text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(header) = line.strip_prefix('[') {
            // `[[workflow]]` (array of tables) yields `[workflow` here and never matches.
            let name = header.split(']').next().unwrap_or_default().trim();
            in_workflow = name == "workflow";
            found_table |= in_workflow;
            continue;
        }
        if !in_workflow {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != "default_branch" {
            continue;
        }
        if configured.is_some() {
            return Err(DefaultBranchError::Invalid(
                "workflow.default_branch is defined more than once".to_owned(),
            ));
        }
        let branch = parse_string(value.trim()).ok_or_else(|| {
            DefaultBranchError::Invalid(
                "workflow.default_branch must be a nonempty string".to_owned(),
            )
        })?;
        configured = Some(branch.to_owned());
    }

    if !found_table {
        return Err(DefaultBranchError::Invalid(
            "workflow must be a [workflow] table".to_owned(),
        ));
    }
    configured.ok_or_else(|| {
        DefaultBranchError::Invalid("[workflow] is missing default_branch".to_owned())
    })
}

/// Accepts one single-line TOML string with an optional trailing comment.
fn parse_string(text: &str) -> Option<&str> {
    let quote = text.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let rest = &text[1..];
    let end = rest.find(quote)?;
    let content = &rest[..end];
    let tail = rest[end + 1..].trim();
    if !(tail.is_empty() || tail.starts_with('#')) {
        return None;
    }
    if content.is_empty() || content.contains('\\') {
        return None;
    }
    Some(content)
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
    let config = include_str!("../.velnor/config.toml");
    assert_eq!(check_default_branch(config, "main"), Ok(()));
}
