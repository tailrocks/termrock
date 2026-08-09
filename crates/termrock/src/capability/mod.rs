// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Terminal capability architecture: detect, override, profile, fallback.
//!
//! Optional features must never become hidden hard dependencies. Hosts resolve
//! an [`EffectiveCapabilities`] once (or on resize/env change) and pass it into
//! paint / session setup.
//!
//! See `docs/design/terminal-capability-architecture.md`.

mod detect;
mod doctor;
mod profile;
mod set;

pub use detect::{CapabilitySource, DetectionReport, EnvHints, detect_environment};
pub use doctor::{
    DoctorFinding, DoctorReport, DoctorSeverity, build_doctor_report, format_doctor_text,
};
pub use profile::{
    CapabilityOverrides, CapabilityProfile, EffectiveCapabilities, SessionFlags,
    resolve_capabilities,
};
pub use set::{CapabilityKind, CapabilitySet, FallbackPolicy, fallback_policies};
