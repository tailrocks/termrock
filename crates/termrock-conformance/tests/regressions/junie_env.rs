//! Guards the rename away from the old product names (#566, #570). Every
//! tracked text file outside the historical records must be free of the old
//! environment prefix, of the old paste-protocol prefix, and of the old
//! package name in any letter case, with either separator, so a stray reader,
//! fixture or doc cannot quietly bring any of them back.
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

/// The old package name with either separator, matched case-insensitively.
const OLD_PACKAGE_HYPHEN: &str = concat!("junie", "-tui");
const OLD_PACKAGE_UNDERSCORE: &str = concat!("junie", "_tui");

/// Every place the old package name may stay: the audit and decision records,
/// all of `docs/testing` (receipts, case manifests and the nextest list), the
/// frozen reference binaries, the legacy reference modules of the functional
/// harness, and the negative detectors in `facade_contract.rs`.
const NAME_EXEMPT_PREFIXES: &[&str] = &[
    "docs/audits/",
    "docs/testing/",
    "crates/termrock-functional/tests/functional/reference-bins/",
    "crates/termrock-functional/tests/functional/legacy_reference_",
];

const NAME_EXEMPT_FILES: &[&str] = &[
    "docs/decisions.md",
    "docs/P9.3-red-green.md",
    "crates/termrock/tests/regressions/facade_contract.rs",
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

/// True when `text` has the old protocol prefix (any case) followed by a
/// lowercase letter. A nextest binary id and prose after the name do not match;
/// only a protocol request does.
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

/// True when `text` contains the old package name with `-` or `_`, in any case.
fn spells_package_name(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains(OLD_PACKAGE_HYPHEN) || lower.contains(OLD_PACKAGE_UNDERSCORE)
}

#[test]
fn tracked_sources_do_not_spell_the_old_environment_prefix() {
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

#[test]
fn tracked_files_do_not_spell_the_old_package_name_in_any_case() {
    let offenders = offenders(NAME_EXEMPT_PREFIXES, NAME_EXEMPT_FILES, spells_package_name);

    assert!(
        offenders.is_empty(),
        "the old package name must not appear outside the historical records; use termrock: {offenders:#?}"
    );
}

#[test]
fn the_package_name_detector_is_case_insensitive_in_both_separators() {
    assert!(spells_package_name(concat!("junie", "-tui")));
    assert!(spells_package_name(concat!("JUNIE", "_TUI")));
    assert!(spells_package_name(concat!("Junie", "-Tui", " package")));
    assert!(spells_package_name(concat!("junie", "_tui::ui")));
    assert!(!spells_package_name("Junie TUI (termrock) design system"));
    assert!(!spells_package_name("termrock junie"));
}
