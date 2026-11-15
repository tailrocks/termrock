//! Source-only layer of the declared-API gate (#14).
//!
//! Three proof layers stay separate. This module is layer 1: it checks that
//! every declared public API has a catalog line and one valid disposition,
//! and that every capability proof names a callable scenario. The scenarios
//! themselves run in `src/bin/showcase/app_tests_catalog.rs` (layer 2) and
//! the tuiscotti journeys in `tests/functional` give visual evidence
//! (layer 3). Name matching here is never behavioral proof.

#[path = "../tests/support/api_inventory.rs"]
mod api_inventory;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use api_inventory::{ApiKind, Expansion, Inventory, MATRIX, Record, SourceTree, Value, build};
use syn::visit::Visit;

const EXPANSIONS: &[Expansion] = &[Expansion {
    site: "core::id::parts",
    ids: &[
        "core::id::Part::CONTAINER",
        "core::id::Part::BORDER",
        "core::id::Part::BACKDROP",
        "core::id::Part::GUTTER",
        "core::id::Part::MARKER",
        "core::id::Part::ICON",
        "core::id::Part::LABEL",
        "core::id::Part::META",
        "core::id::Part::HELP",
        "core::id::Part::TITLE",
        "core::id::Part::BODY",
        "core::id::Part::ACTIONS",
        "core::id::Part::FIELD",
        "core::id::Part::TEXT",
        "core::id::Part::PLACEHOLDER",
        "core::id::Part::ROW",
        "core::id::Part::CELL",
        "core::id::Part::HEADER",
        "core::id::Part::TRACK",
        "core::id::Part::THUMB",
        "core::id::Part::RULE",
        "core::id::Part::TAB",
        "core::id::Part::CLOSE",
        "core::id::Part::PREFIX",
        "core::id::Part::BADGE",
        "core::id::Part::OVERFLOW",
        "core::id::Part::NEW",
        "core::id::Part::EMPTY",
        "core::id::Part::QUERY",
        "core::id::Part::SEAM",
        "core::id::Part::SUMMARY",
        "core::id::Part::DETAIL",
        "core::id::Part::KEY",
        "core::id::Part::ACTION",
        "core::id::Part::CHANGE",
        "core::id::Part::ROW_NUMBER",
    ],
}];

/// Specimen states are not API: they stay catalog-only.
const STATE_KIND: &str = "state";

#[derive(Clone)]
struct Sources {
    tree: SourceTree,
    catalog: String,
    coverage: String,
    cases: String,
    app: String,
    debt: String,
    showcase: Vec<(String, String)>,
    tests: BTreeSet<String>,
}

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

impl Sources {
    fn load() -> Self {
        let read = |path: &str| {
            std::fs::read_to_string(root().join(path))
                .unwrap_or_else(|e| panic!("read {path}: {e}"))
        };
        let showcase = SourceTree::from_dir(root(), "src/bin/showcase");
        Self {
            tree: SourceTree::from_dir(root(), "src"),
            catalog: read("src/catalog.rs"),
            coverage: read("src/bin/showcase/coverage.rs"),
            cases: read("src/bin/showcase/app_tests_catalog.rs"),
            app: read("src/bin/showcase/app.rs"),
            debt: read("src/bin/showcase/coverage_debt.txt"),
            showcase: showcase
                .files
                .into_iter()
                .filter(|(path, _)| {
                    !path.ends_with("coverage.rs") && !path.ends_with("app_tests_catalog.rs")
                })
                .map(|(path, text)| (path.display().to_string(), text))
                .collect(),
            tests: api_inventory::test_names(root(), &["src", "tests", "apps"]),
        }
    }
}

fn rows(tree: &SourceTree) -> Result<Vec<(&'static str, Inventory)>, Vec<String>> {
    MATRIX
        .iter()
        .map(|row| build(tree, row, EXPANSIONS).map(|inv| (row.name, inv)))
        .collect()
}

