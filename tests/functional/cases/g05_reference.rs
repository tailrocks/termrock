use std::path::{Path, PathBuf};
use std::process::Command;
use std::{fs::File, io::Read};

use serde_json::Value;
use sha2::{Digest, Sha256};
use tui_test::profile::Rgb;
use tui_test::{
    SnapshotCell, SnapshotColor, SnapshotCursor, SnapshotCursorShape, SnapshotMetadata,
    SnapshotParts, SnapshotScope, SnapshotUnderline, TerminalSnapshot,
};

use crate::bins::showcase;
use crate::support::{
    App, Launch, SessionMode, reference_binary, reference_digest, text, try_launch,
};

const REFERENCE_SHA: &str = "fffe002e936d3621f22f9132144066a1374d5d74";
const SHOWCASE_REFERENCE_SHA256: &str =
    "430460e90a752587d7f1f6d2e37b4d8343076a2787a343048fbd3448b3d54a0b";
const COMPARATOR_REV: &str = "92ef2d9ee082b35f68e066c5129fedbf07675a80";
const ACCEPTANCE_COMPARATOR_REV: &str = "d8d54155947f6303a41dfd6079b53c0abc16f29a";
const ACCEPTANCE_LOCKFILE_SHA256: &str =
    "582b6cd514d0bd9c90966bdea90028ce24b652a0607d0ef7b8369a1a7532e319";

fn sha256_file(path: &Path) -> String {
    let mut file = File::open(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    format!("{:x}", digest.finalize())
}

fn manifest() -> Value {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/testing/reference-manifest.json");
    serde_json::from_slice(&std::fs::read(&path).expect("reference manifest")).expect("manifest")
}

fn reference_showcase() -> PathBuf {
    let bin = reference_binary("showcase");
    assert!(
        bin.is_file(),
        "missing reference showcase {}",
        bin.display()
    );
    assert_eq!(sha256_file(&bin), reference_digest("showcase"));
    bin
}

fn launch_showcase(name: &str) -> App {
    let mut launch = Launch::new(name, showcase());
    launch.args = vec![
        "--page".to_string(),
        "overview".to_string(),
        "--frame".to_string(),
        "40".to_string(),
    ];
    launch.motion = Some("paused".to_string());
    launch.color = Some("truecolor".to_string());
    launch.cols = 120;
    launch.rows = 40;
    try_launch(launch).expect("launch candidate showcase")
}

fn baseline_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/functional/baselines")
}

fn tree_digest(root: &Path) -> String {
    let mut files = Vec::new();
    let mut dirs = vec![root.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        let mut entries = std::fs::read_dir(&dir)
            .unwrap_or_else(|error| panic!("read {}: {error}", dir.display()))
            .collect::<Result<Vec<_>, _>>()
            .unwrap_or_else(|error| panic!("entry: {error}"));
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let kind = entry.file_type().expect("file type");
            assert!(!kind.is_symlink(), "symlink {}", path.display());
            if kind.is_dir() {
                dirs.push(path);
            } else {
                files.push(path);
            }
        }
    }
    files.sort();
    let output = if Path::new("/usr/bin/shasum").is_file() {
        Command::new("/usr/bin/shasum")
            .arg("-a")
            .arg("256")
            .args(&files)
            .output()
    } else {
        Command::new("/usr/bin/sha256sum").args(&files).output()
    }
    .expect("hash baseline tree");
    assert!(
        output.status.success(),
        "baseline tree hash failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("hash output")
}

fn line_index(actions: &str, exact: &str) -> usize {
    actions
        .lines()
        .position(|line| line == exact)
        .unwrap_or_else(|| panic!("missing {exact:?}\n{actions}"))
}

fn prefix_index(actions: &str, prefix: &str) -> usize {
    actions
        .lines()
        .position(|line| line.starts_with(prefix))
        .unwrap_or_else(|| panic!("missing prefix {prefix:?}\n{actions}"))
}

fn read_disposition_sample(path: &Path) -> tuiscotti_latest::sample::Sample {
    tuiscotti_latest::sample::Sample::from_file(path)
        .unwrap_or_else(|error| panic!("read disposition sample {}: {error}", path.display()))
}

/// Decodes every sample and checks that it carries the test ID its record
/// names. Decoding one sample costs about 10 ms of CPU, and the manifest holds
/// about two thousand, so one thread needs 20 s of CPU and, on a loaded machine,
/// outlasts the 90 s nextest terminates a functional test at. The samples are
/// independent, so they are split across threads, which also gives the test
/// more than one share of a busy machine.
fn assert_sample_ids(samples: Vec<(PathBuf, String)>) {
    let workers = std::thread::available_parallelism()
        .map_or(4, usize::from)
        .clamp(4, 16);
    let chunk = samples.len().div_ceil(workers).max(1);
    std::thread::scope(|scope| {
        for part in samples.chunks(chunk) {
            scope.spawn(move || {
                for (path, test_id) in part {
                    let sample = read_disposition_sample(path);
                    assert_eq!(sample.test_id(), test_id, "{test_id}");
                }
            });
        }
    });
}

fn active_baseline_path(root: &Path, test_id: &str) -> PathBuf {
    root.join(format!("{test_id}.tui.pb"))
}

fn validate_refresh_provenance(
    provenance: &Value,
    test_id: &str,
    previous_sha: &str,
    active_sha: &str,
    archive_path: &str,
    receipt_path: &str,
    provenance_path: &str,
) -> Result<(), String> {
    if provenance["test_id"].as_str() != Some(test_id) {
        return Err(format!(
            "refresh provenance test ID does not match {test_id}"
        ));
    }

    fn check_digests(
        provenance: &Value,
        test_id: &str,
        expected: &str,
        pointers: &[&str],
        label: &str,
    ) -> Result<(), String> {
        let mut found = false;
        for pointer in pointers {
            let Some(value) = provenance.pointer(pointer) else {
                continue;
            };
            let Some(actual) = value.as_str() else {
                return Err(format!("{label} digest is not a string for {test_id}"));
            };
            if actual != expected {
                return Err(format!(
                    "{label} digest conflicts with refresh ledger for {test_id}"
                ));
            }
            found = true;
        }
        if !found {
            return Err(format!("{label} digest is missing for {test_id}"));
        }
        Ok(())
    }

    check_digests(
        provenance,
        test_id,
        previous_sha,
        &[
            "/acceptance/expected_baseline_sha256",
            "/history/superseded_baseline_sha256",
            "/previous_baseline/sha256",
        ],
        "previous baseline",
    )?;
    check_digests(
        provenance,
        test_id,
        active_sha,
        &[
            "/acceptance/accepted_baseline_sha256",
            "/acceptance/candidate_sha256",
            "/acceptance/accept_command/candidate_sha256",
            "/acceptance/accept_command/baseline_sha256",
            "/candidate/sha256",
        ],
        "accepted baseline",
    )?;

    for (field, expected) in [
        ("previous_archive_path", archive_path),
        ("acceptance_receipt_path", receipt_path),
        ("provenance_path", provenance_path),
    ] {
        if provenance[field].as_str() != Some(expected) {
            return Err(format!(
                "refresh provenance {field} conflicts with ledger for {test_id}"
            ));
        }
    }
    for (pointer, expected, label) in [
        (
            "/history/superseded_baseline",
            archive_path,
            "history archive",
        ),
        (
            "/acceptance/transaction_receipt",
            receipt_path,
            "acceptance receipt",
        ),
    ] {
        let Some(value) = provenance.pointer(pointer) else {
            continue;
        };
        if value.as_str() != Some(expected) {
            return Err(format!(
                "refresh provenance {label} conflicts with ledger for {test_id}"
            ));
        }
    }
    Ok(())
}

