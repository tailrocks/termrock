//! Crate layering (#439, #440, #452): no dependency may point up the layers.
//!
//! The rules live in `docs/layering.toml`. `cargo metadata` gives the real
//! dependency graph of the workspace, which this file checks against that table:
//! a crate may depend only on crates below it, an application depends on
//! `termrock` only, and nothing depends on the `junie-tui` shim. The check
//! also fails a feature that a crate declares but does not forward to a
//! dependency that declares it, a dependency cycle, and a `crates/termrock-*` or
//! `apps/*` directory that is not a workspace member. A test asserts that it
//! ran on the extracted crates, so it cannot pass by checking nothing.
//!
//! Before the split the same table also scanned the modules of the single
//! package (stage A, with a `[files]` section and an allowlist that only
//! shrank). #452 deleted it with the last file of that package.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const LAYERING: &str = "docs/layering.toml";
const FIX: &str = "Move the item down or invert the dependency; there is no allowlist.";

/// The repository root: `termrock` sits at `crates/termrock`, two levels below it.
fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("termrock sits at crates/termrock")
}

// ---------------------------------------------------------------------------
// The table
// ---------------------------------------------------------------------------

struct Layering {
    order: Vec<String>,
    layer_order: Vec<String>,
    layers: BTreeMap<String, String>,
    allow: BTreeMap<String, BTreeSet<String>>,
}

fn strings(value: &toml::Value, what: &str) -> Vec<String> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("{what} must be an array"))
        .iter()
        .map(|v| {
            v.as_str()
                .unwrap_or_else(|| panic!("{what} must hold strings"))
                .to_string()
        })
        .collect()
}

fn string_table(value: &toml::Value, what: &str) -> BTreeMap<String, String> {
    value
        .as_table()
        .unwrap_or_else(|| panic!("{what} must be a table"))
        .iter()
        .map(|(k, v)| {
            let v = v
                .as_str()
                .unwrap_or_else(|| panic!("{what}.{k} must be a string"));
            (k.clone(), v.to_string())
        })
        .collect()
}

impl Layering {
    fn parse(text: &str) -> Self {
        let doc: toml::Table = text.parse().expect("layering table is valid TOML");
        let section = |name: &str| {
            doc.get(name)
                .unwrap_or_else(|| panic!("layering table has no `{name}`"))
        };
        let allow = section("allow")
            .as_table()
            .expect("allow must be a table")
            .iter()
            .map(|(k, v)| (k.clone(), strings(v, k).into_iter().collect()))
            .collect();
        Self {
            order: strings(section("order"), "order"),
            layer_order: strings(section("layer_order"), "layer_order"),
            layers: string_table(section("layers"), "layers"),
            allow,
        }
    }

    fn load() -> Self {
        let text = std::fs::read_to_string(root().join(LAYERING)).expect("read docs/layering.toml");
        Self::parse(&text)
    }

    fn position(&self, name: &str) -> Option<usize> {
        self.order.iter().position(|c| c == name)
    }

    /// Everything `krate` may depend on: its listed dependencies and, because
    /// "every row also allows the foundation crates that the row's
    /// dependencies already use", whatever those may depend on in turn.
    fn permitted(&self, krate: &str) -> BTreeSet<String> {
        let mut seen = BTreeSet::new();
        let mut todo = vec![krate.to_string()];
        while let Some(next) = todo.pop() {
            for dep in self.allow.get(&next).into_iter().flatten() {
                if seen.insert(dep.clone()) {
                    todo.push(dep.clone());
                }
            }
        }
        seen
    }

    /// The package name that stands behind a short crate name.
    fn package(krate: &str) -> String {
        match krate {
            "termrock" | "junie-tui" => krate.to_string(),
            other => format!("termrock-{other}"),
        }
    }

    /// The short crate name of a workspace package, if the table lists it.
    fn member(&self, package: &str) -> Option<String> {
        let short = match package {
            "termrock" | "junie-tui" => package,
            other => other.strip_prefix("termrock-")?,
        };
        self.position(short).map(|_| short.to_string())
    }

