//! Elapsed-time reports for release measurements (#56).
//!
//! A [`MeasurementReport`] is what the ignored `work_measurements` test writes
//! for one scenario and size. It carries the raw timing statistics together
//! with the environment that produced them, so a number is never read without
//! its machine, build and instrumentation mode. Nothing here gates on time:
//! no wall-clock budget exists until repeatable baselines do.
//!
//! The count-only reports of `report` stay as they are. A
//! timing report is a different file (`*.timing.json`) because its run is a
//! different build: elapsed time is measured with instrumentation off.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use super::WorkDelta;

/// Version of the timing report schema. A reader rejects a version it does not
/// know instead of guessing at fields.
pub const SCHEMA_VERSION: u32 = 2;

/// Warm-up operations run before the first sample, per scenario and size.
pub const WARMUPS: usize = 20;

/// Samples recorded per scenario and size.
pub const SAMPLES: usize = 200;

/// Every field a timing report must carry: the 21 fields #56 specifies plus
/// `warmups`. A report with a field missing or renamed fails
/// `measurement_required_fields_are_all_present`.
pub const REQUIRED_FIELDS: [&str; 22] = [
    "schema_version",
    "scenario",
    "operation",
    "record_count",
    "viewport",
    "samples",
    "median_ns",
    "p95_ns",
    "work_delta",
    "allocations_or_null",
    "rustc",
    "profile",
    "target",
    "os",
    "cpu",
    "backend",
    "terminal_io_included",
    "instrumentation_enabled",
    "correctness_oracle",
    "setup_ns_or_null",
    "commit",
    "warmups",
];

/// Where and how a measurement ran. Every string is read from the running
/// machine or build, never typed in by hand; a value that cannot be read is
/// the literal `"unknown"`, which a reader must not mistake for a measurement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Environment {
    /// `rustc --version`, verbatim.
    pub rustc: String,
    /// `release` or `debug`, from the build the measurement ran in.
    pub profile: String,
    /// Target triple of the build.
    pub target: String,
    /// Operating system name and release.
    pub os: String,
    /// CPU model string.
    pub cpu: String,
    /// What the frame was drawn into, for example the in-memory ratatui
    /// `Buffer`. Never a real terminal unless `terminal_io_included` says so.
    pub backend: String,
    /// Full git commit hash of the measured tree.
    pub commit: String,
    /// Whether the tree had uncommitted changes, or `None` if git could not
    /// say.
    pub commit_dirty: Option<bool>,
    /// `uptime` output read when the scenario starts, verbatim, or `None`.
    pub load_average: Option<String>,
}

/// Elapsed time of one operation at one fixture size.
#[derive(Clone, Debug)]
pub struct MeasurementReport {
    pub scenario: &'static str,
    pub operation: &'static str,
    /// Domain records or lines the fixture held. Never a nominal size.
    pub record_count: usize,
    pub viewport: (u16, u16),
    /// Warm-up operations that ran before the samples and are not in them.
    pub warmups: usize,
    /// Elapsed nanoseconds of each sampled operation, in the order taken.
    pub sample_ns: Vec<u64>,
    /// Work of one operation. `None` in a build without `work-metrics`: the
    /// counts are unavailable there, which is not the same as zero.
    pub work_delta: Option<WorkDelta>,
    /// Setup or rebuild cost kept out of the samples, or `None` if the
    /// scenario has no setup worth reporting.
    pub setup_ns: Option<u64>,
    /// Whether child-process or real terminal I/O is inside the timed
    /// interval.
    pub terminal_io_included: bool,
    /// What the test checked before and after every sampled operation, outside
    /// the timed interval, so a fast no-op cannot pass as a result.
    pub correctness_oracle: String,
    pub environment: Environment,
}

/// Median as `sorted[(n - 1) / 2]`. `None` for an empty input.
pub fn median_ns(sorted: &[u64]) -> Option<u64> {
    (!sorted.is_empty()).then(|| sorted[(sorted.len() - 1) / 2])
}

/// 95th percentile as `sorted[ceil(0.95 * n) - 1]`, in integer arithmetic.
/// `None` for an empty input.
pub fn p95_ns(sorted: &[u64]) -> Option<u64> {
    let n = sorted.len();
    (n != 0).then(|| sorted[(n * 95).div_ceil(100) - 1])
}

impl MeasurementReport {
    /// The report as JSON. Panics when there are no samples: a report of
    /// nothing is a bug in the measuring test, not a result.
    pub fn to_json(&self) -> String {
        assert!(!self.sample_ns.is_empty(), "a report needs samples");
        let mut sorted = self.sample_ns.clone();
        sorted.sort_unstable();
        let env = &self.environment;
        let work_delta = match &self.work_delta {
            Some(work) => Value::Object(
                super::Metric::ALL
                    .into_iter()
                    .map(|m| (m.name().to_owned(), json!(work.get(m))))
                    .collect::<Map<_, _>>(),
            ),
            None => Value::Null,
        };
        let report = json!({
            "schema_version": SCHEMA_VERSION,
            "scenario": self.scenario,
            "operation": self.operation,
            "record_count": self.record_count,
            "viewport": { "width": self.viewport.0, "height": self.viewport.1 },
            "warmups": self.warmups,
            "samples": sorted.len(),
            "median_ns": median_ns(&sorted),
            "p95_ns": p95_ns(&sorted),
            "min_ns": sorted[0],
            "max_ns": sorted[sorted.len() - 1],
            "work_delta": work_delta,
            // No allocator instrumentation exists yet, so there is nothing to
            // report; null is the honest value.
            "allocations_or_null": Value::Null,
            "rustc": env.rustc,
            "profile": env.profile,
            "target": env.target,
            "os": env.os,
            "cpu": env.cpu,
            "backend": env.backend,
            "terminal_io_included": self.terminal_io_included,
            "instrumentation_enabled": super::ENABLED,
            "correctness_oracle": self.correctness_oracle,
            "setup_ns_or_null": self.setup_ns,
            "commit": env.commit,
            "commit_dirty": env.commit_dirty,
            "load_average": env.load_average,
            "timing_scope": "calling thread wall clock around one operation; \
                             worker threads and child processes are not included \
                             unless terminal_io_included says so",
        });
        let mut out = serde_json::to_string_pretty(&report).expect("serialize a report");
        out.push('\n');
        out
    }

