//! The candidate application binaries the journeys launch.
//!
//! Showcase, TablePro and Holla are packages of their own, so the root
//! `functional` target is given no `CARGO_BIN_EXE_<name>` for them. `SHOWCASE`,
//! `TABLEPRO` and `HOLLA` read `TERMROCK_TEST_<NAME>` at run time instead, when a
//! journey launches the binary. `scripts/run_functional.py` builds the candidates
//! and exports the variables; the nextest setup script `app-bins` does the same
//! for the generated CI suite. A missing, relative or wrong file fails the test
//! that asked for it, before a session opens. Nothing falls back to an arbitrary
//! `target/debug` file or to an archived reference binary.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::sync::{Mutex, OnceLock};

/// One candidate application binary, named by its file name.
#[derive(Clone, Copy, Debug)]
pub struct Bin(&'static str);

pub const SHOWCASE: Bin = Bin("showcase");
pub const TABLEPRO: Bin = Bin("tablepro");
pub const HOLLA: Bin = Bin("holla");

/// Function spelling for new journeys; the older ones pass the `SHOWCASE` value.
#[allow(dead_code)]
pub fn showcase() -> &'static str {
    SHOWCASE.path()
}

#[allow(dead_code)]
pub fn tablepro() -> &'static str {
    TABLEPRO.path()
}

#[allow(dead_code)]
pub fn holla() -> &'static str {
    HOLLA.path()
}

impl Bin {
    /// The validated absolute path, resolved once. Panics with an actionable
    /// message when the environment does not name a usable candidate.
    pub fn path(self) -> &'static str {
        static RESOLVED: OnceLock<Mutex<BTreeMap<&'static str, &'static str>>> = OnceLock::new();
        let mut resolved = RESOLVED
            .get_or_init(|| Mutex::new(BTreeMap::new()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(path) = resolved.get(self.0) {
            return path;
        }
        let path = resolve(self.0, std::env::var_os(variable(self.0)))
            .unwrap_or_else(|message| panic!("{message}"));
        let path: &'static str = Box::leak(path.into_boxed_str());
        resolved.insert(self.0, path);
        path
    }
}

impl AsRef<str> for Bin {
    fn as_ref(&self) -> &str {
        self.path()
    }
}

impl AsRef<OsStr> for Bin {
    fn as_ref(&self) -> &OsStr {
        OsStr::new(self.path())
    }
}

impl AsRef<Path> for Bin {
    fn as_ref(&self) -> &Path {
        Path::new(self.path())
    }
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
