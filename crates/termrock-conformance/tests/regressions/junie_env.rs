//! Guards the rename away from the old junie-tui names (#566, #570). Every
//! tracked text file outside the historical records must be free of the old
//! environment prefix and of the old paste-protocol prefix, so a stray reader
//! or fixture cannot quietly bring either back.
//!
//! Exempt on purpose: the audit and acceptance records and the P9.3 red and
//! green log are dated evidence of what ran, and `reference-bins/` holds frozen
//! binaries. The only sources that name the old names for those binaries are
//! `legacy_reference_env.rs` (environment) and `legacy_reference_protocol.rs`
//! (paste protocol) in the functional harness. The needles are assembled with
//! `concat!` so this file never matches itself.

use std::path::Path;
use std::process::Command;

const OLD_PREFIX: &str = concat!("JUNIE", "_TUI_");

const OLD_PROTOCOL_PREFIX: &str = concat!("junie", "-tui:");

const ENV_EXEMPT_PREFIXES: &[&str] = &[
    "docs/audits/",
    "docs/testing/acceptance-receipts/",
    "crates/termrock-functional/tests/functional/reference-bins/",
];

const ENV_EXEMPT_FILES: &[&str] = &[
    "docs/P9.3-red-green.md",
    "crates/termrock-functional/tests/functional/legacy_reference_env.rs",
];

const PROTOCOL_EXEMPT_PREFIXES: &[&str] = &[
    "docs/audits/",
    "docs/testing/acceptance-receipts/",
    "docs/testing/probe-receipts/",
    "docs/testing/cases/",
    "crates/termrock-functional/tests/functional/reference-bins/",
];

const PROTOCOL_EXEMPT_FILES: &[&str] = &[
    "docs/decisions.md",
    "docs/P9.3-red-green.md",
    "crates/termrock-functional/tests/functional/legacy_reference_protocol.rs",
];

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("termrock-conformance sits at crates/termrock-conformance")
}

fn exempt(path: &str, prefixes: &[&str], files: &[&str]) -> bool {
    files.contains(&path) || prefixes.iter().any(|prefix| path.starts_with(prefix))
}

/// The tracked text files whose contents satisfy `spells` and which are not exempt.
fn offenders(prefixes: &[&str], files: &[&str], spells: impl Fn(&str) -> bool) -> Vec<String> {
    let listed = Command::new("git")
        .current_dir(root())
        .args(["ls-files", "-z"])
        .output()
        .expect("run git ls-files");
    assert!(listed.status.success(), "git ls-files failed");

    let mut found = Vec::new();
    for raw in listed.stdout.split(|byte| *byte == 0) {
        let Ok(path) = std::str::from_utf8(raw) else {
            continue;
        };
        if path.is_empty() || exempt(path, prefixes, files) {
            continue;
        }
        let Ok(bytes) = std::fs::read(root().join(path)) else {
            continue;
        };
        if bytes.contains(&0) {
            continue;
        }
        if spells(&String::from_utf8_lossy(&bytes)) {
            found.push(path.to_owned());
        }
    }
    found
}

/// True when `text` has `junie-tui:` (any case) followed by a lowercase letter.
/// The nextest binary id `junie-tui::functional` and the prose `junie-tui: the`
/// do not match; only a protocol request does.
fn spells_protocol(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower
        .match_indices(OLD_PROTOCOL_PREFIX)
        .any(|(at, needle)| {
            lower
                .as_bytes()
                .get(at + needle.len())
                .is_some_and(u8::is_ascii_lowercase)
        })
}

#[test]
fn tracked_sources_do_not_spell_the_old_junie_tui_prefix() {
    let offenders = offenders(ENV_EXEMPT_PREFIXES, ENV_EXEMPT_FILES, |text| {
        text.contains(OLD_PREFIX)
    });

    assert!(
        offenders.is_empty(),
        "the old environment prefix must not appear in tracked sources; rename it to TERMROCK_: {offenders:#?}"
    );
}

#[test]
fn tracked_sources_do_not_spell_the_old_protocol_prefix() {
    let offenders = offenders(
        PROTOCOL_EXEMPT_PREFIXES,
        PROTOCOL_EXEMPT_FILES,
        spells_protocol,
    );

    assert!(
        offenders.is_empty(),
        "the old paste-protocol prefix must not appear in tracked sources; rename it to termrock: (a Reference-mode launch sends it only from legacy_reference_protocol.rs): {offenders:#?}"
    );
}

#[test]
fn the_protocol_detector_matches_requests_not_binary_ids() {
    assert!(spells_protocol(concat!("junie", "-tui:fence")));
    assert!(spells_protocol(concat!("JUNIE", "-TUI:advance-clock:250")));
    assert!(!spells_protocol(concat!(
        "junie",
        "-tui::functional showcase::x"
    )));
    assert!(!spells_protocol(concat!(
        "junie",
        "-tui: the shim was removed"
    )));
    assert!(!spells_protocol("termrock:advance-clock:250"));
}