    /// Problems with the table itself.
    fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        let mut seen = BTreeSet::new();
        for name in &self.order {
            if !seen.insert(name) {
                problems.push(format!("order lists `{name}` twice"));
            }
        }
        let rank = |layer: &str| self.layer_order.iter().position(|l| l == layer);
        for name in &self.order {
            let Some(layer) = self.layers.get(name) else {
                problems.push(format!("[layers] has no entry for `{name}`"));
                continue;
            };
            if rank(layer).is_none() {
                problems.push(format!("`{name}` is in unknown layer `{layer}`"));
            }
            if !self.allow.contains_key(name) {
                problems.push(format!("[allow] has no row for `{name}`"));
            }
        }
        for (name, deps) in &self.allow {
            let Some(at) = self.position(name) else {
                problems.push(format!("[allow] row `{name}` is not in `order`"));
                continue;
            };
            for dep in deps {
                match self.position(dep) {
                    None => problems.push(format!("`{name}` may depend on unknown crate `{dep}`")),
                    Some(below) if below >= at => problems.push(format!(
                        "`{name}` may depend on `{dep}`, which is not below it in `order`"
                    )),
                    Some(_) => {
                        let (own, theirs) = (
                            self.layers.get(name).and_then(|l| rank(l)),
                            self.layers.get(dep).and_then(|l| rank(l)),
                        );
                        if own < theirs {
                            problems
                                .push(format!("`{name}` may depend on `{dep}`, a higher layer"));
                        }
                    }
                }
            }
        }
        for name in self.layers.keys() {
            if self.position(name).is_none() {
                problems.push(format!("[layers] entry `{name}` is not in `order`"));
            }
        }
        problems
    }
}

// ---------------------------------------------------------------------------
// Stage B: crate level, from `cargo metadata`
// ---------------------------------------------------------------------------

/// Checks the workspace packages in `metadata` (the output of `cargo metadata
/// --no-deps`) against the table.
fn check_metadata(metadata: &serde_json::Value, layering: &Layering) -> (usize, Vec<String>) {
    let workspace = metadata["workspace_root"].as_str().unwrap_or_default();
    let mut problems = Vec::new();
    let mut graph: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut checked = 0;
    let packages = metadata["packages"].as_array().cloned().unwrap_or_default();
    for package in &packages {
        let name = package["name"].as_str().unwrap_or_default();
        let manifest = package["manifest_path"].as_str().unwrap_or_default();
        let relative = manifest
            .strip_prefix(workspace)
            .unwrap_or(manifest)
            .trim_start_matches('/');
        let deps: Vec<&str> = package["dependencies"]
            .as_array()
            .map(|d| d.iter().filter_map(|d| d["name"].as_str()).collect())
            .unwrap_or_default();

        if relative.starts_with("apps/") {
            checked += 1;
            if !deps.contains(&"termrock") {
                problems.push(format!("{name}: an application must depend on `termrock`"));
            }
            for dep in &deps {
                if layering.member(dep).is_some() && *dep != "termrock" {
                    problems.push(format!(
                        "{name}: an application depends on `termrock` only, not on `{dep}`. Import `termrock::` paths instead."
                    ));
                }
            }
            continue;
        }

        let Some(short) = layering.member(name) else {
            if relative.starts_with("crates/") && name.starts_with("termrock-") {
                problems.push(format!(
                    "{name}: not in {LAYERING}; add it to `order`, `layers` and `allow` before it ships"
                ));
            }
            continue;
        };
        checked += 1;
        let permitted = layering.permitted(&short);
        for dep in deps {
            let Some(dep_short) = layering.member(dep) else {
                continue;
            };
            if dep_short == short {
                continue;
            }
            graph
                .entry(short.clone())
                .or_default()
                .insert(dep_short.clone());
            if !permitted.contains(&dep_short) {
                problems.push(format!(
                    "{from}: `{from}` must not depend on `{to}` (Cargo.toml dependency `{dep}`). {FIX}",
                    from = Layering::package(&short),
                    to = Layering::package(&dep_short),
                ));
            }
        }
    }
    problems.extend(cycles(&graph));
    problems.extend(feature_forwarding_problems(&packages, layering));
    (checked, problems)
}

