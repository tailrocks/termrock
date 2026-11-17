use std::sync::Arc;

use super::pane::{AgentProcess, SessionScript, Step};
use crate::domain::agent::Agent;
use crate::domain::instance::AgentState;
use termrock::theme::Tone;
use termrock::widgets::viewport::{Line, Span};

fn sp(text: &str, tone: Tone) -> Span {
    Span::new(text, tone)
}

fn line(text: &str, tone: Tone) -> Line {
    vec![sp(text, tone)]
}

fn bold(text: &str) -> Line {
    vec![sp(text, Tone::Normal).bold()]
}

fn muted(text: &str) -> Line {
    line(text, Tone::Muted)
}

fn err(text: &str) -> Line {
    line(text, Tone::Error)
}

fn mixed(parts: &[(&str, Tone)]) -> Line {
    parts.iter().map(|(t, tone)| sp(t, *tone)).collect()
}

fn e(delay: i64, l: Line) -> Step {
    Step::Emit(delay, l)
}

pub fn script(agent: Option<Agent>, workspace: &str) -> Vec<Step> {
    let ws = format!("~/{workspace}");
    match agent {
        Some(Agent::ClaudeCode) => vec![
            Step::State(AgentState::Working),
            e(
                0,
                mixed(&[
                    ("▐ ", Tone::Success),
                    ("Claude Code v2.1.14", Tone::Normal),
                    (" · Opus 4.5 · ", Tone::Muted),
                    (&ws, Tone::Secondary),
                ]),
            ),
            e(200, vec![]),
            e(
                300,
                mixed(&[
                    ("› ", Tone::Muted),
                    (
                        "Refactor the settlement retry loop so failed batches",
                        Tone::Normal,
                    ),
                ]),
            ),
            e(
                60,
                line(
                    "  back off exponentially and cap at 5 attempts.",
                    Tone::Normal,
                ),
            ),
            e(400, vec![]),
            e(
                500,
                mixed(&[
                    ("● ", Tone::Secondary),
                    ("I'll read the retry loop first.", Tone::Normal),
                ]),
            ),
            e(500, vec![]),
            e(
                600,
                mixed(&[
                    ("● ", Tone::Secondary),
                    ("Read ", Tone::Normal),
                    ("src/settlement/retry.rs", Tone::Secondary),
                    (" (142 lines)", Tone::Muted),
                ]),
            ),
            e(
                400,
                mixed(&[
                    ("● ", Tone::Secondary),
                    ("Read ", Tone::Normal),
                    ("src/settlement/mod.rs", Tone::Secondary),
                    (" (88 lines)", Tone::Muted),
                ]),
            ),
            e(700, vec![]),
            e(
                200,
                mixed(&[
                    ("● ", Tone::Secondary),
                    (
                        "The loop retries with a fixed 3 attempts. I'll add",
                        Tone::Normal,
                    ),
                ]),
            ),
            e(
                60,
                line(
                    "  exponential backoff with jitter and cap it at 5.",
                    Tone::Normal,
                ),
            ),
            e(800, vec![]),
            e(
                200,
                mixed(&[
                    ("● ", Tone::Secondary),
                    ("Edit ", Tone::Normal),
                    ("src/settlement/retry.rs", Tone::Secondary),
                ]),
            ),
            Step::Touch("src/settlement/retry.rs"),
            e(
                200,
                mixed(&[
                    ("  +  ", Tone::Success),
                    ("const MAX_ATTEMPTS: u32 = 5;", Tone::Secondary),
                ]),
            ),
            e(
                120,
                mixed(&[
                    ("  +  ", Tone::Success),
                    (
                        "let delay = BASE * 2u32.pow(attempt) + jitter();",
                        Tone::Secondary,
                    ),
                ]),
            ),
            e(
                120,
                mixed(&[
                    ("  -  ", Tone::Error),
                    ("for attempt in 0..3 {", Tone::Muted),
                ]),
            ),
            e(
                120,
                mixed(&[
                    ("  +  ", Tone::Success),
                    ("for attempt in 0..MAX_ATTEMPTS {", Tone::Secondary),
                ]),
            ),
            e(900, vec![]),
            e(
                200,
                mixed(&[
                    ("● ", Tone::Secondary),
                    ("Bash ", Tone::Normal),
                    ("cargo test -p settlement retry", Tone::Secondary),
                ]),
            ),
            e(1400, muted("  running 6 tests … 6 passed (1.42 s)")),
            e(600, vec![]),
            e(
                200,
                mixed(&[
                    ("● ", Tone::Secondary),
                    ("Edit ", Tone::Normal),
                    ("src/settlement/mod.rs", Tone::Secondary),
                ]),
            ),
            Step::Touch("src/settlement/mod.rs"),
            e(
                200,
                mixed(&[
                    ("  +  ", Tone::Success),
                    (
                        "pub use retry::{MAX_ATTEMPTS, RetryPolicy};",
                        Tone::Secondary,
                    ),
                ]),
            ),
            e(800, vec![]),
            e(
                200,
                mixed(&[
                    ("● ", Tone::Secondary),
                    (
                        "Retries now back off 250 ms → 4 s, capped at 5 tries.",
                        Tone::Normal,
                    ),
                ]),
            ),
            e(
                80,
                line(
                    "  One more edit: expose the policy in settlement config.",
                    Tone::Normal,
                ),
            ),
            e(600, vec![]),
            e(
                300,
                mixed(&[
                    ("▶ ", Tone::Warning),
                    (
                        "Allow edit to src/settlement/config.rs? (y/n)",
                        Tone::Normal,
                    ),
                ]),
            ),
            Step::State(AgentState::Blocked),
            Step::Await,
        ],
        Some(Agent::Codex) => vec![
            Step::State(AgentState::Working),
            e(
                0,
                mixed(&[
                    ("OpenAI Codex v0.48", Tone::Normal),
                    (" · gpt-5.5-codex · ", Tone::Muted),
                    (&ws, Tone::Secondary),
                ]),
            ),
            e(200, vec![]),
            e(
                300,
                mixed(&[
                    ("› ", Tone::Muted),
                    (
                        "run the ledger integration tests and summarise failures",
                        Tone::Normal,
                    ),
                ]),
            ),
            e(500, vec![]),
            e(
                300,
                mixed(&[
                    ("• exec  ", Tone::Secondary),
                    ("cargo test -p ledger --test integration", Tone::Normal),
                ]),
            ),
            e(700, muted("  running 24 tests")),
            e(500, muted("  test reconcile::daily_close ........... ok")),
            e(
                400,
                mixed(&[
                    ("  test reconcile::multi_currency ........ ", Tone::Muted),
                    ("FAILED", Tone::Error),
                ]),
            ),
            e(400, muted("  test settle::partial_refund ........... ok")),
            e(
                900,
                mixed(&[
                    ("  22 passed · ", Tone::Muted),
                    ("1 failed", Tone::Error),
                    (" · 1 ignored (6.8 s)", Tone::Muted),
                ]),
            ),
            e(600, vec![]),
            e(
                300,
                mixed(&[
                    ("• ", Tone::Secondary),
                    ("1 failure: reconcile::multi_currency", Tone::Normal),
                ]),
            ),
            e(
                200,
                line("  expected 1,204.50 EUR, got 1,204.49 EUR", Tone::Normal),
            ),
            e(
                200,
                mixed(&[
                    ("  rounding precedes FX conversion in ", Tone::Normal),
                    ("ledger/fx.rs:71", Tone::Secondary),
                ]),
            ),
            e(700, vec![]),
            e(
                200,
                mixed(&[
                    ("○ ", Tone::Secondary),
                    ("Done · 38 s · 12.4k tokens", Tone::Muted),
                ]),
            ),
            Step::State(AgentState::Done),
            e(300, vec![]),
            Step::Await,
        ],
        Some(Agent::Amp) => vec![
            Step::State(AgentState::Working),
            e(
                0,
                mixed(&[
                    ("Amp 1.9.3", Tone::Normal),
                    (" · ", Tone::Muted),
                    (&ws, Tone::Secondary),
                ]),
            ),
            e(200, vec![]),
            e(
                300,
                mixed(&[
                    ("› ", Tone::Muted),
                    ("Why is the controller reconcile loop hot?", Tone::Normal),
                ]),
            ),
            e(500, vec![]),
            e(
                300,
                mixed(&[
                    ("⠿ ", Tone::Success),
                    ("Searching kube/controllers/**/*.go", Tone::Muted),
                ]),
            ),
            e(
                600,
                mixed(&[
                    ("⠿ ", Tone::Success),
                    ("Read kube/controllers/node_pool.go:118-164", Tone::Muted),
                ]),
            ),
            e(800, vec![]),
            e(
                200,
                line(
                    "The loop re-queues every object on every informer resync",
                    Tone::Normal,
                ),
            ),
            e(
                80,
                line(
                    "(resyncPeriod = 30s) instead of only on spec changes.",
                    Tone::Normal,
                ),
            ),
            e(600, vec![]),
            e(200, line("Suggested change:", Tone::Normal)),
            e(
                150,
                mixed(&[
                    ("  - ", Tone::Error),
                    (
                        ".WithEventFilter(predicate.ResourceVersionChangedPredicate{})",
                        Tone::Muted,
                    ),
                ]),
            ),
            e(
                150,
                mixed(&[
                    ("  + ", Tone::Success),
                    (
                        ".WithEventFilter(predicate.GenerationChangedPredicate{})",
                        Tone::Secondary,
                    ),
                ]),
            ),
            e(600, vec![]),
            e(
                200,
                mixed(&[
                    ("Apply this edit? ", Tone::Normal),
                    ("[Y/n]", Tone::Warning),
                ]),
            ),
            Step::State(AgentState::Blocked),
            Step::Await,
        ],
        Some(Agent::KimiCode) => vec![
            Step::State(AgentState::Working),
            e(
                0,
                mixed(&[
                    ("Kimi Code 0.7.2", Tone::Normal),
                    (" · kimi-k2 · ", Tone::Muted),
                    (&ws, Tone::Secondary),
                ]),
            ),
            e(200, vec![]),
            e(
                300,
                mixed(&[
                    ("› ", Tone::Muted),
                    ("add loading skeletons to the invoices table", Tone::Normal),
                ]),
            ),
            e(500, vec![]),
            e(300, bold("▸ Plan")),
            e(
                150,
                line("  1. Add <SkeletonRow/> in components/table/", Tone::Normal),
            ),
            e(
                150,
                line("  2. Render 8 rows while `isLoading`", Tone::Normal),
            ),
            e(150, line("  3. Story for the loading state", Tone::Normal)),
            e(700, vec![]),
            e(
                300,
                mixed(&[
                    ("▸ ", Tone::Secondary),
                    (
                        "Wrote components/table/SkeletonRow.tsx (41 lines)",
                        Tone::Normal,
                    ),
                ]),
            ),
            Step::Touch("components/table/SkeletonRow.tsx"),
            e(
                500,
                mixed(&[
                    ("▸ ", Tone::Secondary),
                    ("Edited pages/invoices/index.tsx (+9 −2)", Tone::Normal),
                ]),
            ),
            e(
                500,
                mixed(&[
                    ("▸ ", Tone::Secondary),
                    (
                        "Wrote components/table/SkeletonRow.stories.tsx",
                        Tone::Normal,
                    ),
                ]),
            ),
            e(600, vec![]),
            e(
                200,
                mixed(&[("✔ ", Tone::Success), ("Done · 3 files", Tone::Muted)]),
            ),
            Step::State(AgentState::Done),
            Step::Await,
        ],
        Some(Agent::OpenCode) => vec![
            Step::State(AgentState::Working),
            e(
                0,
                mixed(&[
                    ("opencode 0.5.11", Tone::Normal),
                    (" · ", Tone::Muted),
                    (&ws, Tone::Secondary),
                ]),
            ),
            e(200, vec![]),
            e(
                300,
                mixed(&[
                    ("> ", Tone::Muted),
                    (
                        "bump the release workflow to node 22 and pin actions",
                        Tone::Normal,
                    ),
                ]),
            ),
            e(500, vec![]),
            e(
                300,
                mixed(&[
                    ("┃ read  ", Tone::Secondary),
                    (".github/workflows/release.yml", Tone::Normal),
                ]),
            ),
            e(
                500,
                mixed(&[
                    ("┃ edit  ", Tone::Secondary),
                    (".github/workflows/release.yml", Tone::Normal),
                ]),
            ),
            Step::Touch(".github/workflows/release.yml"),
            e(
                150,
                mixed(&[
                    ("┃   - ", Tone::Error),
                    ("uses: actions/setup-node@v3", Tone::Muted),
                ]),
            ),
            e(
                150,
                mixed(&[
                    ("┃   + ", Tone::Success),
                    ("uses: actions/setup-node@v4.0.3", Tone::Secondary),
                ]),
            ),
            e(
                150,
                mixed(&[("┃   -   ", Tone::Error), ("node-version: 18", Tone::Muted)]),
            ),
            e(
                150,
                mixed(&[
                    ("┃   +   ", Tone::Success),
                    ("node-version: 22", Tone::Secondary),
                ]),
            ),
            e(
                500,
                mixed(&[
                    ("┃ edit  ", Tone::Secondary),
                    (".github/workflows/publish.yml (same pin)", Tone::Normal),
                ]),
            ),
            e(
                500,
                mixed(&[
                    ("┃ bash  ", Tone::Secondary),
                    ("act -j release --dryrun", Tone::Normal),
                ]),
            ),
            e(
                1200,
                mixed(&[
                    ("┃       ", Tone::Secondary),
                    ("✓ dry run ok (0 errors, 1 warning)", Tone::Muted),
                ]),
            ),
            e(600, vec![]),
            e(
                200,
                mixed(&[
                    ("Two workflows updated. ", Tone::Normal),
                    ("Warning: cache key still says node18.", Tone::Warning),
                ]),
            ),
            Step::State(AgentState::Idle),
            Step::Await,
        ],
        Some(Agent::GrokBuild) => vec![
            Step::State(AgentState::Working),
            e(
                0,
                mixed(&[
                    ("Grok Build 0.3", Tone::Normal),
                    (" · grok-4-code · ", Tone::Muted),
                    (&ws, Tone::Secondary),
                ]),
            ),
            e(200, vec![]),
            e(
                300,
                mixed(&[
                    ("> ", Tone::Muted),
                    (
                        "generate terraform for a private GKE node pool",
                        Tone::Normal,
                    ),
                ]),
            ),
            e(500, vec![]),
            e(
                300,
                mixed(&[
                    ("⟩ ", Tone::Secondary),
                    ("writing modules/gke/node_pool.tf", Tone::Normal),
                ]),
            ),
            Step::Touch("modules/gke/node_pool.tf"),
            e(
                500,
                mixed(&[
                    ("⟩ ", Tone::Secondary),
                    ("writing modules/gke/variables.tf", Tone::Normal),
                ]),
            ),
            e(
                700,
                mixed(&[
                    ("⟩ ", Tone::Secondary),
                    ("terraform fmt -check … ok", Tone::Muted),
                ]),
            ),
            e(
                700,
                mixed(&[
                    ("⟩ ", Tone::Secondary),
                    ("terraform validate … ok", Tone::Muted),
                ]),
            ),
            e(600, vec![]),
            e(
                200,
                line(
                    "Created 2 files (118 lines). Private nodes, 3 zones,",
                    Tone::Normal,
                ),
            ),
            e(80, line("e2-standard-8, autoscaling 1–6.", Tone::Normal)),
            Step::State(AgentState::Done),
            Step::Await,
        ],
        Some(Agent::MuseCode)
        | Some(Agent::Antigravity)
        | Some(Agent::MinimaxCode)
        | Some(Agent::MimoCode)
        | Some(Agent::Omp) => vec![
            Step::State(AgentState::Working),
            e(0, mixed(&[("agent ready", Tone::Normal)])),
            Step::State(AgentState::Done),
            Step::Await,
        ],
        None => {
            let p = format!("{workspace} ❯ ");
            vec![
                e(
                    0,
                    mixed(&[(&p, Tone::Secondary), ("git status -sb", Tone::Normal)]),
                ),
                e(
                    300,
                    muted(
                        "## feature/settlement-backoff…origin/feature/settlement-backoff [ahead 2]",
                    ),
                ),
                e(
                    60,
                    mixed(&[
                        (" M ", Tone::Warning),
                        ("src/settlement/retry.rs", Tone::Normal),
                    ]),
                ),
                e(
                    60,
                    mixed(&[
                        (" M ", Tone::Warning),
                        ("src/settlement/mod.rs", Tone::Normal),
                    ]),
                ),
                e(
                    60,
                    mixed(&[
                        ("?? ", Tone::Muted),
                        ("docs/adr/0007-retry-backoff.md", Tone::Normal),
                    ]),
                ),
                e(
                    900,
                    mixed(&[
                        (&p, Tone::Secondary),
                        ("cargo clippy -p settlement", Tone::Normal),
                    ]),
                ),
                e(
                    500,
                    muted("    Checking settlement v0.9.2 (crates/settlement)"),
                ),
                e(
                    1500,
                    muted("    Finished dev [unoptimized] target(s) in 3.12s"),
                ),
                e(
                    700,
                    mixed(&[(&p, Tone::Secondary), ("ls docs/adr", Tone::Normal)]),
                ),
                e(
                    200,
                    line(
                        "0001-record-architecture.md  0004-ledger-precision.md",
                        Tone::Normal,
                    ),
                ),
                e(
                    40,
                    line(
                        "0002-settlement-batches.md   0005-fx-rounding.md",
                        Tone::Normal,
                    ),
                ),
                e(
                    40,
                    line(
                        "0003-retry-policy.md         0007-retry-backoff.md",
                        Tone::Normal,
                    ),
                ),
                Step::Await,
            ]
        }
    }
}

