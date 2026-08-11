// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! End-to-end: install four application blocks + demo/tiny into a consumer
//! crate that path-depends on termrock, then `cargo check`.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use termrock_cli::{apply_plan, load_entry, plan_install};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

fn scratch_dir() -> PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!(
        "termrock-block-compile-{}-{}",
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
fn install_four_blocks_and_cargo_check() {
    let root = repo_root();
    let fixtures = root.join("registry/fixtures");
    let consumer = scratch_dir();
    let termrock_path = root.join("crates/termrock");

    // Minimal consumer crate depending on public termrock API only.
    fs::write(
        consumer.join("Cargo.toml"),
        format!(
            r#"[package]
name = "termrock-block-consumer"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
termrock = {{ path = "{}" }}
"#,
            termrock_path.display()
        ),
    )
    .unwrap();
    fs::create_dir_all(consumer.join("src/ui/blocks")).unwrap();
    fs::write(
        consumer.join("src/lib.rs"),
        r#"//! Consumer that only uses installed source + public termrock kernel.
pub mod ui;
"#,
    )
    .unwrap();
    fs::write(
        consumer.join("src/ui/mod.rs"),
        r#"
pub mod demo_block;
pub mod tiny_component;
pub mod blocks;
"#,
    )
    .unwrap();
    fs::write(
        consumer.join("src/ui/blocks/mod.rs"),
        r#"
pub mod ops_dashboard;
pub mod resource_browser;
pub mod settings_shell;
pub mod form_wizard;
"#,
    )
    .unwrap();

    let entries = [
        "demo-block",
        "tiny-component",
        "ops-dashboard",
        "resource-browser",
        "settings-shell",
        "form-wizard",
    ];
    for name in entries {
        let entry_dir = fixtures.join(name);
        let entry = load_entry(&entry_dir).expect(name);
        let plan = plan_install(&entry, &entry_dir, &consumer, false);
        assert!(
            plan.errors.is_empty(),
            "{name} plan errors: {:?}",
            plan.errors
        );
        apply_plan(
            &plan,
            &consumer,
            &entry,
            &consumer.join("termrock.lock.json"),
        )
        .unwrap_or_else(|e| panic!("{name} apply: {e}"));
    }

    assert!(consumer.join("src/ui/blocks/ops_dashboard.rs").exists());
    assert!(consumer.join("src/ui/blocks/resource_browser.rs").exists());
    assert!(consumer.join("src/ui/blocks/settings_shell.rs").exists());
    assert!(consumer.join("src/ui/blocks/form_wizard.rs").exists());
    assert!(consumer.join("src/ui/demo_block.rs").exists());
    assert!(consumer.join("src/ui/tiny_component.rs").exists());

    let output = Command::new("cargo")
        .args(["check", "--manifest-path"])
        .arg(consumer.join("Cargo.toml"))
        .output()
        .expect("spawn cargo");
    assert!(
        output.status.success(),
        "cargo check failed for installed blocks in {}\nstdout:\n{}\nstderr:\n{}",
        consumer.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
