//! Required-suite inventory gate for the `performance` nextest profile (#56).
//!
//! #56 asks that every library family and every app contributes a cost
//! contract and that none can drop out unnoticed. `performance-inventory.tsv` names each
//! required suite: the nextest binary, the test filter that selects it, the
//! source file that holds it and the tests it must contain. The gate then
//! fails when
//!
//! - a required suite's operand is missing from `[profile.performance]` in
//!   `.config/nextest.toml`, so nextest would no longer run it;
//! - the operand is present but would not select one of the suite's required
//!   tests (a renamed module, a narrowed regex);
//! - a required test is no longer a `#[test]` function in its source file.
//!
//! The checks above read text. What nextest actually runs is checked on its
//! own output: `performance_nextest_list_runs_every_required_test` runs
//! `cargo nextest list --profile performance --workspace --all-features
//! --message-format json` and fails unless every required test is listed,
//! not `#[ignore]`d and matched by the profile filter. That catches what the
//! text checks cannot: a `mod` line removed from a binary, an `#[ignore]`, a
//! `#[cfg]` that drops the test, a filter that no longer selects it. It runs
//! only under `--profile performance` and does nothing under other profiles.
//! The same list, taken with `--profile ci`, must also select every required
//! test: that is the profile of the `Rust / <crate>` jobs, which build with
//! `--all-features` and so are where the counted contracts execute (the
//! generator gives the `performance` suite no feature field).
//! `performance_contracts_run_in_required_all_features_ci_jobs` pins, in the
//! generated task manifest and workflow, that each owning package has such a
//! job and that `Required` waits for it.
//!
//! The inventory is a floor. Adding a test to an existing suite needs no edit
//! here as long as the operand still selects it; adding a new suite means
//! adding an entry and an operand together.
//!
//! The profile filter is read as text, not evaluated. The profile must be a
//! plain union (`a | b | c`) of `binary_id(=ID) & test(MATCHER)` terms, and
//! the gate refuses `not` and set difference, which could drop a suite while
//! its operand stays in the text. A matcher may be `/^prefix/`, `/substring/`
//! or `=exact`; anything richer is refused rather than guessed at.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

use syn::visit::Visit;

const NEXTEST_CONFIG: &str = ".config/nextest.toml";
const PROFILE: &str = "performance";
const MEASURE_PROFILE: &str = "performance-measure";
const CI_PROFILE: &str = "ci";
const TASK_MANIFEST: &str = ".github/velnor/task-execution-manifest-v1.json";
const VELNOR_CONFIG: &str = ".velnor/config.toml";
const CI_WORKFLOW: &str = ".github/workflows/ci.yml";
const MEASUREMENT_FILE: &str = "crates/termrock-conformance/tests/regressions/work_measurements.rs";

/// One required suite.
struct Suite {
    /// What it covers, for failure messages.
    family: &'static str,
    /// The nextest binary id, for example `termrock-conformance::regressions`.
    binary_id: &'static str,
    /// The argument of `test(...)` in the profile operand, if the operand has
    /// one. `None` selects the whole binary.
    matcher: Option<&'static str>,
    /// The source file, relative to the repository root.
    file: &'static str,
    /// Nextest test names (module path inside the binary) that must exist and
    /// be selected by the operand.
    tests: &'static [&'static str],
}

/// The required suites, one `S` line each followed by its `T` lines, in
/// `performance-inventory.tsv` beside this file.
const INVENTORY_FILE: &str =
    "crates/termrock-conformance/tests/regressions/performance-inventory.tsv";

fn leak(text: &str) -> &'static str {
    Box::leak(text.to_owned().into_boxed_str())
}

