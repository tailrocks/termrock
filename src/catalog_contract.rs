//! Source-only layer of the declared-API gate (#14).
//!
//! Three proof layers stay separate. This module is layer 1: it checks that
//! every declared public API has a catalog line and one valid disposition,
//! and that every capability proof names a callable scenario. The scenarios
//! themselves run in `apps/showcase/src/app_tests_catalog.rs` (layer 2) and
//! the tuiscotti journeys in `tests/functional` give visual evidence
//! (layer 3). Name matching here is never behavioral proof.

#[path = "../tests/support/api_inventory.rs"]
mod api_inventory;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[path = "../tests/support/scenario_reach.rs"]
mod scenario_reach;

use api_inventory::{
    ApiKind, Expansion, Inventory, MATRIX, Record, SourceTree, TestFn, Value, build, idents,
};
use scenario_reach::FnIndex;
use syn::visit::Visit;

const EXPANSIONS: &[Expansion] = &[
    Expansion {
        site: "core::id::parts",
        ids: &[
            "core::id::Part::CONTAINER",
            "core::id::Part::BACKDROP",
            "core::id::Part::MARKER",
            "core::id::Part::ICON",
            "core::id::Part::TITLE",
            "core::id::Part::ACTIONS",
            "core::id::Part::FIELD",
            "core::id::Part::ROW",
            "core::id::Part::CELL",
            "core::id::Part::HEADER",
            "core::id::Part::TRACK",
            "core::id::Part::THUMB",
            "core::id::Part::TAB",
            "core::id::Part::CLOSE",
            "core::id::Part::OVERFLOW",
            "core::id::Part::NEW",
            "core::id::Part::QUERY",
            "core::id::Part::SEAM",
            "core::id::Part::KEY",
            "core::id::Part::ROW_NUMBER",
        ],
    },
    Expansion {
        site: "metrics::metrics",
        ids: &[
            "metrics::Metric",
            "metrics::Metric::SegmentedLines",
            "metrics::Metric::ReflowedLines",
            "metrics::Metric::IndexRebuilds",
            "metrics::Metric::IndexExtends",
            "metrics::Metric::IndexShifts",
            "metrics::Metric::PayloadBytesCloned",
            "metrics::Metric::AllocatedBufferCells",
            "metrics::Metric::BufferCellsFilled",
            "metrics::Metric::BufferCellsCopied",
            "metrics::Metric::VisibleRowsPainted",
            "metrics::Metric::IntrinsicWidthCalls",
            "metrics::Metric::TextBytesMeasured",
            "metrics::Metric::LabelsNormalized",
            "metrics::Metric::LabelBytesNormalized",
            "metrics::Metric::ProjectionRebuilds",
            "metrics::Metric::RankRowsVisited",
            "metrics::Metric::SchedulingRecordsVisited",
            "metrics::Metric::SchedulingPolls",
            "metrics::Metric::AnnotationQueries",
            "metrics::Metric::IdentityComparisons",
            "metrics::Metric::FitVisits",
            "metrics::Metric::FitCompares",
            "metrics::Metric::DiffCellsVisited",
            "metrics::Metric::InputDispatches",
            "metrics::Metric::SourceRowsEnumerated",
            "metrics::Metric::KeyIndexInserts",
            "metrics::Metric::KeyLookups",
            "metrics::Metric::WidthRowsMeasured",
            "metrics::Metric::OverlayRowsSnapshotted",
            "metrics::Metric::LabelsRead",
            "metrics::Metric::LineCharsCounted",
            "metrics::Metric::NulBytesProbed",
            "metrics::Metric::PrefixRollbackProbes",
            "metrics::Metric::RawEventsFiltered",
            "metrics::Metric::RawEventsRead",
            "metrics::Metric::SourceRowsRead",
            "metrics::Metric::TerminalBytesFed",
            "metrics::Metric::TerminalBytesWritten",
            "metrics::Metric::TextBytesSanitized",
            "metrics::Metric::Utf8BytesValidated",
            "metrics::Metric::ALL",
            "metrics::Metric::name",
        ],
    },
];

/// The `ExceptionKind` variant that says a demo is only postponed. It is the
/// one structural signal: the reason text is prose and never decides anything.
const DEFERRED_KIND: &str = "Deferred";

/// The tracking issues the #14 deferrals are filed under (#296 to #308, one per
/// area). A `Deferred` line must name one of them: `Some(0)` or a stray number
/// is a typo, not a plan. Add an issue here when a new area is opened.
const KNOWN_TRACKING_ISSUES: &[u64] = &[
    296, 297, 298, 299, 300, 301, 302, 303, 304, 305, 306, 307, 308,
];

/// A reason shorter than this is a label (a page name), not an explanation.
const MIN_REASON_WORDS: usize = 4;

/// Specimen states are not API: they stay catalog-only.
const STATE_KIND: &str = "state";

#[derive(Clone)]
struct Sources {
    tree: SourceTree,
    catalog: String,
    coverage: String,
    /// `apps/showcase/src/coverage/*.rs`: one proof/exception file per lane.
    coverage_lanes: Vec<(String, String)>,
    cases: String,
    /// `apps/showcase/src/app_tests_catalog/*.rs`: one scenario file per lane.
    case_lanes: Vec<(String, String)>,
    app: String,
    showcase: Vec<(String, String)>,
    tests: BTreeMap<String, TestFn>,
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
        let showcase = SourceTree::from_dir(root(), "apps/showcase/src");
        let dir_files = |dir: &str, ext: &str| -> Vec<(String, String)> {
            let mut paths: Vec<PathBuf> = std::fs::read_dir(root().join(dir))
                .map(|d| d.flatten().map(|e| e.path()).collect())
                .unwrap_or_default();
            paths.sort();
            paths
                .into_iter()
                .filter(|p| p.extension().is_some_and(|e| e == ext))
                .map(|p| {
                    let name = p
                        .file_name()
                        .expect("file name")
                        .to_string_lossy()
                        .into_owned();
                    let text = std::fs::read_to_string(&p)
                        .unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
                    (name, text)
                })
                .collect()
        };
        Self {
            tree: SourceTree::from_dir(root(), "src"),
            catalog: read("src/catalog.rs"),
            coverage: read("apps/showcase/src/coverage.rs"),
            coverage_lanes: dir_files("apps/showcase/src/coverage", "rs"),
            cases: read("apps/showcase/src/app_tests_catalog.rs"),
            case_lanes: dir_files("apps/showcase/src/app_tests_catalog", "rs"),
            app: read("apps/showcase/src/app.rs"),
            showcase: showcase
                .files
                .into_iter()
                .filter(|(path, _)| {
                    let path = path.display().to_string();
                    !path.ends_with("coverage.rs")
                        && !path.ends_with("app_tests_catalog.rs")
                        && !path.contains("/coverage/")
                        && !path.contains("/app_tests_catalog/")
                })
                .map(|(path, text)| (path.display().to_string(), text))
                .collect(),
            tests: api_inventory::test_names(root(), &["src", "tests", "apps"]),
        }
    }
}

