// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Terminal capability architecture: detect, override, profile, boundary, fallback.
//!
//! Optional features must never become hidden hard dependencies. Hosts resolve
//! [`TerminalCapabilities`] once (or on resize/env change), derive a
//! [`CapabilityBoundary`] for widgets, and never read env vars in paint paths.
//!
//! See `docs/design/terminal-capability-architecture.md`.

mod boundary;
mod detect;
mod doctor;
mod profile;
mod set;

pub use boundary::{CapabilityBoundary, ComponentCapabilityHints};
pub use detect::{
    CapabilitySource, DetectionReport, EnvHints, detect_environment, detect_from_hints,
};
pub use doctor::{
    DoctorFinding, DoctorReport, DoctorSeverity, build_doctor_report,
    build_doctor_report_from_detection, format_doctor_text,
};
pub use profile::{
    CapabilityOverrides, CapabilityProfile, EffectiveCapabilities, SessionFlags,
    TerminalCapabilities, resolve_capabilities, resolve_from_detection,
};
pub use set::{CapabilityKind, CapabilitySet, FallbackPolicy, fallback_policies};