fn inventory() -> &'static [Suite] {
    static SUITES: OnceLock<Vec<Suite>> = OnceLock::new();
    SUITES.get_or_init(|| {
        let text = fs::read_to_string(root().join(INVENTORY_FILE))
            .unwrap_or_else(|error| panic!("{INVENTORY_FILE}: {error}"));
        let mut suites: Vec<(Suite, Vec<&'static str>)> = Vec::new();
        for (number, line) in text.lines().enumerate() {
            let fields: Vec<&str> = line.split('\t').collect();
            match fields.as_slice() {
                [] | [""] => {}
                [comment] if comment.starts_with('#') => {}
                ["S", family, binary_id, matcher, file] => suites.push((
                    Suite {
                        family: leak(family),
                        binary_id: leak(binary_id),
                        matcher: (*matcher != "-").then(|| leak(matcher)),
                        file: leak(file),
                        tests: &[],
                    },
                    Vec::new(),
                )),
                ["T", test] => suites
                    .last_mut()
                    .unwrap_or_else(|| panic!("{INVENTORY_FILE}:{}: T before any S", number + 1))
                    .1
                    .push(leak(test)),
                _ => panic!("{INVENTORY_FILE}:{}: unreadable line `{line}`", number + 1),
            }
        }
        suites
            .into_iter()
            .map(|(mut suite, tests)| {
                suite.tests = Box::leak(tests.into_boxed_slice());
                suite
            })
            .collect()
    })
}

/// The ignored measurement tests the `performance-measure` profile runs.
const MEASUREMENTS: &[&str] = &[
    "measurement_viewport_append_one_line",
    "measurement_viewport_prepared_draw",
    "measurement_list_settled_frame",
    "measurement_tree_filter_change",
    "measurement_scroll_view_render",
    "measurement_scroll_view_render_window",
    "measurement_editor_keystroke_then_frame",
    "measurement_textarea_keystroke_then_frame",
];

/// The repository root: this crate is `crates/termrock-conformance`, two levels below it.
fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("termrock-conformance sits at crates/termrock-conformance")
}

fn squash(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The text of one `[profile.NAME]` table: the lines after its header up to
/// the next table header.
fn profile_section(config: &str, name: &str) -> Result<String, String> {
    let header = format!("[profile.{name}]");
    let mut lines = config.lines();
    lines
        .find(|line| line.trim() == header)
        .ok_or_else(|| format!("{NEXTEST_CONFIG} has no `{header}` table"))?;
    Ok(lines
        .take_while(|line| !line.trim_start().starts_with('['))
        .collect::<Vec<_>>()
        .join("\n"))
}

/// The string literal of `key` in a profile section, without its quotes.
fn string_value(section: &str, key: &str) -> Result<String, String> {
    let start = section
        .lines()
        .scan(0usize, |offset, line| {
            let here = *offset;
            *offset += line.len() + 1;
            Some((here, line))
        })
        .find(|(_, line)| {
            line.trim_start()
                .strip_prefix(key)
                .is_some_and(|rest| rest.trim_start().starts_with('='))
        })
        .map(|(offset, _)| offset)
        .ok_or_else(|| format!("the profile has no `{key}`"))?;
    let after_eq = &section[start..];
    let literal = after_eq[after_eq.find('=').expect("checked above") + 1..].trim_start();
    for quote in ["'''", "\"\"\"", "'", "\""] {
        if let Some(rest) = literal.strip_prefix(quote) {
            return rest
                .find(quote)
                .map(|end| rest[..end].to_owned())
                .ok_or_else(|| format!("`{key}` has an unterminated string"));
        }
    }
    Err(format!("`{key}` is not a string"))
}

/// Splits a filter on its top-level `|` into squashed operands. Refuses
/// operators that could remove a suite while leaving its operand in the text.
fn operands(filter: &str) -> Result<Vec<String>, String> {
    let squashed = squash(filter);
    for banned in ["not ", "not(", " - ", "!"] {
        if squashed.contains(banned) {
            return Err(format!(
                "the filter uses `{}`, which the gate cannot read as a plain union",
                banned.trim()
            ));
        }
    }
    let mut depth = 0i32;
    let mut out = Vec::new();
    let mut current = String::new();
    for c in squashed.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            '|' if depth == 0 => {
                out.push(std::mem::take(&mut current).trim().to_owned());
                continue;
            }
            _ => {}
        }
        current.push(c);
    }
    out.push(current.trim().to_owned());
    if depth != 0 || out.iter().any(String::is_empty) {
        return Err("the filter has unbalanced parentheses or an empty operand".to_owned());
    }
    Ok(out)
}

/// The operand a suite must have in the profile.
fn operand(suite: &Suite) -> String {
    match suite.matcher {
        Some(matcher) => format!("binary_id(={}) & test({matcher})", suite.binary_id),
        None => format!("binary_id(={})", suite.binary_id),
    }
}

