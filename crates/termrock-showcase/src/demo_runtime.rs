// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! The scripted agent behind the showcase.
//!
//! Not a TermRock type and never will be: the library is product-neutral, and
//! an agent runtime is a product. This is a demo driver that emits the same
//! event shapes a real one would, on a clock the host advances, so the
//! showcase proves the *UI* end to end without a provider, a network, or a
//! shell (SKD-2).

use std::collections::VecDeque;

/// One thing the scripted agent says happened.
///
/// Shaped after a streaming provider's event feed so that swapping a real
/// runtime in later is a translation layer, not a rewrite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DemoEvent {
    /// A token (or several) of assistant text.
    TextDelta {
        /// Text to append to the streaming assistant block.
        text: String,
    },
    /// A tool started running.
    ToolStart {
        /// Stable tool-run id.
        id: String,
        /// Tool name (`bash`, `edit`, …).
        name: String,
        /// One-line detail: the command, the path.
        detail: String,
    },
    /// A line of tool output.
    ToolStdout {
        /// Tool-run id.
        id: String,
        /// One output line.
        line: String,
    },
    /// A tool finished.
    ToolEnd {
        /// Tool-run id.
        id: String,
        /// Whether it succeeded.
        ok: bool,
    },
    /// The agent needs permission before it may continue.
    PermissionRequired {
        /// Request id.
        id: String,
        /// Tool being requested.
        tool: String,
        /// Scope (`workspace`, `network`, …).
        scope: String,
        /// The exact command, shown verbatim.
        command: String,
        /// Whether the action is high risk.
        high_risk: bool,
    },
    /// A plan is ready for review.
    PlanReady,
    /// A diff is ready for review.
    DiffReady,
    /// The agent has a question.
    Question,
    /// A subagent was spawned.
    SubagentSpawn {
        /// Subagent id.
        id: String,
        /// What it is doing.
        title: String,
    },
    /// The turn ended.
    Done,
}

/// Priority of an event under backpressure.
///
/// Text may coalesce or drop; everything that changes trust, state or safety
/// never may. This mirrors the coalescer's own vocabulary so the host can hand
/// it straight through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemoPriority {
    /// Coalescable prose.
    Normal,
    /// Structural: tools, tasks, reviews.
    High,
    /// Trust and terminal states.
    Critical,
}

impl DemoEvent {
    /// What this event may never lose to a full buffer.
    #[must_use]
    pub const fn priority(&self) -> DemoPriority {
        match self {
            Self::TextDelta { .. } => DemoPriority::Normal,
            Self::ToolStart { .. }
            | Self::ToolStdout { .. }
            | Self::ToolEnd { .. }
            | Self::PlanReady
            | Self::DiffReady
            | Self::Question
            | Self::SubagentSpawn { .. } => DemoPriority::High,
            Self::PermissionRequired { .. } | Self::Done => DemoPriority::Critical,
        }
    }
}

/// A named script the demo can run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scenario {
    /// Streams a paragraph and ends. The 20-second version of the pitch.
    HelloStream,
    /// Runs a tool, streams its output, ends green.
    ToolRun,
    /// Asks for a high-risk permission before touching anything.
    PermissionHigh,
    /// Produces a plan for review.
    PlanBuild,
    /// Produces a diff for review.
    DiffReview,
    /// Asks a clarifying question.
    Question,
    /// Spawns two subagents that report in.
    MultiSubagent,
}

impl Scenario {
    /// Every scenario, in demo order.
    pub const ALL: [Self; 7] = [
        Self::HelloStream,
        Self::ToolRun,
        Self::PermissionHigh,
        Self::PlanBuild,
        Self::DiffReview,
        Self::Question,
        Self::MultiSubagent,
    ];

    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::HelloStream => "hello-stream",
            Self::ToolRun => "tool-run",
            Self::PermissionHigh => "permission-high",
            Self::PlanBuild => "plan-build",
            Self::DiffReview => "diff-review",
            Self::Question => "question",
            Self::MultiSubagent => "multi-subagent",
        }
    }

    /// One-line label for the palette.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::HelloStream => "Stream a reply",
            Self::ToolRun => "Run a tool",
            Self::PermissionHigh => "Ask for a high-risk permission",
            Self::PlanBuild => "Propose a plan",
            Self::DiffReview => "Propose a diff",
            Self::Question => "Ask a question",
            Self::MultiSubagent => "Spawn subagents",
        }
    }
}

/// One scheduled event.
#[derive(Debug, Clone)]
struct Scheduled {
    at_ms: u64,
    event: DemoEvent,
}