#[test]
fn g05_refresh_provenance_rejects_conflicting_digest_alias_without_mutation() {
    let test_id = "functional/example";
    let previous_sha = "a".repeat(64);
    let active_sha = "b".repeat(64);
    let provenance = serde_json::json!({
        "test_id": test_id,
        "acceptance": {
            "expected_baseline_sha256": previous_sha,
            "accepted_baseline_sha256": active_sha,
            "candidate_sha256": "c".repeat(64),
        },
        "previous_baseline": {"sha256": previous_sha},
        "candidate": {"sha256": active_sha},
        "previous_archive_path": "archive.tui.pb",
        "acceptance_receipt_path": "accept.json",
        "provenance_path": "provenance.json",
    });
    let original = provenance.clone();
    let baseline = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/functional/baselines/functional/showcase_page_pickers.tui.pb");
    let baseline_sha = sha256_file(&baseline);

    assert!(
        validate_refresh_provenance(
            &provenance,
            test_id,
            &previous_sha,
            &active_sha,
            "archive.tui.pb",
            "accept.json",
            "provenance.json",
        )
        .is_err()
    );
    assert_eq!(provenance, original);
    assert_eq!(sha256_file(&baseline), baseline_sha);
}

#[test]
fn g05_refresh_provenance_rejects_ledger_path_mismatch_without_mutation() {
    let test_id = "functional/example";
    let previous_sha = "a".repeat(64);
    let active_sha = "b".repeat(64);
    let provenance = serde_json::json!({
        "test_id": test_id,
        "acceptance": {
            "expected_baseline_sha256": previous_sha,
            "accepted_baseline_sha256": active_sha,
        },
        "previous_baseline": {"sha256": previous_sha},
        "candidate": {"sha256": active_sha},
        "previous_archive_path": "wrong-archive.tui.pb",
        "acceptance_receipt_path": "accept.json",
        "provenance_path": "provenance.json",
        "history": {"superseded_baseline": "wrong-archive.tui.pb"},
    });
    let original = provenance.clone();
    let baseline = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/functional/baselines/functional/showcase_page_pickers.tui.pb");
    let baseline_sha = sha256_file(&baseline);

    assert!(
        validate_refresh_provenance(
            &provenance,
            test_id,
            &previous_sha,
            &active_sha,
            "archive.tui.pb",
            "accept.json",
            "provenance.json",
        )
        .is_err()
    );
    assert_eq!(provenance, original);
    assert_eq!(sha256_file(&baseline), baseline_sha);
}

fn disposition_sample_fixture(cols: u32, rows: u32) -> TerminalSnapshot {
    let cell = SnapshotCell::new(
        "x".to_string(),
        1,
        SnapshotColor::Default,
        SnapshotColor::Default,
        tui_test::terminal::cell::Attrs::empty(),
        SnapshotUnderline::None,
        SnapshotColor::Default,
        None,
        None,
    )
    .expect("fixture cell");
    let cell_count = usize::try_from(cols * rows).expect("fixture cell count fits usize");
    TerminalSnapshot::try_new(SnapshotParts {
        cols,
        rows,
        scope: SnapshotScope::Viewport,
        viewport_origin: 0,
        cells: vec![cell; cell_count],
        palette: [Rgb::new(0, 0, 0); 256],
        default_foreground: Rgb::new(255, 255, 255),
        default_background: Rgb::new(0, 0, 0),
        default_cursor: Rgb::new(255, 255, 255),
        cursor: SnapshotCursor::new(
            0,
            0,
            true,
            true,
            SnapshotCursorShape::Block,
            Rgb::new(255, 255, 255),
            None,
        ),
        title: None,
        metadata: SnapshotMetadata::default(),
    })
    .expect("fixture snapshot")
}

fn assert_superseded_json_dispositions(active_root: &Path) {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/testing/baseline-dispositions.json");
    let value: Value =
        serde_json::from_slice(&std::fs::read(&path).expect("dispositions")).expect("dispositions");
    let rows = value.as_array().expect("dispositions array");
    let expected = [
        (
            "functional/holla_resize_narrow",
            120_u64,
            40_u64,
            "functional/holla_browser_80x24",
            80_u64,
            24_u64,
        ),
        (
            "functional/tablepro_resize_60x20",
            120_u64,
            40_u64,
            "functional/tablepro_workbench_60x20",
            60_u64,
            20_u64,
        ),
        (
            "functional/showcase_help_resize",
            160_u64,
            50_u64,
            "functional/showcase_help_80x24",
            80_u64,
            24_u64,
        ),
    ];
    for (id, cols, rows_n, replacement, replacement_cols, replacement_rows) in expected {
        let row = rows
            .iter()
            .find(|row| row["id"] == id)
            .unwrap_or_else(|| panic!("missing disposition {id}"));
        assert_eq!(row["status"].as_str(), Some("superseded"), "{id}");
        assert_eq!(row["cols"].as_u64(), Some(cols), "{id}");
        assert_eq!(row["rows"].as_u64(), Some(rows_n), "{id}");
        assert_eq!(row["replacement_id"].as_str(), Some(replacement), "{id}");
        assert!(row.get("retired_id").is_none(), "{id}");
        assert!(row.get("commit").is_none(), "{id}");
        let reason = row["reason"].as_str().expect("reason");
        assert!(
            reason.contains(
                "The file's dimensions are not the dimensions named by the test, so it cannot be the narrow or named-size checkpoint."
            ),
            "{id}: {reason}"
        );
        let baseline =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(row["path"].as_str().expect("path"));
        assert!(
            !baseline.exists(),
            "superseded JSON sample remains: {}",
            baseline.display()
        );
        let replacement_row = rows
            .iter()
            .find(|row| row["id"] == replacement)
            .unwrap_or_else(|| panic!("missing replacement disposition {replacement}"));
        assert_eq!(
            replacement_row["status"].as_str(),
            Some("accepted"),
            "{replacement}"
        );
        assert_eq!(
            replacement_row["replaces"].as_str(),
            Some(id),
            "{replacement}"
        );
        let replacement_path = active_baseline_path(active_root, replacement);
        let replacement_sample = read_disposition_sample(&replacement_path);
        assert_eq!(replacement_sample.test_id(), replacement, "{replacement}");
        assert_eq!(
            replacement_sample.snapshot().cols(),
            u32::try_from(replacement_cols).expect("replacement columns fit u32"),
            "{replacement}"
        );
        assert_eq!(
            replacement_sample.snapshot().rows(),
            u32::try_from(replacement_rows).expect("replacement rows fit u32"),
            "{replacement}"
        );
    }
}