/// Whether `test(matcher)` selects `name`. Reads only `/^prefix/`,
/// `/substring/` and `=exact`.
fn selects(matcher: &str, name: &str) -> Result<bool, String> {
    const META: &[char] = &[
        '\\', '.', '*', '+', '?', '[', ']', '(', ')', '{', '}', '$', '|',
    ];
    let plain = |text: &str| -> Result<(), String> {
        match text.chars().find(|c| META.contains(c)) {
            Some(c) => Err(format!(
                "matcher `{matcher}` uses `{c}`; the gate reads only prefixes, substrings and exact names"
            )),
            None => Ok(()),
        }
    };
    if let Some(exact) = matcher.strip_prefix('=') {
        return Ok(name == exact);
    }
    let pattern = matcher
        .strip_prefix('/')
        .and_then(|m| m.strip_suffix('/'))
        .ok_or_else(|| format!("matcher `{matcher}` is not `/regex/` or `=exact`"))?;
    match pattern.strip_prefix('^') {
        Some(prefix) => plain(prefix).map(|()| name.starts_with(prefix)),
        None => plain(pattern).map(|()| name.contains(pattern)),
    }
}

/// Every missing or unselected piece of the inventory, one line each.
fn check_profile(filter: &str) -> Vec<String> {
    let have = match operands(filter) {
        Ok(have) => have,
        Err(why) => return vec![why],
    };
    let mut problems = Vec::new();
    for suite in inventory() {
        let want = operand(suite);
        if !have.contains(&want) {
            problems.push(format!(
                "[profile.{PROFILE}] lacks the operand for `{}`: {want}",
                suite.family
            ));
            continue;
        }
        if let Some(matcher) = suite.matcher {
            for test in suite.tests {
                match selects(matcher, test) {
                    Ok(true) => {}
                    Ok(false) => problems.push(format!(
                        "`{}`: operand {want} does not select required test `{test}`",
                        suite.family
                    )),
                    Err(why) => problems.push(format!("`{}`: {why}", suite.family)),
                }
            }
        }
    }
    problems
}

/// Every required test that the `cargo nextest list --message-format json`
/// output `json` does not show as a listed, not-ignored test that the
/// profile's filter selects, one line each.
fn check_list(json: &str, profile: &str) -> Vec<String> {
    let list: serde_json::Value = match serde_json::from_str(json) {
        Ok(list) => list,
        Err(why) => return vec![format!("cannot parse the nextest list output: {why}")],
    };
    let Some(suites) = list.get("rust-suites").and_then(|s| s.as_object()) else {
        return vec!["the nextest list output has no `rust-suites` object".to_owned()];
    };
    let mut problems = Vec::new();
    for suite in inventory() {
        for test in suite.tests {
            let case = suites
                .get(suite.binary_id)
                .and_then(|binary| binary.get("testcases"))
                .and_then(|cases| cases.get(*test));
            let Some(case) = case else {
                problems.push(format!(
                    "`{}`: nextest does not list `{test}` in binary `{}`",
                    suite.family, suite.binary_id
                ));
                continue;
            };
            if case.get("ignored").and_then(|i| i.as_bool()) != Some(false) {
                problems.push(format!(
                    "`{}`: `{test}` is #[ignore]d (or its `ignored` flag is unreadable)",
                    suite.family
                ));
            }
            let status = case
                .get("filter-match")
                .and_then(|m| m.get("status"))
                .and_then(|s| s.as_str());
            if status != Some("matches") {
                problems.push(format!(
                    "`{}`: [profile.{profile}] does not select `{test}` (filter-match status {status:?})",
                    suite.family
                ));
            }
        }
    }
    problems
}

/// The list command the gate runs. `NEXTEST_*` variables of the enclosing run
/// are removed so that nothing of that run steers the nested list.
fn nextest_list_json(profile: &str) -> Result<String, String> {
    let mut command = Command::new("cargo");
    command
        .current_dir(root())
        .args(["nextest", "list", "--profile", profile])
        .args(["--workspace", "--all-features", "--message-format", "json"]);
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("NEXTEST") {
            command.env_remove(name);
        }
    }
    let output = command
        .output()
        .map_err(|e| format!("cannot run `cargo nextest list`: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "`cargo nextest list` failed ({}):\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8(output.stdout).map_err(|e| format!("nextest list output is not UTF-8: {e}"))
}

#[derive(Default)]
struct TestFns {
    /// `(name, ignored)` of every `#[test]` function, however deeply nested.
    found: Vec<(String, bool)>,
}

impl<'ast> Visit<'ast> for TestFns {
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        let has = |name: &str| item.attrs.iter().any(|a| a.path().is_ident(name));
        if has("test") {
            self.found.push((item.sig.ident.to_string(), has("ignore")));
        }
        syn::visit::visit_item_fn(self, item);
    }
}

