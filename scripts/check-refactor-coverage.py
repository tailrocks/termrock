#!/usr/bin/env python3
# Scanning source is not behavior proof. This script checks that an inventory
# names the routes, option keys, and scenarios declared in source, and that
# PASS rows and active baselines have the receipt and baseline files they
# claim. It does not run the programs or the tests. A green result is an
# inventory check, not evidence that the product behaves.

"""Check a refactor-coverage inventory against source, tests, receipts, and baselines."""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path

KIND_ORDER = (
    "route",
    "section",
    "section-size",
    "state",
    "transition",
    "option",
    "scenario",
)
FAILURE_ORDER = (
    "inventory",
    "source-scan",
    "source-disagreement",
    "missing-route",
    "missing-option",
    "missing-scenario",
    "missing-test",
    "bad-receipt",
    "missing-receipt",
    "missing-baseline",
    "duplicate-baseline",
    "orphan-baseline",
    "not-applicable-without-reason",
    "missing-result",
)
SOURCE_FILES = {
    "routes": Path("apps/showcase/src/app.rs"),
    "options": Path("apps/showcase/src/options.rs"),
    "holla": Path("apps/holla/src/scenario.rs"),
    "jackin": Path("src/bin/jackin/scenario.rs"),
}
FIXTURES = (
    ("complete", 0, "PASS counts"),
    ("missing-route", 1, "missing-route"),
    ("missing-test", 1, "missing-test"),
    ("missing-receipt", 1, "missing-receipt"),
    ("missing-baseline", 1, "missing-baseline"),
    ("duplicate-baseline", 1, "duplicate-baseline"),
    ("orphan-baseline", 1, "orphan-baseline"),
    ("not-applicable-without-reason", 1, "not-applicable-without-reason"),
)


class ScanError(Exception):
    """The source scanner could not read a declared name. Never means zero."""