fn assert_no_legacy_json_samples(root: &Path) {
    if !root.exists() {
        return;
    }
    for entry in
        std::fs::read_dir(root).unwrap_or_else(|error| panic!("{}: {error}", root.display()))
    {
        let entry = entry.expect("read sample directory entry");
        let path = entry.path();
        let file_type = entry.file_type().expect("sample directory entry type");
        if file_type.is_dir() {
            assert_no_legacy_json_samples(&path);
        } else if file_type.is_file() {
            assert!(
                !path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().ends_with(".tui.json")),
                "legacy Tuiscotti JSON sample remains: {}",
                path.display()
            );
        }
    }
}

fn assert_no_legacy_sample_trees() {
    let workspace = crate::support::workspace_root();
    for relative in [
        "tests/functional/baselines",
        "docs/testing/acceptance-receipts/historical-baselines-json",
        "scripts/coverage-fixtures",
    ] {
        assert_no_legacy_json_samples(&workspace.join(relative));
    }
}

fn legacy_sample_json_hashes() -> std::collections::BTreeMap<PathBuf, String> {
    let workspace = crate::support::workspace_root();
    let manifest = workspace.join("docs/testing/legacy-sample-json-20261108.tsv");
    let contents = std::fs::read_to_string(&manifest)
        .unwrap_or_else(|error| panic!("{}: {error}", manifest.display()));
    let mut lines = contents.lines();
    assert_eq!(
        lines.next(),
        Some("source_path\tsha256\tdisposition\treplacement_pb_path\tprovenance")
    );
    let mut samples = std::collections::BTreeMap::new();
    let mut dispositions = std::collections::BTreeMap::<String, usize>::new();
    for line in lines {
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 5, "malformed legacy sample receipt: {line}");
        let [source, digest, disposition, replacement, provenance] = fields.as_slice() else {
            unreachable!("five fields checked above")
        };
        let source = PathBuf::from(source);
        assert!(source.to_string_lossy().ends_with(".tui.json"));
        assert!(
            !workspace.join(&source).exists(),
            "legacy sample remains: {}",
            source.display()
        );
        assert_eq!(
            digest.len(),
            64,
            "legacy sample digest for {}",
            source.display()
        );
        assert!(
            digest.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "legacy sample digest for {}",
            source.display()
        );
        assert!(
            !provenance.is_empty(),
            "legacy sample provenance for {}",
            source.display()
        );
        if !replacement.is_empty() {
            let replacement = workspace.join(replacement);
            assert!(
                replacement.is_file(),
                "missing PB replacement: {}",
                replacement.display()
            );
            if *disposition != "test-fixture" {
                assert!(
                    std::fs::read(&replacement)
                        .expect("replacement PB")
                        .starts_with(b"TUISCPB\0"),
                    "replacement is not a PB container: {}",
                    replacement.display()
                );
            }
        }
        assert!(
            samples
                .insert(source.clone(), (*digest).to_owned())
                .is_none(),
            "duplicate legacy sample receipt: {}",
            source.display()
        );
        *dispositions.entry((*disposition).to_owned()).or_default() += 1;
    }
    assert_eq!(samples.len(), 1890, "legacy sample hash receipt count");
    assert_eq!(dispositions.get("same-id"), Some(&1868));
    assert_eq!(dispositions.get("superseded"), Some(&8));
    assert_eq!(dispositions.get("archived-copy"), Some(&12));
    assert_eq!(dispositions.get("test-fixture"), Some(&2));
    samples
}

fn assert_historical_json_hash_receipts(dispositions: &[Value], baseline_root: &Path) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("docs/testing/baseline-archives-20261108.tsv");
    let contents = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let mut lines = contents.lines();
    assert_eq!(
        lines.next(),
        Some(
            "test_id\tactive_path_before\thistorical_path_after\tsha256\tdisposition\treplacement_pointer"
        )
    );
    let mut seen = std::collections::BTreeSet::new();
    for line in lines {
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 6, "malformed archive receipt row: {line}");
        let [id, _active_path, historical_path, sha, status, replacement] = fields.as_slice()
        else {
            unreachable!("six fields checked above")
        };
        assert!(
            seen.insert(*id),
            "duplicate historical JSON receipt for {id}"
        );
        assert_eq!(*status, "superseded", "{id}");
        assert_eq!(sha.len(), 64, "historical JSON SHA256 for {id}");
        assert!(sha.bytes().all(|byte| byte.is_ascii_hexdigit()), "{id}");

        let source_relative = PathBuf::from("tests/functional/baselines").join(historical_path);
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(&source_relative);
        assert!(
            !source.exists(),
            "legacy sample remains for {id}: {}",
            source.display()
        );
        let disposition = dispositions
            .iter()
            .find(|row| row["id"] == *id)
            .unwrap_or_else(|| panic!("missing disposition for {id}"));
        assert_eq!(disposition["status"].as_str(), Some("superseded"), "{id}");
        assert_eq!(
            disposition["historical_json_sha256"].as_str(),
            Some(*sha),
            "{id}"
        );
        assert_eq!(
            Path::new(disposition["path"].as_str().expect("historical path")),
            source_relative,
            "{id}"
        );

        if !replacement.starts_with("none ") && *replacement != "none" {
            let replacement_path = active_baseline_path(baseline_root, replacement);
            let sample = read_disposition_sample(&replacement_path);
            assert_eq!(sample.test_id(), *replacement, "{id} replacement");
        }
    }
    assert_eq!(
        seen.len(),
        8,
        "expected all eight superseded historical JSON samples"
    );
}

