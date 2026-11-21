//! Guards the environment rename from the old junie-tui prefix to TERMROCK_ (#566). Every
//! tracked text file outside the historical records must be free of the old
//! prefix, so a stray reader or fixture cannot quietly bring it back.
//!
//! Exempt on purpose: the audit and acceptance records and the P9.3 red and
//! green log are dated evidence of what ran, and `reference-bins/` holds frozen
//! binaries. The one source that names the old variables for those binaries is
//! `legacy_reference_env.rs` in the functional harness. The needle is assembled
//! with `concat!` so this file never matches itself.

use std::path::Path;
use std::process::Command;

const OLD_PREFIX: &str = concat!("JUNIE", "_TUI_");

const EXEMPT_PREFIXES: &[&str] = &[
    "docs/audits/",
    "docs/testing/acceptance-receipts/",
    "crates/termrock-functional/tests/functional/reference-bins/",
];

const EXEMPT_FILES: &[&str] = &[
    "docs/P9.3-red-green.md",
    "crates/termrock-functional/tests/functional/legacy_reference_env.rs",
];

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("termrock-conformance sits at crates/termrock-conformance")
}

fn exempt(path: &str) -> bool {
    EXEMPT_FILES.contains(&path)
        || EXEMPT_PREFIXES
            .iter()
            .any(|prefix| path.starts_with(prefix))
}

#[test]
fn tracked_sources_do_not_spell_the_old_junie_tui_prefix() {
    let listed = Command::new("git")
        .current_dir(root())
        .args(["ls-files", "-z"])
        .output()
        .expect("run git ls-files");
    assert!(listed.status.success(), "git ls-files failed");

    let mut offenders = Vec::new();
    for raw in listed.stdout.split(|byte| *byte == 0) {
        let Ok(path) = std::str::from_utf8(raw) else {
            continue;
        };
        if path.is_empty() || exempt(path) {
            continue;
        }
        let Ok(bytes) = std::fs::read(root().join(path)) else {
            continue;
        };
        if bytes.contains(&0) {
            continue;
        }
        if String::from_utf8_lossy(&bytes).contains(OLD_PREFIX) {
            offenders.push(path.to_owned());
        }
    }

    assert!(
        offenders.is_empty(),
        "the old environment prefix must not appear in tracked sources; rename it to TERMROCK_: {offenders:#?}"
    );
}
