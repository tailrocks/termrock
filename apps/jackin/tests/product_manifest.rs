use std::path::Path;
use std::process::Command;

use serde_json::Value;

const MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");

fn metadata() -> Value {
    let output = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--no-deps",
            "--offline",
            "--format-version",
            "1",
        ])
        .arg("--manifest-path")
        .arg(Path::new(MANIFEST_DIR).join("Cargo.toml"))
        .output()
        .expect("cargo metadata starts");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("cargo metadata prints JSON")
}

fn package(meta: &Value) -> &Value {
    meta["packages"]
        .as_array()
        .expect("packages")
        .iter()
        .find(|p| p["name"] == "jackin-console")
        .expect("jackin-console is a workspace member")
}

fn feature(package: &Value, name: &str) -> Option<Vec<String>> {
    let list = package["features"].get(name)?.as_array()?;
    let mut items: Vec<String> = list
        .iter()
        .map(|v| v.as_str().expect("feature entry").to_owned())
        .collect();
    items.sort();
    Some(items)
}

fn required_features(package: &Value, kind: &str, name: &str) -> Option<Vec<String>> {
    let target = package["targets"]
        .as_array()
        .expect("targets")
        .iter()
        .find(|t| {
            t["name"] == name
                && t["kind"]
                    .as_array()
                    .is_some_and(|k| k.iter().any(|k| k == kind))
        })?;
    let list = target["required-features"].as_array()?;
    Some(
        list.iter()
            .map(|v| v.as_str().expect("required feature").to_owned())
            .collect(),
    )
}

fn violations(package: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let default = feature(package, "default").unwrap_or_default();
    if !default.is_empty() {
        out.push(format!(
            "`default` must be empty so the library carries no demo, found {default:?}"
        ));
    }
    let clap = package["dependencies"]
        .as_array()
        .expect("dependencies")
        .iter()
        .find(|d| d["name"] == "clap");
    if clap.is_none_or(|d| d["optional"] != true) {
        out.push("`clap` must be an optional dependency, enabled only by `demo`".into());
    }
    let features = package["features"].as_object().expect("features");
    for (name, list) in features.iter().filter(|(name, _)| *name != "demo") {
        for entry in list.as_array().expect("feature list") {
            let entry = entry.as_str().expect("feature entry");
            let enables_clap = ["clap", "dep:clap", "clap?"]
                .iter()
                .any(|c| entry == *c || entry.starts_with(&format!("{c}/")));
            if enables_clap {
                out.push(format!(
                    "`clap` is enabled by the feature `{name}` through `{entry}`; only `demo` may enable it"
                ));
            }
        }
    }
    let demo = feature(package, "demo");
    if demo != Some(vec!["dep:clap".into(), "termrock/cli".into()]) {
        out.push(format!(
            "`demo` must be exactly [dep:clap, termrock/cli], found {demo:?}"
        ));
    }
    let product = feature(package, "product");
    if product != Some(Vec::new()) {
        out.push(format!(
            "`product` must exist and enable nothing, found {product:?}"
        ));
    }
    let demo_only = Some(vec!["demo".to_owned()]);
    if required_features(package, "bin", "jackin-preview") != demo_only {
        out.push("the bin `jackin-preview` must carry required-features = [\"demo\"]".into());
    }
    if required_features(package, "test", "functional") != demo_only {
        out.push("the `functional` test target must carry required-features = [\"demo\"]".into());
    }
    let bins: Vec<&str> = package["targets"]
        .as_array()
        .expect("targets")
        .iter()
        .filter(|t| {
            t["kind"]
                .as_array()
                .is_some_and(|k| k.iter().any(|k| k == "bin"))
        })
        .filter_map(|t| t["name"].as_str())
        .collect();
    if bins != ["jackin-preview"] {
        out.push(format!(
            "the only bin must be `jackin-preview`, found {bins:?}"
        ));
    }
    out
}

#[test]
fn manifest_shape() {
    let meta = metadata();
    let found = violations(package(&meta));
    assert!(found.is_empty(), "{}", found.join("\n"));
}

#[test]
fn only_the_demo_may_enable_clap() {
    let package = |features: Value| {
        serde_json::json!({
            "name": "jackin-console",
            "features": features,
            "dependencies": [{ "name": "clap", "optional": true }],
            "targets": [],
        })
    };
    let clean = package(serde_json::json!({
        "default": [],
        "demo": ["dep:clap", "termrock/cli"],
        "product": [],
    }));
    let found = violations(&clean);
    assert!(
        found
            .iter()
            .all(|v| !v.contains("only `demo` may enable it")),
        "{found:?}"
    );
    for (feature, entry) in [
        ("product", "dep:clap"),
        ("cli", "clap/derive"),
        ("tools", "clap"),
    ] {
        let mut features = serde_json::json!({
            "default": [],
            "demo": ["dep:clap", "termrock/cli"],
            "product": [],
        });
        features[feature] = serde_json::json!([entry]);
        let found = violations(&package(features));
        let named = format!("`clap` is enabled by the feature `{feature}` through `{entry}`");
        assert!(
            found.iter().any(|v| v.starts_with(&named)),
            "{feature}: {found:?}"
        );
    }
}

#[test]
fn product_mode_test_is_gated_on_the_demo_being_off() {
    let source = std::fs::read_to_string(Path::new(MANIFEST_DIR).join("tests/product_mode.rs"))
        .expect("tests/product_mode.rs exists");
    assert_eq!(
        source.lines().next(),
        Some(r#"#![cfg(not(feature = "demo"))]"#),
        "the first line of tests/product_mode.rs must gate it on the demo being off, or the product job lists no test"
    );
}