#[test]
fn g05_approved_binary_manifest_matches_active_samples() {
    let root = baseline_root();
    assert_no_legacy_sample_trees();
    let legacy_json_hashes = legacy_sample_json_hashes();
    let dispositions_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/testing/baseline-dispositions.json");
    let dispositions: Value =
        serde_json::from_slice(&std::fs::read(&dispositions_path).expect("baseline dispositions"))
            .expect("baseline dispositions JSON");
    assert_historical_json_hash_receipts(
        dispositions.as_array().expect("dispositions array"),
        &root,
    );
    let manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("docs/testing/binary-baselines-20261108.tsv");
    let manifest = std::fs::read_to_string(&manifest_path).expect("binary baseline manifest");
    let mut lines = manifest.lines();
    assert_eq!(
        lines.next(),
        Some("test_id\tbaseline_path\tsha256\treceipt_path\tacceptance_sequence\tsource_lineage")
    );
    let mut sample_ids: Vec<(PathBuf, String)> = Vec::new();
    let mut ids = std::collections::BTreeSet::new();
    let mut paths = std::collections::BTreeSet::new();
    let mut active_records = std::collections::BTreeMap::new();
    let mut count = 0;
    for line in lines {
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 6, "{line}");
        let [
            test_id,
            baseline_path,
            expected_sha,
            receipt_path,
            sequence,
            source_lineage,
        ] = fields.as_slice()
        else {
            unreachable!("field count checked")
        };
        assert!(ids.insert(*test_id), "duplicate test ID {test_id}");
        assert!(
            !source_lineage.is_empty(),
            "missing source lineage for {test_id}"
        );
        assert!(
            sequence.parse::<u16>().is_ok(),
            "bad acceptance sequence {sequence}"
        );
        assert!(
            baseline_path.ends_with(".tui.pb"),
            "not a PB path: {baseline_path}"
        );
        assert!(baseline_path.starts_with("tests/functional/baselines/"));
        let relative = Path::new(baseline_path)
            .strip_prefix("tests/functional/baselines")
            .expect("baseline-relative path");
        let path = root.join(relative);
        assert!(
            paths.insert(relative.to_path_buf()),
            "duplicate path {baseline_path}"
        );
        assert!(
            !root.join(format!("{test_id}.tui.json")).exists(),
            "active JSON remains for {test_id}"
        );
        let bytes =
            std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert!(
            bytes.starts_with(b"TUISCPB\0"),
            "{} is not a protobuf container",
            path.display()
        );
        assert_eq!(sha256_file(&path), *expected_sha, "{test_id}");
        sample_ids.push((path.clone(), (*test_id).to_owned()));
        let receipt_relative_path = *receipt_path;
        let receipt_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(receipt_relative_path);
        let receipt: Value = serde_json::from_slice(
            &std::fs::read(&receipt_path)
                .unwrap_or_else(|error| panic!("{}: {error}", receipt_path.display())),
        )
        .expect("binary acceptance receipt");
        assert_eq!(
            receipt["candidate_sha256"].as_str(),
            Some(*expected_sha),
            "{test_id}"
        );
        active_records.insert(
            (*test_id).to_owned(),
            (
                (*baseline_path).to_owned(),
                (*expected_sha).to_owned(),
                receipt_relative_path.to_owned(),
            ),
        );
        count += 1;
    }
    assert_eq!(count, 140);
    assert!(!ids.contains("functional/tablepro_query_syntax_error"));
    let historical_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("docs/testing/acceptance-receipts/historical-binary-20261108/transactions.jsonl");
    let historical = std::fs::read_to_string(&historical_path)
        .unwrap_or_else(|error| panic!("{}: {error}", historical_path.display()));
    let refresh_manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("docs/testing/baseline-refreshes-20261108.tsv");
    let refresh_manifest = std::fs::read_to_string(&refresh_manifest_path)
        .unwrap_or_else(|error| panic!("{}: {error}", refresh_manifest_path.display()));
    let mut refresh_lines = refresh_manifest.lines();
    assert_eq!(
        refresh_lines.next(),
        Some(
            "test_id\tactive_baseline_path\tactive_sha256\tprevious_sha256\tprevious_archive_path\tacceptance_receipt\tprovenance_path"
        )
    );
    let mut refreshes: std::collections::BTreeMap<String, Vec<Vec<String>>> =
        std::collections::BTreeMap::new();
    for line in refresh_lines {
        let fields: Vec<String> = line.split('\t').map(str::to_owned).collect();
        assert_eq!(fields.len(), 7, "{line}");
        refreshes
            .entry(fields[0].clone())
            .or_default()
            .push(fields[1..].to_vec());
    }
    let mut inherited_approved = std::collections::BTreeMap::new();
    let inherited_early = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("docs/testing/acceptance-receipts/inherited-main-20261108/baselines.tsv"),
    )
    .expect("inherited main baseline manifest");
    for line in inherited_early.lines().skip(1) {
        let fields: Vec<_> = line.split('\t').collect();
        let (Some(test_id), Some(baseline_path), Some(approved)) =
            (fields.first(), fields.get(1), fields.get(2))
        else {
            continue;
        };
        let Some(chain) = refreshes.get(*test_id) else {
            continue;
        };
        let last = chain.last().expect("nonempty refresh chain");
        active_records.insert(
            (*test_id).to_owned(),
            (
                (*baseline_path).to_owned(),
                last[1].clone(),
                last[4].clone(),
            ),
        );
        inherited_approved.insert((*test_id).to_owned(), (*approved).to_owned());
    }
    let mut refreshed_ids = std::collections::BTreeSet::new();
    for (test_id, chain) in &refreshes {
        let Some((active_path, active_sha, active_receipt)) = active_records.get(test_id) else {
            continue;
        };
        if let Some(approved) = inherited_approved.get(test_id) {
            assert_eq!(
                &chain[0][2], approved,
                "refresh chain starts from the inherited approval {test_id}"
            );
        }
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        if ids.contains(test_id.as_str()) {
            // The manifest row carries the latest digest, so nothing else ties the first
            // refresh to the approval it replaced. An original approval receipt of this id
            // outside the refresh chain does.
            assert!(
                original_approval_receipt_digests(test_id).contains(&chain[0][2]),
                "first refresh of {test_id} replaces no original approval receipt"
            );
        }
        let active_path = repo_root.join(active_path);
        let mut prior_active_sha = None;
        for fields in chain {
            let [
                manifest_path,
                manifest_sha,
                previous_sha,
                archive_path,
                receipt_path,
                provenance_path,
            ] = fields.as_slice()
            else {
                unreachable!("refresh field count checked")
            };
            assert_eq!(
                manifest_path.as_str(),
                active_records[test_id].0.as_str(),
                "active refresh path {test_id}"
            );
            if let Some(expected_previous) = prior_active_sha {
                assert_eq!(
                    previous_sha, expected_previous,
                    "broken refresh chain {test_id}"
                );
            }
            let archive = repo_root.join(archive_path);
            assert_eq!(
                sha256_file(&archive),
                previous_sha.as_str(),
                "archived sample {test_id}"
            );
            let receipt: Value = serde_json::from_slice(
                &std::fs::read(repo_root.join(receipt_path)).expect("active refresh receipt"),
            )
            .expect("parse active refresh receipt");
            assert_eq!(receipt["command"].as_str(), Some("accept"), "{test_id}");
            assert_eq!(receipt["status"].as_str(), Some("pass"), "{test_id}");
            assert_eq!(
                receipt["test_id"].as_str(),
                Some(test_id.as_str()),
                "{test_id}"
            );
            assert_eq!(
                receipt["candidate_sha256"].as_str(),
                Some(manifest_sha.as_str()),
                "{test_id}"
            );
            assert_eq!(
                receipt["baseline_sha256"].as_str(),
                Some(manifest_sha.as_str()),
                "{test_id}"
            );
            let provenance: Value = serde_json::from_slice(
                &std::fs::read(repo_root.join(provenance_path)).expect("active refresh provenance"),
            )
            .expect("parse active refresh provenance");
            validate_refresh_provenance(
                &provenance,
                test_id,
                previous_sha,
                manifest_sha,
                archive_path,
                receipt_path,
                provenance_path,
            )
            .unwrap_or_else(|error| panic!("{error}"));
            prior_active_sha = Some(manifest_sha.as_str());
        }
        let last = chain.last().expect("nonempty refresh chain");
        let [_, last_sha, _, _, last_receipt, _] = last.as_slice() else {
            unreachable!("refresh field count checked")
        };
        assert_eq!(last_sha, active_sha, "active refresh SHA {test_id}");
        assert_eq!(
            last_receipt, active_receipt,
            "active refresh receipt {test_id}"
        );
        assert_eq!(
            sha256_file(&active_path),
            active_sha.as_str(),
            "active sample {test_id}"
        );
        assert!(
            refreshed_ids.insert(test_id.clone()),
            "duplicate refresh chain {test_id}"
        );
    }
    let mut historical_ids = std::collections::BTreeSet::new();
    let mut historical_paths = std::collections::BTreeSet::new();
    let mut historical_count = 0;
    for line in historical.lines() {
        let receipt: Value = serde_json::from_str(line).expect("historical transaction receipt");
        assert_eq!(
            receipt["schema"].as_str(),
            Some("tcb.historical-binary-transaction.v1")
        );
        let test_id = receipt["test_id"].as_str().expect("historical test ID");
        assert!(
            historical_ids.insert(test_id.to_string()),
            "duplicate historical test ID {test_id}"
        );
        assert!(
            !ids.contains(test_id),
            "historical ID collides with active ID {test_id}"
        );
        let baseline_path = receipt["baseline_path"].as_str().expect("baseline path");
        assert!(baseline_path.starts_with("tests/functional/baselines/"));
        assert!(baseline_path.ends_with(".tui.pb"));
        let relative = Path::new(baseline_path)
            .strip_prefix("tests/functional/baselines")
            .expect("historical baseline is under baseline root");
        assert!(
            historical_paths.insert(relative.to_path_buf()),
            "duplicate historical path {baseline_path}"
        );
        let path = root.join(relative);
        let expected_sha = receipt["baseline_sha256"].as_str().expect("baseline SHA");
        let sample_path = if sha256_file(&path) == expected_sha {
            path
        } else {
            let chain = refreshes.get(test_id).unwrap_or_else(|| {
                panic!("historical sample changed without an explicit refresh record: {test_id}")
            });
            let first = chain.first().expect("nonempty refresh chain");
            let last = chain.last().expect("nonempty refresh chain");
            let [active_baseline, _, previous_sha, archive_path, _, _] = first.as_slice() else {
                unreachable!("refresh field count checked")
            };
            let [_, active_sha, _, _, _, _] = last.as_slice() else {
                unreachable!("refresh field count checked")
            };
            assert_eq!(active_baseline, baseline_path, "{test_id}");
            assert_eq!(previous_sha, expected_sha, "{test_id}");
            assert_eq!(
                sha256_file(&path),
                active_sha.as_str(),
                "active refreshed sample {test_id}"
            );
            let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let archive = repo_root.join(archive_path);
            assert_eq!(
                sha256_file(&archive),
                expected_sha,
                "archived historical sample {test_id}"
            );
            assert!(
                refreshed_ids.insert(test_id.to_string()),
                "duplicate refresh use {test_id}"
            );
            archive
        };
        assert!(
            std::fs::read(&sample_path)
                .expect("historical sample")
                .starts_with(b"TUISCPB\0")
        );
        sample_ids.push((sample_path, test_id.to_owned()));

        let historical_json_relative = PathBuf::from(
            receipt["historical_json"]
                .as_str()
                .expect("historical JSON source path"),
        );
        assert!(
            historical_json_relative
                .to_string_lossy()
                .ends_with(".tui.json"),
            "historical source is not a Tuiscotti sample path for {test_id}"
        );
        let historical_json =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(&historical_json_relative);
        assert!(
            !historical_json.exists(),
            "superseded JSON sample remains for {test_id}: {}",
            historical_json.display()
        );
        let historical_sha = receipt["historical_json_sha256"]
            .as_str()
            .expect("historical JSON sha256 receipt");
        assert_eq!(
            legacy_json_hashes
                .get(&historical_json_relative)
                .map(String::as_str),
            Some(historical_sha),
            "historical JSON hash receipt for {test_id}"
        );
        assert_eq!(
            historical_sha.len(),
            64,
            "historical JSON digest for {test_id}"
        );
        assert!(
            historical_sha.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "historical JSON digest is malformed for {test_id}"
        );
        let expected_path = historical_json
            .strip_prefix(
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/functional/baselines"),
            )
            .expect("historical JSON source is under baseline root")
            .with_extension("pb");
        assert_eq!(
            relative, expected_path,
            "history-derived path for {test_id}"
        );
        assert!(receipt["path_id"].as_str().is_some());

        let candidate = &receipt["candidate_record"];
        assert_eq!(candidate["test_id"].as_str(), Some(test_id));
        assert_eq!(candidate["candidate_sha256"].as_str(), Some(expected_sha));
        assert!(
            candidate["source"]
                .as_str()
                .is_some_and(|source| !source.is_empty())
        );
        let alternates = candidate["alternates"]
            .as_array()
            .expect("candidate alternatives");
        assert!(
            alternates.is_empty()
                || alternates.iter().any(|alternate| {
                    alternate.as_str() == Some(expected_sha)
                        || alternate["sha256"].as_str() == Some(expected_sha)
                }),
            "selected candidate absent from alternatives for {test_id}"
        );

        let acceptance = &receipt["acceptance"];
        assert_eq!(acceptance["exit_code"].as_i64(), Some(0), "{test_id}");
        assert_eq!(acceptance["cli_version"].as_str(), Some("tuiscotti 0.4.0"));
        assert_eq!(acceptance["cli_sha256"].as_str().map(str::len), Some(64));
        assert_eq!(
            acceptance["staging_baseline_sha256"].as_str(),
            Some(expected_sha)
        );
        let response = &acceptance["stdout"];
        assert_eq!(response["schema"].as_str(), Some("tuiscotti.cli.v1"));
        assert_eq!(response["command"].as_str(), Some("accept"));
        assert_eq!(response["status"].as_str(), Some("pass"));
        assert_eq!(response["candidate_sha256"].as_str(), Some(expected_sha));
        assert_eq!(response["baseline_sha256"].as_str(), Some(expected_sha));
        let raw_response = acceptance["stdout_raw"].as_str().expect("raw CLI result");
        assert_eq!(
            serde_json::from_str::<Value>(raw_response).expect("raw CLI JSON"),
            *response
        );
        let command = acceptance["command"].as_array().expect("accept command");
        assert!(command.iter().any(|part| part == "--expect-baseline"));
        assert!(command.iter().any(|part| part == "missing"));
        historical_count += 1;
    }
    assert_eq!(historical_count, 1868);
    assert_sample_ids(sample_ids);
    assert_eq!(
        refreshed_ids.len(),
        refreshes.len(),
        "unused baseline refresh record"
    );
    fn collect_pb_paths(root: &Path, dir: &Path, paths: &mut std::collections::BTreeSet<PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("read active baseline directory") {
            let path = entry.expect("baseline entry").path();
            if path.is_dir() {
                collect_pb_paths(root, &path, paths);
            } else if path.extension().is_some_and(|extension| extension == "pb") {
                paths.insert(
                    path.strip_prefix(root)
                        .expect("path is inside baseline root")
                        .to_path_buf(),
                );
            }
        }
    }
    let mut actual_paths = std::collections::BTreeSet::new();
    collect_pb_paths(&root, &root, &mut actual_paths);
    let inherited_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("docs/testing/acceptance-receipts/inherited-main-20261108");
    let inherited_manifest_path = inherited_root.join("baselines.tsv");
    let inherited_manifest = std::fs::read_to_string(&inherited_manifest_path)
        .unwrap_or_else(|error| panic!("{}: {error}", inherited_manifest_path.display()));
    let mut inherited_lines = inherited_manifest.lines();
    assert_eq!(
        inherited_lines.next(),
        Some("test_id\tbaseline_path\tsha256\tsource_commit\tapproval_receipts")
    );
    let inherited_receipts_path = inherited_root.join("receipts.jsonl");
    let inherited_receipts_text = std::fs::read_to_string(&inherited_receipts_path)
        .unwrap_or_else(|error| panic!("{}: {error}", inherited_receipts_path.display()));
    let mut inherited_receipts = std::collections::BTreeMap::new();
    for line in inherited_receipts_text.lines() {
        let receipt: Value = serde_json::from_str(line).expect("inherited main approval receipt");
        let test_id = receipt["test_id"]
            .as_str()
            .expect("inherited main test ID")
            .to_owned();
        assert!(
            inherited_receipts
                .insert(test_id.clone(), receipt)
                .is_none(),
            "duplicate inherited receipt {test_id}"
        );
    }
    let mut inherited_paths = std::collections::BTreeSet::new();
    for line in inherited_lines {
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 5, "{line}");
        let [
            test_id,
            baseline_path,
            expected_sha,
            source_commit,
            receipts_path,
        ] = fields.as_slice()
        else {
            unreachable!("field count checked")
        };
        assert_eq!(
            *source_commit, "1d7f63dbb13ecb2bf3e3295e7a296dc2080a62b1",
            "inherited main source for {test_id}"
        );
        assert_eq!(
            *receipts_path,
            "docs/testing/acceptance-receipts/inherited-main-20261108/receipts.jsonl"
        );
        let relative = Path::new(baseline_path)
            .strip_prefix("tests/functional/baselines/")
            .expect("inherited baseline path under root");
        assert!(
            inherited_paths.insert(relative.to_path_buf()),
            "duplicate inherited path {baseline_path}"
        );
        let path = root.join(relative);
        let active_sha = inherited_approved.get(*test_id).map_or(*expected_sha, |_| {
            refreshes[*test_id].last().expect("chain")[1].as_str()
        });
        assert_eq!(sha256_file(&path), active_sha, "{test_id}");
        let receipt = inherited_receipts
            .get(*test_id)
            .unwrap_or_else(|| panic!("missing inherited main approval receipt for {test_id}"));
        assert_eq!(receipt["schema"].as_str(), Some("tuiscotti.cli.v1"));
        assert_eq!(receipt["command"].as_str(), Some("accept"));
        assert_eq!(receipt["status"].as_str(), Some("pass"));
        assert_eq!(receipt["candidate_sha256"].as_str(), Some(*expected_sha));
        assert_eq!(receipt["baseline_sha256"].as_str(), Some(*expected_sha));
        assert_eq!(receipt["test_id"].as_str(), Some(*test_id));
    }
    assert_eq!(inherited_paths.len(), 17, "inherited main baseline count");
    assert_eq!(inherited_receipts.len(), inherited_paths.len());
    paths.extend(inherited_paths);
    paths.extend(historical_paths);
    assert_eq!(actual_paths, paths);
}