fn replies(agent: Option<Agent>) -> Vec<Vec<Line>> {
    match agent {
        Some(Agent::ClaudeCode) => vec![
            vec![
                mixed(&[
                    ("● ", Tone::Secondary),
                    ("Read src/settlement/config.rs (61 lines)", Tone::Normal),
                ]),
                mixed(&[
                    ("● ", Tone::Secondary),
                    (
                        "The policy is already wired; I'll add a test for the cap.",
                        Tone::Normal,
                    ),
                ]),
                mixed(&[
                    ("● ", Tone::Secondary),
                    ("Bash cargo test -p settlement cap", Tone::Normal),
                ]),
                muted("  running 1 test … 1 passed (0.31 s)"),
                mixed(&[
                    ("○ ", Tone::Secondary),
                    ("Done · 1 file changed", Tone::Muted),
                ]),
            ],
            vec![
                mixed(&[
                    ("● ", Tone::Secondary),
                    ("Grep MAX_ATTEMPTS src/ (3 matches)", Tone::Normal),
                ]),
                mixed(&[
                    ("● ", Tone::Secondary),
                    (
                        "All callers use the constant; nothing else references 3.",
                        Tone::Normal,
                    ),
                ]),
                mixed(&[("○ ", Tone::Secondary), ("Done", Tone::Muted)]),
            ],
            vec![
                mixed(&[
                    ("● ", Tone::Secondary),
                    ("I can also open a PR with these changes.", Tone::Normal),
                ]),
                mixed(&[
                    ("▶ ", Tone::Warning),
                    ("Run gh pr create? (y/n)", Tone::Normal),
                ]),
            ],
        ],
        Some(Agent::Codex) => vec![
            vec![
                mixed(&[
                    ("• exec  ", Tone::Secondary),
                    (
                        "cargo test -p ledger reconcile::multi_currency",
                        Tone::Normal,
                    ),
                ]),
                muted("  running 1 test … ok (0.9 s)"),
                mixed(&[
                    ("○ ", Tone::Secondary),
                    ("Done · 4 s · 1.1k tokens", Tone::Muted),
                ]),
            ],
            vec![
                mixed(&[
                    ("• ", Tone::Secondary),
                    (
                        "Patched ledger/fx.rs:71 to round after conversion.",
                        Tone::Normal,
                    ),
                ]),
                mixed(&[
                    ("○ ", Tone::Secondary),
                    ("Done · 6 s · 2.4k tokens", Tone::Muted),
                ]),
            ],
        ],
        None => vec![],
        _ => vec![
            vec![
                line("Understood. Working on it.", Tone::Normal),
                mixed(&[("○ ", Tone::Secondary), ("Done", Tone::Muted)]),
            ],
            vec![line("Nothing else to change here.", Tone::Normal)],
        ],
    }
}