fn rows(s: &Sources) -> Result<Vec<(&'static str, Inventory)>, Vec<String>> {
    let preferred: BTreeSet<String> = catalog_entries(&s.catalog)
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    MATRIX
        .iter()
        .map(|row| build(&s.tree, row, EXPANSIONS, &preferred).map(|inv| (row.name, inv)))
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

/// What a test must say to count as naming `id`: the member, and for a member
/// of a type, that type too (in the body, or anywhere in the test's file, where
/// the `use` and the helpers that build the value live).
fn names_api(test: &TestFn, id: &str, inv: &Inventory) -> bool {
    let (owner, member) = owner_and_member(id, inv);
    let member = member.trim_end_matches('!');
    test.body.contains(member)
        && owner.is_none_or(|o| {
            let o = o.rsplit("::").next().unwrap_or(o);
            test.body.contains(o) || test.file.contains(o)
        })
}

/// Enums whose variants a scenario meets all at once, through a const that the
/// same macro builds from the one variant list (`Metric::ALL`), so naming the
/// const exercises every variant it generated.
const VARIANT_SETS: &[(&str, &str)] = &[("metrics::Metric", "ALL")];

/// Why the identifiers a scenario reaches do not show API `id`, or `None` when
/// they do. A member (method, variant, const, free fn, macro) must appear by
/// name. Its owning type need not appear when no other type exports that name,
/// because a scenario often calls a method on a value that something else
/// returned; a name that others share (`new`, `len`, `key`) needs the owner
/// named too, or any other type's call would satisfy it. A variant needs its enum beside it. A type is shown by its own name or by any of its methods,
/// variants, consts or trait items, since a scenario meets most types only
/// through what returns them. Fields and trait items inherit their owner and
/// never reach here as an API of their own. This is an identifier check, not behavioral proof.
fn unexercised(
    id: &str,
    kind: ApiKind,
    exports: &BTreeMap<&str, ApiKind>,
    inv: &Inventory,
    tokens: &BTreeSet<String>,
) -> Option<String> {
    let last = |id: &str| {
        id.rsplit("::")
            .next()
            .unwrap_or(id)
            .trim_end_matches('!')
            .to_owned()
    };
    if kind == ApiKind::Type {
        let own = last(id);
        let prefix = format!("{id}::");
        let shown = tokens.contains(&own)
            || exports
                .iter()
                .filter(|(member, kind)| {
                    member.starts_with(&prefix)
                        && (!inherits(**kind) || **kind == ApiKind::TraitItem)
                })
                .any(|(member, _)| tokens.contains(&last(member)));
        return (!shown).then(|| {
            format!("neither the type `{own}` nor any of its methods, variants, consts or trait items appears")
        });
    }
    let (owner, member) = owner_and_member(id, inv);
    let member = member.trim_end_matches('!');
    if kind == ApiKind::Variant
        && let Some((enum_id, _)) = id.rsplit_once("::")
    {
        // A bare `Open` is as likely to be another type's `Open`, so a variant
        // is shown by its `Enum::Variant` path, or by its own name beside its
        // enum's (a `use Enum::{Variant}` import).
        let enum_name = last(enum_id);
        let qualified = format!("{enum_name}::{member}");
        let shown =
            tokens.contains(&qualified) || (tokens.contains(member) && tokens.contains(&enum_name));
        return (!shown).then(|| format!("`{qualified}` never appears"));
    }
    if tokens.contains(member) {
        // A name that other exported types also use (`new`, `len`, `key`,
        // `render`) is satisfied by any of their calls, so the owner must be
        // named in the scenario as well. A name only this owner has stays
        // shown by itself, since the value often comes back from a call.
        if let Some(owner) = owner
            && is_shared_member(id, owner, member, exports)
            && !tokens.contains(&last(owner))
        {
            return Some(format!(
                "`{member}` appears, but other types export a `{member}` too and the owner `{}` is never named",
                last(owner)
            ));
        }
        return None;
    }
    if kind == ApiKind::Expansion
        && let Some((enum_id, _)) = id.rsplit_once("::")
        && let Some((_, set)) = VARIANT_SETS
            .iter()
            .find(|(set_enum, _)| *set_enum == enum_id)
        && tokens.contains(*set)
        && tokens.contains(last(enum_id).as_str())
    {
        return None;
    }
    Some(format!("`{member}` never appears"))
}

/// Whether another type exports a callable or const member named `member`.
fn is_shared_member(
    id: &str,
    owner: &str,
    member: &str,
    exports: &BTreeMap<&str, ApiKind>,
) -> bool {
    let own_prefix = format!("{owner}::");
    exports.iter().any(|(other, kind)| {
        *other != id
            && !other.starts_with(&own_prefix)
            && matches!(
                kind,
                ApiKind::Function | ApiKind::Const | ApiKind::TraitItem
            )
            && other.rsplit("::").next() == Some(member)
    })
}

fn mention_needed(id: &str, inv: &Inventory) -> String {
    let (owner, member) = owner_and_member(id, inv);
    let member = member.trim_end_matches('!');
    match owner {
        Some(o) => format!("{}::{member}", o.rsplit("::").next().unwrap_or(o)),
        None => member.to_owned(),
    }
}

impl Sources {
    /// The files that hold `CapabilityProof` and `CapabilityException` records.
    fn coverage_sources(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.coverage.as_str())
            .chain(self.coverage_lanes.iter().map(|(_, text)| text.as_str()))
    }

    /// The files that hold `ScenarioCase` records and scenario functions.
    fn case_sources(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.cases.as_str())
            .chain(self.case_lanes.iter().map(|(_, text)| text.as_str()))
    }

    fn records(&self, name: &str) -> Vec<Record> {
        self.coverage_sources()
            .flat_map(|text| api_inventory::records_from_source(text, name))
            .collect()
    }
}