#[test]
fn g05_disposition_sample_reader_accepts_pb_path() {
    let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/functional")
        .join(format!(
            "g05-disposition-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
    std::fs::create_dir_all(&fixture_root).expect("create temporary fixture root");
    let replacements = [
        ("functional/holla_browser_80x24", 80, 24),
        ("functional/tablepro_workbench_60x20", 60, 20),
        ("functional/showcase_help_80x24", 80, 24),
    ];
    for (test_id, cols, rows) in replacements {
        let sample = tuiscotti_latest::sample::Sample::state(
            test_id,
            disposition_sample_fixture(cols, rows),
            tuiscotti_latest::sample::Observation::empty(),
        )
        .expect("build binary sample fixture");
        let bytes = sample.to_bytes().expect("serialize binary sample fixture");
        assert_eq!(&bytes[..8], b"TUISCPB\0", "binary container magic");
        let fixture = active_baseline_path(&fixture_root, test_id);
        std::fs::create_dir_all(fixture.parent().expect("fixture parent"))
            .expect("create temporary baseline directory");
        std::fs::write(&fixture, bytes).expect("write temporary binary fixture");
    }
    assert_superseded_json_dispositions(&fixture_root);

    std::fs::remove_dir_all(fixture_root).expect("remove temporary fixture root");
}

fn assert_acceptance_receipts() {
    let manifest = manifest();
    let cases = [
        (
            "functional_holla_browser_80x24",
            "holla",
            "functional/holla_browser_80x24",
        ),
        (
            "functional_tablepro_workbench_60x20",
            "tablepro",
            "functional/tablepro_workbench_60x20",
        ),
        (
            "functional_showcase_help_80x24",
            "showcase",
            "functional/showcase_help_80x24",
        ),
        (
            "functional_showcase_keyboard_run_task",
            "showcase",
            "functional/showcase_keyboard_run_task",
        ),
        (
            "functional_tablepro_query_recovered",
            "tablepro",
            "functional/tablepro_query_recovered",
        ),
        (
            "functional_jackin_stop_instance_dialog",
            "jackin",
            "functional/jackin_stop_instance_dialog",
        ),
        (
            "functional_jackin_settings_dirty_leave",
            "jackin",
            "functional/jackin_settings_dirty_leave",
        ),
        (
            "functional_jackin_launch_failed",
            "jackin",
            "functional/jackin_launch_failed",
        ),
        (
            "functional_jackin_launch_cockpit",
            "jackin",
            "functional/jackin_launch_cockpit",
        ),
        (
            "functional_jackin_launch_capsule",
            "jackin",
            "functional/jackin_launch_capsule",
        ),
        (
            "functional_jackin_launch_detached",
            "jackin",
            "functional/jackin_launch_detached",
        ),
        (
            "functional_jackin_launch_reconnected",
            "jackin",
            "functional/jackin_launch_reconnected",
        ),
    ];
    for (file, binary, baseline_id) in cases {
        let receipt_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("docs/testing/acceptance-receipts/protobuf-20261107")
            .join(format!("{file}.json"));
        let receipt: Value =
            serde_json::from_slice(&std::fs::read(&receipt_path).expect("receipt"))
                .expect("receipt json");
        assert_eq!(
            receipt["source_sha"].as_str(),
            Some(REFERENCE_SHA),
            "{file}"
        );
        assert_eq!(
            receipt["lockfile_sha256"].as_str(),
            Some(ACCEPTANCE_LOCKFILE_SHA256),
            "{file}"
        );
        assert_eq!(
            receipt["comparator_rev"].as_str(),
            Some(ACCEPTANCE_COMPARATOR_REV),
            "{file}"
        );
        let expected_previous = match file {
            "functional_tablepro_query_recovered" => {
                "ed9b89699ee57340d92bc47bd37169b523bd39c321cda73d1b55bccf07543c0b"
            }
            "functional_jackin_launch_failed" => {
                "2b2c711b926a175a2bde8fa73921fbc4249828b5d2d66bfd9b71af0a829b67f2"
            }
            _ => "missing",
        };
        assert_eq!(
            receipt["expect_baseline"].as_str(),
            Some(expected_previous),
            "{file}"
        );
        let precondition = receipt["precondition"].as_str().unwrap_or("");
        assert!(!precondition.is_empty(), "{file}");
        let expected_sha = manifest["binaries"][binary]["sha256"]
            .as_str()
            .unwrap_or_else(|| panic!("missing binary {binary}"));
        assert_eq!(
            receipt["binary_sha256"].as_str(),
            Some(expected_sha),
            "{file}"
        );
        let baseline = baseline_root().join(format!("{baseline_id}.tui.pb"));
        let accepted =
            first_refresh_previous_sha(baseline_id).unwrap_or_else(|| sha256_file(&baseline));
        assert_eq!(
            receipt["candidate_sha256"].as_str(),
            Some(accepted.as_str()),
            "{file}"
        );
    }
}

fn original_approval_receipt_digests(test_id: &str) -> Vec<String> {
    let safe = test_id.replace('/', "_");
    let receipts =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/testing/acceptance-receipts");
    let mut digests = Vec::new();
    for dir in std::fs::read_dir(&receipts).expect("acceptance receipts dir") {
        let dir = dir.expect("receipt dir entry").path();
        let name = dir.file_name().and_then(|name| name.to_str()).unwrap_or("");
        if !dir.is_dir() || name.starts_with("refreshes-") || name.starts_with("flash-") {
            continue;
        }
        for file in [format!("{safe}.json"), format!("{safe}.accept.json")] {
            let Ok(bytes) = std::fs::read(dir.join(&file)) else {
                continue;
            };
            let receipt: Value = serde_json::from_slice(&bytes).expect("original approval receipt");
            if let Some(digest) = receipt["candidate_sha256"].as_str() {
                digests.push(digest.to_owned());
            }
        }
    }
    digests
}

fn first_refresh_previous_sha(test_id: &str) -> Option<String> {
    let manifest = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("docs/testing/baseline-refreshes-20261108.tsv"),
    )
    .expect("baseline refresh manifest");
    manifest
        .lines()
        .skip(1)
        .map(|line| line.split('\t').collect::<Vec<_>>())
        .find(|fields| fields.first() == Some(&test_id))
        .map(|fields| fields[3].to_owned())
}

fn refuse_candidate(bin: &Path) {
    let mut launch = Launch::new(
        "g05-candidate-refuses",
        bin.to_str().expect("reference path"),
    );
    launch.mode = SessionMode::Candidate;
    let Err(error) = try_launch(launch) else {
        panic!("candidate must refuse the reference binary");
    };
    let rendered = error.to_string();
    assert!(rendered.contains("reference binary"), "{rendered}");
    assert!(
        error.message.contains("reference binary"),
        "{}",
        error.message
    );
    let saved = std::fs::read_to_string(error.evidence.join("spawn-error.txt")).expect("error");
    assert!(saved.contains("reference binary"), "{saved}");
    assert!(!error.evidence.join("session.json").exists());
}

#[test]
fn g05_candidate_refuses_reference_binary() {
    let bin = reference_showcase();
    refuse_candidate(&bin);
    let value = manifest();
    let platforms = value["binaries"]["showcase"]["platforms"]
        .as_object()
        .expect("showcase platforms");
    for (target, platform) in platforms {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            platform["path"]
                .as_str()
                .unwrap_or_else(|| panic!("path for {target}")),
        );
        assert!(
            path.is_file(),
            "missing {target} showcase {}",
            path.display()
        );
        assert_eq!(
            sha256_file(&path),
            platform["sha256"]
                .as_str()
                .unwrap_or_else(|| panic!("sha256 for {target}"))
        );
        if path.canonicalize().expect("platform binary") == bin.canonicalize().expect("host") {
            continue;
        }
        refuse_candidate(&path);
    }
}

