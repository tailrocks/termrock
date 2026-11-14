//! Guards the `[workflow].default_branch` source setting that generated CI
//! push filters and follow-up `head_branch` predicates are derived from.
//! Runs the offline validator on temporary fixtures; no network access.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn validator() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/check_default_branch.py")
}

fn write_config(name: &str, body: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("ci_default_branch");
    std::fs::create_dir_all(&dir).expect("fixture directory");
    let path = dir.join(format!("{name}.toml"));
    std::fs::write(&path, body).expect("fixture write");
    path
}

fn run(config: &Path, expected: &str) -> Output {
    Command::new("python3")
        .arg(validator())
        .arg("--expected")
        .arg(expected)
        .arg("--config")
        .arg(config)
        .output()
        .expect("python3 must be available")
}

fn code(output: &Output) -> i32 {
    output.status.code().expect("validator exited by signal")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn default_branch_accepts_match() {
    let config = write_config("accepts_match", "[workflow]\ndefault_branch = \"main\"\n");
    let output = run(&config, "main");
    assert_eq!(code(&output), 0, "{}", stderr(&output));
}

#[test]
fn default_branch_rejects_stale_source() {
    let config = write_config(
        "rejects_stale",
        "[workflow]\ndefault_branch = \"design/interaction-polish\"\n",
    );
    let output = run(&config, "main");
    assert_eq!(code(&output), 1);
    let message = stderr(&output);
    assert!(message.contains("design/interaction-polish"), "{message}");
    assert!(message.contains("main"), "{message}");
}

#[test]
fn default_branch_supports_fork_name() {
    let config = write_config("supports_fork", "[workflow]\ndefault_branch = \"trunk\"\n");
    let output = run(&config, "trunk");
    assert_eq!(code(&output), 0, "{}", stderr(&output));
}

#[test]
fn default_branch_rejects_missing_value() {
    let config = write_config("rejects_missing", "[workflow]\npolicy = \"consumer-v1\"\n");
    let output = run(&config, "main");
    assert_eq!(code(&output), 2);
    assert!(
        stderr(&output).contains("default_branch"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn default_branch_rejects_wrong_toml_shapes() {
    let shapes = [
        ("workflow_scalar", "workflow = 1\n"),
        ("workflow_list", "workflow = []\n"),
        ("branch_integer", "[workflow]\ndefault_branch = 1\n"),
    ];
    for (name, body) in shapes {
        let config = write_config(name, body);
        let output = run(&config, "main");
        assert_eq!(code(&output), 2, "{name}: {}", stderr(&output));
        assert!(
            !stderr(&output).contains("Traceback"),
            "{name}: {}",
            stderr(&output)
        );
    }
}

#[test]
fn repository_config_default_branch_is_main() {
    let config = Path::new(env!("CARGO_MANIFEST_DIR")).join(".velnor/config.toml");
    let output = run(&config, "main");
    assert_eq!(code(&output), 0, "{}", stderr(&output));
}
