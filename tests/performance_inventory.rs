//! Required-suite inventory gate for the `performance` nextest profile (#56).
//!
//! #56 asks that every library family and every app contributes a cost
//! contract and that none can drop out unnoticed. `INVENTORY` names each
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

use syn::visit::Visit;

const NEXTEST_CONFIG: &str = ".config/nextest.toml";
const PROFILE: &str = "performance";
const MEASURE_PROFILE: &str = "performance-measure";

/// One required suite.
struct Suite {
    /// What it covers, for failure messages.
    family: &'static str,
    /// The nextest binary id, for example `junie-tui::regressions`.
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

const INVENTORY: &[Suite] = &[
    Suite {
        family: "counter core (thread isolation, epochs, overflow, budgets)",
        binary_id: "junie-tui",
        matcher: Some("/^metrics::/"),
        file: "src/metrics/tests.rs",
        tests: &[
            "metrics::tests::metrics_delta_counts_units_between_snapshots",
            "metrics::tests::metrics_delta_rejects_other_thread",
            "metrics::tests::metrics_delta_rejects_reset",
            "metrics::tests::metrics_delta_rejects_a_counter_that_went_backwards",
            "metrics::tests::metrics_record_panics_on_counter_overflow",
            "metrics::tests::metrics_check_exact_flags_undercounts_overcounts_and_unlisted_work",
        ],
    },
    Suite {
        family: "timing report schema and statistics",
        binary_id: "junie-tui",
        matcher: Some("/^metrics::/"),
        file: "src/metrics/measurement.rs",
        tests: &[
            "metrics::measurement::tests::measurement_required_fields_are_all_present",
            "metrics::measurement::tests::measurement_unavailable_values_are_null_not_zero",
            "metrics::measurement::tests::measurement_percentiles_follow_the_specified_indices",
        ],
    },
    Suite {
        family: "responsive fit linearity",
        binary_id: "junie-tui",
        matcher: Some("/^ui::overflow_work_tests::/"),
        file: "src/ui/overflow_work_tests.rs",
        tests: &[
            "ui::overflow_work_tests::large_overflow_chip_prefix_work_is_linear",
            "ui::overflow_work_tests::large_overflow_breadcrumb_work_is_linear",
            "ui::overflow_work_tests::large_overflow_segments_work_is_linear",
            "ui::overflow_work_tests::large_overflow_statusbar_work_is_linear",
            "ui::overflow_work_tests::large_overflow_keyhint_work_is_linear",
            "ui::overflow_work_tests::large_overflow_toolbar_work_is_linear",
            "ui::overflow_work_tests::large_overflow_tabs_work_is_linear",
            "ui::overflow_work_tests::tabs_render_measures_each_tab_once",
        ],
    },
    Suite {
        family: "keyed collection joins",
        binary_id: "junie-tui",
        matcher: Some("/^widgets::keyed_cost_tests::/"),
        file: "src/widgets/keyed_cost_tests.rs",
        tests: &["widgets::keyed_cost_tests::picker_projection_join_is_linear"],
    },
    Suite {
        family: "library widget budgets (viewport, scroll view, list, table, tree, picker, select, completion, editor, textarea)",
        binary_id: "junie-tui::regressions",
        matcher: Some("/^work_budgets::/"),
        file: "tests/regressions/work_budgets.rs",
        tests: &[
            "work_budgets::work_contract_viewport_first_use_and_prepared_draw",
            "work_budgets::work_contract_viewport_one_append_is_incremental",
            "work_budgets::work_contract_viewport_adapter_unprepared_draw",
            "work_budgets::work_contract_scroll_view_content_sized_canvas",
            "work_budgets::work_contract_settled_frame_paints_only_visible_rows",
            "work_budgets::work_contract_tree_filtered_rebuild",
            "work_budgets::work_contract_picker_linear_projection",
            "work_budgets::work_contract_select_open_frame",
            "work_budgets::work_contract_completion_open_frame",
            "work_budgets::work_contract_editor_prepared_frame",
            "work_budgets::work_contract_textarea_prepared_frame",
            "work_budgets::work_report_is_count_only_json_with_every_metric",
        ],
    },
    Suite {
        family: "Holla page scenarios",
        binary_id: "junie-tui::bin/holla",
        matcher: Some("/^work_metrics_tests::/"),
        file: "src/bin/holla/work_metrics_tests.rs",
        tests: &["work_metrics_tests::work_contract_holla_idle_and_source_update"],
    },
    Suite {
        family: "Holla projection reuse (app)",
        binary_id: "junie-tui::bin/holla",
        matcher: Some("/holla_projection_/"),
        file: "src/bin/holla/app/tests.rs",
        tests: &[
            "app::tests::holla_projection_idle_ticks_do_no_source_work",
            "app::tests::holla_projection_idle_cleanup_page_ticks_do_no_source_work",
            "app::tests::holla_projection_hidden_pages_catch_up_once",
        ],
    },
    Suite {
        family: "Holla projection reuse (Finder)",
        binary_id: "junie-tui::bin/holla",
        matcher: Some("/holla_projection_/"),
        file: "src/bin/holla/screens/finder/tests.rs",
        tests: &[
            "screens::finder::tests::holla_projection_group_change_reranks_without_source_work",
        ],
    },
    Suite {
        family: "TablePro page scenarios",
        binary_id: "junie-tui::bin/tablepro",
        matcher: Some("/^work_metrics_tests::/"),
        file: "src/bin/tablepro/work_metrics_tests.rs",
        tests: &["work_metrics_tests::work_contract_tablepro_idle_and_cell_edit"],
    },
    Suite {
        family: "Showcase page scenarios and stress fits",
        binary_id: "junie-tui::bin/showcase",
        matcher: Some("/^work_metrics_tests::/"),
        file: "src/bin/showcase/work_metrics_tests.rs",
        tests: &[
            "work_metrics_tests::work_contract_showcase_idle_and_switch",
            "work_metrics_tests::work_contract_stress_toolbars_items",
            "work_metrics_tests::work_contract_stress_breadcrumbs_items",
            "work_metrics_tests::work_contract_stress_chrome_items",
            "work_metrics_tests::work_contract_stress_chips_items",
        ],
    },
    Suite {
        family: "Jackin roster and capsule scenarios",
        binary_id: "jackin-console::regressions",
        matcher: Some("/^work_budgets::/"),
        file: "apps/jackin/tests/regressions/work_budgets.rs",
        tests: &[
            "work_budgets::work_contract_jackin_idle_and_roster_update",
            "work_budgets::work_contract_jackin_capsule_idle_frame",
            "work_budgets::work_contract_jackin_capsule_frame_after_typing",
        ],
    },
    Suite {
        family: "Jackin projection epochs",
        binary_id: "jackin-console::world",
        matcher: Some("/^projection_work::/"),
        file: "apps/jackin/tests/world.rs",
        tests: &[
            "projection_work::one_epoch_change_builds_once_before_render",
            "projection_work::quiet_many_daemons_do_not_scan_full_roster",
            "projection_work::quiet_panes_do_not_rebuild_roster_or_accounts",
            "projection_work::roster_index_visits_each_instance_once",
        ],
    },
    Suite {
        family: "measurement sampling is real",
        binary_id: "junie-tui::work_measurements",
        matcher: Some("=measurement_edit_samples_are_real_changes"),
        file: "tests/work_measurements.rs",
        tests: &["measurement_edit_samples_are_real_changes"],
    },
    Suite {
        family: "this gate",
        binary_id: "junie-tui::performance_inventory",
        matcher: None,
        file: "tests/performance_inventory.rs",
        tests: &[
            "performance_profile_selects_every_required_suite",
            "performance_required_tests_exist_in_source",
            "performance_measure_profile_is_serial_and_runs_only_ignored_measurements",
        ],
    },
];

/// The ignored measurement tests the `performance-measure` profile runs.
const MEASUREMENTS: &[&str] = &[
    "measurement_viewport_append_one_line",
    "measurement_viewport_prepared_draw",
    "measurement_list_settled_frame",
    "measurement_tree_filter_change",
    "measurement_scroll_view_render",
];

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
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
    for suite in INVENTORY {
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

#[test]
fn performance_required_tests_exist_in_source() {
    fail(
        INVENTORY
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
        Ok(filter) if filter.contains("binary_id(=junie-tui::work_measurements)") => {}
        Ok(filter) => problems.push(format!(
            "[profile.{MEASURE_PROFILE}] filter does not select junie-tui::work_measurements: {}",
            squash(&filter)
        )),
        Err(why) => problems.push(why),
    }
    let found = test_fns(
        &read("tests/work_measurements.rs"),
        "tests/work_measurements.rs",
    )
    .unwrap_or_else(|why| panic!("{why}"));
    for name in MEASUREMENTS {
        match found.iter().find(|(f, _)| f == name) {
            None => problems.push(format!("tests/work_measurements.rs lacks `{name}`")),
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
        INVENTORY
            .iter()
            .map(operand)
            .collect::<Vec<_>>()
            .join(" | ")
    }

    #[test]
    fn a_complete_profile_passes() {
        assert_eq!(check_profile(&full_filter()), Vec::<String>::new());
    }

    #[test]
    fn a_removed_suite_fails() {
        for removed in INVENTORY {
            let kept: Vec<String> = INVENTORY
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
        let suite = &INVENTORY[0];
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
        let suite = &INVENTORY[3];
        let name = suite.tests[0].rsplit("::").next().unwrap();
        assert_eq!(missing_tests(suite, &format!("fn {name}() {{}}")).len(), 1);
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