#[test]
fn g05_candidate_cannot_select_reference_manifest_override() {
    let mut launch = Launch::new("g05-candidate-reference-manifest", showcase());
    launch.reference_manifest = Some(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("docs/testing/reference-manifest-j06-pr2.json"),
    );
    let Err(error) = try_launch(launch) else {
        panic!("candidate mode must not select an alternate reference manifest");
    };
    assert!(
        error
            .message
            .contains("candidate mode cannot select a reference manifest"),
        "{}",
        error.message
    );
    assert!(!error.evidence.join("session.json").exists());
}

#[test]
fn g05_reference_mode_requires_manifest_identity() {
    let candidate = Path::new(showcase());
    assert!(candidate.is_file(), "missing candidate showcase");
    assert_ne!(sha256_file(candidate), SHOWCASE_REFERENCE_SHA256);
    let mut launch = Launch::new("g05-reference-identity", showcase());
    launch.mode = SessionMode::Reference;
    let Err(error) = try_launch(launch) else {
        panic!("candidate digest is not the reference identity");
    };
    let rendered = error.to_string();
    assert!(rendered.contains("reference identity"), "{rendered}");
    assert!(
        error.message.contains("reference identity"),
        "{}",
        error.message
    );
    assert!(!error.evidence.join("session.json").exists());
}