/// A crate that declares feature `F` must forward it to every normal dependency
/// that also declares `F`. Without the forward, `cargo check -p <crate>
/// --all-features` fails on its own, and the workspace build only passes by
/// feature unification (#446). A crate that does not declare `F` is exempt, so a
/// dependency's dev-only feature (`termrock-text/test-support`) stays out. `default` is a choice of features, not one.
fn feature_forwarding_problems(packages: &[serde_json::Value], layering: &Layering) -> Vec<String> {
    let declared: BTreeMap<&str, BTreeSet<&str>> = packages
        .iter()
        .filter_map(|p| {
            let features = p["features"].as_object()?;
            Some((
                p["name"].as_str()?,
                features.keys().map(String::as_str).collect(),
            ))
        })
        .collect();
    let mut problems = Vec::new();
    for package in packages {
        let name = package["name"].as_str().unwrap_or_default();
        if layering.member(name).is_none() {
            continue;
        }
        let Some(features) = package["features"].as_object() else {
            continue;
        };
        for (feature, list) in features {
            if feature == "default" {
                continue;
            }
            let forwarded: BTreeSet<&str> = list
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str())
                .collect();
            for dep in package["dependencies"].as_array().into_iter().flatten() {
                let dep_name = dep["name"].as_str().unwrap_or_default();
                if !dep["kind"].is_null()
                    || !dep_name.starts_with("termrock-") && dep_name != "termrock"
                {
                    continue;
                }
                if !declared
                    .get(dep_name)
                    .is_some_and(|f| f.contains(feature.as_str()))
                {
                    continue;
                }
                let rename = dep["rename"].as_str().unwrap_or(dep_name);
                let plain = format!("{rename}/{feature}");
                let weak = format!("{rename}?/{feature}");
                if !forwarded.contains(plain.as_str()) && !forwarded.contains(weak.as_str()) {
                    problems.push(format!(
                        "{name}: feature `{feature}` must forward to `{dep_name}`, which also declares it. Add \"{plain}\" to `{feature}` in its Cargo.toml, or `cargo check -p {name} --all-features` fails on its own."
                    ));
                }
            }
        }
    }
    problems
}

/// A `crates/termrock-*` or `apps/*` directory that `cargo metadata` does not
/// list is outside the workspace, so stage B could not see its dependencies.
/// `directories` are workspace-relative, like `crates/termrock-core`.
fn unlisted_directories(metadata: &serde_json::Value, directories: &[String]) -> Vec<String> {
    let workspace = metadata["workspace_root"].as_str().unwrap_or_default();
    let listed: BTreeSet<String> = metadata["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|package| package["manifest_path"].as_str())
        .map(|manifest| {
            manifest
                .strip_prefix(workspace)
                .unwrap_or(manifest)
                .trim_start_matches('/')
                .trim_end_matches("/Cargo.toml")
                .to_string()
        })
        .collect();
    directories
        .iter()
        .filter(|dir| !listed.contains(*dir))
        .map(|dir| {
            format!(
                "{dir} has a Cargo.toml but is not a workspace package, so stage B cannot check it; add it to `members` in the root Cargo.toml"
            )
        })
        .collect()
}

fn cycles(graph: &BTreeMap<String, BTreeSet<String>>) -> Vec<String> {
    fn visit(
        node: &str,
        graph: &BTreeMap<String, BTreeSet<String>>,
        stack: &mut Vec<String>,
        done: &mut BTreeSet<String>,
        out: &mut Vec<String>,
    ) {
        if let Some(at) = stack.iter().position(|n| n == node) {
            let mut cycle = stack[at..].to_vec();
            cycle.push(node.to_string());
            out.push(format!("dependency cycle: {}", cycle.join(" -> ")));
            return;
        }
        if !done.insert(node.to_string()) {
            return;
        }
        stack.push(node.to_string());
        for next in graph.get(node).into_iter().flatten() {
            visit(next, graph, stack, done, out);
        }
        stack.pop();
    }
    let mut out = Vec::new();
    let mut done = BTreeSet::new();
    for node in graph.keys() {
        visit(node, graph, &mut Vec::new(), &mut done, &mut out);
    }
    out
}

/// The `crates/termrock-*` and `apps/*` directories that hold a `Cargo.toml`.
fn package_directories() -> Vec<String> {
    let mut found = Vec::new();
    for (parent, prefix) in [("crates", Some("termrock-")), ("apps", None)] {
        let Ok(entries) = std::fs::read_dir(root().join(parent)) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().to_string();
            if prefix.is_some_and(|p| !name.starts_with(p)) {
                continue;
            }
            if entry.path().join("Cargo.toml").is_file() {
                found.push(format!("{parent}/{name}"));
            }
        }
    }
    found.sort();
    found
}

