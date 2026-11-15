#!/usr/bin/env python3
"""Re-accept every changed functional baseline through accept-baseline.py.

Run a functional suite first so `target/functional/*/check-artifacts/` holds a
sample for each test. The script then finds each test whose latest sample
differs from the baseline on disk, confirms with `tuiscotti check` that real
cells changed, and drives `scripts/accept-baseline.py` once per test with
`--expect-baseline` set to the digest of the baseline now in the tree. After a
rebase that digest is main's current one, so a stale capture cannot overwrite a
baseline another change already moved.

Receipts, provenance and the superseded baselines go under the topic directory,
and each acceptance appends a row to baseline-refreshes-20261108.tsv.
`--dry` lists the candidates and accepts nothing. `--tests FILE` limits the run
to the test ids listed in FILE, one per line.

    python3 scripts/accept-changed-baselines.py \\
        --topic docs/testing/acceptance-receipts/<topic>-<issue>-<date> \\
        --capture-note "Why the capture changed" [--tests changed.txt] [--dry]
"""

import argparse
import hashlib
import json
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BASELINES = ROOT / "tests/functional/baselines"
REFRESH_TSV = ROOT / "docs/testing/baseline-refreshes-20261108.tsv"
BINARY_TSV = ROOT / "docs/testing/binary-baselines-20261108.tsv"


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def find_baseline(test_id):
    direct = BASELINES / f"{test_id}.tui.pb"
    if direct.exists():
        return direct
    hits = list(BASELINES.glob(f"pre_refactor/*/{test_id}.tui.pb"))
    return hits[0] if len(hits) == 1 else None


def baseline_root(test_id, baseline):
    return BASELINES if (BASELINES / f"{test_id}.tui.pb").exists() else baseline.parent


def latest_samples(evidence):
    """The newest sample per test id whose digest differs from its baseline."""
    found = {}
    runs = sorted(evidence.iterdir(), key=lambda p: p.stat().st_mtime)
    for run in runs:
        artifacts = run / "check-artifacts"
        if not artifacts.is_dir():
            continue
        for sample in artifacts.rglob("sample.tui.pb"):
            parts = sample.relative_to(artifacts).parts
            test_id, digest = "/".join(parts[:-2]), parts[-2]
            baseline = find_baseline(test_id)
            if baseline is None or sha256(baseline) == digest:
                continue
            mtime = run.stat().st_mtime
            if test_id not in found or mtime >= found[test_id][3]:
                found[test_id] = (sample, run, digest, mtime)
    return found


def cells_changed(tuiscotti, test_id, sample):
    """False when the sample matches the baseline cell for cell."""
    baseline = find_baseline(test_id)
    with tempfile.TemporaryDirectory() as artifacts:
        result = subprocess.run(
            [tuiscotti, "--json", "check", "--sample", str(sample),
             "--baselines", str(baseline_root(test_id, baseline)),
             "--artifacts", artifacts],
            capture_output=True, text=True, check=False,
        )
        verdict = json.loads(result.stdout)
        if verdict["status"] == "pass":
            return False
        report = Path(verdict["output_path"]["value"])
        text = report.read_text() if report.exists() else ""
        match = re.search(r"Changed cells:\s*(?:<[^>]+>\s*|</[^>]+>\s*)*(\d+)", text)
        return not (match and int(match.group(1)) == 0)


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--topic", required=True, help="receipt directory under the repo root")
    parser.add_argument("--capture-note", required=True)
    parser.add_argument("--precondition", default="")
    parser.add_argument("--evidence", default=str(ROOT / "target/functional"))
    parser.add_argument("--tests", help="file of test ids to accept, one per line")
    parser.add_argument("--tuiscotti", default="tuiscotti")
    parser.add_argument("--dry", action="store_true")
    args = parser.parse_args()

    topic = args.topic.rstrip("/")
    wanted = None
    if args.tests:
        wanted = {
            line.strip() for line in Path(args.tests).read_text().splitlines() if line.strip()
        }

    candidates = {}
    for test_id, (sample, run, digest, _) in latest_samples(Path(args.evidence)).items():
        if wanted is not None and test_id not in wanted:
            continue
        if cells_changed(args.tuiscotti, test_id, sample):
            candidates[test_id] = (sample, run, digest)
    print(f"candidates {len(candidates)}")
    if wanted is not None:
        for missing in sorted(wanted - candidates.keys()):
            print(f"NO CHANGED SAMPLE {missing}")
    if args.dry:
        for test_id in sorted(candidates):
            print(f"would accept {test_id}")
        return 0

    binary_rows = BINARY_TSV.read_text().splitlines()
    binary_index = {row.split("\t")[0]: i for i, row in enumerate(binary_rows) if i}
    refresh_rows = []
    failed = 0
    for test_id, (sample, run, digest) in sorted(candidates.items()):
        baseline = find_baseline(test_id)
        expect = sha256(baseline)
        flat = test_id.replace("/", "_")
        archive = f"{topic}/superseded/{test_id}.tui.pb"
        receipt = f"{topic}/{flat}.accept.json"
        provenance = f"{topic}/{flat}.provenance.json"
        (ROOT / archive).parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(baseline, ROOT / archive)
        session = json.loads((run / "session.json").read_text())
        command = [
            sys.executable, str(ROOT / "scripts/accept-baseline.py"),
            "--candidate", str(sample), "--candidate-sha256", digest,
            "--expect-baseline", expect,
            "--baselines", str(baseline_root(test_id, baseline)),
            "--receipt", receipt, "--provenance", provenance,
            "--previous-archive", archive,
            "--precondition", args.precondition,
            "--capture-note", args.capture_note,
            "--binary", session["binary_path"],
            "--binary-sha256", session["binary_sha256"],
            "--expected-test-id", test_id,
            "--tuiscotti", args.tuiscotti,
        ]
        command += [f"--fixture-arg={arg}" for arg in session["args"]]
        done = subprocess.run(command, capture_output=True, text=True, cwd=ROOT, check=False)
        if done.returncode != 0:
            failed += 1
            print(f"FAIL {test_id}: {done.stderr[-300:]}")
            continue
        accepted = sha256(baseline)
        if accepted != digest:
            failed += 1
            print(f"FAIL {test_id}: baseline digest {accepted} is not the candidate {digest}")
            continue
        refresh_rows.append("\t".join([
            test_id, str(baseline.relative_to(ROOT)), accepted, expect,
            archive, receipt, provenance,
        ]))
        if test_id in binary_index:
            fields = binary_rows[binary_index[test_id]].split("\t")
            fields[2], fields[3] = accepted, receipt
            binary_rows[binary_index[test_id]] = "\t".join(fields)

    with REFRESH_TSV.open("a") as handle:
        handle.writelines(row + "\n" for row in refresh_rows)
    BINARY_TSV.write_text("\n".join(binary_rows) + "\n")
    print(f"accepted {len(refresh_rows)}, failed {failed}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
