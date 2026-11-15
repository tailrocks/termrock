#[path = "../../../../tests/functional/support.rs"]
mod shared;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

pub use shared::*;

pub fn showcase_bin() -> &'static str {
    app_binary("showcase")
}

pub fn tablepro_bin() -> &'static str {
    app_binary("tablepro")
}

pub fn holla_bin() -> &'static str {
    env!("CARGO_BIN_EXE_holla-xtest")
}

/// Path of a sibling app binary.
///
/// `TCB_BIN_<NAME>` wins, which is how a nextest setup script or a developer
/// points at a specific build. Otherwise the binary is the sibling of the
/// `deps/` directory this test executable lives in, which holds for an
/// in-place run and for a nextest archive extracted with its target dir.
fn app_binary(name: &str) -> &'static str {
    static CACHE: OnceLock<Mutex<BTreeMap<String, &'static str>>> = OnceLock::new();
    let mut cache = CACHE
        .get_or_init(|| Mutex::new(BTreeMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(path) = cache.get(name) {
        return path;
    }
    let variable = format!("TCB_BIN_{}", name.to_ascii_uppercase());
    let path = match std::env::var_os(&variable) {
        Some(value) => PathBuf::from(value),
        None => std::env::current_exe()
            .expect("current test executable")
            .parent()
            .and_then(|deps| deps.parent())
            .expect("test executable sits in <target>/<profile>/deps")
            .join(name),
    };
    assert!(
        path.is_file(),
        "app binary `{name}` not found at {}. Build it first (`cargo build --workspace --bins`) or set {variable}.",
        path.display()
    );
    let leaked: &'static str = Box::leak(path.to_string_lossy().into_owned().into_boxed_str());
    cache.insert(name.to_string(), leaked);
    leaked
}