/// The scripted agent.
#[derive(Debug, Default)]
pub struct DemoRuntime {
    queue: VecDeque<Scheduled>,
    started_ms: u64,
    /// Paused while the host reports backpressure or a trust gate is open.
    paused: bool,
}

impl DemoRuntime {
    /// A runtime with nothing scheduled.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether a turn is in flight.
    #[must_use]
    pub fn is_busy(&self) -> bool {
        !self.queue.is_empty()
    }

    /// Pauses or resumes the script.
    ///
    /// The host pauses while a permission is open: an agent that keeps
    /// streaming behind a modal is exactly the behaviour the trust surface
    /// exists to prevent.
    pub const fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    /// Drops everything still scheduled (Esc, cancel, new turn).
    pub fn cancel(&mut self) {
        self.queue.clear();
    }

    /// Schedules `scenario` starting from `now_ms`.
    pub fn start(&mut self, scenario: Scenario, prompt: &str, now_ms: u64) {
        self.queue.clear();
        self.started_ms = now_ms;
        self.paused = false;
        let mut at = 0u64;
        let mut push = |queue: &mut VecDeque<Scheduled>, gap: u64, event: DemoEvent| {
            at += gap;
            queue.push_back(Scheduled { at_ms: at, event });
        };

        match scenario {
            Scenario::HelloStream => {
                for word in reply_for(prompt) {
                    push(
                        &mut self.queue,
                        45,
                        DemoEvent::TextDelta {
                            text: format!("{word} "),
                        },
                    );
                }
                push(&mut self.queue, 120, DemoEvent::Done);
            }
            Scenario::ToolRun => {
                push(
                    &mut self.queue,
                    60,
                    DemoEvent::TextDelta {
                        text: "Running the test suite. ".into(),
                    },
                );
                push(
                    &mut self.queue,
                    120,
                    DemoEvent::ToolStart {
                        id: "t1".into(),
                        name: "bash".into(),
                        detail: "cargo test -p termrock --lib".into(),
                    },
                );
                for line in [
                    "   Compiling termrock v0.11.0",
                    "    Finished test profile in 8.42s",
                    "running 3090 tests",
                    "test result: ok. 3090 passed; 0 failed",
                ] {
                    push(
                        &mut self.queue,
                        180,
                        DemoEvent::ToolStdout {
                            id: "t1".into(),
                            line: line.into(),
                        },
                    );
                }
                push(
                    &mut self.queue,
                    140,
                    DemoEvent::ToolEnd {
                        id: "t1".into(),
                        ok: true,
                    },
                );
                push(
                    &mut self.queue,
                    80,
                    DemoEvent::TextDelta {
                        text: "All green.".into(),
                    },
                );
                push(&mut self.queue, 100, DemoEvent::Done);
            }
            Scenario::PermissionHigh => {
                push(
                    &mut self.queue,
                    60,
                    DemoEvent::TextDelta {
                        text: "This one needs your approval. ".into(),
                    },
                );
                push(
                    &mut self.queue,
                    150,
                    DemoEvent::PermissionRequired {
                        id: "p1".into(),
                        tool: "bash".into(),
                        scope: "workspace".into(),
                        command: "rm -rf target/debug/incremental".into(),
                        high_risk: true,
                    },
                );
            }
            Scenario::PlanBuild => {
                push(
                    &mut self.queue,
                    60,
                    DemoEvent::TextDelta {
                        text: "Here is the plan. ".into(),
                    },
                );
                push(&mut self.queue, 150, DemoEvent::PlanReady);
            }
            Scenario::DiffReview => {
                push(
                    &mut self.queue,
                    60,
                    DemoEvent::TextDelta {
                        text: "One file changed. ".into(),
                    },
                );
                push(&mut self.queue, 150, DemoEvent::DiffReady);
            }
            Scenario::Question => {
                push(&mut self.queue, 120, DemoEvent::Question);
            }
            Scenario::MultiSubagent => {
                push(
                    &mut self.queue,
                    80,
                    DemoEvent::SubagentSpawn {
                        id: "s1".into(),
                        title: "audit widgets".into(),
                    },
                );
                push(
                    &mut self.queue,
                    120,
                    DemoEvent::SubagentSpawn {
                        id: "s2".into(),
                        title: "audit patterns".into(),
                    },
                );
                push(
                    &mut self.queue,
                    260,
                    DemoEvent::TextDelta {
                        text: "Two subagents are working. ".into(),
                    },
                );
                push(&mut self.queue, 400, DemoEvent::Done);
            }
        }
    }

