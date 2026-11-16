#!/usr/bin/env python3
"""Classify the #14 declared-API debt and split it into per-lane files.

Planning and enabling tool for the burn-down. It is not part of the gate:
`src/catalog_contract.rs` is. The gate reads every `*.txt` under
`src/bin/showcase/coverage_debt/`, so each lane edits only its own file.

Two steps, both rerunnable:

  1. Emit the facts (kind, shape, owner, definition file) for every debt id:

       cargo nextest run -p junie-tui --lib \
         --run-ignored only -E 'test(catalog_debt_facts)'

  2. Classify and split:

       python3 -I scripts/catalog-debt-lanes.py report
       python3 -I scripts/catalog-debt-lanes.py write 

`write` only redistributes ids that are already in the frozen baseline. It
never adds an id and never drops one, so it cannot loosen the ratchet. `list` shows a lane's
remaining ids and `proofs` records proofs for a selection and deletes exactly
those debt lines, so a lane never hand-edits a debt file. After a
lane lands proofs, `prune` removes every line the gate would call stale (an id
that is no longer an export or already has a proof or exception), straight from
the gate's own error output, so no lane hand-edits its debt file.

Classes (see docs in the plan, not in the gate):

  A  inherits its owner today (field, trait item, trait impl): never in debt.
  B  needs a behavioral proof: a callable scenario that uses the API.
  C  candidate for a Nonvisual exception (host plumbing, constants, aliases,
     macro expansions) with a real named regression test.

`demote` is a flag, not a class: nothing outside the defining file names the
API, so the owner may decide it should not be public.
"""

import argparse
import collections
import csv
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SHOWCASE = ROOT / "src/bin/showcase"
DEBT_DIR = SHOWCASE / "coverage_debt"
BASELINE = SHOWCASE / "coverage_debt_baseline.txt"
LEGACY = SHOWCASE / "coverage_debt.txt"
FACTS = ROOT / "target/debt-facts.tsv"
REPORT = ROOT / "target/debt-classified.tsv"
COVERAGE = SHOWCASE / "coverage.rs"

# Longest prefix wins. Every id must match one prefix: an unassigned module is
# an error, so a new module cannot hide from the plan.
LANES = {
    "shell": (
        "shell runtime clock motion cli catalog docs reach_tests!"
    ),
    "theme": (
        "theme ui::specimen ui::text ui::fade ui::measure ui::content ui::row "
        "widgets::brand widgets::keyref widgets::segments"
    ),
    "routing": (
        "core::pointer core::hit core::event core::keyboard core::keymap "
        "core::intent core::hints core::refusal"
    ),
    "state": (
        "core::focus core::response core::id core::selection core::list "
        "core::nav core::shape core::typeahead core::edit core::text"
    ),
    "scroll": (
        "core::scroll widgets::viewport widgets::viewport_ops ui::scroll_view "
        "widgets::scrollbar ui::reveal terminal widgets::panel"
    ),
    "gridtable": (
        "widgets::grid widgets::table widgets::diff widgets::inspect_changes"
    ),
    "fields": (
        "widgets::edit widgets::input widgets::textarea widgets::field_common "
        "widgets::code widgets::choice widgets::button widgets::icon_button "
        "widgets::action_row"
    ),
    "dialogs": (
        "widgets::dialog widgets::form_dialog widgets::info_dialog "
        "widgets::choice_dialog ui::modal ui::leave ui::layer ui::layers ui::ctx"
    ),
    "menus": (
        "widgets::menu ui::popup ui::context ui::edit_menu widgets::row_menu "
        "widgets::select ui::tooltip widgets::help widgets::hintbar "
        "widgets::statusbar widgets::keyhint widgets::widget_hints"
    ),
    "pickers": (
        "widgets::picker widgets::option_list widgets::option_editor "
        "widgets::completion widgets::meter_list widgets::progress widgets::props"
    ),
    "collections": (
        "widgets::tree widgets::tabs widgets::list widgets::nav_list "
        "widgets::list_detail ui::master_detail widgets::file_browser"
    ),
    "layout": (
        "ui::pane_tree ui::layout ui::columns widgets::splitter widgets::chips "
        "widgets::toolbar widgets::steps widgets::rail widgets::breadcrumb "
        "ui::list_core widgets::notice widgets::empty widgets::page_header "
        "widgets::detail_card widgets::too_small"
    ),
}

# Families the host application or a test harness feeds, not a page. Anything
# here is a Nonvisual-exception candidate; the lane still owes a real test.
# `shell` and `runtime` are not here: Footer, KeyRouter, Cx and the job runtime
# have behavior a scenario can drive, so they need a proof.
HOST_FAMILIES = ("clock", "cli", "catalog", "docs", "motion", "reach_tests!")
CONST_SHAPES = ("const", "assoc_const", "static", "type:alias")

