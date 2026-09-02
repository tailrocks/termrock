// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Structured report for `termrock doctor` (CLI or embed).
use super::detect::DetectionReport;
use super::profile::{
    CapabilityOverrides, CapabilityProfile, TerminalCapabilities, resolve_from_detection,
};
use super::set::{CapabilityKind, fallback_policies};

/// Severity of a doctor finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DoctorSeverity {
    /// Informational.
    Info,
    /// Suspicious config.
    Warning,
    /// Broken / unsupported combination.
    Error,
}

/// One line of doctor output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorFinding {
    /// Severity.
    pub severity: DoctorSeverity,
    /// Stable code.
    pub code: String,
    /// Human message.
    pub message: String,
}

/// Full doctor report (no I/O — caller prints / paints).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorReport {
    /// Resolved capabilities ([`TerminalCapabilities`]).
    pub effective: TerminalCapabilities,
    /// Per-capability status lines.
    pub capabilities: Vec<(CapabilityKind, bool, &'static str)>,
    /// Findings / recommendations.
    pub findings: Vec<DoctorFinding>,
    /// Suggested override env examples.
    pub recommended_env: Vec<String>,
    /// Sample palette description for live visual (host renders).
    pub sample_hint: String,
}

/// Build a doctor report from optional preferred profile + overrides.
#[must_use]
pub fn build_doctor_report(
    preferred_profile: Option<CapabilityProfile>,
    overrides: CapabilityOverrides,
) -> DoctorReport {
    build_doctor_report_from_detection(super::detect_environment(), preferred_profile, overrides)
}

/// Build a doctor report from an injected detection report (pure — no process env).
#[must_use]
pub fn build_doctor_report_from_detection(
    detection: DetectionReport,
    preferred_profile: Option<CapabilityProfile>,
    overrides: CapabilityOverrides,
) -> DoctorReport {
    let effective = resolve_from_detection(detection, preferred_profile, overrides);
    let mut capabilities = Vec::new();
    for kind in CapabilityKind::ALL {
        let on = effective.set.enabled(kind);
        let fb = fallback_policies()
            .iter()
            .find(|p| p.kind == kind)
            .map(|p| p.fallback)
            .unwrap_or("see docs");
        capabilities.push((kind, on, fb));
    }

    let mut findings = Vec::new();
    for w in &effective.detection.warnings {
        findings.push(DoctorFinding {
            severity: DoctorSeverity::Warning,
            code: "env_warning".into(),
            message: w.clone(),
        });
    }
    findings.push(DoctorFinding {
        severity: DoctorSeverity::Info,
        code: "profile".into(),
        message: format!(
            "profile={} ({:?}) — {}",
            effective.profile.id(),
            effective.profile_source,
            effective.profile.description()
        ),
    });
    findings.push(DoctorFinding {
        severity: DoctorSeverity::Info,
        code: "color".into(),
        message: format!(
            "color={:?} source={:?}",
            effective.set.color, effective.color_source
        ),
    });
    if effective.set.ssh && effective.set.clipboard {
        findings.push(DoctorFinding {
            severity: DoctorSeverity::Warning,
            code: "ssh_clipboard".into(),
            message: "clipboard enabled over SSH — OSC 52 may be blocked; consider override".into(),
        });
    }
    if effective.set.multiplexer
        && matches!(
            effective.set.color,
            crate::style::ColorCapability::Truecolor
        )
    {
        findings.push(DoctorFinding {
            severity: DoctorSeverity::Warning,
            code: "mux_truecolor".into(),
            message:
                "truecolor under multiplexer — verify outer terminal-overrides / RGB enablement"
                    .into(),
        });
    }
    if !effective.set.keyboard {
        findings.push(DoctorFinding {
            severity: DoctorSeverity::Info,
            code: "headless".into(),
            message: "keyboard disabled (headless) — interactive Session should not start".into(),
        });
    }

    let mut recommended_env = Vec::new();
    if effective.detection.env.no_color {
        recommended_env.push("# chromatic color disabled by NO_COLOR".into());
    }
    if effective.set.ssh {
        recommended_env.push("TERMROCK_PROFILE=compatible".into());
        recommended_env.push("# TERMROCK_COLOR=256  # if truecolor flaky over SSH".into());
    }
    if effective.detection.env.multiplexer.is_some() {
        recommended_env.push("# tmux: set -as terminal-overrides ',*:Tc'".into());
    }
    recommended_env.push("# TERMROCK_PROFILE=modern|compatible|minimal|inline|headless".into());
    recommended_env.push("# TERMROCK_COLOR=truecolor|256|16|mono".into());

    let sample_hint = format!(
        "sample: color={:?} mouse={} alt_screen={} paste={}",
        effective.set.color,
        effective.set.mouse,
        effective.set.alternate_screen,
        effective.set.bracketed_paste
    );

    DoctorReport {
        effective,
        capabilities,
        findings,
        recommended_env,
        sample_hint,
    }
}

