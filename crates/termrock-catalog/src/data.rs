// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Copied from junie-tui src/bin/showcase/data.rs (MIT).

//! Demo/example data. Kept apart from components so catalog content can
//! change without touching rendering or interaction code.

/// Fixture tree node (page data, not a widget).
#[derive(Debug, Clone)]
pub struct TreeNode {
    pub label: String,
    pub children: Vec<TreeNode>,
    pub meta: Option<String>,
}

impl TreeNode {
    pub fn dir(label: &str, children: Vec<Self>) -> Self {
        Self {
            label: label.to_owned(),
            children,
            meta: None,
        }
    }
    pub fn leaf_meta(label: &str, meta: &str) -> Self {
        Self {
            label: label.to_owned(),
            children: vec![],
            meta: Some(meta.to_owned()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRow {
    pub id: u32,
    pub name: String,
    pub owner: String,
    pub status: TaskStatus,
    pub branch: String,
    pub changes: u32,
    pub duration_s: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TaskStatus {
    Queued,
    Running,
    Done,
    Failed,
    Paused,
}

pub fn tasks() -> Vec<TaskRow> {
    let raw: &[(&str, &str, TaskStatus, &str, u32, u32)] = &[
        (
            "Add rate limiting to auth endpoints",
            "mira",
            TaskStatus::Done,
            "feat/rate-limit",
            14,
            412,
        ),
        (
            "Migrate sessions table to UUID keys",
            "jonas",
            TaskStatus::Running,
            "chore/uuid-sessions",
            31,
            96,
        ),
        (
            "Fix flaky checkout integration test",
            "ana",
            TaskStatus::Failed,
            "fix/checkout-flake",
            3,
            58,
        ),
        (
            "Write release notes for 3.2",
            "mira",
            TaskStatus::Queued,
            "docs/release-3.2",
            0,
            0,
        ),
        (
            "Replace deprecated Vue mixins",
            "kai",
            TaskStatus::Done,
            "refactor/mixins",
            87,
            1330,
        ),
        (
            "Upgrade Postgres driver to 0.9",
            "jonas",
            TaskStatus::Paused,
            "chore/pg-driver",
            5,
            240,
        ),
        (
            "Extract billing service module",
            "sofia",
            TaskStatus::Done,
            "refactor/billing",
            52,
            908,
        ),
        (
            "Add OpenTelemetry tracing spans",
            "kai",
            TaskStatus::Running,
            "feat/otel",
            22,
            130,
        ),
        (
            "Remove legacy feature flags",
            "ana",
            TaskStatus::Queued,
            "chore/flags",
            0,
            0,
        ),
        (
            "Generate API client from OpenAPI",
            "sofia",
            TaskStatus::Done,
            "feat/api-client",
            118,
            2210,
        ),
        (
            "Harden CSP headers",
            "mira",
            TaskStatus::Done,
            "sec/csp",
            4,
            77,
        ),
        (
            "Speed up cold start of worker",
            "jonas",
            TaskStatus::Failed,
            "perf/worker-boot",
            9,
            601,
        ),
        (
            "Localize onboarding emails",
            "kai",
            TaskStatus::Queued,
            "feat/i18n-emails",
            0,
            0,
        ),
        (
            "Add pagination to audit log",
            "ana",
            TaskStatus::Done,
            "feat/audit-pages",
            16,
            344,
        ),
        (
            "Refactor retry helper into crate",
            "sofia",
            TaskStatus::Running,
            "refactor/retry",
            11,
            45,
        ),
        (
            "Rotate signing keys quarterly",
            "mira",
            TaskStatus::Queued,
            "sec/key-rotation",
            0,
            0,
        ),
        (
            "Fix timezone bug in scheduler",
            "jonas",
            TaskStatus::Done,
            "fix/tz-scheduler",
            7,
            188,
        ),
        (
            "Document webhook retry semantics",
            "kai",
            TaskStatus::Done,
            "docs/webhooks",
            2,
            65,
        ),
        (
            "Add dark mode to admin panel",
            "ana",
            TaskStatus::Paused,
            "feat/admin-dark",
            40,
            720,
        ),
        (
            "Bump minimum Node to 22",
            "sofia",
            TaskStatus::Queued,
            "chore/node-22",
            0,
            0,
        ),
        (
            "Cache dependency graph between runs",
            "mira",
            TaskStatus::Running,
            "perf/dep-cache",
            19,
            210,
        ),
        (
            "Clean up unused SQL views",
            "jonas",
            TaskStatus::Done,
            "chore/sql-views",
            12,
            155,
        ),
        (
            "Add health endpoint for gateway",
            "kai",
            TaskStatus::Done,
            "feat/health",
            3,
            42,
        ),
        (
            "Investigate memory growth in parser",
            "ana",
            TaskStatus::Running,
            "perf/parser-mem",
            6,
            380,
        ),
    ];
    raw.iter()
        .enumerate()
        .map(
            |(i, (name, owner, status, branch, changes, duration_s))| TaskRow {
                id: 1040 + i as u32,
                name: (*name).to_owned(),
                owner: (*owner).to_owned(),
                status: *status,
                branch: (*branch).to_owned(),
                changes: *changes,
                duration_s: *duration_s,
            },
        )
        .collect()
}

pub fn project_tree() -> Vec<TreeNode> {
    vec![
        TreeNode::dir(
            "src",
            vec![
                TreeNode::dir(
                    "api",
                    vec![
                        TreeNode::leaf_meta("auth.rs", "2.1 KB"),
                        TreeNode::leaf_meta("billing.rs", "6.4 KB"),
                        TreeNode::leaf_meta("mod.rs", "312 B"),
                        TreeNode::dir(
                            "webhooks",
                            vec![
                                TreeNode::leaf_meta("dispatch.rs", "3.9 KB"),
                                TreeNode::leaf_meta("retry.rs", "1.7 KB"),
                                TreeNode::leaf_meta("mod.rs", "180 B"),
                            ],
                        ),
                    ],
                ),
                TreeNode::dir(
                    "db",
                    vec![
                        TreeNode::leaf_meta("migrations.rs", "9.2 KB"),
                        TreeNode::leaf_meta("pool.rs", "1.1 KB"),
                        TreeNode::leaf_meta("schema.rs", "14.8 KB"),
                    ],
                ),
                TreeNode::dir(
                    "workers",
                    vec![
                        TreeNode::leaf_meta("scheduler.rs", "4.6 KB"),
                        TreeNode::leaf_meta("mailer.rs", "2.8 KB"),
                    ],
                ),
                TreeNode::leaf_meta("config.rs", "1.9 KB"),
                TreeNode::leaf_meta("lib.rs", "640 B"),
                TreeNode::leaf_meta("main.rs", "1.2 KB"),
            ],
        ),
        TreeNode::dir(
            "tests",
            vec![
                TreeNode::leaf_meta("checkout.rs", "5.3 KB"),
                TreeNode::leaf_meta("auth_flow.rs", "3.0 KB"),
                TreeNode::dir(
                    "fixtures",
                    vec![
                        TreeNode::leaf_meta("users.json", "18 KB"),
                        TreeNode::leaf_meta("orders.json", "44 KB"),
                    ],
                ),
            ],
        ),
        TreeNode::dir(
            "docs",
            vec![
                TreeNode::leaf_meta("architecture.md", "7.7 KB"),
                TreeNode::leaf_meta("webhooks.md", "2.2 KB"),
            ],
        ),
        TreeNode::leaf_meta("Cargo.toml", "1.4 KB"),
        TreeNode::leaf_meta("README.md", "3.5 KB"),
    ]
}

pub fn languages() -> Vec<&'static str> {
    vec![
        "Rust",
        "TypeScript",
        "Python",
        "Kotlin",
        "Go",
        "Java",
        "Swift",
        "C#",
        "Ruby",
        "Scala",
        "Elixir",
        "Haskell",
        "Zig",
        "Dart",
        "PHP",
        "C++",
        "Lua",
        "OCaml",
        "Clojure",
        "Erlang",
    ]
}

pub fn log_lines(n: usize) -> Vec<String> {
    let steps = [
        ("info", "Resolving workspace members"),
        ("info", "Fetching crates.io index"),
        ("info", "Compiling proc-macro2 v1.0.86"),
        ("info", "Compiling serde v1.0.210"),
        ("warn", "unused import: `std::fmt` in src/api/mod.rs:3"),
        ("info", "Compiling tokio v1.40.0"),
        ("info", "Running unittests src/lib.rs"),
        ("info", "test api::auth::tests::rejects_expired ... ok"),
        ("info", "test db::pool::tests::reuses_connections ... ok"),
        ("error", "test checkout::places_order ... FAILED"),
        (
            "info",
            "test workers::scheduler::tests::respects_timezone ... ok",
        ),
        ("info", "Linking target/debug/deps/app-4f2c1b"),
    ];
    (0..n)
        .map(|i| {
            let (level, msg) = steps[i % steps.len()];
            let secs = i as f64 * 0.37;
            format!("{secs:7.2}s  {level:<5}  {msg}")
        })
        .collect()
}

pub fn prose() -> &'static str {
    "Junie works through a task the way a careful engineer would: it reads the \
relevant code, forms a plan, makes focused changes, runs the tests, and reports \
back with a summary you can review before anything is merged.\n\n\
Each step is visible. You can pause, redirect, or take over at any point, and \
every change lands as an ordinary diff in your working tree.\n\n\
The design system in this prototype exists so that the terminal version of that \
experience feels as deliberate as the web version: quiet surfaces, one accent, \
clear focus, and no decoration that does not carry information.\n\n\
Scroll with the mouse wheel, PageUp/PageDown, or the arrow keys while this panel \
has focus. The scrollbar on the right shows where you are and how much remains."
}
