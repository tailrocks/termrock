//! The candidate application binaries the journeys launch.
//!
//! Showcase, TablePro and Holla live in their own packages, so this target is
//! given no `CARGO_BIN_EXE_<name>` for them. Each path comes from
//! `TERMROCK_TEST_<NAME>` at run time. `scripts/run_functional.py` builds the
//! candidates and exports the variables; the nextest setup script `app-bins`
//! does the same for the generated CI suite. A missing, relative or wrong file
//! fails the test that asked for it, before a session opens. Nothing falls back
//! to an arbitrary `target/debug` file or to an archived reference binary.

use std::ffi::OsString;
use std::path::Path;
use std::sync::OnceLock;

static SHOWCASE: OnceLock<String> = OnceLock::new();
static TABLEPRO: OnceLock<String> = OnceLock::new();
static HOLLA: OnceLock<String> = OnceLock::new();

pub fn showcase() -> &'static str {
    candidate(&SHOWCASE, "showcase")
}

pub fn tablepro() -> &'static str {
    candidate(&TABLEPRO, "tablepro")
}

pub fn holla() -> &'static str {
    candidate(&HOLLA, "holla")
}

fn candidate(cell: &'static OnceLock<String>, name: &str) -> &'static str {
    cell.get_or_init(|| {
        resolve(name, std::env::var_os(variable(name)))
            .unwrap_or_else(|message| panic!("{message}"))
    })
}

fn variable(name: &str) -> String {
    format!("TERMROCK_TEST_{}", name.to_ascii_uppercase())
}

/// Validates the value of `TERMROCK_TEST_<NAME>` without launching anything.
fn resolve(name: &str, value: Option<OsString>) -> Result<String, String> {
    let variable = variable(name);
    let value = value.filter(|v| !v.is_empty()).ok_or_else(|| {
        format!(
            "{variable} is not set: run the functional suite through `python3 scripts/run_functional.py`, which builds the candidate `{name}` and exports it"
        )
    })?;
    let path = Path::new(&value);
    if !path.is_absolute() {
        return Err(format!(
            "{variable}={} is not an absolute path",
            path.display()
        ));
    }
    if !path.is_file() {
        return Err(format!(
            "{variable}={} is not a file: build the candidate again",
            path.display()
        ));
    }
    if path.file_name().is_none_or(|file| file != name) {
        return Err(format!(
            "{variable}={} is not a binary named `{name}`",
            path.display()
        ));
    }
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("{variable}={} is not valid UTF-8", path.display()))
}

#[test]
fn functional_binary_resolution_rejects_missing_candidate() {
    let directory = std::env::temp_dir().join(format!("termrock-bins-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("create scratch directory");
    let present = directory.join("showcase");
    std::fs::write(&present, b"").expect("write stand-in candidate");

    let unset = resolve("showcase", None).expect_err("an unset variable fails");
    assert!(
        unset.contains("TERMROCK_TEST_SHOWCASE is not set"),
        "{unset}"
    );
    assert!(resolve("showcase", Some(OsString::new())).is_err());
    let relative = resolve("showcase", Some("target/debug/showcase".into()))
        .expect_err("a relative path fails");
    assert!(relative.contains("not an absolute path"), "{relative}");
    let missing =
        resolve("showcase", Some(directory.join("gone").into())).expect_err("a missing file fails");
    assert!(missing.contains("is not a file"), "{missing}");
    let renamed =
        resolve("tablepro", Some(present.clone().into())).expect_err("another app's binary fails");
    assert!(
        renamed.contains("not a binary named `tablepro`"),
        "{renamed}"
    );
    assert_eq!(
        resolve("showcase", Some(present.clone().into())).expect("a real candidate resolves"),
        present.to_str().expect("utf-8 path")
    );
    std::fs::remove_dir_all(&directory).expect("remove scratch directory");
}