    /// Events due at `now_ms`, in order.
    ///
    /// While paused, prose waits but trust and terminal events still arrive:
    /// a permission that cannot be delivered because an earlier permission is
    /// open would be a queue that loses safety-critical work
    /// (the SoT's backpressure table).
    pub fn drain_due(&mut self, now_ms: u64) -> Vec<DemoEvent> {
        let elapsed = now_ms.saturating_sub(self.started_ms);
        let mut out = Vec::new();
        if self.paused {
            // Prose keeps its place in the queue; trust and terminal events
            // are lifted out of it. Skipping the prose would drop it silently.
            let mut index = 0;
            while index < self.queue.len() {
                let entry = &self.queue[index];
                if entry.at_ms > elapsed {
                    break;
                }
                if entry.event.priority() == DemoPriority::Critical {
                    out.push(self.queue.remove(index).expect("index in range").event);
                } else {
                    index += 1;
                }
            }
            return out;
        }
        while let Some(front) = self.queue.front() {
            if front.at_ms > elapsed {
                break;
            }
            out.push(self.queue.pop_front().expect("front exists").event);
        }
        out
    }

    /// When the next event is due, in runner milliseconds.
    #[must_use]
    pub fn next_due_ms(&self) -> Option<u64> {
        let next = if self.paused {
            self.queue
                .iter()
                .find(|entry| entry.event.priority() == DemoPriority::Critical)?
        } else {
            self.queue.front()?
        };
        Some(self.started_ms.saturating_add(next.at_ms))
    }
}

/// The scripted reply, seeded by what the operator typed.
fn reply_for(prompt: &str) -> Vec<&'static str> {
    let mut words = vec![
        "Reading",
        "the",
        "workspace",
        "—",
        "TermRock",
        "paints",
        "every",
        "surface",
        "here",
        "through",
        "public",
        "widgets",
        "only,",
        "so",
        "what",
        "you",
        "see",
        "is",
        "what",
        "a",
        "consumer",
        "gets.",
    ];
    if prompt.to_lowercase().contains("test") {
        words = vec![
            "The", "suite", "runs", "green:", "3090", "tests,", "no", "skips.",
        ];
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scenario_delivers_its_events_in_order_on_the_clock() {
        let mut runtime = DemoRuntime::new();
        runtime.start(Scenario::ToolRun, "run the tests", 1_000);
        assert!(runtime.is_busy());
        // Nothing is due before its scheduled time.
        assert!(runtime.drain_due(1_000).is_empty());
        let due = runtime.next_due_ms().expect("something is scheduled");
        let events = runtime.drain_due(due);
        assert!(matches!(events.first(), Some(DemoEvent::TextDelta { .. })));

        // Draining to the end delivers the terminal event exactly once.
        let mut seen_done = 0;
        for step in 0..200 {
            for event in runtime.drain_due(1_000 + step * 50) {
                if matches!(event, DemoEvent::Done) {
                    seen_done += 1;
                }
            }
        }
        assert_eq!(seen_done, 1);
        assert!(!runtime.is_busy());
    }

    #[test]
    fn a_paused_runtime_holds_prose_but_never_holds_trust() {
        let mut runtime = DemoRuntime::new();
        runtime.start(Scenario::HelloStream, "hello", 0);
        runtime.set_paused(true);
        let while_paused = runtime.drain_due(10_000);
        assert!(
            !while_paused
                .iter()
                .any(|event| matches!(event, DemoEvent::TextDelta { .. })),
            "prose waits: {while_paused:?}"
        );
        assert!(
            while_paused.contains(&DemoEvent::Done),
            "a terminal event is never withheld: {while_paused:?}"
        );
        runtime.set_paused(false);
        assert!(
            runtime
                .drain_due(10_000)
                .iter()
                .any(|event| matches!(event, DemoEvent::TextDelta { .. })),
            "the held prose arrives when the gate clears"
        );

        // A trust request arrives even while the runtime is held.
        let mut gated = DemoRuntime::new();
        gated.start(Scenario::PermissionHigh, "delete it", 0);
        gated.set_paused(true);
        let delivered = gated.drain_due(10_000);
        assert!(
            delivered
                .iter()
                .any(|event| matches!(event, DemoEvent::PermissionRequired { .. })),
            "a permission is never withheld: {delivered:?}"
        );
    }

    #[test]
    fn trust_and_terminal_events_are_never_droppable() {
        let critical = DemoEvent::PermissionRequired {
            id: "p".into(),
            tool: "bash".into(),
            scope: "workspace".into(),
            command: "rm -rf /".into(),
            high_risk: true,
        };
        assert_eq!(critical.priority(), DemoPriority::Critical);
        assert_eq!(DemoEvent::Done.priority(), DemoPriority::Critical);
        assert_eq!(
            DemoEvent::TextDelta { text: "x".into() }.priority(),
            DemoPriority::Normal
        );
    }
}