/// Stage B: the real crate graph from `cargo metadata`, checked against the table.
fn crate_level(layering: &Layering) -> Result<usize, Vec<String>> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = std::process::Command::new(cargo)
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--offline",
        ])
        .arg("--manifest-path")
        .arg(root().join("Cargo.toml"))
        .output()
        .expect("run cargo metadata");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("cargo metadata prints JSON");
    let (packages, mut problems) = check_metadata(&metadata, layering);
    problems.extend(unlisted_directories(&metadata, &package_directories()));
    if problems.is_empty() {
        Ok(packages)
    } else {
        Err(problems)
    }
}

// ---------------------------------------------------------------------------
// Tests on the real repository
// ---------------------------------------------------------------------------

fn report(what: &str, problems: &[String]) {
    assert!(problems.is_empty(), "{what}:\n  {}", problems.join("\n  "));
}

#[test]
fn layering_table_is_consistent() {
    report(
        "docs/layering.toml is inconsistent",
        &Layering::load().validate(),
    );
}
/// The repository root holds no package and no code (#439, #452): its manifest is virtual,
/// and `src/`, `tests/` and `examples/` are gone. Everything lives under `crates/` and `apps/`.
#[test]
fn layering_the_root_is_a_virtual_manifest_without_code() {
    let manifest = std::fs::read_to_string(root().join("Cargo.toml")).expect("read Cargo.toml");
    let table: toml::Table = manifest.parse().expect("the root manifest is valid TOML");
    assert!(
        !table.contains_key("package"),
        "the root Cargo.toml must have no [package]"
    );
    assert!(
        table.contains_key("workspace"),
        "the root Cargo.toml must be a workspace"
    );
    assert_eq!(
        table["workspace"]["resolver"].as_str(),
        Some("3"),
        "a virtual manifest does not infer the resolver from an edition"
    );
    for dir in ["src", "tests", "examples"] {
        assert!(
            !root().join(dir).exists(),
            "the repository root must have no `{dir}/` directory"
        );
    }
}

#[test]
fn layering_crates_follow_the_table() {
    match crate_level(&Layering::load()) {
        Ok(packages) => {
            eprintln!("layering stage B (crates) checked {packages} packages");
        }
        Err(problems) => report("layering violations (stage B, crates)", &problems),
    }
}

// ---------------------------------------------------------------------------
// Tests of the check itself, on small synthetic inputs
// ---------------------------------------------------------------------------

const MINI: &str = r#"
schema = 1
order = ["text", "core", "controls", "dialogs", "termrock", "junie-tui"]
layer_order = ["foundations", "components", "facade"]

[layers]
text = "foundations"
core = "foundations"
controls = "components"
dialogs = "components"
termrock = "facade"
"junie-tui" = "facade"

[allow]
text = []
core = ["text"]
controls = ["core"]
dialogs = ["controls"]
termrock = ["text", "core", "controls", "dialogs"]
"junie-tui" = ["termrock"]
"#;

