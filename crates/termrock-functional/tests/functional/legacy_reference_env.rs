//! Environment names the frozen reference binaries still read.
//!
//! The binaries under `reference-bins/` were built before the `JUNIE_TUI_*`
//! names became `TERMROCK_*`, and they read the old names at run time. A
//! Reference-mode launch therefore carries the old name next to each new one,
//! so the frozen build sees the environment it was captured with. Candidate
//! launches never get these names, and no library or app code reads them.
//! This is the only place in the tree that spells the old prefix.

use std::collections::BTreeMap;

const REFERENCE_READS: &[(&str, &str)] = &[
    ("TERMROCK_CLIENT_PLATFORM", "JUNIE_TUI_CLIENT_PLATFORM"),
    ("TERMROCK_TEST_CLOCK", "JUNIE_TUI_TEST_CLOCK"),
    ("TERMROCK_GLYPHS", "JUNIE_TUI_GLYPHS"),
    ("TERMROCK_KEYBOARD_PROTOCOL", "JUNIE_TUI_KEYBOARD_PROTOCOL"),
    ("TERMROCK_ALT_KEYS", "JUNIE_TUI_ALT_KEYS"),
    (
        "TERMROCK_FIRST_POLL_DELAY_MS",
        "JUNIE_TUI_FIRST_POLL_DELAY_MS",
    ),
    ("TERMROCK_START_DELAY_MS", "JUNIE_TUI_START_DELAY_MS"),
];

/// Adds the frozen-binary name for every new name already in `environment`.
pub fn mirror_for_reference(environment: &mut BTreeMap<String, String>) {
    let legacy: Vec<(String, String)> = REFERENCE_READS
        .iter()
        .filter_map(|(new, old)| {
            environment
                .get(*new)
                .map(|value| ((*old).to_string(), value.clone()))
        })
        .collect();
    environment.extend(legacy);
}
