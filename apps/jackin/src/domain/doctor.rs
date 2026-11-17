use termrock::theme::StatusKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorSurface {
    pub noun: String,
    pub state: DoctorState,
    /// Run the checks again this many seconds after the last run ended. The
    /// world starts the run; `None` leaves it to the operator.
    pub rerun_secs: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DoctorState {
    NeverRun,
    Running {
        previous: Option<DoctorReport>,
    },
    Done(DoctorReport),
    /// The checks could not run at all. The report of the run before, if
    /// there was one, stays.
    Failed {
        previous: Option<DoctorReport>,
        failure: DoctorFailure,
        at_secs: i64,
    },
}

/// Why the checks could not run, as opposed to a check that ran and failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DoctorFailure {
    TimedOut,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorReport {
    pub checks: Vec<DoctorCheck>,
    pub at_secs: i64,
    pub gateway: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorCheck {
    pub label: String,
    pub status: StatusKind,
    pub summary: String,
    pub found: Option<String>,
    pub expected: Option<String>,
    pub fix: Option<String>,
}

pub const STALE_AFTER_SECS: i64 = 60;

impl DoctorReport {
    pub fn failing(&self) -> usize {
        self.count(StatusKind::Fail)
    }

    pub fn warnings(&self) -> usize {
        self.count(StatusKind::Warn)
    }

    fn count(&self, kind: StatusKind) -> usize {
        self.checks.iter().filter(|c| c.status == kind).count()
    }

    pub fn stale(&self, now_secs: i64) -> bool {
        now_secs - self.at_secs > STALE_AFTER_SECS
    }

    /// The run found no gateway to check, so the rest of its rows are
    /// skipped and say nothing about health.
    pub fn no_gateway(&self) -> bool {
        self.gateway.is_none()
    }
}

impl DoctorSurface {
    pub fn report(&self) -> Option<&DoctorReport> {
        match &self.state {
            DoctorState::NeverRun => None,
            DoctorState::Running { previous } | DoctorState::Failed { previous, .. } => {
                previous.as_ref()
            }
            DoctorState::Done(r) => Some(r),
        }
    }

    /// Why the last run could not happen, and when that was.
    pub fn failure(&self) -> Option<(&DoctorFailure, i64)> {
        match &self.state {
            DoctorState::Failed {
                failure, at_secs, ..
            } => Some((failure, *at_secs)),
            _ => None,
        }
    }

    /// Marks a run as started. False when one is already out.
    pub fn start(&mut self) -> bool {
        if self.running() {
            return false;
        }
        let previous = self.report().cloned();
        self.state = DoctorState::Running { previous };
        true
    }

    /// Takes in how a run ended.
    pub fn finish(&mut self, result: &Result<DoctorReport, DoctorFailure>, now_secs: i64) {
        self.state = match result {
            Ok(report) => DoctorState::Done(report.clone()),
            Err(failure) => DoctorState::Failed {
                previous: self.report().cloned(),
                failure: failure.clone(),
                at_secs: now_secs,
            },
        };
    }

    /// Whether the rerun timer has run out at `now_secs`.
    pub fn due(&self, now_secs: i64) -> bool {
        let Some(every) = self.rerun_secs else {
            return false;
        };
        match &self.state {
            DoctorState::Running { .. } => false,
            DoctorState::NeverRun => true,
            DoctorState::Done(report) => now_secs - report.at_secs >= every,
            DoctorState::Failed { at_secs, .. } => now_secs - at_secs >= every,
        }
    }

    pub fn running(&self) -> bool {
        matches!(self.state, DoctorState::Running { .. })
    }
}