#[test]
fn layering_table_validation_catches_a_bad_table() {
    let upward = MINI.replace("text = []", "text = [\"core\"]");
    let problems = Layering::parse(&upward).validate();
    assert!(
        problems.iter().any(|p| p.contains("not below it")),
        "{problems:?}"
    );
    let higher = MINI.replace("controls = \"components\"", "controls = \"facade\"");
    assert!(
        Layering::parse(&higher)
            .validate()
            .iter()
            .any(|p| p.contains("higher layer"))
    );
}

#[test]
fn layering_permits_what_a_dependency_already_uses() {
    let layering = Layering::parse(MINI);
    let permitted = layering.permitted("dialogs");
    for krate in ["controls", "core", "text"] {
        assert!(permitted.contains(krate), "{krate}");
    }
    assert!(!permitted.contains("termrock"));
    assert!(layering.permitted("text").is_empty());
}

fn metadata(packages: &[(&str, &str, &[&str])]) -> serde_json::Value {
    serde_json::json!({
        "workspace_root": "/w",
        "packages": packages.iter().map(|(name, manifest, deps)| serde_json::json!({
            "name": name,
            "manifest_path": format!("/w/{manifest}"),
            "dependencies": deps
                .iter()
                .map(|d| serde_json::json!({"name": d, "kind": null}))
                .collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

#[test]
fn layering_crate_level_accepts_the_table() {
    let layering = Layering::parse(MINI);
    let meta = metadata(&[
        ("termrock-text", "crates/termrock-text/Cargo.toml", &[]),
        (
            "termrock-core",
            "crates/termrock-core/Cargo.toml",
            &["termrock-text", "serde_json"],
        ),
        (
            "termrock-dialogs",
            "crates/termrock-dialogs/Cargo.toml",
            &["termrock-controls", "termrock-core"],
        ),
        (
            "termrock-controls",
            "crates/termrock-controls/Cargo.toml",
            &["termrock-core"],
        ),
        (
            "termrock",
            "crates/termrock/Cargo.toml",
            &["termrock-dialogs", "termrock-text"],
        ),
        (
            "termrock-showcase",
            "apps/showcase/Cargo.toml",
            &["termrock"],
        ),
    ]);
    let (checked, problems) = check_metadata(&meta, &layering);
    assert_eq!(problems, Vec::<String>::new());
    assert_eq!(checked, 6);
}

#[test]
fn layering_crate_level_rejects_an_upward_dependency_and_names_the_fix() {
    let layering = Layering::parse(MINI);
    let meta = metadata(&[
        (
            "termrock-controls",
            "crates/termrock-controls/Cargo.toml",
            &["termrock-core", "termrock-dialogs"],
        ),
        ("termrock-core", "crates/termrock-core/Cargo.toml", &[]),
        (
            "termrock-dialogs",
            "crates/termrock-dialogs/Cargo.toml",
            &["termrock-controls"],
        ),
    ]);
    let (_, problems) = check_metadata(&meta, &layering);
    let joined = problems.join("\n");
    assert!(
        joined.contains("`termrock-controls` must not depend on `termrock-dialogs`"),
        "{joined}"
    );
    assert!(joined.contains(FIX), "{joined}");
    assert!(joined.contains("dependency cycle"), "{joined}");
}

#[test]
fn layering_crate_level_checks_applications_and_unlisted_crates() {
    let layering = Layering::parse(MINI);
    let meta = metadata(&[
        (
            "termrock-showcase",
            "apps/showcase/Cargo.toml",
            &["termrock", "termrock-core"],
        ),
        ("termrock-holla", "apps/holla/Cargo.toml", &["ratatui"]),
        (
            "termrock-surprise",
            "crates/termrock-surprise/Cargo.toml",
            &[],
        ),
    ]);
    let (_, problems) = check_metadata(&meta, &layering);
    let joined = problems.join("\n");
    assert!(joined.contains("termrock-showcase: an application depends on `termrock` only"));
    assert!(joined.contains("termrock-holla: an application must depend on `termrock`"));
    assert!(joined.contains("termrock-surprise: not in docs/layering.toml"));
}

#[test]
fn layering_crate_level_rejects_a_directory_outside_the_workspace() {
    let meta = metadata(&[
        ("termrock-core", "crates/termrock-core/Cargo.toml", &[]),
        (
            "termrock-showcase",
            "apps/showcase/Cargo.toml",
            &["termrock"],
        ),
    ]);
    let dirs = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<Vec<_>>();
    assert!(
        unlisted_directories(&meta, &dirs(&["crates/termrock-core", "apps/showcase"])).is_empty()
    );
    let problems = unlisted_directories(
        &meta,
        &dirs(&["crates/termrock-core", "crates/termrock-ghost"]),
    );
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].contains("crates/termrock-ghost"),
        "{problems:?}"
    );
    assert!(
        problems[0].contains("not a workspace package"),
        "{problems:?}"
    );
}

#[test]
fn layering_stage_b_runs_on_the_extracted_crates() {
    let extracted: Vec<_> = std::fs::read_dir(root().join("crates"))
        .expect("crates/")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("termrock-"))
        .collect();
    assert!(
        extracted.iter().any(|name| name == "termrock-metrics"),
        "crates/termrock-metrics must exist: #444 extracted it first"
    );
    match crate_level(&Layering::load()) {
        Ok(packages) => assert!(
            packages > extracted.len(),
            "stage B checked {packages} packages, fewer than the {} extracted crates plus the facade",
            extracted.len()
        ),
        Err(problems) => panic!("{problems:?}"),
    }
}

#[test]
fn layering_crate_level_rejects_a_dependency_on_the_shim() {
    let layering = Layering::parse(MINI);
    let meta = metadata(&[
        (
            "termrock-core",
            "crates/termrock-core/Cargo.toml",
            &["junie-tui"],
        ),
        ("termrock", "crates/termrock/Cargo.toml", &["termrock-core"]),
        ("junie-tui", "crates/junie-tui/Cargo.toml", &["termrock"]),
        (
            "termrock-showcase",
            "apps/showcase/Cargo.toml",
            &["termrock", "junie-tui"],
        ),
    ]);
    let (_, problems) = check_metadata(&meta, &layering);
    let joined = problems.join("\n");
    assert!(
        joined.contains("`termrock-core` must not depend on `junie-tui`"),
        "{joined}"
    );
    assert!(
        joined.contains(
            "termrock-showcase: an application depends on `termrock` only, not on `junie-tui`"
        ),
        "{joined}"
    );
    assert!(
        !joined.contains("`junie-tui` must not depend on `termrock`"),
        "the shim re-exports the facade: {joined}"
    );
}

fn with_features(name: &str, features: serde_json::Value, dependency: &str) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "manifest_path": format!("/w/crates/{name}/Cargo.toml"),
        "features": features,
        "dependencies": [{"name": dependency, "kind": null}],
    })
}

#[test]
fn layering_feature_forwarding_covers_the_shim_and_the_facade() {
    let layering = Layering::parse(MINI);
    let facade = serde_json::json!({
        "name": "termrock",
        "manifest_path": "/w/crates/termrock/Cargo.toml",
        "features": {"cli": [], "terminal": []},
        "dependencies": [],
    });
    let forwarding = with_features(
        "junie-tui",
        serde_json::json!({"default": ["cli"], "cli": ["termrock/cli"], "terminal": ["termrock/terminal"]}),
        "termrock",
    );
    assert!(
        feature_forwarding_problems(&[facade.clone(), forwarding], &layering).is_empty(),
        "a shim that forwards every feature is accepted"
    );
    let dropping = with_features(
        "junie-tui",
        serde_json::json!({"cli": ["termrock/cli"], "terminal": []}),
        "termrock",
    );
    let problems = feature_forwarding_problems(&[facade, dropping], &layering);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].contains("junie-tui: feature `terminal` must forward to `termrock`"),
        "{problems:?}"
    );
}
