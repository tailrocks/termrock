//! Count-only scenario reports.
//!
//! A report states how much work one operation did, in the units of
//! [`Metric`]. It carries no elapsed time and makes no latency claim.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use termrock_metrics::{ENABLED, Metric, WorkDelta};

pub const SCHEMA_VERSION: u32 = 1;

/// What one scenario operation did.
#[derive(Clone, Debug)]
pub struct ScenarioReport {
    /// The family the scenario exercises, for example `list_settled`.
    pub scenario: &'static str,
    /// The single operation measured, for example `settled_tick`.
    pub operation: &'static str,
    /// Domain records or lines the fixture held. Never a nominal size.
    pub record_count: usize,
    /// Terminal size the frame was drawn into.
    pub viewport: (u16, u16),
    pub work: WorkDelta,
    /// What the test checked outside the measured operation, so a no-op or
    /// stale result cannot pass as cheap.
    pub correctness_oracle: String,
}

impl ScenarioReport {
    pub fn to_json(&self) -> String {
        let mut out = String::new();
        out.push_str("{\n");
        field(&mut out, "schema_version", &SCHEMA_VERSION.to_string());
        field(&mut out, "scenario", &quote(self.scenario));
        field(&mut out, "operation", &quote(self.operation));
        field(&mut out, "record_count", &self.record_count.to_string());
        field(
            &mut out,
            "viewport",
            &format!(
                "{{\"width\": {}, \"height\": {}}}",
                self.viewport.0, self.viewport.1
            ),
        );
        let counts: Vec<String> = Metric::ALL
            .into_iter()
            .map(|m| format!("    {}: {}", quote(m.name()), self.work.get(m)))
            .collect();
        field(
            &mut out,
            "work_delta",
            &format!("{{\n{}\n  }}", counts.join(",\n")),
        );
        field(&mut out, "instrumentation_enabled", &ENABLED.to_string());
        field(
            &mut out,
            "thread_scope",
            &quote("calling thread only; worker threads and child processes are not included"),
        );
        field(&mut out, "timing", &quote("not measured"));
        out.push_str(&format!(
            "  \"correctness_oracle\": {}\n}}\n",
            quote(&self.correctness_oracle)
        ));
        out
    }

    /// Writes the report under `<target dir>/performance/` and returns its path.
    pub fn write(&self) -> std::io::Result<PathBuf> {
        let target = std::env::var_os("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/target")));
        self.write_in(&target.join("performance"))
    }

    /// Writes the report into `dir` and returns its path.
    ///
    /// The JSON goes to a temp file that no other writer can name, in the same
    /// directory, and is then renamed over the final path. A concurrent writer
    /// of the same scenario replaces the file whole, so a reader never sees a
    /// truncated or interleaved report.
    pub fn write_in(&self, dir: &Path) -> std::io::Result<PathBuf> {
        static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

        std::fs::create_dir_all(dir)?;
        let name = format!(
            "{}-{}-{}.json",
            self.scenario, self.operation, self.record_count
        );
        let path = dir.join(&name);
        let temp = dir.join(format!(
            ".{name}.{}.{}.tmp",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let written = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .and_then(|mut file| file.write_all(self.to_json().as_bytes()))
            .and_then(|()| std::fs::rename(&temp, &path));
        if written.is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        written.map(|()| path)
    }
}

fn field(out: &mut String, key: &str, value: &str) {
    out.push_str(&format!("  \"{key}\": {value},\n"));
}

fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