/// Format a plain-text doctor report (CLI-friendly).
#[must_use]
pub fn format_doctor_text(report: &DoctorReport) -> String {
    let mut out = String::new();
    out.push_str("termrock doctor\n");
    out.push_str("===============\n\n");
    out.push_str(&format!(
        "Profile: {} ({})\n",
        report.effective.profile.id(),
        report.effective.profile.description()
    ));
    out.push_str(&format!(
        "TERM={} COLORTERM={} NO_COLOR={}\n",
        report
            .effective
            .detection
            .env
            .term
            .as_deref()
            .unwrap_or("(unset)"),
        report
            .effective
            .detection
            .env
            .colorterm
            .as_deref()
            .unwrap_or("(unset)"),
        report.effective.detection.env.no_color
    ));
    if let Some(m) = &report.effective.detection.env.multiplexer {
        out.push_str(&format!("Multiplexer: {m}\n"));
    }
    out.push_str(&format!("SSH: {}\n", report.effective.set.ssh));
    out.push('\n');
    out.push_str("Capabilities\n");
    out.push_str("------------\n");
    for (kind, on, fb) in &report.capabilities {
        out.push_str(&format!(
            "  [{:>3}] {:<22} {}\n",
            if *on { "on" } else { "off" },
            kind.id(),
            kind.title()
        ));
        if !*on {
            out.push_str(&format!("        fallback: {fb}\n"));
        }
    }
    out.push('\n');
    out.push_str("Findings\n");
    out.push_str("--------\n");
    for f in &report.findings {
        out.push_str(&format!("  [{:?}] {}: {}\n", f.severity, f.code, f.message));
    }
    out.push('\n');
    out.push_str("Recommended overrides\n");
    out.push_str("---------------------\n");
    for line in &report.recommended_env {
        out.push_str(&format!("  {line}\n"));
    }
    out.push('\n');
    out.push_str(&format!("Live sample: {}\n", report.sample_hint));
    out.push_str(
        "(Host should paint Role swatches + ascii/unicode markers using effective.set.)\n",
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctor_report_lists_all_kinds() {
        let report = build_doctor_report(
            Some(CapabilityProfile::Modern),
            CapabilityOverrides::default(),
        );
        assert_eq!(report.capabilities.len(), CapabilityKind::ALL.len());
        let text = format_doctor_text(&report);
        assert!(text.contains("termrock doctor"));
        assert!(text.contains("Profile:"));
    }

    #[test]
    fn headless_doctor_notes_keyboard_off() {
        // Force headless via overrides so ambient TERMROCK_PROFILE cannot win.
        let overrides = CapabilityOverrides {
            profile: Some(CapabilityProfile::Headless),
            ..CapabilityOverrides::default()
        };
        let report = build_doctor_report(None, overrides);
        assert!(!report.effective.set.keyboard);
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.code == "headless" || f.message.contains("keyboard"))
        );
    }
}