#[test]
fn g05_reference_session_receipt_records_identity() {
    let bin = reference_showcase();
    let manifest = manifest();
    assert_eq!(manifest["reference_sha"], REFERENCE_SHA);
    assert_eq!(
        manifest["binaries"]["showcase"]["sha256"],
        SHOWCASE_REFERENCE_SHA256
    );
    assert_eq!(manifest["comparator"]["rev"], COMPARATOR_REV);
    assert_eq!(manifest["acceptance"]["session_file"], "session.json");
    let mut launch = Launch::new(
        "g05-reference-session",
        bin.to_str().expect("reference path"),
    );
    launch.mode = SessionMode::Reference;
    launch.args = vec![
        "--page".to_string(),
        "overview".to_string(),
        "--frame".to_string(),
        "40".to_string(),
    ];
    launch.motion = Some("paused".to_string());
    launch.color = Some("truecolor".to_string());
    launch.cols = 120;
    launch.rows = 40;
    launch.env.push((
        "TUISCOTTI_CAPTURE_MARKER".to_string(),
        "recorded".to_string(),
    ));
    let app = try_launch(launch).expect("launch reference showcase");
    let screen = app.wait_screen("reference showcase painted", |screen| {
        text(screen).chars().any(|ch| ch.is_ascii_alphanumeric())
    });
    assert!(text(&screen).chars().any(|ch| ch.is_ascii_alphanumeric()));
    let evidence = app.evidence_dir();
    let baselines = baseline_root();
    assert!(
        evidence.starts_with(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/functional"))
    );
    assert!(!evidence.starts_with(&baselines));
    let session: Value = serde_json::from_slice(
        &std::fs::read(evidence.join("session.json")).expect("session.json"),
    )
    .expect("session json");
    let mut keys: Vec<_> = session
        .as_object()
        .expect("session object")
        .keys()
        .cloned()
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "TERM",
            "args",
            "binary_path",
            "binary_sha256",
            "clock",
            "color",
            "cols",
            "comparator_name",
            "comparator_rev",
            "env",
            "history",
            "lockfile_sha256",
            "mode",
            "motion",
            "platform",
            "reference_sha",
            "rows",
        ]
    );
    assert_eq!(session["mode"].as_str(), Some("reference"));
    assert_eq!(session["reference_sha"], manifest["reference_sha"]);
    assert_eq!(session["reference_sha"].as_str(), Some(REFERENCE_SHA));
    assert_eq!(session["lockfile_sha256"], manifest["lockfile_sha256"]);
    assert_eq!(
        session["binary_sha256"].as_str(),
        Some(reference_digest("showcase").as_str())
    );
    let recorded = PathBuf::from(session["binary_path"].as_str().expect("binary path"));
    assert_eq!(
        recorded.canonicalize().expect("binary"),
        bin.canonicalize().expect("bin")
    );
    assert_eq!(
        session["args"],
        serde_json::json!(["--page", "overview", "--frame", "40"])
    );
    assert_eq!(session["cols"].as_u64(), Some(120));
    assert_eq!(session["rows"].as_u64(), Some(40));
    assert_eq!(session["color"].as_str(), Some("truecolor"));
    assert_eq!(session["platform"].as_str(), Some("macos"));
    assert_eq!(session["history"].as_bool(), Some(false));
    assert_eq!(session["motion"].as_str(), Some("paused"));
    assert_eq!(session["clock"].as_bool(), Some(false));
    assert_eq!(session["TERM"].as_str(), Some("xterm-256color"));
    assert_eq!(session["env"]["COLORTERM"].as_str(), Some("truecolor"));
    assert_eq!(session["env"]["LANG"].as_str(), Some("C.UTF-8"));
    assert_eq!(session["env"]["LC_ALL"].as_str(), Some("C.UTF-8"));
    assert_eq!(
        session["env"]["TUISCOTTI_CAPTURE_MARKER"].as_str(),
        Some("recorded")
    );
    assert_eq!(session["comparator_name"], manifest["comparator"]["name"]);
    assert_eq!(session["comparator_rev"].as_str(), Some(COMPARATOR_REV));
    app.finish();
}