def brace_body(text, open_at, path, pattern):
    depth = 0
    for index in range(open_at, len(text)):
        char = text[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return text[open_at + 1 : index]
    raise ScanError("{0}: pattern {1} has an unclosed brace".format(path, pattern))


def enum_variants(body):
    variants = []
    for line in body.splitlines():
        variant = line.partition("//")[0].strip()
        if variant.endswith(","):
            variant = variant[:-1].rstrip()
        if (
            variant
            and "A" <= variant[0] <= "Z"
            and all(
                character == "_"
                or "A" <= character <= "Z"
                or "a" <= character <= "z"
                or "0" <= character <= "9"
                for character in variant
            )
        ):
            variants.append(variant)
    return variants


def route_id_from_label(label):
    # PageId::from_name keeps ASCII letters and digits and lowercases them.
    # Inventory route ids are those names, not the Rust variant identifiers.
    return "".join(ch.lower() for ch in label if ch.isascii() and ch.isalnum())


def read_text(path):
    if not path.is_file():
        raise ScanError("{0}: file not found for pattern read".format(path))
    return path.read_text(encoding="utf-8")


def scan_routes(path):
    text = read_text(path)
    pattern = "pub enum PageId"
    match = re.search(r"pub enum PageId \{", text)
    if not match:
        raise ScanError("{0}: pattern {1} matched nothing".format(path, pattern))
    body = brace_body(text, match.end() - 1, path, pattern)
    variants = enum_variants(body)
    if not variants:
        raise ScanError("{0}: pattern PageId variant matched nothing".format(path))
    pairs = re.findall(
        r'id:\s*PageId::([A-Za-z0-9_]+)\s*,\s*label:\s*"([^"]*)"',
        text,
    )
    labels = {}
    for variant, label in pairs:
        route_id = route_id_from_label(label)
        if not route_id:
            raise ScanError(
                "{0}: pattern PageId::{1} label produced an empty route id".format(
                    path, variant
                )
            )
        previous = labels.get(variant)
        if previous is not None and previous != route_id:
            raise ScanError(
                "{0}: pattern PageId::{1} label is ambiguous".format(path, variant)
            )
        labels[variant] = route_id
    missing = [variant for variant in variants if variant not in labels]
    if missing:
        raise ScanError(
            "{0}: pattern PageId label matched nothing for {1}".format(
                path, ", ".join(missing)
            )
        )
    extra = sorted(set(labels) - set(variants))
    if extra:
        raise ScanError(
            "{0}: pattern PageId label names unknown variants {1}".format(
                path, ", ".join(extra)
            )
        )
    route_ids = [labels[variant] for variant in variants]
    if len(route_ids) != len(set(route_ids)):
        raise ScanError("{0}: pattern PageId route id is duplicated".format(path))
    return route_ids


def scan_option_keys(path):
    text = read_text(path)
    keys = []
    pattern = 'Opt::new("'
    for match in re.finditer(r"\bOpt::new\s*\(", text):
        key = re.match(r'\s*"([^"]+)"', text[match.end() :])
        if not key:
            raise ScanError("{0}: pattern {1} is not a string literal".format(path, pattern))
        keys.append(key.group(1))
    if not keys:
        raise ScanError("{0}: pattern {1} matched nothing".format(path, pattern))
    if len(keys) != len(set(keys)):
        raise ScanError("{0}: pattern {1} found duplicate keys".format(path, pattern))
    return keys


def scan_scenario_names(path):
    text = read_text(path)
    name_pattern = "fn name(self)"
    name_match = re.search(r"fn name\(self\)[^{]*\{", text)
    if not name_match:
        raise ScanError("{0}: pattern {1} matched nothing".format(path, name_pattern))
    name_body = brace_body(text, name_match.end() - 1, path, name_pattern)
    names = re.findall(r'=>\s*"([^"]+)"', name_body)
    if not names:
        raise ScanError(
            '{0}: pattern scenario name string => "..." matched nothing'.format(path)
        )
    enum_pattern = "pub enum Scenario"
    enum_match = re.search(r"pub enum Scenario \{", text)
    if not enum_match:
        raise ScanError("{0}: pattern {1} matched nothing".format(path, enum_pattern))
    enum_body = brace_body(text, enum_match.end() - 1, path, enum_pattern)
    variants = enum_variants(enum_body)
    all_pattern = "pub const ALL: [Scenario; N]"
    all_match = re.search(r"pub const ALL:\s*\[Scenario;\s*(\d+)\]", text)
    if not all_match:
        raise ScanError("{0}: pattern {1} matched nothing".format(path, all_pattern))
    declared = int(all_match.group(1))
    if not (len(names) == len(variants) == declared):
        raise ScanError(
            "{0}: scenario names {1}, enum variants {2}, and {3} value {4} disagree".format(
                path, len(names), len(variants), all_pattern, declared
            )
        )
    if len(names) != len(set(names)):
        raise ScanError("{0}: pattern scenario name string is duplicated".format(path))
    return names


def scan_rust_source():
    return {
        "routes": scan_routes(SOURCE_FILES["routes"]),
        "option_keys": scan_option_keys(SOURCE_FILES["options"]),
        "holla_scenarios": scan_scenario_names(SOURCE_FILES["holla"]),
        "jackin_scenarios": scan_scenario_names(SOURCE_FILES["jackin"]),
    }


def load_source_listing(path):
    data = json.loads(read_text(path))
    source = {}
    for key in ("routes", "option_keys", "holla_scenarios", "jackin_scenarios"):
        value = data.get(key) if isinstance(data, dict) else None
        if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
            raise ScanError("{0}: source listing {1} is not a list of strings".format(path, key))
        if len(value) != len(set(value)):
            raise ScanError("{0}: source listing {1} contains duplicates".format(path, key))
        source[key] = value
    return source


def load_json(path):
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        raise ScanError("{0}: file not found".format(path))
    except json.JSONDecodeError as exc:
        raise ScanError("{0}: invalid JSON ({1})".format(path, exc.msg))


def nextest_names(path):
    names = set()
    for raw in read_text(path).splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        names.add(line)
        names.add(line.split()[-1])
    return names


def load_receipts(directory):
    found = set()
    problems = []
    if not directory.is_dir():
        return found, problems
    for path in sorted(directory.rglob("*.json")):
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
        except json.JSONDecodeError:
            problems.append(str(path))
            continue
        case_id = data.get("case_id") if isinstance(data, dict) else None
        if not isinstance(case_id, str) or not case_id:
            problems.append(str(path))
            continue
        found.add(case_id)
    return found, problems


def strip_baseline_suffix(relative):
    if relative.endswith(".tui.pb"):
        return relative[: -len(".tui.pb")]
    if relative.endswith(".tui.json"):
        return relative[: -len(".tui.json")]
    if relative.endswith(".json"):
        return relative[: -len(".json")]
    return relative


def id_from_inventory_path(raw, baselines):
    posix = Path(raw).as_posix()
    marker = baselines.as_posix().rstrip("/") + "/"
    index = posix.find(marker)
    relative = posix[index + len(marker) :] if index != -1 else posix
    return strip_baseline_suffix(relative)


def baseline_files(baselines):
    files = {}
    if not baselines.is_dir():
        return files
    for path in sorted(baselines.rglob("*")):
        if path.is_file() and path.name.endswith(".tui.pb"):
            relative = path.relative_to(baselines).as_posix()
            files[strip_baseline_suffix(relative)] = path
    return files


def file_exists(baselines, files, baseline_id, hinted_path):
    if baseline_id in files:
        return True
    if not isinstance(hinted_path, str) or not hinted_path.endswith(".tui.pb"):
        return False
    hinted = Path(hinted_path)
    return hinted.is_file() or (baselines / hinted).is_file()


def summary_int(summary, key):
    if not isinstance(summary, dict) or key not in summary:
        return None
    value = summary[key]
    if isinstance(value, bool) or not isinstance(value, int):
        return None
    return value


def join_ids(values):
    shown = list(values)[:8]
    text = ", ".join(shown)
    extra = len(values) - len(shown)
    if extra:
        text += " and {0} more".format(extra)
    return text


def kind_counts(rows):
    counts = {kind: 0 for kind in KIND_ORDER}
    for row in rows:
        kind = row.get("kind")
        if kind in counts:
            counts[kind] += 1
    return counts


def rows_where(rows, kind, application=None):
    chosen = []
    for row in rows:
        if row.get("kind") != kind:
            continue
        if application is not None and row.get("application") != application:
            continue
        chosen.append(row)
    return chosen


def receipt_covers(row, receipts):
    case_id = row.get("case_id")
    if isinstance(case_id, str) and case_id in receipts:
        return True
    checkpoints = row.get("checkpoint_ids") or []
    if not isinstance(checkpoints, list) or not checkpoints:
        return False
    return all(isinstance(item, str) and item in receipts for item in checkpoints)


def disposition_ids(dispositions, baselines):
    retired = set()
    explained = set()
    for entry in dispositions:
        if not isinstance(entry, dict):
            continue
        status = entry.get("status")
        id_values = []
        if isinstance(entry.get("id"), str) and entry["id"]:
            id_values.append(entry["id"])
        if isinstance(entry.get("path"), str) and entry["path"]:
            id_values.append(id_from_inventory_path(entry["path"], baselines))
        for value in id_values:
            explained.add(value)
            if status == "retired":
                retired.add(value)
    return explained, retired


def active_row_baseline_ids(row, baselines, retired):
    if row.get("baseline_active") is False:
        return []
    found = []
    baseline_id = row.get("baseline_id")
    if isinstance(baseline_id, str) and baseline_id:
        found.append(baseline_id)
    paths = row.get("baseline_paths") or []
    if isinstance(paths, list):
        for raw in paths:
            if isinstance(raw, str) and raw:
                found.append(id_from_inventory_path(raw, baselines))
    unique = []
    for value in found:
        if value in retired or value in unique:
            continue
        unique.append(value)
    return unique


def compare_source(source, rows, summary, failures):
    checks = (
        ("routes", "showcase_routes", "route", "showcase", "route", "missing-route", "route"),
        ("option_keys", "option_rows", "option", None, "case_id", "missing-option", "option key"),
        (
            "holla_scenarios",
            "holla_scenarios",
            "scenario",
            "holla",
            "scenario",
            "missing-scenario",
            "holla scenario",
        ),
        (
            "jackin_scenarios",
            "jackin_scenarios",
            "scenario",
            "jackin",
            "scenario",
            "missing-scenario",
            "jackin scenario",
        ),
    )
    disagreements = []
    missing = defaultdict(list)
    for source_key, summary_key, kind, application, field, failure_class, label in checks:
        scanned = source[source_key]
        expected = summary_int(summary, summary_key)
        selected = rows_where(rows, kind, application)
        if expected is None:
            disagreements.append("{0} summary missing".format(summary_key))
        else:
            if len(scanned) != expected:
                disagreements.append(
                    "scanned {0}={1} summary {2}={3}".format(
                        label, len(scanned), summary_key, expected
                    )
                )
            if len(selected) != expected:
                disagreements.append(
                    "inventory {0} rows={1} summary {2}={3}".format(
                        label, len(selected), summary_key, expected
                    )
                )
        if field == "case_id":
            present = {
                row.get("case_id")
                for row in selected
                if isinstance(row.get("case_id"), str)
            }
            for key in scanned:
                if "opt.{0}".format(key) not in present and key not in present:
                    missing[failure_class].append(key)
        else:
            present = {
                row.get(field) for row in selected if isinstance(row.get(field), str)
            }
            for name in scanned:
                if name not in present:
                    missing[failure_class].append("{0} {1}".format(label, name))
    if disagreements:
        failures["source-disagreement"] = "source-disagreement: " + "; ".join(disagreements)
    for failure_class, names in missing.items():
        prefix = failures.get(failure_class)
        detail = join_ids(names)
        line = "{0}: source not in inventory: {1}".format(failure_class, detail)
        failures[failure_class] = line if prefix is None else prefix + "; " + detail


def agreement_line(source, summary, scan_error):
    if scan_error or source is None:
        return (
            "source-agreement showcase_routes=unscanned option_keys=unscanned "
            "holla_scenarios=unscanned jackin_scenarios=unscanned"
        )
    parts = []
    # option_rows is the inventory denominator for Opt::new keys.
    mapping = (
        ("showcase_routes", "routes", "showcase_routes"),
        ("option_keys", "option_keys", "option_rows"),
        ("holla_scenarios", "holla_scenarios", "holla_scenarios"),
        ("jackin_scenarios", "jackin_scenarios", "jackin_scenarios"),
    )
    for label, source_key, summary_key in mapping:
        scanned = len(source[source_key])
        expected = summary_int(summary, summary_key)
        if expected == scanned:
            parts.append("{0}={1}".format(label, scanned))
        elif expected is None:
            parts.append("{0}={1}/missing".format(label, scanned))
        else:
            parts.append("{0}={1}!={2}".format(label, scanned, expected))
    return "source-agreement " + " ".join(parts)


def check_inventory(inventory, source, nextest, receipts, receipt_problems, dispositions, baselines):
    failures = {}
    if not isinstance(inventory, dict):
        failures["inventory"] = "inventory: expected an object"
        return None, failures, 0
    rows = inventory.get("rows")
    summary = inventory.get("summary") if isinstance(inventory.get("summary"), dict) else {}
    if not isinstance(rows, list):
        failures["inventory"] = "inventory: rows is not a list"
        rows = []
    counts = kind_counts(row for row in rows if isinstance(row, dict))
    if source is None:
        failures["source-scan"] = "source-scan: source listing was not loaded"
    else:
        compare_source(source, [row for row in rows if isinstance(row, dict)], summary, failures)

    declared = inventory.get("current_tests")
    if not isinstance(declared, list):
        failures["inventory"] = failures.get(
            "inventory", "inventory: current_tests is not a list"
        )
        declared = []
    absent_tests = []
    for entry in declared:
        if isinstance(entry, str):
            name = entry
        elif isinstance(entry, dict):
            name = entry.get("test_name")
        else:
            name = None
        if not isinstance(name, str) or not name:
            absent_tests.append("(entry has no test_name)")
            continue
        if name not in nextest:
            absent_tests.append(name)
    if absent_tests:
        failures["missing-test"] = (
            "missing-test: declared test absent from the nextest list: {0}".format(
                join_ids(absent_tests)
            )
        )

    if receipt_problems:
        failures["bad-receipt"] = "bad-receipt: no case_id in {0}".format(
            join_ids(receipt_problems)
        )

    catalog = inventory.get("current_baselines")
    if not isinstance(catalog, list):
        failures["inventory"] = failures.get(
            "inventory", "inventory: current_baselines is not a list"
        )
        catalog = []
    if not isinstance(dispositions, list):
        failures["inventory"] = failures.get(
            "inventory", "inventory: dispositions is not a list"
        )
        dispositions = []

    explained, retired = disposition_ids(dispositions, baselines)
    files = baseline_files(baselines)
    row_owners = defaultdict(list)
    catalog_ids = []
    missing_files = []
    na_bad = []
    missing_receipt_rows = []
    result_counts = Counter()

    for entry in catalog:
        if not isinstance(entry, dict):
            continue
        test_id = entry.get("test_id")
        if isinstance(test_id, str) and test_id:
            explained.add(test_id)
            if entry.get("active", True) is not False and test_id not in retired:
                catalog_ids.append(test_id)
                if not file_exists(baselines, files, test_id, entry.get("path")):
                    missing_files.append(test_id)
        hinted = entry.get("path")
        if isinstance(hinted, str) and hinted:
            explained.add(id_from_inventory_path(hinted, baselines))

    for row in rows:
        if not isinstance(row, dict):
            continue
        case_id = row.get("case_id")
        label = case_id if isinstance(case_id, str) and case_id else "(row has no case_id)"
        for baseline_id in active_row_baseline_ids(row, baselines, retired):
            explained.add(baseline_id)
            row_owners[baseline_id].append(label)
            if not file_exists(baselines, files, baseline_id, None):
                missing_files.append(baseline_id)
        paths = row.get("baseline_paths") or []
        if isinstance(paths, list):
            for raw in paths:
                if isinstance(raw, str) and raw:
                    explained.add(id_from_inventory_path(raw, baselines))
        result = row.get("result")
        if result == "NOT_APPLICABLE":
            reason = row.get("disposition_reason")
            if not isinstance(reason, str) or not reason.strip():
                na_bad.append(label)
            continue
        if result == "PASS":
            if not receipt_covers(row, receipts):
                missing_receipt_rows.append(label)
            continue
        if not isinstance(result, str) or not result:
            result_counts["missing-result"] += 1
        else:
            result_counts[result] += 1

    duplicate_bits = []
    repeated_catalog = [item for item, count in sorted(Counter(catalog_ids).items()) if count > 1]
    if repeated_catalog:
        duplicate_bits.append(
            "current_baselines repeats {0}".format(join_ids(repeated_catalog))
        )
    shared_rows = [
        "{0} is shared by {1}".format(baseline_id, join_ids(owners))
        for baseline_id, owners in sorted(row_owners.items())
        if len(owners) > 1
    ]
    if shared_rows:
        duplicate_bits.extend(shared_rows)
    if duplicate_bits:
        failures["duplicate-baseline"] = "duplicate-baseline: " + "; ".join(duplicate_bits)

    unique_missing = sorted(set(missing_files))
    if unique_missing:
        failures["missing-baseline"] = (
            "missing-baseline: active baseline id has no file: {0}".format(
                join_ids(unique_missing)
            )
        )

    orphans = sorted(file_id for file_id in files if file_id not in explained)
    if orphans:
        failures["orphan-baseline"] = (
            "orphan-baseline: baseline file has no inventory row and no disposition: {0}".format(
                join_ids(orphans)
            )
        )
    if na_bad:
        failures["not-applicable-without-reason"] = (
            "not-applicable-without-reason: NOT_APPLICABLE row has an empty disposition_reason: {0}".format(
                join_ids(na_bad)
            )
        )
    if missing_receipt_rows:
        failures["missing-receipt"] = "missing-receipt: PASS row has no receipt: {0}".format(
            join_ids(missing_receipt_rows)
        )
    for result, count in sorted(result_counts.items()):
        if result == "missing-result":
            failures["missing-result"] = (
                "missing-result: {0} rows have no result".format(count)
            )
        else:
            failures[result] = "{0}: {1} required rows have result {2}".format(
                result, count, result
            )
    blocked = result_counts.get("BLOCKED", 0)
    return counts, failures, blocked


def render(counts, failures, agreement, blocked):
    ordered = []
    for key in FAILURE_ORDER:
        if key in failures:
            ordered.append(failures[key])
    for key in sorted(failures):
        if key not in FAILURE_ORDER:
            ordered.append(failures[key])
    if counts is None:
        status = "FAIL"
        count_text = "unavailable"
    else:
        if not ordered:
            status = "PASS"
        elif blocked:
            status = "BLOCKED"
        else:
            status = "FAIL"
        count_text = " ".join("{0}={1}".format(kind, counts[kind]) for kind in KIND_ORDER)
    lines = ["{0} counts {1}".format(status, count_text), agreement]
    lines.extend(ordered)
    return lines, (0 if not ordered else 1)


def evaluate(
    inventory_path,
    source,
    nextest_path,
    receipts_dir,
    dispositions_path,
    baselines_dir,
    scan_rust,
):
    try:
        inventory = load_json(inventory_path)
    except ScanError as exc:
        lines = [
            "FAIL counts unavailable",
            agreement_line(None, {}, True),
            "inventory: {0}".format(exc),
        ]
        return lines, 1
    scan_error = None
    if source is None and scan_rust:
        try:
            source = scan_rust_source()
        except ScanError as exc:
            scan_error = str(exc)
            source = None
    try:
        names = nextest_names(nextest_path)
    except ScanError as exc:
        names = set()
        nextest_error = "missing-test: {0}".format(exc)
    else:
        nextest_error = None
    receipts, receipt_problems = load_receipts(receipts_dir)
    try:
        dispositions = load_json(dispositions_path)
    except ScanError as exc:
        dispositions = []
        disposition_error = "inventory: {0}".format(exc)
    else:
        disposition_error = None
    summary = inventory.get("summary") if isinstance(inventory, dict) else {}
    counts, failures, blocked = check_inventory(
        inventory,
        source,
        names,
        receipts,
        receipt_problems,
        dispositions,
        baselines_dir,
    )
    if scan_error:
        failures["source-scan"] = "source-scan: {0}".format(scan_error)
    if nextest_error and "missing-test" not in failures:
        failures["missing-test"] = nextest_error
    if disposition_error:
        failures["inventory"] = disposition_error
    agreement = agreement_line(source, summary if isinstance(summary, dict) else {}, scan_error)
    if not isinstance(counts, dict):
        return render(None, failures, agreement, blocked)
    return render(counts, failures, agreement, blocked)


def run_fixtures(root):
    mismatched = []
    for name, expected, token in FIXTURES:
        directory = root / name
        if not directory.is_dir():
            print("fixture {0}: missing directory".format(name))
            mismatched.append(name)
            continue
        listing = None
        listing_error = None
        try:
            listing = load_source_listing(directory / "source.json")
        except ScanError as exc:
            listing_error = str(exc)
        lines, code = evaluate(
            directory / "inventory.json",
            listing,
            directory / "nextest-list.txt",
            directory / "receipts",
            directory / "dispositions.json",
            directory / "baselines",
            False,
        )
        if listing_error:
            lines = list(lines) + ["source-scan: {0}".format(listing_error)]
            code = 1
        ok_code = code == 0 if expected == 0 else code != 0
        text = "\n".join(lines)
        print("fixture {0}: exit {1}".format(name, code))
        for line in lines:
            print("  {0}".format(line))
        if not ok_code or token not in text:
            mismatched.append(name)
    if mismatched:
        print("fixtures FAIL " + ", ".join(mismatched))
        return 1
    print("fixtures PASS")
    return 0


def parse_args(argv):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--inventory", type=Path)
    parser.add_argument("--nextest-list", type=Path)
    parser.add_argument("--baselines", type=Path)
    parser.add_argument("--dispositions", type=Path)
    parser.add_argument("--receipts", type=Path)
    parser.add_argument("--fixtures", type=Path)
    parser.add_argument(
        "--require-pass",
        action="store_true",
        help="Fail unless every required row result is PASS. This is the default.",
    )
    return parser.parse_args(argv)


def main(argv=None):
    args = parse_args(argv)
    if args.fixtures:
        return run_fixtures(args.fixtures)
    required = {
        "--inventory": args.inventory,
        "--nextest-list": args.nextest_list,
        "--baselines": args.baselines,
        "--dispositions": args.dispositions,
        "--receipts": args.receipts,
    }
    missing = [flag for flag, value in required.items() if value is None]
    if missing:
        print("missing {0}".format(" ".join(missing)), file=sys.stderr)
        return 2
    # Required rows that are not PASS fail the process. --require-pass states
    # that rule; omitting it does not weaken the gate, because a BLOCKED
    # inventory is not evidence.
    del args.require_pass
    lines, code = evaluate(
        args.inventory,
        None,
        args.nextest_list,
        args.receipts,
        args.dispositions,
        args.baselines,
        True,
    )
    for line in lines:
        print(line)
    return code


if __name__ == "__main__":
    sys.exit(main())