fn test_fns(source: &str, file: &str) -> Result<Vec<(String, bool)>, String> {
    let parsed = syn::parse_file(source).map_err(|e| format!("cannot parse {file}: {e}"))?;
    let mut visitor = TestFns::default();
    visitor.visit_file(&parsed);
    Ok(visitor.found)
}

/// Required tests that `source` does not define as `#[test]` functions.
fn missing_tests(suite: &Suite, source: &str) -> Vec<String> {
    let found = match test_fns(source, suite.file) {
        Ok(found) => found,
        Err(why) => return vec![why],
    };
    suite
        .tests
        .iter()
        .filter(|test| {
            let name = test.rsplit("::").next().expect("a test name");
            !found.iter().any(|(f, _)| f == name)
        })
        .map(|test| {
            format!(
                "`{}`: {} has no #[test] fn for required `{test}`",
                suite.family, suite.file
            )
        })
        .collect()
}

/// The lines of the `[[stacks.rust.suites]]` entry whose `id` is `id`, without
/// spaces, up to the next table header.
fn suite_entry(config: &str, id: &str) -> Option<Vec<String>> {
    let want = format!("id=\"{id}\"");
    let mut entries: Vec<Vec<String>> = Vec::new();
    let mut inside = false;
    for line in config.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            inside = line == "[[stacks.rust.suites]]";
            if inside {
                entries.push(Vec::new());
            }
        } else if inside
            && !line.is_empty()
            && !line.starts_with('#')
            && let Some(entry) = entries.last_mut()
        {
            entry.push(line.split_whitespace().collect());
        }
    }
    entries.into_iter().find(|entry| entry.contains(&want))
}

/// What must hold for `performance` to be a required CI suite: it is declared
/// in `.velnor/config.toml` with the profile, four partitions, the three
/// triggers and `required = true`, and the generated workflow has its job and
/// makes the `Required` verdict wait for it. A suite that is declared but not
/// generated, or generated but not required, fails here.
fn check_suite(config: &str, workflow: &str) -> Vec<String> {
    let mut problems = Vec::new();
    match suite_entry(config, PROFILE) {
        None => problems.push(format!(
            "{VELNOR_CONFIG} declares no `[[stacks.rust.suites]]` with id = \"{PROFILE}\""
        )),
        Some(entry) => {
            for (line, why) in [
                (
                    "profile=\"performance\"",
                    "run the `performance` nextest profile",
                ),
                ("required=true", "be a required check"),
                ("partitions=4", "run four partitions"),
            ] {
                if !entry.iter().any(|l| l == line) {
                    problems.push(format!("the `{PROFILE}` suite must {why} (`{line}`)"));
                }
            }
            let triggers = entry.iter().find(|l| l.starts_with("triggers=")).cloned();
            for trigger in ["pull_request", "push", "workflow_dispatch"] {
                if !triggers.as_deref().is_some_and(|t| t.contains(trigger)) {
                    problems.push(format!(
                        "the `{PROFILE}` suite lacks the `{trigger}` trigger"
                    ));
                }
            }
        }
    }
    let job = format!("suite-{PROFILE}");
    if !workflow.contains(&format!("\n  {job}:")) {
        problems.push(format!(
            "{CI_WORKFLOW} has no `{job}` job: regenerate with `velnor-actions generate`"
        ));
    }
    if !workflow.contains(&format!("\n      - {job}\n")) {
        problems.push(format!(
            "the generated `Required` job does not wait for `{job}`"
        ));
    }
    if !workflow.contains("--profile performance") {
        problems.push(format!("{CI_WORKFLOW} never runs `--profile performance`"));
    }
    problems
}