fn catalog_entries(source: &str) -> Vec<(String, String)> {
    #[derive(Default)]
    struct Calls(Vec<(String, String)>);
    impl<'ast> Visit<'ast> for Calls {
        fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
            if let syn::Expr::Path(path) = node.func.as_ref()
                && path.path.segments.len() == 2
                && path.path.segments[0].ident == "Capability"
                && let Some(syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(id),
                    ..
                })) = node.args.first()
            {
                let kind = path.path.segments[1].ident.to_string();
                self.0.push((id.value(), kind));
            }
            syn::visit::visit_expr_call(self, node);
        }
    }
    let file = syn::parse_file(source).expect("catalog source parses");
    let mut calls = Calls::default();
    calls.visit_file(&file);
    calls.0
}

fn enum_variants(source: &str, name: &str) -> BTreeSet<String> {
    let file = syn::parse_file(source).expect("source parses");
    file.items
        .iter()
        .find_map(|item| match item {
            syn::Item::Enum(e) if e.ident == name => {
                Some(e.variants.iter().map(|v| v.ident.to_string()).collect())
            }
            _ => None,
        })
        .unwrap_or_default()
}

fn zero_arg_fns(source: &str) -> BTreeSet<String> {
    let file = syn::parse_file(source).expect("source parses");
    file.items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Fn(f) if f.sig.inputs.is_empty() => Some(f.sig.ident.to_string()),
            _ => None,
        })
        .collect()
}

fn idents(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .collect()
}

fn debt_ids(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.split('#').next().unwrap_or("").trim())
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect()
}

fn inherits(kind: ApiKind) -> bool {
    matches!(
        kind,
        ApiKind::Field | ApiKind::TraitItem | ApiKind::TraitImpl
    )
}

#[derive(Debug, Default)]
struct Report {
    exports: BTreeMap<String, usize>,
    proofs: usize,
    exceptions: usize,
    inherited: usize,
    evidence: usize,
    debt: usize,
    scenarios: usize,
}

fn owner_and_member<'a>(id: &'a str, inv: &Inventory) -> (Option<&'a str>, &'a str) {
    let mut cut = id.len();
    while let Some(pos) = id[..cut].rfind("::") {
        cut = pos;
        if inv
            .exports
            .get(&id[..cut])
            .is_some_and(|e| e.kind == ApiKind::Type)
        {
            let owner = &id[..cut];
            let member = id[cut + 2..].rsplit("::").next().unwrap_or(id);
            return (Some(owner), member);
        }
    }
    (None, id.rsplit("::").next().unwrap_or(id))
}

