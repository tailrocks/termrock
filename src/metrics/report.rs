//! Count-only scenario reports.
//!
//! A report states how much work one operation did, in the units of
//! [`Metric`]. It carries no elapsed time and makes no latency claim.

use std::path::PathBuf;

use super::{ENABLED, Metric, WorkDelta};

pub const SCHEMA_VERSION: u32 = 1;

/// What one scenario operation did.
#[derive(Clone, Debug)]
pub struct ScenarioReport {
    /// App or widget family, for example `holla_finder`.
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
        let dir = target.join("performance");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!(
            "{}-{}-{}.json",
            self.scenario, self.operation, self.record_count
        ));
        std::fs::write(&path, self.to_json())?;
        Ok(path)
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
