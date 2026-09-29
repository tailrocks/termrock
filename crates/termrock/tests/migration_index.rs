//! Repository contract checks for the public migration index.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[test]
fn every_migration_has_one_ordered_index_entry() {
    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let migration_dir = repository_root.join("migrations");
    let mut files = BTreeMap::new();

    for entry in fs::read_dir(&migration_dir).expect("migration directory exists") {
        let path = entry.expect("migration directory entry is readable").path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some((sequence, _)) = name.split_once('-') else {
            continue;
        };
        let Ok(sequence) = sequence.parse::<u32>() else {
            continue;
        };
        assert!(
            files.insert(sequence, name.to_owned()).is_none(),
            "more than one migration file uses sequence {sequence:04}"
        );
    }

    let index =
        fs::read_to_string(repository_root.join("MIGRATING.md")).expect("migration index exists");
    let mut rows = BTreeMap::new();
    for line in index.lines() {
        let columns: Vec<_> = line.split('|').map(str::trim).collect();
        let Some(sequence) = columns.get(1).and_then(|value| value.parse::<u32>().ok()) else {
            continue;
        };
        let link = columns
            .get(3)
            .and_then(|value| value.split_once("](migrations/"))
            .map(|(_, tail)| tail.trim_end_matches(')'))
            .expect("each migration row links to a migration file");
        assert!(
            rows.insert(sequence, link.to_owned()).is_none(),
            "more than one index row uses sequence {sequence:04}"
        );
    }

    assert_eq!(
        rows.len(),
        files.len(),
        "index and migration file counts differ"
    );
    for (offset, (sequence, filename)) in files.iter().enumerate() {
        assert_eq!(
            *sequence as usize,
            offset + 1,
            "migration sequence is not contiguous at {sequence:04}"
        );
        assert_eq!(
            rows.get(sequence).map(String::as_str),
            Some(filename.as_str()),
            "migration {sequence:04} is missing or links to the wrong file"
        );
    }
}