/// A lane file that the Showcase never compiles would let records pass the
/// source gate that no build ever sees. Check each is declared and chained.
fn lane_wiring_errors(s: &Sources) -> Vec<String> {
    let mut errors = Vec::new();
    for (file, _) in &s.coverage_lanes {
        let stem = file.trim_end_matches(".rs");
        for needle in [
            format!("#[path = \"coverage/{file}\"]\nmod {stem};"),
            format!("{stem}::PROOFS"),
            format!("{stem}::EXCEPTIONS"),
        ] {
            if !s.coverage.contains(&needle) {
                errors.push(format!(
                    "lane file coverage/{file} is not wired into coverage.rs (missing `{needle}`)"
                ));
            }
        }
    }
    for (file, _) in &s.case_lanes {
        let stem = file.trim_end_matches(".rs");
        for needle in [
            format!("#[path = \"app_tests_catalog/{file}\"]\nmod lane_{stem};"),
            format!("lane_{stem}::CASES"),
        ] {
            if !s.cases.contains(&needle) {
                errors.push(format!(
                    "scenario file app_tests_catalog/{file} is not wired into app_tests_catalog.rs (missing `{needle}`)"
                ));
            }
        }
    }
    errors
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
    let mut errors = lane_wiring_errors(s);
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
    let case_fns: BTreeSet<String> = s.case_sources().flat_map(zero_arg_fns).collect();
    let cases: Vec<Record> = s
        .case_sources()
        .flat_map(|text| api_inventory::records_from_source(text, "ScenarioCase"))
        .collect();
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
    let index = FnIndex::from_sources(
        s.case_sources(),
        s.showcase
            .iter()
            .map(|(path, text)| (path.as_str(), text.as_str())),
    );
    let reach: BTreeMap<String, BTreeSet<String>> = cases
        .iter()
        .filter_map(|case| {
            let id = case.get("id").and_then(Value::leaf)?;
            let run = case.get("run").and_then(Value::leaf)?;
            Some((id.to_owned(), index.reach(run)))
        })
        .collect();

    let mut proofs: BTreeMap<String, &Record> = BTreeMap::new();
    let proof_records = s.records("CapabilityProof");
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
        for scenario in &listed {
            if !scenario_ids.contains(*scenario) {
                errors.push(format!("proof for {api} names unknown scenario {scenario}"));
            } else if !registered.contains(*scenario) {
                errors.push(format!(
                    "proof for {api} names scenario {scenario}, which has no callable case"
                ));
            }
        }
        let reached: BTreeSet<String> = listed
            .iter()
            .filter_map(|scenario| reach.get(*scenario))
            .flatten()
            .cloned()
            .collect();
        if let Some(kind) = exports.get(api)
            && let Some(why) = unexercised(api, *kind, &exports, superset, &reached)
        {
            errors.push(format!(
                "proof for {api} names scenarios {listed:?} that never exercise it: {why}"
            ));
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

    let kinds = enum_variants(&s.coverage, "ExceptionKind");
    let mut exceptions: BTreeMap<String, &Record> = BTreeMap::new();
    let exception_records = s.records("CapabilityException");
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
        let words = reason.split_whitespace().count();
        if words < MIN_REASON_WORDS {
            errors.push(format!(
                "exception reason is a bare label, not a reason: {api} says `{reason}` ({words} word(s), at least {MIN_REASON_WORDS} wanted)"
            ));
        }
        let kind = record.get("kind").and_then(Value::leaf).unwrap_or("?");
        if !kinds.contains(kind) {
            errors.push(format!(
                "exception has no known kind: {api} says `{kind}`, expected one of {kinds:?}"
            ));
        }
        let test = record
            .get("regression_test")
            .and_then(Value::str)
            .unwrap_or_default();
        match s.tests.get(test) {
            None => errors.push(format!(
                "exception test does not exist: {api} names `{test}`, which is no #[test] fn at that `path/to/file.rs::name`"
            )),
            Some(found) if found.ignored => errors.push(format!(
                "exception test is ignored: {api} names `{test}`, which never runs"
            )),
            Some(found) => {
                if !names_api(found, api, superset) {
                    errors.push(format!(
                        "exception test does not mention the API: {api} names `{test}`, whose body never uses `{}`",
                        mention_needed(api, superset)
                    ));
                }
            }
        }
        if kind == DEFERRED_KIND {
            match record.get("tracking_issue") {
                Some(Value::Some(issue)) => match **issue {
                    Value::Num(n) if KNOWN_TRACKING_ISSUES.contains(&n) => {}
                    Value::Num(n) => errors.push(format!(
                        "deferred exception names an unknown tracking issue: {api} says #{n}, expected one of {KNOWN_TRACKING_ISSUES:?}"
                    )),
                    _ => errors.push(format!(
                        "deferred exception has a tracking issue that is not a number: {api}"
                    )),
                },
                _ => errors.push(format!("deferred exception lacks a tracking issue: {api}")),
            }
        }
        if proofs.contains_key(api) {
            errors.push(format!("API has both a proof and an exception: {api}"));
        }
    }
    report.exceptions = exceptions.len();

    for (id, kind) in &exports {
        if inherits(*kind) {
            report.inherited += 1;
            continue;
        }
        if !proofs.contains_key(*id) && !exceptions.contains_key(*id) {
            errors.push(format!(
                "API has no disposition: {id}: add a CapabilityProof with a callable scenario or a CapabilityException; a Showcase page that merely names it is not proof"
            ));
        }
    }
    (errors, report)
}