PREFIX_TO_LANE = sorted(
    ((p, lane) for lane, ps in LANES.items() for p in ps.split()),
    key=lambda kv: -len(kv[0]),
)


def lane_of(api_id):
    for prefix, lane in PREFIX_TO_LANE:
        if api_id == prefix or api_id.startswith(prefix + "::"):
            return lane
    return None


def family_of(api_id):
    segs = api_id.split("::")
    if segs[0] in ("widgets", "core", "ui") and len(segs) > 1:
        return "::".join(segs[:2])
    return segs[0]


def ids_in(text):
    out = []
    for line in text.splitlines():
        line = line.split("#", 1)[0].strip()
        if line:
            out.append(line)
    return out


def debt_lines():
    """Every debt line as written, with its trailing `# note`, keyed by id."""
    paths = sorted(DEBT_DIR.glob("*.txt")) if DEBT_DIR.is_dir() else [LEGACY]
    out = {}
    for path in paths:
        for line in path.read_text().splitlines():
            api = line.split("#", 1)[0].strip()
            if api:
                if api in out:
                    sys.exit(f"duplicate debt id {api}")
                out[api] = line.rstrip()
    return out


def current_debt():
    return list(debt_lines())


def proof_ids():
    """Every `api:` literal in coverage.rs and the lane proof files."""
    text = COVERAGE.read_text()
    for path in sorted((SHOWCASE / "coverage").glob("*.rs")):
        text += path.read_text()
    return set(re.findall(r'CapabilityProof\s*\{\s*api:\s*"([^"]+)"', text))


def read_facts(path):
    with open(path, newline="") as fh:
        return list(csv.DictReader(fh, delimiter="\t"))


def token_sets():
    """Identifier sets per source file outside the library's own definition."""
    sets = {}
    roots = ["apps", "tests", "src", "crates", "docs"]
    for root in roots:
        for path in (ROOT / root).rglob("*"):
            if path.suffix not in (".rs", ".md") or "target" in path.parts:
                continue
            try:
                sets[str(path.relative_to(ROOT))] = set(
                    re.findall(r"[A-Za-z_][A-Za-z0-9_]*", path.read_text())
                )
            except UnicodeDecodeError:
                continue
    return sets


def classify(rows, proven):
    tokens = token_sets()
    out = []
    for row in rows:
        api = row["id"]
        member = row["member"]
        owner = row["owner"]
        shape = row["shape"]
        defined_in = row["file"]
        family = family_of(api)
        used_outside = 0
        for path, toks in tokens.items():
            if path == defined_in:
                continue
            if member in toks and (not owner or owner in toks):
                used_outside += 1
        owner_id = api.rsplit("::", 1)[0] if owner else ""
        owner_proven = bool(owner_id) and owner_id in proven
        if row["inv_kind"] in ("Field", "TraitItem", "TraitImpl"):
            klass = "A"
        elif (
            family.split("::")[0] in HOST_FAMILIES
            or shape in CONST_SHAPES
            or row["inv_kind"] in ("Expansion", "Macro")
        ):
            klass = "C"
        else:
            klass = "B"
        if klass == "B":
            if row["inv_kind"] == "Type":
                tier = "type"
            elif row["inv_kind"] == "Variant":
                tier = "variant"
            elif shape.startswith("fn:") and (":ret=self" in shape and "own_self" in shape):
                tier = "builder"
            elif shape.startswith("fn:ref_self:args=0"):
                tier = "getter"
            elif shape.startswith("fn:mut_self"):
                tier = "mutator"
            elif shape.startswith("fn:assoc") and ":ret=self" in shape:
                tier = "constructor"
            else:
                tier = "behavior"
        else:
            tier = {"C": "nonvisual"}.get(klass, "-")
        out.append(
            {
                "id": api,
                "lane": lane_of(api),
                "family": family,
                "class": klass,
                "tier": tier,
                "owner_proven": owner_proven,
                "demote": used_outside == 0 and shape != "unresolved",
                "used_outside": used_outside,
                "named_by": int(row["named_by"]),
                "first_test": row["first_test"],
                "kind": row["inv_kind"],
                "shape": shape,
            }
        )
    return out