fn validate(rows: &[(&'static str, Inventory)], s: &Sources) -> (Vec<String>, Report) {
    let mut errors = Vec::new();
    let mut report = Report::default();
    let mut exports: BTreeMap<&str, ApiKind> = BTreeMap::new();
    let mut presence: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (name, inv) in rows {
        for (id, export) in &inv.exports {
            exports.insert(id, export.kind);
            presence.entry(id).or_default().insert(name);
        }
    }
    let superset = &rows.last().expect("matrix has rows").1;

    let catalog = catalog_entries(&s.catalog);
    let mut declared: BTreeSet<&str> = BTreeSet::new();
    for (id, kind) in &catalog {
        if !declared.insert(id) {
            errors.push(format!("duplicate catalog entry: {id}"));
        }
        if kind != STATE_KIND && !exports.contains_key(id.as_str()) {
            errors.push(format!(
                "catalog entry is not an exported API (stale): {id}"
            ));
        }
    }
    for (id, kind) in &exports {
        *report.exports.entry(format!("{kind:?}")).or_default() += 1;
        if !inherits(*kind) && !declared.contains(id) {
            errors.push(format!("exported API is missing from catalog: {id}"));
        }
    }

    let routes = enum_variants(&s.app, "PageId");
    let scenario_ids = enum_variants(&s.coverage, "ScenarioId");
    let case_fns = zero_arg_fns(&s.cases);
    let cases = api_inventory::records_from_source(&s.cases, "ScenarioCase");
    let mut registered: BTreeSet<String> = BTreeSet::new();
    for case in &cases {
        let id = case
            .get("id")
            .and_then(Value::leaf)
            .unwrap_or("?")
            .to_owned();
        let run = case.get("run").and_then(Value::leaf).unwrap_or("?");
        if !scenario_ids.contains(&id) {
            errors.push(format!("scenario case {id} is not a ScenarioId variant"));
        }
        if !case_fns.contains(run) {
            errors.push(format!(
                "scenario case {id} runs `{run}`, which is not an fn() in app_tests_catalog.rs"
            ));
        }
        if !registered.insert(id.clone()) {
            errors.push(format!("scenario case registered twice: {id}"));
        }
    }
    report.scenarios = registered.len();

    let mut proofs: BTreeMap<String, &Record> = BTreeMap::new();
    let proof_records = api_inventory::records_from_source(&s.coverage, "CapabilityProof");
    for record in &proof_records {
        let Some(api) = record.get("api").and_then(Value::str) else {
            errors.push("CapabilityProof without a literal api".into());
            continue;
        };
        if proofs.insert(api.to_owned(), record).is_some() {
            errors.push(format!("duplicate proof: {api}"));
        }
        if !exports.contains_key(api) {
            errors.push(format!("stale proof (not an exported API): {api}"));
        }
        let route = record.get("route").and_then(Value::leaf).unwrap_or("?");
        if !routes.contains(route) {
            errors.push(format!("proof for {api} names unknown route {route}"));
        }
        let listed = match record.get("scenarios") {
            Some(Value::List(list)) => list.iter().filter_map(Value::leaf).collect(),
            _ => Vec::new(),
        };
        if listed.is_empty() {
            errors.push(format!("proof for {api} has no scenarios"));
        }
        for scenario in listed {
            if !scenario_ids.contains(scenario) {
                errors.push(format!("proof for {api} names unknown scenario {scenario}"));
            } else if !registered.contains(scenario) {
                errors.push(format!(
                    "proof for {api} names scenario {scenario}, which has no callable case"
                ));
            }
        }
        if let Some(Value::Some(feature)) = record.get("feature")
            && let Some(feature) = feature.str()
        {
            for (name, _) in rows {
                let row = MATRIX.iter().find(|r| r.name == *name).expect("matrix row");
                let enabled = row.features.contains(&feature);
                let present = presence.get(api).is_some_and(|rows| rows.contains(name));
                if exports.contains_key(api) && enabled != present {
                    errors.push(format!(
                        "proof for {api} declares feature `{feature}` but the API is {} in row {name}",
                        if present { "present" } else { "absent" }
                    ));
                }
            }
        }
    }
    report.proofs = proofs.len();

    let mut exceptions: BTreeMap<String, &Record> = BTreeMap::new();
    let exception_records = api_inventory::records_from_source(&s.coverage, "CapabilityException");
    for record in &exception_records {
        let Some(api) = record.get("api").and_then(Value::str) else {
            errors.push("CapabilityException without a literal api".into());
            continue;
        };
        if exceptions.insert(api.to_owned(), record).is_some() {
            errors.push(format!("duplicate exception: {api}"));
        }
        if !exports.contains_key(api) {
            errors.push(format!("stale exception (not an exported API): {api}"));
        }
        let reason = record
            .get("reason")
            .and_then(Value::str)
            .unwrap_or_default();
        if reason.trim().is_empty() {
            errors.push(format!("exception has an empty reason: {api}"));
        }
        let test = record
            .get("regression_test")
            .and_then(Value::str)
            .unwrap_or_default();
        if !s.tests.contains(test) {
            errors.push(format!(
                "exception test does not exist: {api} names `{test}`, which is no #[test] fn in the repository"
            ));
        }
        let tracked = matches!(record.get("tracking_issue"), Some(Value::Some(_)));
        if reason.starts_with("Deferred") && !tracked {
            errors.push(format!("deferred exception lacks a tracking issue: {api}"));
        }
        if proofs.contains_key(api) {
            errors.push(format!("API has both a proof and an exception: {api}"));
        }
    }
    report.exceptions = exceptions.len();

    let tokens: Vec<BTreeSet<String>> = s.showcase.iter().map(|(_, text)| idents(text)).collect();
    let evidence = |id: &str| -> bool {
        let (owner, member) = owner_and_member(id, superset);
        let member = member.trim_end_matches('!');
        tokens.iter().any(|file| {
            file.contains(member)
                && owner.is_none_or(|o| file.contains(o.rsplit("::").next().unwrap_or(o)))
        })
    };

    let mut debt: BTreeSet<&str> = BTreeSet::new();
    let debt_list = debt_ids(&s.debt);
    for id in &debt_list {
        if !debt.insert(id) {
            errors.push(format!("duplicate debt entry: {id}"));
        }
    }
    for (id, kind) in &exports {
        if inherits(*kind) {
            report.inherited += 1;
            continue;
        }
        let proof = proofs.contains_key(*id);
        let exception = exceptions.contains_key(*id);
        let shown = evidence(id);
        let owed = debt.contains(id);
        if owed && (proof || exception || shown) {
            errors.push(format!(
                "debt entry is stale, the API now has a disposition (delete the line): {id}"
            ));
        }
        if proof || exception {
            continue;
        }
        if shown {
            report.evidence += 1;
        } else if owed {
            report.debt += 1;
        } else {
            errors.push(format!(
                "API has no disposition: {id}: show it on a Showcase page, add a CapabilityProof with a callable scenario, or add a CapabilityException"
            ));
        }
    }
    for id in &debt_list {
        if !exports.contains_key(id.as_str()) {
            errors.push(format!("debt entry is stale (not an exported API): {id}"));
        }
    }
    (errors, report)
}

fn check(s: &Sources) -> Vec<String> {
    match rows(&s.tree) {
        Ok(rows) => validate(&rows, s).0,
        Err(errors) => errors,
    }
}

fn expect_error(errors: &[String], needle: &str) {
    assert!(
        errors.iter().any(|e| e.contains(needle)),
        "expected an error containing `{needle}`, got:\n{}",
        errors.join("\n")
    );
}

fn replaced(text: &str, old: &str, new: &str) -> String {
    assert!(text.contains(old), "mutation target not found: {old}");
    text.replacen(old, new, 1)
}

#[test]
fn catalog_repository_gate_is_closed() {
    let s = Sources::load();
    let errors = check(&s);
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn catalog_reports_the_proof_layers_separately() {
    let s = Sources::load();
    let rows = rows(&s.tree).expect("inventory builds");
    let (errors, report) = validate(&rows, &s);
    assert!(errors.is_empty());
    let functional = std::fs::read_dir(root().join("tests/functional/baselines/functional"))
        .map(|dir| {
            dir.flatten()
                .filter(|e| e.file_name().to_string_lossy().starts_with("showcase_"))
                .count()
        })
        .unwrap_or(0);
    let all = &rows.last().expect("rows").1;
    eprintln!(
        "[source coverage] {} exports ({:?}); {} inherit their owner; {} show on a page; {} owe a disposition (shrink only, see coverage_debt.txt); {} exceptions",
        all.exports.len(),
        report.exports,
        report.inherited,
        report.evidence,
        report.debt,
        report.exceptions
    );
    eprintln!(
        "[behavioral conformance] {} proofs over {} callable scenarios",
        report.proofs, report.scenarios
    );
    eprintln!(
        "[visual evidence] {functional} Showcase captures under tests/functional/baselines/functional"
    );
    assert!(report.proofs > 0 && report.scenarios > 0);
}

#[test]
fn catalog_module_graph_covers_every_source_file() {
    let s = Sources::load();
    let rows = rows(&s.tree).expect("inventory builds");
    let all = &rows.last().expect("rows").1;
    let on_disk: BTreeSet<_> = s.tree.files.keys().cloned().collect();
    let missing: Vec<_> = on_disk
        .difference(&all.files_read)
        .filter(|path| !path.ends_with("tests.rs") && !path.ends_with("catalog_contract.rs"))
        .collect();
    assert!(
        missing.is_empty(),
        "source files outside the module graph (test-only modules are exempt): {missing:?}"
    );
    let widget_files = on_disk
        .iter()
        .filter(|p| p.starts_with("src/widgets") && p.file_name().is_some_and(|n| n != "mod.rs"))
        .count();
    let widget_modules = all
        .modules
        .iter()
        .filter(|m| m.starts_with("widgets::") && m.matches("::").count() == 1)
        .count();
    assert_eq!(
        widget_modules, widget_files,
        "widget modules counted from the graph"
    );
}

#[test]
fn catalog_rejects_unlisted_public_api() {
    let mut s = Sources::load();
    let clock = std::path::PathBuf::from("src/clock.rs");
    s.tree
        .files
        .get_mut(&clock)
        .expect("clock.rs")
        .push_str("\npub fn undeclared_probe() {}\n");
    expect_error(
        &check(&s),
        "exported API is missing from catalog: clock::undeclared_probe",
    );
}

#[test]
fn catalog_rejects_an_api_without_a_disposition() {
    let mut s = Sources::load();
    let clock = std::path::PathBuf::from("src/clock.rs");
    s.tree
        .files
        .get_mut(&clock)
        .expect("clock.rs")
        .push_str("\npub fn undeclared_probe() {}\n");
    s.catalog = replaced(
        &s.catalog,
        "static MEMBERS: &[Capability] = &[",
        "static MEMBERS: &[Capability] = &[\n    Capability::function(\"clock::undeclared_probe\"),",
    );
    expect_error(
        &check(&s),
        "API has no disposition: clock::undeclared_probe",
    );
}

#[test]
fn catalog_rejects_private_api_claim() {
    let mut s = Sources::load();
    let clock = std::path::PathBuf::from("src/clock.rs");
    let text = s.tree.files.get_mut(&clock).expect("clock.rs");
    *text = text.replacen("pub fn step_wall", "pub(crate) fn step_wall", 1);
    s.coverage.push_str(
        "\nconst CLAIM: CapabilityProof = CapabilityProof { api: \"clock::step_wall\", route: PageId::Overview, scenarios: &[ScenarioId::NarrowLayout], feature: None };\n",
    );
    let errors = check(&s);
    expect_error(
        &errors,
        "stale proof (not an exported API): clock::step_wall",
    );
    expect_error(
        &errors,
        "catalog entry is not an exported API (stale): clock::step_wall",
    );
}

#[test]
fn catalog_restricted_visibility_is_not_export() {
    let tree = SourceTree::from_files(&[(
        "src/lib.rs",
        "pub mod a { pub fn open() {} pub(crate) fn crate_only() {} pub(super) fn parent_only() {} pub(in crate::a) fn scoped() {} pub struct S { pub shown: u8, pub(crate) hidden: u8, private: u8 } }\nmod b { pub fn unreachable() {} }\n",
    )]);
    let inv = build(&tree, &MATRIX[2], &[]).expect("fixture builds");
    let ids: Vec<_> = inv.exports.keys().map(String::as_str).collect();
    assert_eq!(ids, ["a::S", "a::S::shown", "a::open"]);
}

#[test]
fn catalog_resolves_reexports_and_cfg() {
    let tree = SourceTree::from_files(&[
        (
            "src/lib.rs",
            "mod inner;\n#[cfg(feature = \"terminal\")]\npub use inner::Gated;\npub use inner::Plain as Renamed;\npub mod outer { pub use crate::inner::*; }\n",
        ),
        (
            "src/inner.rs",
            "pub struct Plain;\nimpl Plain { pub fn go(&self) {} }\n#[cfg(feature = \"terminal\")]\npub struct Gated { pub n: u8 }\n#[cfg(all(unix, not(test)))]\npub fn on_unix() {}\n#[cfg(windows)]\npub fn on_windows() {}\n",
        ),
    ]);
    let minimal = build(&tree, &MATRIX[0], &[]).expect("minimal builds");
    let all = build(&tree, &MATRIX[2], &[]).expect("all builds");
    let ids = |inv: &Inventory| inv.exports.keys().cloned().collect::<Vec<_>>();
    assert_eq!(ids(&minimal), ["Renamed", "Renamed::go", "outer::on_unix"]);
    assert!(all.exports.contains_key("Gated") && all.exports.contains_key("Gated::n"));
    assert!(!minimal.exports.contains_key("Gated"));
    assert_eq!(
        all.aliases.get("outer::Plain").map(String::as_str),
        Some("Renamed")
    );
    assert!(!all.exports.keys().any(|id| id.contains("windows")));
    assert_eq!(
        all.exports.keys().filter(|id| id.ends_with("::go")).count(),
        1,
        "aliases add no second obligation"
    );
}

#[test]
fn catalog_rejects_unsupported_public_expansion() {
    let tree = SourceTree::from_files(&[(
        "src/lib.rs",
        "macro_rules! make { () => { pub fn made() {} } }\nmake!();\n",
    )]);
    let errors = build(&tree, &MATRIX[2], &[]).expect_err("unsupported macro fails");
    expect_error(&errors, "unsupported item macro `make!`");
    expect_error(&errors, "expansion manifest");
}

#[test]
fn catalog_rejects_unsupported_cfg_and_reexports() {
    let tree = SourceTree::from_files(&[(
        "src/lib.rs",
        "#[cfg(target_os = \"linux\")]\npub fn os() {}\n",
    )]);
    expect_error(
        &build(&tree, &MATRIX[2], &[]).expect_err("cfg fails"),
        "unsupported cfg key",
    );
    let tree = SourceTree::from_files(&[("src/lib.rs", "pub use external::Thing;\n")]);
    expect_error(
        &build(&tree, &MATRIX[2], &[]).expect_err("re-export fails"),
        "unsupported public re-export `external::Thing`",
    );
}

#[test]
fn catalog_rejects_missing_callable_scenario() {
    let mut s = Sources::load();
    let case = s
        .cases
        .lines()
        .find(|l| l.contains("id: ScenarioId::NarrowLayout"))
        .expect("narrow layout case")
        .to_owned();
    s.cases = replaced(&s.cases, &case, "");
    expect_error(&check(&s), "which has no callable case");
}

#[test]
fn catalog_rejects_unregistered_and_uncallable_scenarios() {
    let mut s = Sources::load();
    let unknown = replaced(
        &s.coverage,
        "ScenarioId::NarrowLayout",
        "ScenarioId::Imaginary",
    );
    let mut mutated = s.clone();
    mutated.coverage = unknown;
    expect_error(&check(&mutated), "names unknown scenario Imaginary");

    s.cases = replaced(&s.cases, "run: narrow_layout", "run: no_such_scenario");
    expect_error(&check(&s), "runs `no_such_scenario`, which is not an fn()");
}

#[test]
fn catalog_rejects_stale_exception() {
    let mut s = Sources::load();
    s.coverage = replaced(
        &s.coverage,
        "api: \"widgets::picker::Picker::wheel\"",
        "api: \"widgets::picker::Picker::removed_method\"",
    );
    expect_error(
        &check(&s),
        "stale exception (not an exported API): widgets::picker::Picker::removed_method",
    );
}

#[test]
fn catalog_rejects_nonexistent_exception_test() {
    let mut s = Sources::load();
    let record = api_inventory::records_from_source(&s.coverage, "CapabilityException")
        .into_iter()
        .next()
        .expect("an exception");
    let Some(Value::Str(test)) = record.get("regression_test") else {
        panic!("exception names a test")
    };
    s.coverage = s.coverage.replacen(
        &format!("regression_test: \"{test}\""),
        "regression_test: \"no_such_test_anywhere\"",
        1,
    );
    expect_error(&check(&s), "names `no_such_test_anywhere`");
}

#[test]
fn catalog_rejects_proof_and_exception_on_one_api() {
    let mut s = Sources::load();
    let api = api_inventory::exceptions_from_source(&s.coverage)
        .into_keys()
        .next()
        .expect("an exception");
    s.coverage.push_str(&format!(
        "\nconst BOTH: CapabilityProof = CapabilityProof {{ api: \"{api}\", route: PageId::Overview, scenarios: &[ScenarioId::NarrowLayout], feature: None }};\n"
    ));
    expect_error(&check(&s), "API has both a proof and an exception");
}

#[test]
fn catalog_rejects_a_stale_debt_entry() {
    let mut s = Sources::load();
    s.debt.push_str("\nwidgets::picker::Picker::vanished\n");
    expect_error(&check(&s), "debt entry is stale (not an exported API)");
    let mut s = Sources::load();
    s.debt.push_str("\nwidgets::picker::Picker::new\n");
    expect_error(
        &check(&s),
        "debt entry is stale, the API now has a disposition",
    );
}

#[test]
fn catalog_part_expansion_matches_the_compiled_consumer() {
    use crate::core::id::Part;
    let listed: BTreeSet<String> = EXPANSIONS[0]
        .ids
        .iter()
        .map(|id| id.rsplit("::").next().expect("name").to_owned())
        .collect();
    let compiled: BTreeSet<String> = Part::ALL
        .iter()
        .filter_map(|part| part.name())
        .map(str::to_owned)
        .collect();
    assert_eq!(
        listed, compiled,
        "the manifest lists exactly what parts! generates"
    );
}