    /// The file name: scenario, operation and size, so one run's reports never
    /// overwrite each other and the count reports keep their own names.
    pub fn file_name(&self) -> String {
        format!(
            "{}-{}-{}.timing.json",
            self.scenario, self.operation, self.record_count
        )
    }

    /// Writes the report into `dir` (created if missing) through a temp file
    /// and a rename, and returns its path.
    pub fn write_in(&self, dir: &Path) -> std::io::Result<PathBuf> {
        std::fs::create_dir_all(dir)?;
        let name = self.file_name();
        let path = dir.join(&name);
        let temp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
        let written = std::fs::File::create(&temp)
            .and_then(|mut file| file.write_all(self.to_json().as_bytes()))
            .and_then(|()| std::fs::rename(&temp, &path));
        if written.is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        written.map(|()| path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn environment() -> Environment {
        Environment {
            rustc: "rustc test".into(),
            profile: "release".into(),
            target: "test-target".into(),
            os: "test-os".into(),
            cpu: "test-cpu".into(),
            backend: "test-backend".into(),
            commit: "0".repeat(40),
            commit_dirty: Some(false),
            load_average: None,
        }
    }

    fn report(sample_ns: Vec<u64>) -> MeasurementReport {
        MeasurementReport {
            scenario: "scenario",
            operation: "operation",
            record_count: 1_000,
            viewport: (100, 30),
            warmups: WARMUPS,
            sample_ns,
            work_delta: None,
            setup_ns: None,
            terminal_io_included: false,
            correctness_oracle: "oracle".into(),
            environment: environment(),
        }
    }

    #[test]
    fn measurement_required_fields_are_all_present() {
        let json: Value = serde_json::from_str(&report(vec![3, 1, 2]).to_json()).unwrap();
        let object = json.as_object().expect("a report is an object");
        for field in REQUIRED_FIELDS {
            assert!(object.contains_key(field), "report lacks `{field}`");
        }
        assert_eq!(object["schema_version"], SCHEMA_VERSION);
    }

    #[test]
    fn measurement_unavailable_values_are_null_not_zero() {
        let json: Value = serde_json::from_str(&report(vec![5]).to_json()).unwrap();
        assert_eq!(json["work_delta"], Value::Null);
        assert_eq!(json["allocations_or_null"], Value::Null);
        assert_eq!(json["setup_ns_or_null"], Value::Null);
        assert_eq!(json["instrumentation_enabled"], super::super::ENABLED);
    }

    #[test]
    fn measurement_work_delta_lists_every_metric_when_present() {
        let mut r = report(vec![5]);
        r.work_delta = Some(WorkDelta::ZERO);
        let json: Value = serde_json::from_str(&r.to_json()).unwrap();
        let work = json["work_delta"].as_object().expect("an object");
        assert_eq!(work.len(), super::super::METRIC_COUNT);
    }

    #[test]
    fn measurement_percentiles_follow_the_specified_indices() {
        // 200 samples: median is sorted[99], p95 is sorted[189].
        let sorted: Vec<u64> = (0..200).collect();
        assert_eq!(median_ns(&sorted), Some(99));
        assert_eq!(p95_ns(&sorted), Some(189));
        // Odd and tiny counts: sorted[(n-1)/2] and sorted[ceil(0.95n)-1].
        assert_eq!(median_ns(&[7]), Some(7));
        assert_eq!(p95_ns(&[7]), Some(7));
        assert_eq!(median_ns(&[1, 2, 3, 4]), Some(2));
        assert_eq!(p95_ns(&[1, 2, 3, 4]), Some(4));
        assert_eq!(p95_ns(&(0..20).collect::<Vec<u64>>()), Some(18));
        assert_eq!(median_ns(&[]), None);
        assert_eq!(p95_ns(&[]), None);
    }

    #[test]
    fn measurement_report_sorts_samples_before_taking_statistics() {
        let json: Value = serde_json::from_str(&report(vec![9, 1, 5, 3, 7]).to_json()).unwrap();
        assert_eq!(json["median_ns"], 5);
        assert_eq!(json["p95_ns"], 9);
        assert_eq!(json["samples"], 5);
        assert_eq!(json["min_ns"], 1);
    }

    #[test]
    fn measurement_file_name_does_not_collide_with_count_reports() {
        assert_eq!(
            report(vec![1]).file_name(),
            "scenario-operation-1000.timing.json"
        );
    }
}