def cmd_report(args):
    rows = read_facts(FACTS)
    classified = classify(rows, proof_ids())
    unassigned = [c["id"] for c in classified if c["lane"] is None]
    if unassigned:
        sys.exit(f"{len(unassigned)} ids match no lane, first: {unassigned[:5]}")
    print(f"total debt ids: {len(classified)}")
    by_class = collections.Counter(c["class"] for c in classified)
    print("by class:", dict(sorted(by_class.items())))
    print("by class+tier:")
    for (k, t), n in sorted(collections.Counter((c["class"], c["tier"]) for c in classified).items()):
        print(f"  {k}/{t}: {n}")
    print("owner already proven:", sum(c["owner_proven"] for c in classified))
    print("already named by a live #[test] (the exception check's own test):",
          sum(c["named_by"] > 0 for c in classified),
          "of which class C:", sum(c["named_by"] > 0 and c["class"] == "C" for c in classified))
    print("demote candidates (nothing outside the defining file names them):",
          sum(c["demote"] for c in classified))
    print()
    print(f"{'lane':<12}{'total':>6}{'B':>6}{'C':>6}{'C+test':>8}{'demote':>8}  tiers")
    for lane in LANES:
        mine = [c for c in classified if c["lane"] == lane]
        tiers = collections.Counter(c["tier"] for c in mine)
        print(
            f"{lane:<12}{len(mine):>6}"
            f"{sum(c['class'] == 'B' for c in mine):>6}"
            f"{sum(c['class'] == 'C' for c in mine):>6}"
            f"{sum(c['class'] == 'C' and c['named_by'] > 0 for c in mine):>8}"
            f"{sum(c['demote'] for c in mine):>8}  "
            + " ".join(f"{t}={n}" for t, n in sorted(tiers.items()))
        )
    print()
    print("per family (lane, count, B, C):")
    fam = collections.defaultdict(list)
    for c in classified:
        fam[(c["lane"], c["family"])].append(c)
    for (lane, family), mine in sorted(fam.items()):
        print(
            f"  {lane:<12}{family:<26}{len(mine):>5}"
            f"{sum(c['class'] == 'B' for c in mine):>5}"
            f"{sum(c['class'] == 'C' for c in mine):>5}"
        )
    with open(REPORT, "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=list(classified[0]), delimiter="\t")
        w.writeheader()
        w.writerows(classified)
    print(f"classified rows: {REPORT.relative_to(ROOT)}")


HEADER = """# Lane `{lane}` debt for #14: APIs that still owe a disposition.
# One id per line. This file is owned by exactly one lane; edit no other.
# Delete a line when the API gets a CapabilityProof or CapabilityException
# in src/bin/showcase/coverage/lane_{lane}.rs. Never add an id: the list is a
# ratchet that only shrinks (src/catalog_contract.rs, coverage_debt_baseline.txt).
# `python3 -I scripts/catalog-debt-lanes.py prune` removes the lines the gate
# calls stale.
"""


def cmd_write(args):
    baseline = set(ids_in(BASELINE.read_text()))
    lines = debt_lines()
    debt = list(lines)
    if not set(debt) <= baseline:
        sys.exit("current debt is not a subset of the baseline")
    lanes = collections.defaultdict(list)
    for api in debt:
        lane = lane_of(api)
        if lane is None:
            sys.exit(f"no lane for {api}")
        lanes[lane].append(api)
    DEBT_DIR.mkdir(exist_ok=True)
    for lane in LANES:
        text = HEADER.format(lane=lane) + "".join(f"{lines[i]}\n" for i in sorted(lanes[lane]))
        (DEBT_DIR / f"{lane}.txt").write_text(text)
    for stale in DEBT_DIR.glob("*.txt"):
        if stale.stem not in LANES:
            sys.exit(f"{stale} belongs to no lane")
    if LEGACY.exists():
        LEGACY.unlink()
    total = sum(len(v) for v in lanes.values())
    assert total == len(debt)
    print(f"wrote {len(LANES)} lane files, {total} ids")


def cmd_prune(args):
    """Delete the debt lines the gate reports stale, from the gate's output on stdin.

        cargo nextest run -p junie-tui --lib --no-tests fail \\
          -E 'test(catalog_repository_gate_is_closed)' --no-capture 2>&1 \\
          | python3 -I scripts/catalog-debt-lanes.py prune
    """
    output = sys.stdin.read()
    stale = set(
        re.findall(r"debt entry is stale[^:]*?(?:\(delete the line\))?: (\S+)", output)
    )
    if not stale:
        print("no stale debt lines")
        return
    removed = 0
    for path in sorted(DEBT_DIR.glob("*.txt")):
        kept, lines = [], path.read_text().splitlines(keepends=True)
        for line in lines:
            if line.split("#", 1)[0].strip() in stale:
                removed += 1
            else:
                kept.append(line)
        path.write_text("".join(kept))
    print(f"removed {removed} stale lines")


LANE_DEBT_FILES = {name: DEBT_DIR / f"{name}.txt" for name in LANES}


def lane_debt_path(lane):
    if lane not in LANE_DEBT_FILES:
        sys.exit(f"unknown lane {lane!r}, expected one of {', '.join(LANES)}")
    return LANE_DEBT_FILES[lane]


def select(lane, prefixes, exact):
    """Debt ids of `lane` that equal an `exact` id or start with a prefix."""
    mine = [i for i in ids_in(lane_debt_path(lane).read_text())]
    picked = [
        i for i in mine
        if i in exact or any(i == p or i.startswith(p) for p in prefixes)
    ]
    missing = [e for e in exact if e not in mine]
    if missing:
        sys.exit(f"not in the {lane} debt file: {missing[:5]}")
    return picked


# Fixed lane file paths, built from the lane constants only.
LANE_PROOF_FILES = {name: SHOWCASE / "coverage" / f"lane_{name}.rs" for name in LANES}


def known_lane(name):
    """The lane constant that equals `name`, so a path never carries user text."""
    for lane in LANES:
        if lane == name:
            return lane
    sys.exit(f"unknown lane {name!r}; choose from {', '.join(sorted(LANES))}")


def cmd_list(args):
    for api in select(known_lane(args.lane), args.match or [""], []):
        print(api)


def cmd_proofs(args):
    """Record a proof for each selected debt id and delete its debt line.

    Run this only after the scenario really exercises every selected API: the
    gate checks that the records are well formed and the scenario is callable,
    never that the scenario uses the API.
    """
    lane = known_lane(args.lane)
    picked = select(lane, args.match or [], args.id or [])
    if not picked:
        sys.exit("nothing selected: pass --match PREFIX or --id API")
    scenarios = ", ".join(f"ScenarioId::{s}" for s in args.scenarios.split(","))
    records = "".join(
        f'    CapabilityProof {{ api: "{api}", route: PageId::{args.route}, '
        f"scenarios: &[{scenarios}], feature: None }},\n"
        for api in picked
    )
    lane_file = LANE_PROOF_FILES[lane]
    text = lane_file.read_text()
    anchor = "pub const PROOFS: &[CapabilityProof] = &[\n"
    if anchor not in text:
        sys.exit(f"{lane_file} has no PROOFS array")
    text = text.replace(anchor, anchor + records, 1)
    uses = [line for line in text.splitlines() if line.startswith("use ")]
    if not any("ScenarioId" in line for line in uses):
        text = text.replace(
            "use super::{CapabilityException, CapabilityProof};",
            "use super::{CapabilityException, CapabilityProof, ScenarioId};",
            1,
        )
    if not any("PageId" in line for line in uses):
        text = text.replace("\nuse super::{", "\nuse crate::app::PageId;\nuse super::{", 1)
    lane_file.write_text(text)
    debt = lane_debt_path(lane)
    keep = [
        line for line in debt.read_text().splitlines(keepends=True)
        if line.split("#", 1)[0].strip() not in set(picked)
    ]
    debt.write_text("".join(keep))
    print(f"recorded {len(picked)} proofs in {lane_file.name}, removed them from {debt.name}")


def identifier(text):
    if not re.fullmatch(r"[A-Za-z][A-Za-z0-9]*", text):
        raise argparse.ArgumentTypeError(f"not a Rust identifier: {text!r}")
    return text


def identifier_list(text):
    for part in text.split(","):
        identifier(part)
    return text


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("report", help="print counts per class, lane and family")
    r.set_defaults(fn=cmd_report)
    w = sub.add_parser("write", help="split the debt into coverage_debt/<lane>.txt")
    w.set_defaults(fn=cmd_write)
    p = sub.add_parser("prune", help="delete stale debt lines reported by the gate")
    p.set_defaults(fn=cmd_prune)
    ls = sub.add_parser("list", help="print a lane's remaining debt ids")
    ls.add_argument("lane", choices=sorted(LANES))
    ls.add_argument("--match", action="append", help="id prefix (repeatable)")
    ls.set_defaults(fn=cmd_list)
    pr = sub.add_parser(
        "proofs",
        help="record proofs for selected debt ids and delete their debt lines",
    )
    pr.add_argument("lane", choices=sorted(LANES))
    pr.add_argument("--route", required=True, type=identifier, help="an existing PageId, e.g. Pickers")
    pr.add_argument("--scenarios", required=True, type=identifier_list, help="ScenarioId variants, comma separated")
    pr.add_argument("--match", action="append", help="id prefix (repeatable)")
    pr.add_argument("--id", action="append", help="exact id (repeatable)")
    pr.set_defaults(fn=cmd_proofs)
    args = ap.parse_args()
    args.fn(args)


if __name__ == "__main__":
    main()