#[derive(Debug)]
pub struct SimScripts;

impl SimScripts {
    pub fn shared() -> Arc<dyn SessionScript> {
        Arc::new(SimScripts)
    }
}

impl SessionScript for SimScripts {
    fn script(&self, agent: Option<Agent>, workspace: &str) -> Vec<Step> {
        script(agent, workspace)
    }

    fn respond(
        &self,
        proc: &mut AgentProcess,
        input: &str,
        now_ms: i64,
        workspace: &str,
    ) -> Vec<Line> {
        let mut out = vec![];
        let is_shell = proc.agent.is_none();
        if !is_shell {
            out.push(mixed(&[
                (&proc.prompt, Tone::Secondary),
                (input, Tone::Normal),
            ]));
            if proc.awaiting_permission {
                proc.awaiting_permission = false;
                let yes = input.trim().eq_ignore_ascii_case("y")
                    || input.trim().eq_ignore_ascii_case("yes");
                if yes {
                    proc.state = AgentState::Working;
                    proc.reply_queue.push_back((
                        now_ms + 400,
                        mixed(&[
                            ("● ", Tone::Secondary),
                            ("Edit src/settlement/config.rs", Tone::Normal),
                        ]),
                    ));
                    proc.touched.push("src/settlement/config.rs".into());
                    proc.reply_queue.push_back((
                        now_ms + 900,
                        mixed(&[
                            ("  +  ", Tone::Success),
                            ("pub retry: RetryPolicy,", Tone::Secondary),
                        ]),
                    ));
                    proc.reply_queue.push_back((
                        now_ms + 1500,
                        mixed(&[
                            ("○ ", Tone::Secondary),
                            ("Done · 3 files changed", Tone::Muted),
                        ]),
                    ));
                } else {
                    proc.state = AgentState::Idle;
                    proc.reply_queue
                        .push_back((now_ms + 300, muted("Skipped.")));
                }
                return out;
            }
            proc.state = AgentState::Working;
            out.push(mixed(&[("⠋ ", Tone::Success), ("Thinking…", Tone::Muted)]));
            let rs = replies(proc.agent);
            if !rs.is_empty() {
                let r = &rs[proc.reply_cursor % rs.len()];
                proc.reply_cursor += 1;
                let mut t = now_ms + 700;
                proc.reply_queue.push_back((
                    t,
                    mixed(&[
                        ("↳ ", Tone::Muted),
                        (&format!("\"{}\"", input.trim()), Tone::Muted),
                    ]),
                ));
                for l in r {
                    t += 350;
                    proc.reply_queue.push_back((t, l.clone()));
                }
                if r.iter().any(|l| l.iter().any(|s| s.text.contains("(y/n)"))) {
                    proc.awaiting_permission = true;
                    proc.state = AgentState::Blocked;
                }
            }
            return out;
        }
        let p = proc.prompt.clone();
        out.push(mixed(&[(&p, Tone::Secondary), (input, Tone::Normal)]));
        let cmd = input.trim();
        let mut words = cmd.split_whitespace();
        match words.next() {
            Some("ls") => {
                out.push(line(
                    "Cargo.toml   crates/   docs/   scripts/   README.md",
                    Tone::Normal,
                ));
            }
            Some("pwd") => out.push(line(&format!("/workspace/{workspace}"), Tone::Normal)),
            Some("git") => {
                match words.next() {
                    Some("status") => {
                        out.push(muted("## feature/settlement-backoff…origin/feature/settlement-backoff [ahead 2]"));
                        for t in &proc.touched {
                            out.push(mixed(&[(" M ", Tone::Warning), (t, Tone::Normal)]));
                        }
                    }
                    Some("push") => {
                        out.push(muted("To github.com:chainargos/payments-platform.git"));
                        out.push(mixed(&[
                            ("   9c41e2f..3f8a1d0  ", Tone::Muted),
                            (
                                "feature/settlement-backoff -> feature/settlement-backoff",
                                Tone::Normal,
                            ),
                        ]));
                    }
                    Some("log") => {
                        out.push(mixed(&[
                            ("3f8a1d0 ", Tone::Secondary),
                            ("settlement: exponential backoff (#482)", Tone::Normal),
                        ]));
                        out.push(mixed(&[
                            ("9c41e2f ", Tone::Secondary),
                            ("ledger: fx rounding order", Tone::Normal),
                        ]));
                    }
                    _ => out.push(muted("usage: git <command> [<args>]")),
                }
            }
            Some("cargo") => {
                out.push(muted("    Checking settlement v0.9.2 (crates/settlement)"));
                out.push(muted("    Finished dev [unoptimized] target(s) in 2.04s"));
            }
            Some("clear") => {
                out.push(vec![sp("\u{0}clear", Tone::Normal)]);
            }
            Some("") | None => {}
            Some(other) => out.push(err(&format!("zsh: command not found: {other}"))),
        }
        out
    }
}