/// The cargo packages that own the required tests: the part of each binary id
/// before `::`.
fn required_packages() -> Vec<&'static str> {
    let mut packages: Vec<&str> = inventory()
        .iter()
        .filter_map(|suite| suite.binary_id.split("::").next())
        .collect();
    packages.sort_unstable();
    packages.dedup();
    packages
}

/// What must hold for the `work_contract_*` tests to execute with the counters
/// compiled in, in a required job. The `performance` suite cannot do it (the
/// generator gives a suite no feature field), so each package that owns a
/// required test needs a generated `nextest run --profile ci --package P
/// --all-features` task (`manifest`, the generated task manifest), and the
/// `rust-P` job that runs it must exist and be waited for by `Required`
/// (`workflow`, the generated `ci.yml`). The nested list of
/// `performance_nextest_list_runs_every_required_test` then shows that the
/// `ci` profile selects every required test.
fn check_ci_jobs(manifest: &str, workflow: &str) -> Vec<String> {
    let json: String = manifest
        .lines()
        .filter(|line| !line.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    let parsed: serde_json::Value = match serde_json::from_str(&json) {
        Ok(parsed) => parsed,
        Err(why) => return vec![format!("cannot parse {TASK_MANIFEST}: {why}")],
    };
    let Some(tasks) = parsed.get("tasks").and_then(|t| t.as_object()) else {
        return vec![format!("{TASK_MANIFEST} has no `tasks` object")];
    };
    let argvs: Vec<Vec<&str>> = tasks
        .values()
        .filter_map(|task| task.get("argv")?.as_array())
        .map(|argv| argv.iter().filter_map(|a| a.as_str()).collect())
        .collect();
    let follows =
        |argv: &[&str], flag: &str, value: &str| argv.windows(2).any(|w| w == [flag, value]);
    let mut problems = Vec::new();
    for package in required_packages() {
        let runs = argvs.iter().any(|argv| {
            argv.windows(2).any(|w| w == ["nextest", "run"])
                && follows(argv, "--profile", CI_PROFILE)
                && follows(argv, "--package", package)
                && argv.contains(&"--all-features")
        });
        if !runs {
            problems.push(format!(
                "no generated task runs `nextest run --profile {CI_PROFILE} --package {package} --all-features`: the required tests of `{package}` would run without the work counters"
            ));
        }
        let job = format!("rust-{package}");
        if !workflow.contains(&format!("\n  {job}:")) {
            problems.push(format!("{CI_WORKFLOW} has no `{job}` job"));
        }
        if !workflow.contains(&format!("\n      - {job}\n")) {
            problems.push(format!(
                "the generated `Required` job does not wait for `{job}`"
            ));
        }
    }
    problems
}

fn read(path: &str) -> String {
    fs::read_to_string(root().join(path)).unwrap_or_else(|e| panic!("cannot read {path}: {e}"))
}

fn fail(problems: Vec<String>) {
    assert!(
        problems.is_empty(),
        "the performance inventory is not met:\n  {}",
        problems.join("\n  ")
    );
}

#[test]
fn performance_profile_selects_every_required_suite() {
    let config = read(NEXTEST_CONFIG);
    let filter = profile_section(&config, PROFILE)
        .and_then(|section| string_value(&section, "default-filter"))
        .unwrap_or_else(|why| panic!("{why}"));
    fail(check_profile(&filter));
}

/// Asks nextest what the `performance` profile runs and checks that against
/// the inventory. See the module documentation for what this adds to the text
/// checks. Under any other profile this does nothing: the nested list would
/// not be a cache hit there and could rebuild the workspace in another shape.
#[test]
fn performance_nextest_list_runs_every_required_test() {
    if std::env::var("NEXTEST_PROFILE").as_deref() != Ok(PROFILE) {
        eprintln!("not run under `--profile {PROFILE}`: the nextest list check is skipped");
        return;
    }
    let json = nextest_list_json(PROFILE).unwrap_or_else(|why| panic!("{why}"));
    fail(check_list(&json, PROFILE));
    // The `Rust / <crate>` jobs, which build with `--all-features` and so run
    // the contracts with the counters on, select tests by the `ci` profile.
    // Every required test must be one of them (second list: a cache hit).
    let json = nextest_list_json(CI_PROFILE).unwrap_or_else(|why| panic!("{why}"));
    fail(check_list(&json, CI_PROFILE));
}

/// The `performance` profile is only a contract if CI runs it. The nested
/// `nextest list` of `performance_nextest_list_runs_every_required_test`
/// executes under the generated `suite-performance` job, so dropping that job
/// would silently drop the real inventory proof.
#[test]
fn performance_suite_is_a_required_generated_ci_job() {
    fail(check_suite(&read(VELNOR_CONFIG), &read(CI_WORKFLOW)));
}

/// The contracts need `work-metrics`, which the `performance` suite cannot
/// enable, so they execute in the required per-package `ci` jobs instead. This
/// pins that route in the generated files.
#[test]
fn performance_contracts_run_in_required_all_features_ci_jobs() {
    fail(check_ci_jobs(&read(TASK_MANIFEST), &read(CI_WORKFLOW)));
}

#[test]
fn performance_required_tests_exist_in_source() {
    fail(
        inventory()
            .iter()
            .flat_map(|suite| missing_tests(suite, &read(suite.file)))
            .collect(),
    );
}

#[test]
fn performance_measure_profile_is_serial_and_runs_only_ignored_measurements() {
    let config = read(NEXTEST_CONFIG);
    let section = profile_section(&config, MEASURE_PROFILE).unwrap_or_else(|why| panic!("{why}"));
    let mut problems = Vec::new();
    if !section
        .lines()
        .any(|l| l.split_whitespace().collect::<String>() == "test-threads=1")
    {
        problems.push(format!(
            "[profile.{MEASURE_PROFILE}] must set `test-threads = 1`: parallel measurements time each other"
        ));
    }
    match string_value(&section, "default-filter") {
        Ok(filter)
            if filter.contains("binary_id(=termrock-conformance::regressions)")
                && filter.contains("work_measurements::") => {}
        Ok(filter) => problems.push(format!(
            "[profile.{MEASURE_PROFILE}] filter does not select the work_measurements module of termrock-conformance::regressions: {}",
            squash(&filter)
        )),
        Err(why) => problems.push(why),
    }
    let found =
        test_fns(&read(MEASUREMENT_FILE), MEASUREMENT_FILE).unwrap_or_else(|why| panic!("{why}"));
    for name in MEASUREMENTS {
        match found.iter().find(|(f, _)| f == name) {
            None => problems.push(format!("{MEASUREMENT_FILE} lacks `{name}`")),
            Some((_, false)) => problems.push(format!(
                "`{name}` is not #[ignore]d: a timing test must not run in the default profiles"
            )),
            Some((_, true)) => {}
        }
    }
    fail(problems);
}

/// The gate's own logic, against synthetic input: the mutations the real run
/// relies on must be caught here as well.
#[cfg(test)]
mod gate_logic {
    use super::*;

    fn full_filter() -> String {
        inventory()
            .iter()
            .map(operand)
            .collect::<Vec<_>>()
            .join(" | ")
    }

    /// What `cargo nextest list --message-format json` would print if every
    /// inventory test were listed, not ignored and selected.
    fn full_list() -> serde_json::Value {
        let mut suites = serde_json::Map::new();
        for suite in inventory() {
            let binary = suites
                .entry(suite.binary_id)
                .or_insert_with(|| serde_json::json!({ "testcases": {} }));
            for test in suite.tests {
                binary["testcases"][*test] = serde_json::json!({
                    "ignored": false,
                    "filter-match": { "status": "matches" },
                });
            }
        }
        serde_json::json!({ "rust-suites": suites })
    }

    fn list_problems(list: &serde_json::Value) -> Vec<String> {
        check_list(&list.to_string(), PROFILE)
    }

    const SUITE_CONFIG: &str = "[[stacks.rust.suites]]\nid = \"functional\"\nrequired = true\n\n[[stacks.rust.suites]]\nid = \"performance\"\nprofile = \"performance\"\npartitions = 4\nbuild = \"inline\"\ntriggers = [\"pull_request\", \"push\", \"workflow_dispatch\"]\nrequired = true\n\n[[stacks.rust.configurations]]\nname = \"default\"\n";
    const SUITE_WORKFLOW: &str = "jobs:\n  required:\n    needs:\n      - suite-functional\n      - suite-performance\n      - jactionlint\n  suite-performance:\n    run: mbx nextest run --profile performance\n";

    #[test]
    fn a_declared_generated_required_suite_passes() {
        assert_eq!(
            check_suite(SUITE_CONFIG, SUITE_WORKFLOW),
            Vec::<String>::new()
        );
    }

    #[test]
    fn a_missing_suite_fails() {
        let config = SUITE_CONFIG.replace("id = \"performance\"", "id = \"other\"");
        assert!(check_suite(&config, SUITE_WORKFLOW)[0].contains("declares no"));
    }

    #[test]
    fn a_suite_that_is_not_required_fails() {
        let config = SUITE_CONFIG.replace("partitions = 4\nbuild = \"inline\"\ntriggers = [\"pull_request\", \"push\", \"workflow_dispatch\"]\nrequired = true", "partitions = 4\nbuild = \"inline\"\ntriggers = [\"pull_request\", \"push\", \"workflow_dispatch\"]\nrequired = false");
        let problems = check_suite(&config, SUITE_WORKFLOW);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("required"));
    }

    #[test]
    fn a_suite_without_a_pull_request_trigger_fails() {
        let config = SUITE_CONFIG.replace("\"pull_request\", ", "");
        let problems = check_suite(&config, SUITE_WORKFLOW);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("pull_request"));
    }

    #[test]
    fn a_suite_the_workflow_does_not_generate_or_require_fails() {
        let workflow = SUITE_WORKFLOW.replace("suite-performance", "suite-gone");
        let problems = check_suite(SUITE_CONFIG, &workflow);
        assert_eq!(problems.len(), 2, "{problems:?}");
        let unrequired = SUITE_WORKFLOW.replace("      - suite-performance\n", "");
        let problems = check_suite(SUITE_CONFIG, &unrequired);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("Required"));
    }

    fn ci_manifest(skip: Option<&str>) -> String {
        let tasks: serde_json::Map<String, serde_json::Value> = required_packages()
            .into_iter()
            .filter(|package| Some(*package) != skip)
            .map(|package| {
                (
                    format!("stack/rust/{package}/nextest/default"),
                    serde_json::json!({ "argv": [
                        "mbx", "nextest", "run", "--profile", "ci", "--package", package,
                        "--no-tests", "warn", "--all-features"
                    ] }),
                )
            })
            .collect();
        format!("# Generated\n{}", serde_json::json!({ "tasks": tasks }))
    }

    fn ci_workflow() -> String {
        let mut text = String::from("jobs:\n  required:\n    needs:\n");
        for package in required_packages() {
            text.push_str(&format!("      - rust-{package}\n"));
        }
        text.push_str("      - jactionlint\n");
        for package in required_packages() {
            text.push_str(&format!("  rust-{package}:\n    name: x\n"));
        }
        text
    }

    #[test]
    fn all_features_ci_jobs_for_every_package_pass() {
        assert_eq!(
            check_ci_jobs(&ci_manifest(None), &ci_workflow()),
            Vec::<String>::new()
        );
    }

    #[test]
    fn a_package_without_an_all_features_ci_task_fails() {
        let problems = check_ci_jobs(&ci_manifest(Some("termrock-holla")), &ci_workflow());
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("termrock-holla"));
        let no_features = ci_manifest(None).replace("\"--all-features\"", "\"--locked\"");
        assert!(check_ci_jobs(&no_features, &ci_workflow()).len() >= required_packages().len());
        let other_profile = ci_manifest(None).replace("\"ci\"", "\"functional\"");
        assert!(check_ci_jobs(&other_profile, &ci_workflow()).len() >= required_packages().len());
    }

    #[test]
    fn a_ci_job_that_is_not_required_fails() {
        let workflow = ci_workflow().replace("      - rust-termrock-conformance\n", "");
        let problems = check_ci_jobs(&ci_manifest(None), &workflow);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("Required"));
    }

    #[test]
    fn a_complete_list_passes() {
        assert_eq!(list_problems(&full_list()), Vec::<String>::new());
    }

    #[test]
    fn a_binary_that_lost_its_module_fails() {
        // `mod work_metrics_tests;` removed from the tablepro binary: the
        // binary is still listed, its tests are gone.
        let mut list = full_list();
        list["rust-suites"]["termrock-tablepro::bin/tablepro"]["testcases"] = serde_json::json!({});
        let problems = list_problems(&list);
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(problems[0].contains("work_contract_tablepro_idle_and_cell_edit"));
        assert!(problems[1].contains("work_contract_tablepro_background_tab_edit"));
    }

    #[test]
    fn a_missing_binary_fails() {
        let mut list = full_list();
        list["rust-suites"]
            .as_object_mut()
            .unwrap()
            .remove("jackin-console::world");
        let problems = list_problems(&list);
        assert_eq!(problems.len(), 4, "{problems:?}");
        assert!(problems[0].contains("projection_work::"));
    }

    #[test]
    fn an_ignored_test_fails() {
        let mut list = full_list();
        list["rust-suites"]["termrock-showcase::bin/showcase"]["testcases"]["work_metrics_tests::work_contract_showcase_idle_and_switch"]
            ["ignored"] = serde_json::json!(true);
        let problems = list_problems(&list);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("#[ignore]"));
    }

    #[test]
    fn a_test_the_filter_does_not_select_fails() {
        let mut list = full_list();
        list["rust-suites"]["termrock-conformance::regressions"]["testcases"]["work_budgets::work_contract_tree_filtered_rebuild"]
            ["filter-match"] = serde_json::json!({ "status": "mismatch", "reason": "expression" });
        let problems = list_problems(&list);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("does not select"));
    }

    #[test]
    fn unreadable_list_output_fails() {
        assert!(!check_list("not json", PROFILE).is_empty());
        assert!(!check_list("{}", PROFILE).is_empty());
    }

    #[test]
    fn a_complete_profile_passes() {
        assert_eq!(check_profile(&full_filter()), Vec::<String>::new());
    }

    #[test]
    fn a_removed_suite_fails() {
        for removed in inventory() {
            let kept: Vec<String> = inventory()
                .iter()
                .filter(|s| !std::ptr::eq(*s, removed))
                .map(operand)
                .collect();
            // Two suites can share one operand; the other still provides it.
            let still_there = kept.contains(&operand(removed));
            let problems = check_profile(&kept.join(" | "));
            assert_eq!(
                problems.is_empty(),
                still_there,
                "removing `{}`: {problems:?}",
                removed.family
            );
        }
    }

    #[test]
    fn a_narrowed_matcher_fails() {
        let narrowed =
            full_filter().replace("test(/^work_budgets::/)", "test(/^work_budgets::viewport/)");
        assert!(!check_profile(&narrowed).is_empty());
    }

    #[test]
    fn negation_and_difference_are_refused() {
        let with_not = format!("{} | not test(=x)", full_filter());
        assert!(check_profile(&with_not)[0].contains("not"));
        let with_minus = format!("({}) - test(=x)", full_filter());
        assert!(!check_profile(&with_minus).is_empty());
    }

    #[test]
    fn a_missing_test_fn_fails() {
        let suite = &inventory()[0];
        let without = "#[test] fn something_else() {}";
        assert_eq!(missing_tests(suite, without).len(), suite.tests.len());
        let nested = suite
            .tests
            .iter()
            .map(|t| {
                format!(
                    "mod m {{ #[test] fn {}() {{}} }}",
                    t.rsplit("::").next().unwrap()
                )
            })
            .collect::<String>();
        assert!(missing_tests(suite, &nested).is_empty());
    }

    #[test]
    fn a_function_without_the_test_attribute_does_not_count() {
        let suite = &inventory()[3];
        let name = suite.tests[0].rsplit("::").next().unwrap();
        // Every other required test is missing from this source too, so the
        // count is the whole suite, not one: the plain function counts for none.
        assert_eq!(
            missing_tests(suite, &format!("fn {name}() {{}}")).len(),
            suite.tests.len()
        );
    }

    #[test]
    fn the_filter_is_read_from_a_multi_line_literal() {
        let config = "[profile.a]\nx = 1\n[profile.performance]\n# note\ndefault-filter = '''\n    one\n  | two\n'''\nfail-fast = false\n";
        let section = profile_section(config, PROFILE).unwrap();
        let filter = string_value(&section, "default-filter").unwrap();
        assert_eq!(operands(&filter).unwrap(), ["one", "two"]);
        assert!(profile_section(config, "missing").is_err());
    }
}