#[test]
fn g05_acceptance_receipts_match_reference_baselines() {
    assert_acceptance_receipts();
}

#[test]
fn g05_check_visual_does_not_write_baselines() {
    assert_superseded_json_dispositions(&baseline_root());
    let root = baseline_root();
    let before = tree_digest(&root);
    let app = launch_showcase("g05-check-visual");
    app.wait_text("120×40");
    let session: Value = serde_json::from_slice(
        &std::fs::read(app.evidence_dir().join("session.json")).expect("session.json"),
    )
    .expect("session json");
    assert_eq!(session["mode"].as_str(), Some("candidate"));
    let sha = session["binary_sha256"].as_str().expect("sha");
    assert_ne!(sha, SHOWCASE_REFERENCE_SHA256);
    let manifest = manifest();
    for name in ["showcase", "tablepro", "holla", "jackin"] {
        assert_ne!(sha, manifest["binaries"][name]["sha256"].as_str().unwrap());
        let platforms = manifest["binaries"][name]["platforms"]
            .as_object()
            .unwrap_or_else(|| panic!("platforms for {name}"));
        for (target, platform) in platforms {
            assert_ne!(
                sha,
                platform["sha256"]
                    .as_str()
                    .unwrap_or_else(|| panic!("{name} {target}")),
                "{name} {target}"
            );
        }
    }
    assert!(!app.evidence_dir().starts_with(&root));
    let missing = root.join("functional/g05_read_only.tui.pb");
    assert!(!missing.exists());
    let outcome =
        app.check_visual_with_precondition(&root, "functional/g05_read_only", "screen is visible");
    assert!(
        matches!(outcome, tuiscotti_latest::check::CheckOutcome::Failure(_)),
        "missing baseline must not pass: {outcome:?}"
    );
    let _ = app.check_visual(&root, "functional/g05_read_only_empty");
    let actions = std::fs::read_to_string(app.evidence_dir().join("actions.txt")).expect("actions");
    let stated = line_index(&actions, "precondition screen is visible");
    let compared = prefix_index(&actions, "visual functional/g05_read_only candidate ");
    assert!(stated < compared, "{actions}");
    let empty = line_index(&actions, "precondition ");
    let empty_compared = prefix_index(&actions, "visual functional/g05_read_only_empty candidate ");
    assert!(empty < empty_compared, "{actions}");
    assert!(!missing.exists());
    assert!(!root.join("functional/g05_read_only_empty.tui.pb").exists());
    assert_eq!(tree_digest(&root), before);
    app.finish();
}

#[test]
fn g05_tests_do_not_invoke_accept() {
    let needle = concat!("tuiscotti", " ", "accept");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut pending = vec![root];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir)
            .unwrap_or_else(|error| panic!("read {}: {error}", dir.display()))
        {
            let entry = entry.unwrap_or_else(|error| panic!("entry: {error}"));
            let path = entry.path();
            let kind = entry.file_type().expect("file type");
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                // Baseline captures are about 1.9 GB. Reading every byte in an
                // unoptimized test takes 27s when the machine is quiet and
                // exceeds the functional profile's 90s kill when it is not.
                // Those files are screenshots, not an accept invocation.
                if path.file_name().is_some_and(|name| name == "baselines") {
                    continue;
                }
                pending.push(path);
                continue;
            }
            let bytes = std::fs::read(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            let found = bytes
                .windows(needle.len())
                .any(|window| window == needle.as_bytes());
            assert!(!found, "{} contains the approval command", path.display());
        }
    }
}