fn check(s: &Sources) -> Vec<String> {
    match rows(s) {
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
    let rows = rows(&s).expect("inventory builds");
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
        "[source coverage] {} exports ({:?}); {} inherit their owner; {} proofs, {} exceptions",
        all.exports.len(),
        report.exports,
        report.inherited,
        report.proofs,
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
    let rows = rows(&s).expect("inventory builds");
    let all = &rows.last().expect("rows").1;
    let on_disk: BTreeSet<_> = s.tree.files.keys().cloned().collect();
    let missing = all.orphan_files(&s.tree);
    assert!(
        missing.is_empty(),
        "source files outside the module graph and not declared as `#[cfg(test)] mod`: {missing:?}"
    );
    // A widget file is a module of its own or a `#[path]` child of another
    // widget; either way the graph reads it, and a test-only file is excluded.
    let widget_files: Vec<_> = on_disk
        .iter()
        .filter(|p| {
            p.parent() == Some(Path::new("src/widgets"))
                && p.file_name().is_some_and(|n| n != "mod.rs")
                && !all.test_only_files.contains(*p)
        })
        .collect();
    for file in &widget_files {
        assert!(all.files_read.contains(*file), "{file:?} is not read");
    }
    let own_modules = widget_files
        .iter()
        .filter(|p| {
            let stem = p.file_stem().and_then(|n| n.to_str()).unwrap_or_default();
            all.modules.contains(&format!("widgets::{stem}"))
        })
        .count();
    let widget_modules = all
        .modules
        .iter()
        .filter(|m| m.starts_with("widgets::") && m.matches("::").count() == 1)
        .count();
    assert_eq!(
        widget_modules, own_modules,
        "widget modules counted from the graph"
    );
}

const PROBE_FILE: &str = "src/zz_probe.rs";
const PROBE_SOURCE: &str = "pub fn probe_fn() {}\n";

fn with_probe_module(declaration: &str) -> Sources {
    let mut s = Sources::load();
    s.tree
        .files
        .insert(PathBuf::from(PROBE_FILE), PROBE_SOURCE.to_owned());
    if !declaration.is_empty() {
        s.tree
            .files
            .get_mut(Path::new("src/lib.rs"))
            .expect("lib.rs")
            .push_str(&format!("\n{declaration}\n"));
    }
    s
}

fn orphans(s: &Sources) -> Vec<PathBuf> {
    let rows = rows(s).expect("inventory builds");
    rows.last().expect("rows").1.orphan_files(&s.tree)
}

#[test]
fn catalog_cfg_test_module_file_is_test_only_not_an_orphan() {
    let s = with_probe_module("#[cfg(test)]\nmod zz_probe;");
    assert!(orphans(&s).is_empty(), "{:?}", orphans(&s));
    let rows = rows(&s).expect("inventory builds");
    let all = &rows.last().expect("rows").1;
    assert!(all.test_only_files.contains(Path::new(PROBE_FILE)));
    assert!(!all.files_read.contains(Path::new(PROBE_FILE)));
    assert!(!all.exports.keys().any(|id| id.contains("probe_fn")));
}

#[test]
fn catalog_cfg_test_module_children_are_test_only_too() {
    let mut s = with_probe_module("#[cfg(test)]\nmod zz_probe;");
    s.tree.files.insert(
        PathBuf::from(PROBE_FILE),
        "mod inner;\npub fn probe_fn() {}\n".to_owned(),
    );
    s.tree
        .files
        .insert(PathBuf::from("src/zz_probe/inner.rs"), String::new());
    assert!(orphans(&s).is_empty(), "{:?}", orphans(&s));
}

#[test]
fn catalog_orphan_source_file_still_fails() {
    let s = with_probe_module("");
    assert_eq!(orphans(&s), vec![PathBuf::from(PROBE_FILE)]);
}

#[test]
fn catalog_feature_gated_module_is_not_test_only() {
    let s = with_probe_module("#[cfg(feature = \"no-such-feature\")]\nmod zz_probe;");
    assert_eq!(orphans(&s), vec![PathBuf::from(PROBE_FILE)]);
}

#[test]
fn catalog_non_test_module_still_requires_its_disposition() {
    let s = with_probe_module("pub mod zz_probe;");
    assert!(orphans(&s).is_empty());
    expect_error(
        &check(&s),
        "exported API is missing from catalog: zz_probe::probe_fn",
    );
    let gated = with_probe_module("#[cfg(test)]\npub mod zz_probe;");
    assert!(check(&gated).is_empty(), "{}", check(&gated).join("\n"));
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
    let inv = build(&tree, &MATRIX[2], &[], &BTreeSet::new()).expect("fixture builds");
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
    let minimal = build(&tree, &MATRIX[0], &[], &BTreeSet::new()).expect("minimal builds");
    let all = build(&tree, &MATRIX[2], &[], &BTreeSet::new()).expect("all builds");
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

const MOVED_HOME: &str = "pub mod new_home { pub struct Moved; impl Moved { pub fn go(&self) {} } }\npub mod old_home { pub use crate::new_home::Moved; }\n";

fn moved_ids(preferred: &[&str]) -> Inventory {
    let tree = SourceTree::from_files(&[("src/lib.rs", MOVED_HOME)]);
    let preferred = preferred.iter().map(|id| (*id).to_owned()).collect();
    build(&tree, &MATRIX[2], &[], &preferred).expect("fixture builds")
}

#[test]
fn catalog_a_moved_definition_keeps_the_spelling_the_catalog_already_has() {
    let kept = moved_ids(&["old_home::Moved"]);
    let ids: Vec<_> = kept.exports.keys().map(String::as_str).collect();
    assert_eq!(ids, ["old_home::Moved", "old_home::Moved::go"]);
    assert_eq!(
        kept.aliases.get("new_home::Moved").map(String::as_str),
        Some("old_home::Moved"),
        "the other spelling stays an alias"
    );
}

#[test]
fn catalog_without_a_preferred_spelling_the_definition_path_wins() {
    let ids: Vec<_> = moved_ids(&[]).exports.into_keys().collect();
    assert_eq!(ids, ["new_home::Moved", "new_home::Moved::go"]);
    let unrelated: Vec<_> = moved_ids(&["elsewhere::Moved"])
        .exports
        .into_keys()
        .collect();
    assert_eq!(
        unrelated, ids,
        "a preferred name that is no spelling is ignored"
    );
}

#[test]
fn catalog_the_catalog_spelling_leaves_the_repository_ids_unchanged() {
    let s = Sources::load();
    let with_catalog = rows(&s).expect("inventory builds");
    for (name, inv) in &with_catalog {
        let row = MATRIX.iter().find(|r| r.name == *name).expect("row");
        let bare = build(&s.tree, row, EXPANSIONS, &BTreeSet::new()).expect("builds");
        assert_eq!(
            inv.exports.keys().collect::<Vec<_>>(),
            bare.exports.keys().collect::<Vec<_>>(),
            "row {name}"
        );
    }
}

#[test]
fn catalog_a_moved_definition_adds_no_second_obligation() {
    let inv = moved_ids(&["old_home::Moved"]);
    assert!(inv.exports.contains_key("old_home::Moved"));
    assert!(
        !inv.exports.contains_key("new_home::Moved"),
        "a second spelling adds no second obligation"
    );
}

#[test]
fn catalog_rejects_unsupported_public_expansion() {
    let tree = SourceTree::from_files(&[(
        "src/lib.rs",
        "macro_rules! make { () => { pub fn made() {} } }\nmake!();\n",
    )]);
    let errors =
        build(&tree, &MATRIX[2], &[], &BTreeSet::new()).expect_err("unsupported macro fails");
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
        &build(&tree, &MATRIX[2], &[], &BTreeSet::new()).expect_err("cfg fails"),
        "unsupported cfg key",
    );
    let tree = SourceTree::from_files(&[("src/lib.rs", "pub use external::Thing;\n")]);
    expect_error(
        &build(&tree, &MATRIX[2], &[], &BTreeSet::new()).expect_err("re-export fails"),
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

const PROBE: &str = "\npub fn undeclared_probe() {}\n";

fn with_probe(s: &mut Sources) {
    let clock = std::path::PathBuf::from("src/clock.rs");
    s.tree
        .files
        .get_mut(&clock)
        .expect("clock.rs")
        .push_str(PROBE);
    s.catalog = replaced(
        &s.catalog,
        "static MEMBERS: &[Capability] = &[",
        "static MEMBERS: &[Capability] = &[\n    Capability::function(\"clock::undeclared_probe\"),",
    );
}

#[test]
fn catalog_name_match_is_not_a_disposition() {
    let mut s = Sources::load();
    let clock = std::path::PathBuf::from("src/clock.rs");
    s.tree
        .files
        .get_mut(&clock)
        .expect("clock.rs")
        .push_str("\npub fn render() {}\npub fn area() {}\n");
    s.catalog = replaced(
        &s.catalog,
        "static MEMBERS: &[Capability] = &[",
        "static MEMBERS: &[Capability] = &[\n    Capability::function(\"clock::render\"),\n    Capability::function(\"clock::area\"),",
    );
    let text = s
        .showcase
        .iter()
        .map(|(_, t)| idents(t))
        .collect::<Vec<_>>();
    assert!(
        text.iter()
            .any(|t| t.contains("render") && t.contains("area") && t.contains("Clock")),
        "the probe names must collide with Showcase text for this mutation to mean anything"
    );
    let errors = check(&s);
    expect_error(&errors, "API has no disposition: clock::render");
    expect_error(&errors, "API has no disposition: clock::area");
}

#[test]
fn catalog_rejects_an_exception_test_in_another_file() {
    let mut s = Sources::load();
    s.coverage = replaced(
        &s.coverage,
        "tests/forward_policy.rs::one_line_is_sent_and_many_lines_ask_first",
        "tests/terminal.rs::one_line_is_sent_and_many_lines_ask_first",
    );
    expect_error(&check(&s), "which is no #[test] fn at that");
    let mut s = Sources::load();
    s.coverage = replaced(
        &s.coverage,
        "tests/forward_policy.rs::one_line_is_sent_and_many_lines_ask_first",
        "one_line_is_sent_and_many_lines_ask_first",
    );
    expect_error(&check(&s), "which is no #[test] fn at that");
}

#[test]
fn catalog_rejects_an_ignored_exception_test() {
    let mut s = Sources::load();
    s.tests
        .get_mut("tests/forward_policy.rs::one_line_is_sent_and_many_lines_ask_first")
        .expect("the named test exists")
        .ignored = true;
    expect_error(&check(&s), "which never runs");
}

#[test]
fn catalog_test_scan_sees_ignore_and_the_owning_file() {
    let mut found = BTreeMap::new();
    api_inventory::scan_tests(
        "a.rs",
        "#[test] fn live() {}\n#[test] #[ignore] fn off() {}\n#[test] #[ignore = \"slow\"] fn off_why() {}\n#[cfg_attr(unix, ignore)] #[test] fn off_unix() {}\nmod m { #[test] fn nested() {} }\nfn plain() {}\n",
        &mut found,
    );
    api_inventory::scan_tests("b.rs", "#[test] fn live() {}", &mut found);
    let got: Vec<_> = found.iter().map(|(k, v)| (k.as_str(), v.ignored)).collect();
    assert_eq!(
        got,
        [
            ("a.rs::live", false),
            ("a.rs::nested", false),
            ("a.rs::off", true),
            ("a.rs::off_unix", true),
            ("a.rs::off_why", true),
            ("b.rs::live", false),
        ]
    );
}

#[test]
fn catalog_test_scan_reads_the_body_including_macro_arguments() {
    let mut found = BTreeMap::new();
    api_inventory::scan_tests(
        "a.rs",
        "use owner::Thing;\n#[test] fn t() { let v = make(); assert_eq!(v.member_in_macro(), 1); v.plain_call(); }\nfn helper() { unrelated_helper(); }\n",
        &mut found,
    );
    let t = &found["a.rs::t"];
    for name in ["make", "member_in_macro", "plain_call", "assert_eq"] {
        assert!(t.body.contains(name), "{name} is in the body");
    }
    assert!(
        !t.body.contains("unrelated_helper"),
        "a helper is not the test"
    );
    assert!(t.file.contains("Thing"), "the file's own idents ride along");
}

/// A probe API with a catalog line, so only its disposition is in question.
fn with_probe_exception(kind: &str, reason: &str, test: &str, issue: &str) -> Sources {
    let mut s = Sources::load();
    with_probe(&mut s);
    s.coverage.push_str(&format!(
        "\nconst PROBE_EXCEPTION: CapabilityException = CapabilityException {{ api: \"clock::undeclared_probe\", kind: ExceptionKind::{kind}, reason: \"{reason}\", regression_test: \"{test}\", tracking_issue: {issue} }};\n"
    ));
    s
}

const UNRELATED_TEST: &str = "src/ui/layer.rs::settle_drains_every_queued_result_in_order";
const PROBE_REASON: &str =
    "No page can reach this helper because the host calls it between frames.";

#[test]
fn catalog_rejects_a_bare_page_name_with_an_unrelated_test() {
    let s = with_probe_exception("Nonvisual", "grid", UNRELATED_TEST, "None");
    let errors = check(&s);
    expect_error(&errors, "exception reason is a bare label");
    expect_error(
        &errors,
        "exception test does not mention the API: clock::undeclared_probe",
    );
}

#[test]
fn catalog_rejects_an_exception_test_that_never_names_the_api() {
    let s = with_probe_exception("Nonvisual", PROBE_REASON, UNRELATED_TEST, "None");
    let errors = check(&s);
    expect_error(&errors, "whose body never uses `undeclared_probe`");
    assert!(
        errors.iter().all(|e| !e.contains("bare label")),
        "{errors:?}"
    );
}

#[test]
fn catalog_accepts_an_exception_whose_test_names_the_api() {
    let mut s = with_probe_exception(
        "Nonvisual",
        PROBE_REASON,
        "src/clock.rs::probe_test",
        "None",
    );
    s.tests.insert(
        "src/clock.rs::probe_test".into(),
        TestFn {
            body: ["undeclared_probe".to_owned()].into(),
            ..TestFn::default()
        },
    );
    assert!(check(&s).is_empty(), "{}", check(&s).join("\n"));
}

#[test]
fn catalog_test_scan_ignores_string_literals_and_comments() {
    let mut found = BTreeMap::new();
    api_inventory::scan_tests(
        "a.rs",
        "// Owner in a comment\nuse owner::Thing;\n#[test] fn t() { assert_eq!(1, 1, \"in_a_string\"); assert!(real_call(\"also_a_string\", r#\"raw_string\"#, 'c'), \"msg {inline}\"); /* in_a_block_comment */ // in_a_line_comment\n}\n",
        &mut found,
    );
    let t = &found["a.rs::t"];
    assert!(t.body.contains("real_call"));
    for name in [
        "in_a_string",
        "also_a_string",
        "raw_string",
        "in_a_block_comment",
        "in_a_line_comment",
        "inline",
    ] {
        assert!(
            !t.body.contains(name),
            "{name} is not an identifier of the test"
        );
    }
    assert!(t.file.contains("Thing"));
    assert!(!t.file.contains("Owner"), "a comment is not the owner type");
}

#[test]
fn catalog_owner_type_must_appear_in_the_test_or_its_file() {
    let s = Sources::load();
    let rows = rows(&s).expect("inventory builds");
    let inv = &rows.last().expect("rows").1;
    let id = "widgets::table::DataTable::rows";
    let test = |body: &[&str], file: &[&str]| TestFn {
        ignored: false,
        body: body.iter().map(|n| (*n).to_owned()).collect(),
        file: std::rc::Rc::new(file.iter().map(|n| (*n).to_owned()).collect()),
    };
    assert!(
        !names_api(&test(&["rows"], &["Grid"]), id, inv),
        "a Grid::rows test is not DataTable::rows"
    );
    assert!(!names_api(&test(&[], &["DataTable"]), id, inv));
    assert!(names_api(&test(&["rows"], &["DataTable"]), id, inv));
    assert!(names_api(&test(&["rows", "DataTable"], &[]), id, inv));
    assert!(names_api(
        &test(&["suspended"], &[]),
        "metrics::suspended",
        inv
    ));
}

#[test]
fn catalog_deferred_kind_requires_a_tracking_issue() {
    let s = with_probe_exception("Deferred", PROBE_REASON, "src/clock.rs::probe_test", "None");
    expect_error(
        &check(&s),
        "deferred exception lacks a tracking issue: clock::undeclared_probe",
    );
    let mut s = with_probe_exception(
        "Deferred",
        PROBE_REASON,
        "src/clock.rs::probe_test",
        "Some(296)",
    );
    s.tests.insert(
        "src/clock.rs::probe_test".into(),
        TestFn {
            body: ["undeclared_probe".to_owned()].into(),
            ..TestFn::default()
        },
    );
    assert!(check(&s).is_empty(), "{}", check(&s).join("\n"));
}

#[test]
fn catalog_deferred_tracking_issue_must_be_a_known_issue() {
    for issue in ["Some(0)", "Some(14)", "Some(309)"] {
        let s = with_probe_exception("Deferred", PROBE_REASON, "src/clock.rs::probe_test", issue);
        expect_error(
            &check(&s),
            "deferred exception names an unknown tracking issue: clock::undeclared_probe",
        );
    }
}

#[test]
fn catalog_deferral_is_decided_by_the_kind_and_not_by_the_reason_text() {
    let mut s = with_probe_exception(
        "Nonvisual",
        "No callable scenario drives this helper yet, so a demo is still to come.",
        "src/clock.rs::probe_test",
        "None",
    );
    s.tests.insert(
        "src/clock.rs::probe_test".into(),
        TestFn {
            body: ["undeclared_probe".to_owned()].into(),
            ..TestFn::default()
        },
    );
    assert!(
        check(&s).is_empty(),
        "prose is never read for deferral: {}",
        check(&s).join("\n")
    );
    let s = with_probe_exception("Whenever", PROBE_REASON, UNRELATED_TEST, "None");
    expect_error(
        &check(&s),
        "exception has no known kind: clock::undeclared_probe",
    );
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
fn catalog_metric_expansion_matches_the_compiled_consumer() {
    use crate::metrics::Metric;
    let manifest = EXPANSIONS
        .iter()
        .find(|e| e.site == "metrics::metrics")
        .expect("the metrics! manifest");
    let listed: BTreeSet<&str> = manifest.ids.iter().copied().collect();
    // The enum and its two members are what `metrics!` generates. The variants
    // come from the compiled `Metric::ALL`, so a metric added or deleted there
    // changes this set and the manifest has to follow.
    let mut compiled: BTreeSet<String> = Metric::ALL
        .iter()
        .map(|metric| format!("metrics::Metric::{metric:?}"))
        .collect();
    compiled.insert("metrics::Metric".into());
    compiled.insert("metrics::Metric::ALL".into());
    compiled.insert("metrics::Metric::name".into());
    let compiled: BTreeSet<&str> = compiled.iter().map(String::as_str).collect();
    assert_eq!(
        listed, compiled,
        "the manifest lists exactly what metrics! generates"
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

#[test]
fn catalog_allowlist_records_are_unique_exact_ids_with_a_reason() {
    use crate::catalog::{ALLOWLIST, Allowed};
    let mut seen = BTreeSet::new();
    for entry in ALLOWLIST {
        let entry: &Allowed = entry;
        assert!(seen.insert(entry.id), "{} is listed twice", entry.id);
        assert!(
            !entry.id.contains('*') && entry.id.contains("::"),
            "an exact id, never a prefix: {}",
            entry.id
        );
        assert!(
            entry.reason.split_whitespace().count() >= MIN_REASON_WORDS,
            "{} needs a reason, got `{}`",
            entry.id,
            entry.reason
        );
    }
}

/// A lane file by name, for the mutation tests below.
fn lane_file<'a>(s: &'a mut Sources, name: &str) -> &'a mut String {
    &mut s
        .coverage_lanes
        .iter_mut()
        .find(|(file, _)| file == name)
        .unwrap_or_else(|| panic!("lane file {name}"))
        .1
}

fn lane_proof(api: &str) -> String {
    format!(
        "\nconst LANE_PROBE: CapabilityProof = CapabilityProof {{ api: \"{api}\", route: PageId::Overview, scenarios: &[ScenarioId::NarrowLayout], feature: None }};\n"
    )
}

#[test]
fn catalog_lane_files_exist_and_are_wired() {
    let s = Sources::load();
    assert!(!s.coverage_lanes.is_empty(), "coverage/ holds lane files");
    assert!(
        !s.case_lanes.is_empty(),
        "app_tests_catalog/ holds lane files"
    );
    assert!(lane_wiring_errors(&s).is_empty());
}

#[test]
fn catalog_accepts_a_proof_recorded_in_a_lane_file() {
    let mut s = Sources::load();
    with_probe(&mut s);
    expect_error(
        &check(&s),
        "API has no disposition: clock::undeclared_probe",
    );
    lane_file(&mut s, "lane_shell.rs").push_str(&lane_proof("clock::undeclared_probe"));
    s.cases = replaced(
        &s.cases,
        "fn narrow_layout() {",
        "fn narrow_layout() {\n    undeclared_probe();",
    );
    let errors = check(&s);
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn catalog_rejects_a_duplicate_proof_across_files() {
    let mut s = Sources::load();
    let api = api_inventory::proof_ids_from_source(&s.coverage)
        .into_iter()
        .next()
        .expect("a proof in coverage.rs");
    lane_file(&mut s, "lane_theme.rs").push_str(&lane_proof(&api));
    expect_error(&check(&s), &format!("duplicate proof: {api}"));
}

#[test]
fn catalog_rejects_a_lane_exception_with_a_missing_test() {
    let mut s = Sources::load();
    with_probe(&mut s);
    let api = "clock::undeclared_probe";
    lane_file(&mut s, "lane_state.rs").push_str(&format!(
        "\nconst LANE_PROBE: CapabilityException = CapabilityException {{ api: \"{api}\", kind: ExceptionKind::Nonvisual, reason: \"A lane exception that names no real test.\", regression_test: \"src/lib.rs::no_such_test\", tracking_issue: None }};\n"
    ));
    expect_error(&check(&s), "which is no #[test] fn at that");
}

#[test]
fn catalog_rejects_a_lane_case_registered_twice() {
    let mut s = Sources::load();
    s.case_lanes
        .iter_mut()
        .find(|(file, _)| file == "layout.rs")
        .expect("layout case file")
        .1
        .push_str("\nconst LANE_PROBE: ScenarioCase = ScenarioCase { id: ScenarioId::NarrowLayout, run: narrow_layout };\n");
    expect_error(&check(&s), "scenario case registered twice: NarrowLayout");
}

#[test]
fn catalog_rejects_an_unwired_lane_file() {
    let mut s = Sources::load();
    s.coverage_lanes
        .push(("lane_probe.rs".into(), String::new()));
    expect_error(
        &check(&s),
        "lane file coverage/lane_probe.rs is not wired into coverage.rs",
    );
    let mut s = Sources::load();
    s.case_lanes.push(("probe.rs".into(), String::new()));
    expect_error(
        &check(&s),
        "scenario file app_tests_catalog/probe.rs is not wired",
    );
}

const TYPE_PROBE: &str = "\npub struct ProbeKind;\nimpl ProbeKind { pub fn ping(&self) {} pub fn solo(&self) {} }\npub struct ProbeOther;\nimpl ProbeOther { pub fn ping(&self) {} }\npub enum ProbeEnum { Open, Close }\n";

/// The errors of a tree where clock.rs declares the probe APIs, a lane file
/// proves `apis` through `NarrowLayout`, and that scenario's fn runs `body`
/// beside the extra items `items`.
fn probe_errors(apis: &[&str], body: &str, items: &str) -> Vec<String> {
    let mut s = Sources::load();
    with_probe(&mut s);
    s.tree
        .files
        .get_mut(&std::path::PathBuf::from("src/clock.rs"))
        .expect("clock.rs")
        .push_str(TYPE_PROBE);
    s.catalog = replaced(
        &s.catalog,
        "static MEMBERS: &[Capability] = &[",
        "static MEMBERS: &[Capability] = &[\n    Capability::ty(\"clock::ProbeKind\"),\n    Capability::function(\"clock::ProbeKind::ping\"),\n    Capability::function(\"clock::ProbeKind::solo\"),\n    Capability::ty(\"clock::ProbeOther\"),\n    Capability::function(\"clock::ProbeOther::ping\"),\n    Capability::ty(\"clock::ProbeEnum\"),\n    Capability::variant(\"clock::ProbeEnum::Open\"),\n    Capability::variant(\"clock::ProbeEnum::Close\"),",
    );
    for api in apis {
        lane_file(&mut s, "lane_shell.rs").push_str(&lane_proof(api));
    }
    s.cases = replaced(
        &s.cases,
        "fn narrow_layout() {",
        &format!("fn narrow_layout() {{\n{body}"),
    );
    s.cases.push_str(items);
    check(&s)
}

fn unexercised_errors(errors: &[String]) -> Vec<&String> {
    errors
        .iter()
        .filter(|e| e.contains("never exercise it"))
        .collect()
}

#[test]
fn catalog_rejects_a_proof_whose_scenario_never_uses_the_api() {
    let errors = probe_errors(&["clock::undeclared_probe"], "", "");
    expect_error(
        &errors,
        "proof for clock::undeclared_probe names scenarios [\"NarrowLayout\"] that never exercise it: `undeclared_probe` never appears",
    );
}

#[test]
fn catalog_accepts_a_proof_whose_scenario_calls_the_api() {
    let errors = probe_errors(&["clock::undeclared_probe"], "undeclared_probe();", "");
    assert!(unexercised_errors(&errors).is_empty(), "{errors:#?}");
}

#[test]
fn catalog_accepts_a_proof_the_scenario_reaches_through_a_helper() {
    let items = "\nfn narrow_helper() { undeclared_probe(); }\n";
    let errors = probe_errors(&["clock::undeclared_probe"], "narrow_helper();", items);
    assert!(unexercised_errors(&errors).is_empty(), "{errors:#?}");
    let errors = probe_errors(&["clock::undeclared_probe"], "", items);
    expect_error(&errors, "`undeclared_probe` never appears");
}

#[test]
fn catalog_a_string_or_comment_is_not_a_use_of_the_api() {
    let body = "// undeclared_probe();\n    let _ = \"undeclared_probe\";";
    let errors = probe_errors(&["clock::undeclared_probe"], body, "");
    expect_error(&errors, "`undeclared_probe` never appears");
}

#[test]
fn catalog_a_type_is_shown_by_its_name_or_by_one_of_its_members() {
    let by_name = probe_errors(&["clock::ProbeKind"], "let _ = ProbeKind;", "");
    assert!(unexercised_errors(&by_name).is_empty(), "{by_name:#?}");
    let by_method = probe_errors(&["clock::ProbeKind"], "value().ping();", "");
    assert!(unexercised_errors(&by_method).is_empty(), "{by_method:#?}");
    let neither = probe_errors(&["clock::ProbeKind"], "let _ = other();", "");
    expect_error(&neither, "neither the type `ProbeKind` nor any of its");
}

#[test]
fn catalog_a_method_is_shown_by_its_name_alone() {
    let named = probe_errors(&["clock::ProbeKind::solo"], "value().solo();", "");
    assert!(unexercised_errors(&named).is_empty(), "{named:#?}");
    let type_only = probe_errors(&["clock::ProbeKind::solo"], "let _ = ProbeKind;", "");
    expect_error(&type_only, "`solo` never appears");
}

#[test]
fn catalog_a_method_name_another_type_shares_needs_its_owner_named() {
    let api = ["clock::ProbeKind::ping"];
    let bare = probe_errors(&api, "value().ping();", "");
    expect_error(
        &bare,
        "`ping` appears, but other types export a `ping` too and the owner `ProbeKind` is never named",
    );
    let owned = probe_errors(&api, "let kind: ProbeKind = value();\n    kind.ping();", "");
    assert!(unexercised_errors(&owned).is_empty(), "{owned:#?}");
    let other_type = probe_errors(&api, "ProbeOther.ping();", "");
    expect_error(&other_type, "the owner `ProbeKind` is never named");
}

#[test]
fn catalog_a_variant_needs_its_enum_beside_its_name() {
    let api = ["clock::ProbeEnum::Open"];
    let path = probe_errors(&api, "let _ = ProbeEnum::Open;", "");
    assert!(unexercised_errors(&path).is_empty(), "{path:#?}");
    let renamed = probe_errors(
        &api,
        "use crate::ProbeEnum as Pe;\n    let _ = Pe::Open;",
        "",
    );
    assert!(unexercised_errors(&renamed).is_empty(), "{renamed:#?}");
    let imported = probe_errors(
        &api,
        "use crate::ProbeEnum::Open;\n    let _ = (Open, ProbeEnum::Close);",
        "",
    );
    assert!(unexercised_errors(&imported).is_empty(), "{imported:#?}");
    let bare = probe_errors(&api, "let _ = Open;", "");
    expect_error(&bare, "`ProbeEnum::Open` never appears");
    let other_enum = probe_errors(&api, "let _ = Other::Open;", "");
    expect_error(&other_enum, "`ProbeEnum::Open` never appears");
}

#[test]
fn catalog_keyed_remove_proof_needs_a_scenario_that_calls_remove() {
    let mut s = Sources::load();
    s.cases = s.cases.replace("store.remove(", "store.discard(");
    expect_error(
        &check(&s),
        "proof for core::keyed::KeyedVec::remove names scenarios [\"KeyedStoreIdentity\"] that never exercise it",
    );
}
