use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StepState {
    Ready,

    Waiting,
    Running,
    Succeeded,
    Failed,

    Skipped,

    Blocked,
    Cancelled,

    Excluded,
}

impl StepState {
    pub fn label(self) -> &'static str {
        match self {
            StepState::Ready => "ready",
            StepState::Waiting => "waiting",
            StepState::Running => "running",
            StepState::Succeeded => "succeeded",
            StepState::Failed => "failed",
            StepState::Skipped => "skipped",
            StepState::Blocked => "blocked",
            StepState::Cancelled => "cancelled",
            StepState::Excluded => "excluded",
        }
    }

    pub fn finished(self) -> bool {
        matches!(
            self,
            StepState::Succeeded
                | StepState::Failed
                | StepState::Skipped
                | StepState::Blocked
                | StepState::Cancelled
                | StepState::Excluded
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub id: String,
    pub label: String,
    pub commands: Vec<String>,
    pub cwd: String,

    pub privilege: String,
    pub optional: bool,

    pub included: bool,

    pub deps: Vec<usize>,

    pub exclusive: Option<String>,
    pub effects: Vec<String>,

    pub target: Option<String>,

    pub note: String,
    pub state: StepState,

    pub because: String,

    pub duration_ticks: u64,
    pub fails: bool,
    pub started_tick: Option<u64>,
    pub ended_tick: Option<u64>,
    pub exit: Option<i32>,

    /// Run number of this step: 0 before the first arm, 1 after arm, +1 per retry.
    pub attempt: u32,
    /// Template lines the current attempt has already emitted. Counts only
    /// template lines, never history markers, so it is per attempt.
    emitted_template_lines: usize,

    pub output: Vec<String>,
}

impl Step {
    pub fn new(id: &str, label: &str, commands: &[&str], cwd: &str) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            commands: commands.iter().map(|s| (*s).to_owned()).collect(),
            cwd: cwd.into(),
            privilege: String::new(),
            optional: false,
            included: true,
            deps: vec![],
            exclusive: None,
            target: None,
            effects: vec![],
            note: String::new(),
            state: StepState::Ready,
            because: String::new(),
            duration_ticks: 12,
            fails: false,
            started_tick: None,
            ended_tick: None,
            exit: None,
            attempt: 0,
            emitted_template_lines: 0,
            output: vec![],
        }
    }
    pub fn after(mut self, deps: &[usize]) -> Self {
        self.deps = deps.to_vec();
        self
    }
    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }
    pub fn excluded(mut self) -> Self {
        self.optional = true;
        self.included = false;
        self
    }
    pub fn privilege(mut self, p: &str) -> Self {
        self.privilege = p.into();
        self
    }
    pub fn exclusive(mut self, e: &str) -> Self {
        self.exclusive = Some(e.into());
        self
    }
    pub fn target(mut self, t: &str) -> Self {
        self.target = Some(t.into());
        self
    }
    pub fn effects(mut self, e: &[&str]) -> Self {
        self.effects = e.iter().map(|s| (*s).to_owned()).collect();
        self
    }
    pub fn note(mut self, n: &str) -> Self {
        self.note = n.into();
        self
    }
    pub fn ticks(mut self, t: u64) -> Self {
        self.duration_ticks = t;
        self
    }
    pub fn fails(mut self) -> Self {
        self.fails = true;
        self
    }

    /// Appends the template lines up to `want` that this attempt has not
    /// emitted yet, and returns how many were appended. Progress comes only
    /// from the attempt's own cursor, so history markers in `output` do not
    /// shift which template line comes next.
    pub fn append_attempt_output(&mut self, template: &[String], want: usize) -> usize {
        let end = want.min(template.len());
        let start = self.emitted_template_lines.min(template.len());
        if end <= start {
            return 0;
        }
        self.output.extend(template[start..end].iter().cloned());
        self.emitted_template_lines = end;
        end - start
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanPhase {
    Review,
    Running,
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub id: String,
    pub title: String,

    pub intent: String,
    pub host: String,
    pub steps: Vec<Step>,

    pub phrase: Option<String>,
    pub broad: bool,
    pub phase: PlanPhase,

    pub max_parallel: usize,

    pub continue_on_failure: bool,

    pub started_tick: Option<u64>,
    pub ended_tick: Option<u64>,

    pub facts: Vec<(String, String)>,

    pub drift: Option<String>,
    pub drift_consumed: bool,

    pub follow_ups: Vec<(String, String)>,

    /// Whether the latest run ended by the user stopping the whole plan, as
    /// opposed to cancelling only its remaining steps. Cleared by arm and retry.
    pub stopped: bool,

    /// Step ids whose simulated effects were already applied during this arm.
    /// Scoped to one full arm: retry keeps it, arm clears it.
    applied_steps: BTreeSet<String>,
    /// Whether the docker cleanup usage metric was already counted for this arm.
    completion_counted: bool,
}

impl Plan {
    pub fn new(id: &str, title: &str, intent: &str, host: &str, steps: Vec<Step>) -> Self {
        let mut p = Self {
            id: id.into(),
            title: title.into(),
            intent: intent.into(),
            host: host.into(),
            steps,
            phrase: None,
            broad: false,
            phase: PlanPhase::Review,
            max_parallel: 2,
            continue_on_failure: true,
            started_tick: None,
            ended_tick: None,
            facts: vec![],
            drift: None,
            drift_consumed: false,
            follow_ups: vec![],
            stopped: false,
            applied_steps: BTreeSet::new(),
            completion_counted: false,
        };
        p.recompute();
        p
    }
    pub fn phrase(mut self, p: &str, broad: bool) -> Self {
        self.phrase = Some(p.into());
        self.broad = broad;
        self
    }
    pub fn follow_up(mut self, label: &str, item: &str) -> Self {
        self.follow_ups.push((label.into(), item.into()));
        self
    }
    pub fn fact(mut self, k: &str, v: &str) -> Self {
        self.facts.push((k.into(), v.into()));
        self
    }
    pub fn drift(mut self, d: &str) -> Self {
        self.drift = Some(d.into());
        self
    }

    pub fn gate_phrase(&self) -> Option<String> {
        self.phrase.as_ref().map(|p| {
            if self.broad {
                format!("I UNDERSTAND: {p}")
            } else {
                p.clone()
            }
        })
    }

    pub fn toggle(&mut self, i: usize) -> bool {
        if self.phase != PlanPhase::Review {
            return false;
        }
        let Some(s) = self.steps.get_mut(i) else {
            return false;
        };
        if !s.optional {
            return false;
        }
        s.included = !s.included;
        self.recompute();
        true
    }

    pub fn dependents(&self, i: usize) -> Vec<usize> {
        (0..self.steps.len())
            .filter(|&j| self.steps[j].deps.contains(&i))
            .collect()
    }

    pub fn parallel_with(&self, i: usize) -> Vec<usize> {
        (0..self.steps.len())
            .filter(|&j| j != i && self.steps[j].included && self.steps[i].included)
            .filter(|&j| !self.ancestor(i, j) && !self.ancestor(j, i))
            .filter(|&j| {
                !(self.steps[i].exclusive.is_some()
                    && self.steps[i].exclusive == self.steps[j].exclusive)
            })
            .collect()
    }

    pub fn ancestor(&self, a: usize, b: usize) -> bool {
        let mut stack = vec![b];
        let mut seen = vec![false; self.steps.len()];
        while let Some(x) = stack.pop() {
            if seen[x] {
                continue;
            }
            seen[x] = true;
            for &d in &self.steps[x].deps {
                if d == a {
                    return true;
                }
                stack.push(d);
            }
        }
        false
    }

    pub fn recompute(&mut self) {
        let n = self.steps.len();
        for i in 0..n {
            let s = &self.steps[i];
            if s.state == StepState::Running
                || s.state == StepState::Succeeded
                || s.state == StepState::Failed
                || s.state == StepState::Skipped
                || s.state == StepState::Cancelled
            {
                continue;
            }
            if !s.included {
                self.steps[i].state = StepState::Excluded;
                self.steps[i].because = "excluded".into();
                continue;
            }
            let deps = self.steps[i].deps.clone();
            let mut blocked_by: Option<String> = None;
            let mut waiting = false;
            for d in deps {
                match self.steps[d].state {
                    StepState::Excluded | StepState::Blocked => {
                        blocked_by = Some(format!(
                            "blocked · needs {:02} {}",
                            d + 1,
                            self.steps[d].label.to_lowercase()
                        ));
                    }
                    StepState::Failed | StepState::Cancelled => {
                        blocked_by = Some(format!("blocked · {:02} failed", d + 1));
                    }
                    StepState::Skipped => {
                        blocked_by = Some(format!("blocked · {:02} skipped", d + 1));
                    }
                    StepState::Succeeded => {}
                    _ => waiting = true,
                }
            }
            self.steps[i].state = if let Some(why) = blocked_by {
                self.steps[i].because = why;
                StepState::Blocked
            } else if waiting {
                self.steps[i].because.clear();
                StepState::Waiting
            } else {
                self.steps[i].because.clear();
                StepState::Ready
            };
        }
    }

    pub fn ready(&self) -> Vec<usize> {
        (0..self.steps.len())
            .filter(|&i| self.steps[i].included && self.steps[i].state == StepState::Ready)
            .collect()
    }

    pub fn running(&self) -> Vec<usize> {
        (0..self.steps.len())
            .filter(|&i| self.steps[i].state == StepState::Running)
            .collect()
    }

    pub fn is_done(&self) -> bool {
        self.steps
            .iter()
            .all(|s| s.state.finished() || (!s.included))
            && self.running().is_empty()
            && self.ready().is_empty()
    }

    pub fn outcome(&self) -> (usize, usize, usize, usize, usize) {
        let c = |st: StepState| self.steps.iter().filter(|s| s.state == st).count();
        (
            c(StepState::Succeeded),
            c(StepState::Failed),
            c(StepState::Excluded),
            c(StepState::Skipped),
            c(StepState::Blocked)
                + c(StepState::Cancelled)
                + c(StepState::Waiting)
                + c(StepState::Ready),
        )
    }

    pub fn included_count(&self) -> usize {
        self.steps.iter().filter(|s| s.included).count()
    }

    pub fn optional_count(&self) -> usize {
        self.steps.iter().filter(|s| s.optional).count()
    }

    pub fn estimate_ticks(&self) -> u64 {
        fn finish(p: &Plan, i: usize, memo: &mut Vec<Option<u64>>) -> u64 {
            if let Some(v) = memo[i] {
                return v;
            }
            let start = p.steps[i]
                .deps
                .iter()
                .map(|&d| finish(p, d, memo))
                .max()
                .unwrap_or(0);
            let v = if p.steps[i].included {
                start + p.steps[i].duration_ticks
            } else {
                start
            };
            memo[i] = Some(v);
            v
        }
        let mut memo = vec![None; self.steps.len()];
        (0..self.steps.len())
            .map(|i| finish(self, i, &mut memo))
            .max()
            .unwrap_or(0)
    }

    pub fn roots(&self) -> Vec<usize> {
        (0..self.steps.len())
            .filter(|&i| self.steps[i].deps.is_empty() && self.steps[i].included)
            .collect()
    }

    pub fn parallel_branches(&self) -> usize {
        let mut best = self.roots().len();
        for i in 0..self.steps.len() {
            let n = self
                .dependents(i)
                .into_iter()
                .filter(|&j| self.steps[j].included)
                .count();
            best = best.max(n);
        }
        best
    }

    pub fn retry(&mut self, i: usize) -> bool {
        let Some(s) = self.steps.get_mut(i) else {
            return false;
        };
        if s.state != StepState::Failed {
            return false;
        }
        s.attempt = s.attempt.checked_add(1).expect("attempt count overflow");
        s.state = StepState::Ready;
        s.fails = false;
        s.started_tick = None;
        s.ended_tick = None;
        s.exit = None;
        s.emitted_template_lines = 0;
        s.output.push("retrying…".into());
        let deps = self.dependents(i);
        for d in deps {
            if self.steps[d].state == StepState::Blocked {
                self.steps[d].state = StepState::Waiting;
            }
        }
        self.recompute();
        self.phase = PlanPhase::Running;
        self.ended_tick = None;
        self.stopped = false;
        true
    }

    pub fn skip(&mut self, i: usize) -> bool {
        let Some(s) = self.steps.get_mut(i) else {
            return false;
        };
        if s.state != StepState::Failed || !s.optional {
            return false;
        }
        s.state = StepState::Skipped;
        self.recompute();
        true
    }

    pub fn cancel_remaining(&mut self) {
        for s in &mut self.steps {
            if matches!(s.state, StepState::Ready | StepState::Waiting) {
                s.state = StepState::Cancelled;
                s.because = "cancelled".into();
            }
        }
        self.recompute();
    }

    /// Stops the whole plan: every live included step becomes terminal and the
    /// plan is Done. Unlike [`Plan::cancel_remaining`], running steps do not get
    /// to finish. Returns false (and changes nothing) unless the plan is Running.
    pub fn stop(&mut self, tick: u64) -> bool {
        if self.phase != PlanPhase::Running {
            return false;
        }
        for step in &mut self.steps {
            if step.included
                && matches!(
                    step.state,
                    StepState::Ready | StepState::Waiting | StepState::Running
                )
            {
                let started = step.state == StepState::Running;
                step.state = StepState::Cancelled;
                step.because = "stopped by user".into();
                step.ended_tick = Some(tick);
                step.exit = started.then_some(130);
                if started {
                    step.output.push("cancelled by user".into());
                }
            }
        }
        self.phase = PlanPhase::Done;
        self.ended_tick = Some(tick);
        self.stopped = true;
        true
    }

    /// Succeeded steps whose effects were not yet applied in this arm.
    pub(crate) fn unapplied_successful_step_ids(&self) -> BTreeSet<String> {
        self.steps
            .iter()
            .filter(|step| {
                step.state == StepState::Succeeded && !self.applied_steps.contains(&step.id)
            })
            .map(|step| step.id.clone())
            .collect()
    }

    pub(crate) fn mark_step_effects_applied(&mut self, ids: &BTreeSet<String>) {
        self.applied_steps.extend(ids.iter().cloned());
    }

    /// True once per arm, and only after some step effect was applied.
    pub(crate) fn count_completion_once(&mut self) -> bool {
        if self.completion_counted || self.applied_steps.is_empty() {
            return false;
        }
        self.completion_counted = true;
        true
    }

    /// Starts a full run. A running plan is left untouched.
    pub fn arm(&mut self, tick: u64) -> bool {
        if self.phase == PlanPhase::Running {
            return false;
        }
        for s in &mut self.steps {
            s.state = StepState::Ready;
            s.started_tick = None;
            s.ended_tick = None;
            s.exit = None;
            s.attempt = 1;
            s.emitted_template_lines = 0;
            s.output.clear();
        }
        self.applied_steps.clear();
        self.completion_counted = false;
        self.stopped = false;
        self.phase = PlanPhase::Running;
        self.started_tick = Some(tick);
        self.ended_tick = None;
        self.recompute();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn running_plan() -> Plan {
        let steps = vec![
            Step::new("a", "A", &["a"], "/").ticks(5),
            Step::new("b", "B", &["b"], "/").ticks(50),
            Step::new("c", "C", &["c"], "/").after(&[1]),
            Step::new("d", "D", &["d"], "/").after(&[0]),
        ];
        let mut p = Plan::new("p", "P", "intent", "host", steps);
        p.arm(3);
        p
    }

    fn start(p: &mut Plan, i: usize, tick: u64) {
        p.steps[i].state = StepState::Running;
        p.steps[i].started_tick = Some(tick);
    }

    #[test]
    fn plan_stop_finalizes_running_and_waiting_steps() {
        let mut p = running_plan();
        start(&mut p, 0, 3);
        start(&mut p, 1, 3);
        p.steps[1].output.push("partial".into());
        assert_eq!(p.steps[2].state, StepState::Waiting);

        assert!(p.stop(9));

        assert_eq!(p.phase, PlanPhase::Done);
        assert_eq!(p.ended_tick, Some(9));
        assert!(
            p.steps.iter().all(|s| !matches!(
                s.state,
                StepState::Ready | StepState::Waiting | StepState::Running
            )),
            "no live step remains: {:?}",
            p.steps.iter().map(|s| s.state).collect::<Vec<_>>()
        );
        let b = &p.steps[1];
        assert_eq!(b.state, StepState::Cancelled);
        assert_eq!(b.because, "stopped by user");
        assert_eq!(
            (b.started_tick, b.ended_tick, b.exit),
            (Some(3), Some(9), Some(130))
        );
        assert_eq!(
            b.output,
            vec!["partial".to_owned(), "cancelled by user".to_owned()]
        );
        let c = &p.steps[2];
        assert_eq!(c.state, StepState::Cancelled);
        assert_eq!(
            (c.started_tick, c.ended_tick, c.exit),
            (None, Some(9), None)
        );
        assert!(c.output.is_empty(), "unstarted step gets no output line");
    }

    #[test]
    fn plan_stop_preserves_existing_terminal_outcomes() {
        let steps = vec![
            Step::new("ok", "Ok", &["x"], "/"),
            Step::new("bad", "Bad", &["x"], "/"),
            Step::new("skip", "Skip", &["x"], "/").optional(),
            Step::new("blocked", "Blocked", &["x"], "/").after(&[1]),
            Step::new("off", "Off", &["x"], "/").excluded(),
            Step::new("live", "Live", &["x"], "/"),
        ];
        let mut p = Plan::new("p", "P", "i", "h", steps);
        p.arm(0);
        p.steps[0].state = StepState::Succeeded;
        p.steps[0].ended_tick = Some(1);
        p.steps[0].exit = Some(0);
        p.steps[0].output.push("done".into());
        p.steps[1].state = StepState::Failed;
        p.steps[1].exit = Some(1);
        p.steps[2].state = StepState::Skipped;
        p.recompute();
        assert_eq!(p.steps[3].state, StepState::Blocked);
        assert_eq!(p.steps[4].state, StepState::Excluded);
        let before = p.steps.clone();
        start(&mut p, 5, 2);

        assert!(p.stop(7));

        assert_eq!(p.steps[..5], before[..5], "terminal steps unchanged");
        assert_eq!(p.steps[5].state, StepState::Cancelled);
        assert_eq!(p.steps[5].exit, Some(130));
    }

    #[test]
    fn plan_stopped_marks_only_a_whole_plan_stop_until_the_next_run() {
        let mut p = running_plan();
        p.cancel_remaining();
        assert!(!p.stopped, "cancel remaining is not a stop");

        let mut p = running_plan();
        start(&mut p, 1, 4);
        assert!(p.stop(9));
        assert!(p.stopped);
        assert!(p.arm(10));
        assert!(!p.stopped, "arm starts a fresh run");
    }

    #[test]
    fn plan_stop_is_idempotent_and_refuses_non_running_phases() {
        let mut p = running_plan();
        start(&mut p, 1, 3);
        assert!(p.stop(9));
        let after = p.clone();
        assert!(!p.stop(20));
        assert_eq!(p, after);

        let steps = vec![Step::new("a", "A", &["a"], "/")];
        let mut review = Plan::new("p", "P", "i", "h", steps);
        let before = review.clone();
        assert!(!review.stop(1));
        assert_eq!(review, before);
    }

    #[test]
    fn plan_cancel_remaining_still_allows_running_work_to_finish() {
        let mut p = running_plan();
        start(&mut p, 1, 3);
        p.cancel_remaining();
        assert_eq!(p.steps[1].state, StepState::Running);
        assert_eq!(p.phase, PlanPhase::Running);
        assert_eq!(p.steps[2].state, StepState::Cancelled);
        assert_eq!(p.steps[3].state, StepState::Cancelled);
    }

    #[test]
    fn plan_arm_while_running_is_noop() {
        let mut p = running_plan();
        start(&mut p, 1, 3);
        p.steps[1].output.push("line".into());
        p.steps[0].state = StepState::Succeeded;
        p.mark_step_effects_applied(&BTreeSet::from(["a".to_owned()]));
        let before = p.clone();

        assert!(!p.arm(40));

        assert_eq!(p, before);
    }

    #[test]
    fn plan_stop_effect_tracking_survives_retry_and_resets_on_arm() {
        let mut p = running_plan();
        p.steps[0].state = StepState::Succeeded;
        p.steps[1].state = StepState::Failed;
        assert_eq!(
            p.unapplied_successful_step_ids(),
            BTreeSet::from(["a".to_owned()])
        );
        p.mark_step_effects_applied(&p.unapplied_successful_step_ids());
        assert!(p.unapplied_successful_step_ids().is_empty());
        assert!(p.count_completion_once());
        assert!(!p.count_completion_once());
        p.phase = PlanPhase::Done;

        assert!(p.retry(1));
        p.steps[1].state = StepState::Succeeded;
        assert_eq!(
            p.unapplied_successful_step_ids(),
            BTreeSet::from(["b".to_owned()]),
            "retry keeps the applied id and makes the retried step eligible"
        );
        assert!(!p.count_completion_once(), "retry never recounts");

        p.phase = PlanPhase::Done;
        assert!(p.arm(50));
        p.steps[0].state = StepState::Succeeded;
        assert_eq!(
            p.unapplied_successful_step_ids(),
            BTreeSet::from(["a".to_owned()]),
            "arm clears the applied ids"
        );
        assert!(
            !p.count_completion_once(),
            "nothing applied yet in this arm"
        );
        p.mark_step_effects_applied(&p.unapplied_successful_step_ids());
        assert!(p.count_completion_once(), "arm clears the completion count");
    }

    fn template(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|l| (*l).to_owned()).collect()
    }

    #[test]
    fn plan_retry_output_cursor_ignores_external_history_markers() {
        let mut step = Step::new("a", "A", &["a"], "/");
        let lines = template(&["one", "two", "three"]);
        assert_eq!(step.append_attempt_output(&lines, 1), 1);
        step.output.insert(0, "status: running".into());
        step.output.push("exit status 1".into());

        assert_eq!(step.append_attempt_output(&lines, 3), 2);
        assert_eq!(
            step.output,
            template(&["status: running", "one", "exit status 1", "two", "three"])
        );
    }

    #[test]
    fn plan_retry_no_progress_clones_no_template_lines() {
        let mut step = Step::new("a", "A", &["a"], "/");
        let lines: Vec<String> = (0..100_000).map(|i| format!("line {i}")).collect();
        assert_eq!(step.append_attempt_output(&lines, 40_000), 40_000);
        let before = step.output.len();

        assert_eq!(step.append_attempt_output(&lines, 40_000), 0);
        assert_eq!(step.append_attempt_output(&lines, 10), 0);
        assert_eq!(step.output.len(), before);
        assert_eq!(step.append_attempt_output(&lines, 40_001), 1);
    }

    #[test]
    fn plan_retry_rejected_target_changes_no_attempt_state() {
        let mut p = running_plan();
        p.steps[0].state = StepState::Succeeded;
        p.steps[0].output.push("kept".into());
        p.steps[1].state = StepState::Succeeded;
        p.steps[1].fails = true;
        let before = p.clone();

        assert!(!p.retry(0), "a succeeded step is not retried");
        assert!(!p.retry(9), "an unknown step is not retried");
        assert_eq!(p, before);
        assert_eq!(p.steps[0].attempt, 1);
    }

    #[test]
    fn plan_retry_multiple_attempts_advance_each_cursor_from_zero() {
        let mut p = running_plan();
        assert!(
            p.steps.iter().all(|s| s.attempt == 1),
            "arm starts attempt 1"
        );
        p.steps[0].state = StepState::Failed;
        assert!(p.retry(0));
        assert_eq!(p.steps[0].attempt, 2);
        p.steps[0].state = StepState::Failed;
        assert!(p.retry(0));
        assert_eq!(p.steps[0].attempt, 3);
        assert_eq!(p.steps[1].attempt, 1, "a sibling keeps its attempt");

        p.steps[0].output.clear();
        let lines = template(&["x", "y"]);
        assert_eq!(p.steps[0].append_attempt_output(&lines, 2), 2);
        p.steps[0].state = StepState::Failed;
        assert!(p.retry(0));
        assert_eq!(
            p.steps[0].append_attempt_output(&template(&["fresh"]), 1),
            1,
            "a retry starts its own cursor at zero"
        );
    }
}
