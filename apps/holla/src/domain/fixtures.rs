use std::collections::BTreeMap;

use crate::domain::activity::ActivityKind;
use crate::domain::context::{Host, HostRole, Location, Os, Project, ProjectKind, ScopeTag};
use crate::domain::plan::{Plan, Step};
use crate::domain::scripts;
use crate::domain::stack::*;
use crate::scenario::{Motion, Scenario};
use crate::sim::fs::{BLOCK, Fs};
use crate::sim::world::{Persisted, Source, World};
use std::collections::BTreeSet;

const HOME: &str = "/Users/alex";

fn host_mbp() -> Host {
    Host {
        name: "mbp".into(),
        role: HostRole::Local,
        os: Os::MacOs,
        user: "alex".into(),
        remote: false,
    }
}

fn host_devbox() -> Host {
    Host {
        name: "devbox".into(),
        role: HostRole::Development,
        os: Os::Debian,
        user: "alex".into(),
        remote: true,
    }
}

fn host_prod() -> Host {
    Host {
        name: "prod-eu-1".into(),
        role: HostRole::Production,
        os: Os::Debian,
        user: "deploy".into(),
        remote: true,
    }
}

fn source(name: &str, done_at: u64) -> Source {
    Source {
        name: name.into(),
        done_at,
        failure: None,
    }
}

fn failed(name: &str, done_at: u64, why: &str) -> Source {
    Source {
        name: name.into(),
        done_at,
        failure: Some(why.into()),
    }
}

fn tool(name: &str, requested: &str, active: &str, latest: &str, global: bool) -> MiseTool {
    MiseTool {
        name: name.into(),
        requested: requested.into(),
        active: Some(active.into()),
        latest: latest.into(),
        installed: true,
        global,
    }
}

fn task(
    name: &str,
    namespaced: &str,
    dir: &str,
    run: &str,
    depends: &[&str],
    desc: &str,
) -> MiseTask {
    MiseTask {
        name: name.into(),
        namespaced: namespaced.into(),
        dir: dir.into(),
        run: run.into(),
        depends: depends.iter().map(|s| (*s).to_owned()).collect(),
        description: desc.into(),
    }
}

fn container(
    name: &str,
    image: &str,
    running: bool,
    health: Option<&str>,
    ports: &str,
    project: Option<&str>,
) -> Container {
    Container {
        name: name.into(),
        image: image.into(),
        running,
        health: health.map(str::to_owned),
        ports: ports.into(),
        project: project.map(str::to_owned),
        cpu_pct: if running { 2.4 } else { 0.0 },
        mem_mb: if running { 180 } else { 0 },
        size_mb: 42,
    }
}

fn system_quiet() -> SystemSnapshot {
    SystemSnapshot {
        cpu_pct: 12,
        cores: vec![14, 9, 12, 11, 15, 8, 13, 14],
        load: "1.9 2.1 2.0".into(),
        mem_used_gb: 9.8,
        mem_total_gb: 16.0,
        swap_used_gb: 0.0,
        pressure: None,
        pressure_minutes: 0,
        disk_io: vec![("disk0".into(), "1.2 MB/s read · 0.4 MB/s write".into())],
        net: vec![("en0".into(), "rx 210 KB/s · tx 48 KB/s".into())],
        top: vec![
            Proc {
                pid: 4812,
                name: "rust-analyzer".into(),
                cpu_pct: 8,
                mem_mb: 2900,
                state: "running".into(),
                port: None,
            },
            Proc {
                pid: 2201,
                name: "Docker".into(),
                cpu_pct: 2,
                mem_mb: 3800,
                state: "sleeping".into(),
                port: None,
            },
            Proc {
                pid: 7731,
                name: "node (vite)".into(),
                cpu_pct: 1,
                mem_mb: 420,
                state: "sleeping".into(),
                port: Some(5173),
            },
        ],
        btm_installed: true,
        temp_c: Some(48),
    }
}

fn system_pressure() -> SystemSnapshot {
    SystemSnapshot {
        cpu_pct: 78,
        cores: vec![91, 84, 70, 66, 88, 79, 71, 75],
        load: "9.4 7.8 5.1".into(),
        mem_used_gb: 12.4,
        mem_total_gb: 16.0,
        swap_used_gb: 1.1,
        pressure: None,
        pressure_minutes: 4,
        disk_io: vec![("disk0".into(), "14.2 MB/s read · 3.1 MB/s write".into())],
        net: vec![("en0".into(), "rx 1.2 MB/s · tx 240 KB/s".into())],
        top: vec![
            Proc {
                pid: 4812,
                name: "rust-analyzer".into(),
                cpu_pct: 41,
                mem_mb: 2900,
                state: "running".into(),
                port: None,
            },
            Proc {
                pid: 9120,
                name: "cargo".into(),
                cpu_pct: 32,
                mem_mb: 1100,
                state: "running".into(),
                port: None,
            },
            Proc {
                pid: 2201,
                name: "Docker".into(),
                cpu_pct: 9,
                mem_mb: 3800,
                state: "sleeping".into(),
                port: None,
            },
            Proc {
                pid: 7731,
                name: "node (vite)".into(),
                cpu_pct: 6,
                mem_mb: 420,
                state: "sleeping".into(),
                port: Some(5173),
            },
        ],
        btm_installed: true,
        temp_c: Some(87),
    }
}

fn ssh_personal() -> SshState {
    SshState {
        config: format!("{HOME}/.ssh/config"),
        includes: vec![format!("{HOME}/.ssh/config.d/work")],
        wildcard_rules: 3,
        aliases: vec![
            SshAlias {
                alias: "devbox".into(),
                host: "devbox.internal".into(),
                user: "alex".into(),
                port: 22,
                jump: None,
                identities: vec!["~/.ssh/id_ed25519".into()],
                forwards: vec!["-L 5432:localhost:5432".into()],
                hostkey: "ask · known".into(),
                multiplexed: true,
                from_file: format!("{HOME}/.ssh/config.d/work"),
                role: "development".into(),
            },
            SshAlias {
                alias: "bastion".into(),
                host: "bastion.acme.example".into(),
                user: "alex".into(),
                port: 2222,
                jump: None,
                identities: vec!["~/.ssh/id_ed25519".into()],
                forwards: vec![],
                hostkey: "ask · known".into(),
                multiplexed: false,
                from_file: format!("{HOME}/.ssh/config.d/work"),
                role: "jump host".into(),
            },
            SshAlias {
                alias: "prod-eu-1".into(),
                host: "10.40.1.12".into(),
                user: "deploy".into(),
                port: 22,
                jump: Some("bastion".into()),
                identities: vec!["~/.ssh/id_ed25519_prod".into()],
                forwards: vec![],
                hostkey: "ask · known".into(),
                multiplexed: false,
                from_file: format!("{HOME}/.ssh/config.d/work"),
                role: "production".into(),
            },
            SshAlias {
                alias: "production-db".into(),
                host: "db.prod.acme.example".into(),
                user: "readonly".into(),
                port: 5432,
                jump: Some("bastion".into()),
                identities: vec!["~/.ssh/id_ed25519_prod".into()],
                forwards: vec!["-L 15432:localhost:5432".into()],
                hostkey: "ask · known".into(),
                multiplexed: false,
                from_file: format!("{HOME}/.ssh/config"),
                role: "production".into(),
            },
        ],
    }
}

fn github() -> GithubState {
    GithubState {
        logged_in: true,
        account: "alex-dev".into(),
        host: "github.com".into(),
        orgs: vec!["acme-inc".into(), "northwind-labs".into()],
        protocol: "ssh".into(),
        repos: vec![
            GithubRepo {
                owner: "alex-dev".into(),
                name: "holla".into(),
                private: false,
                primary: "main".into(),
                fork_of: None,
                description: "context-adaptive launcher".into(),
            },
            GithubRepo {
                owner: "alex-dev".into(),
                name: "dotfiles".into(),
                private: true,
                primary: "main".into(),
                fork_of: None,
                description: "personal setup".into(),
            },
            GithubRepo {
                owner: "acme-inc".into(),
                name: "acme".into(),
                private: true,
                primary: "main".into(),
                fork_of: None,
                description: "the monorepo".into(),
            },
            GithubRepo {
                owner: "acme-inc".into(),
                name: "payments".into(),
                private: true,
                primary: "master".into(),
                fork_of: None,
                description: "settlement service".into(),
            },
            GithubRepo {
                owner: "acme-inc".into(),
                name: "infra".into(),
                private: true,
                primary: "trunk".into(),
                fork_of: None,
                description: "terraform and ansible".into(),
            },
            GithubRepo {
                owner: "northwind-labs".into(),
                name: "ratatui".into(),
                private: false,
                primary: "main".into(),
                fork_of: Some("ratatui/ratatui".into()),
                description: "fork".into(),
            },
        ],
    }
}

fn disk_quiet() -> DiskState {
    DiskState {
        filesystems: vec![Filesystem {
            mount: "/".into(),
            total_gb: 926,
            used_gb: 611,
        }],
        large: vec![],
        candidates: vec![],
        protected: vec![format!("{HOME}/Documents"), format!("{HOME}/Photos")],
        history: vec![],
        scan_ticks: 60,
        partial_reason: None,
    }
}

fn disk_full() -> DiskState {
    DiskState {
        filesystems: vec![
            Filesystem {
                mount: "/".into(),
                total_gb: 926,
                used_gb: 842,
            },
            Filesystem {
                mount: "/Volumes/Backup".into(),
                total_gb: 2000,
                used_gb: 1420,
            },
        ],
        large: vec![
            LargeEntry {
                path: format!(
                    "{HOME}/Library/Containers/com.docker.docker/Data/vms/0/data/Docker.raw"
                ),
                gb: 24.0,
                kind: "container disk",
                found_at: 4,
            },
            LargeEntry {
                path: format!("{HOME}/work"),
                gb: 41.6,
                kind: "project collection",
                found_at: 6,
            },
            LargeEntry {
                path: format!("{HOME}/Library/Caches"),
                gb: 11.2,
                kind: "caches",
                found_at: 10,
            },
            LargeEntry {
                path: format!("{HOME}/work/dumps/prod-2026-08.sql.gz"),
                gb: 6.3,
                kind: "large file",
                found_at: 18,
            },
            LargeEntry {
                path: format!("{HOME}/.cargo"),
                gb: 5.9,
                kind: "developer cache",
                found_at: 22,
            },
        ],
        candidates: vec![
            Candidate {
                path: format!("{HOME}/Library/Caches/com.apple.dt.Xcode"),
                family: Family::ApplicationCaches,
                project: None,
                gb: 1.9,
                items: 4_120,
                inactive_days: Some(40),
                why: "Xcode download cache · regenerated on demand".into(),
                regenerate: "Xcode re-downloads what it needs".into(),
                active_process: None,
                privilege: None,
                method: Method::Trash,
                confidence: Confidence::High,
                shares_with: None,
                found_at: 12,
                protected: false,
            },
            Candidate {
                path: format!("{HOME}/.npm/_cacache"),
                family: Family::PackageCaches,
                project: None,
                gb: 2.3,
                items: 61_002,
                inactive_days: Some(21),
                why: "npm content-addressable cache · lockfiles restore it".into(),
                regenerate: "npm install refills it".into(),
                active_process: None,
                privilege: None,
                method: Method::Tool("npm cache clean --force".into()),
                confidence: Confidence::High,
                shares_with: None,
                found_at: 14,
                protected: false,
            },
            Candidate {
                path: format!("{HOME}/Downloads/Xcode_16.xip"),
                family: Family::LargeFiles,
                project: None,
                gb: 7.8,
                items: 1,
                inactive_days: Some(90),
                why: "installer archive · already expanded".into(),
                regenerate: "downloadable again from Apple".into(),
                active_process: None,
                privilege: None,
                method: Method::Trash,
                confidence: Confidence::Medium,
                shares_with: None,
                found_at: 16,
                protected: false,
            },
            Candidate {
                path: format!("{HOME}/work/frontend/node_modules"),
                family: Family::ProjectArtifacts,
                project: Some("frontend".into()),
                gb: 8.4,
                items: 121_402,
                inactive_days: Some(31),
                why: "pnpm dependency tree · lockfile present".into(),
                regenerate: "pnpm install --frozen-lockfile".into(),
                active_process: None,
                privilege: None,
                method: Method::Trash,
                confidence: Confidence::High,
                shares_with: None,
                found_at: 8,
                protected: false,
            },
            Candidate {
                path: format!("{HOME}/work/frontend/dist"),
                family: Family::ProjectArtifacts,
                project: Some("frontend".into()),
                gb: 0.42,
                items: 318,
                inactive_days: Some(0),
                why: "vite build output".into(),
                regenerate: "pnpm build".into(),
                active_process: None,
                privilege: None,
                method: Method::Trash,
                confidence: Confidence::High,
                shares_with: None,
                found_at: 9,
                protected: false,
            },
            Candidate {
                path: format!("{HOME}/work/backend/target"),
                family: Family::ProjectArtifacts,
                project: Some("backend".into()),
                gb: 12.7,
                items: 48_113,
                inactive_days: Some(18),
                why: "Cargo target directory · shared with the backend-wt worktree".into(),
                regenerate: "cargo build".into(),
                active_process: None,
                privilege: None,
                method: Method::Tool("cargo clean".into()),
                confidence: Confidence::High,
                shares_with: Some("backend-wt".into()),
                found_at: 14,
                protected: false,
            },
            Candidate {
                path: format!("{HOME}/work/android/build"),
                family: Family::ProjectArtifacts,
                project: Some("android".into()),
                gb: 3.1,
                items: 9_804,
                inactive_days: Some(22),
                why: "Gradle build output".into(),
                regenerate: "./gradlew assembleDebug".into(),
                active_process: None,
                privilege: None,
                method: Method::Tool("./gradlew clean".into()),
                confidence: Confidence::High,
                shares_with: None,
                found_at: 20,
                protected: false,
            },
            Candidate {
                path: format!("{HOME}/work/android/.gradle"),
                family: Family::ProjectArtifacts,
                project: Some("android".into()),
                gb: 0.9,
                items: 2_210,
                inactive_days: None,
                why: "project-local Gradle state · daemon may be active".into(),
                regenerate: "regenerated on the next build".into(),
                active_process: Some("gradle daemon 71822".into()),
                privilege: None,
                method: Method::Permanent,
                confidence: Confidence::Unverifiable,
                shares_with: None,
                found_at: 21,
                protected: false,
            },
            Candidate {
                path: format!("{HOME}/Library/pnpm/store/v3"),
                family: Family::DeveloperCaches,
                project: None,
                gb: 2.2,
                items: 38_400,
                inactive_days: Some(40),
                why: "pnpm content-addressable store".into(),
                regenerate: "re-downloaded on install".into(),
                active_process: None,
                privilege: None,
                method: Method::Tool("pnpm store prune".into()),
                confidence: Confidence::High,
                shares_with: None,
                found_at: 26,
                protected: false,
            },
            Candidate {
                path: format!("{HOME}/.cargo/registry/cache"),
                family: Family::DeveloperCaches,
                project: None,
                gb: 4.8,
                items: 6_102,
                inactive_days: Some(3),
                why: "crate downloads".into(),
                regenerate: "re-downloaded on build".into(),
                active_process: None,
                privilege: None,
                method: Method::Permanent,
                confidence: Confidence::Medium,
                shares_with: None,
                found_at: 30,
                protected: false,
            },
            Candidate {
                path: format!(
                    "{HOME}/Library/Containers/com.docker.docker/Data/vms/0/data/Docker.raw"
                ),
                family: Family::Containers,
                project: None,
                gb: 24.0,
                items: 1,
                inactive_days: Some(0),
                why: "Docker Desktop disk image · cleaned through Docker, never deleted".into(),
                regenerate: "docker system prune".into(),
                active_process: Some("Docker Desktop".into()),
                privilege: None,
                method: Method::Tool("docker system prune".into()),
                confidence: Confidence::High,
                shares_with: None,
                found_at: 34,
                protected: false,
            },
            Candidate {
                path: format!("{HOME}/Library/Logs"),
                family: Family::Logs,
                project: None,
                gb: 1.4,
                items: 12_050,
                inactive_days: Some(60),
                why: "application logs".into(),
                regenerate: "n/a".into(),
                active_process: None,
                privilege: None,
                method: Method::Trash,
                confidence: Confidence::High,
                shares_with: None,
                found_at: 40,
                protected: false,
            },
            Candidate {
                path: "/private/var/folders".into(),
                family: Family::Temp,
                project: None,
                gb: 3.3,
                items: 88_000,
                inactive_days: None,
                why: "system temporary data".into(),
                regenerate: "n/a".into(),
                active_process: None,
                privilege: Some("sudo".into()),
                method: Method::Permanent,
                confidence: Confidence::Unverifiable,
                shares_with: None,
                found_at: 48,
                protected: true,
            },
        ],
        protected: vec![
            format!("{HOME}/Documents"),
            format!("{HOME}/Photos"),
            "/private/var/folders".into(),
        ],
        history: vec![CleanupRecord {
            when_secs: crate::scenario::EPOCH_SECS - 12 * 86_400,
            target: format!("{HOME}/work/legacy/node_modules"),
            method: "Trash".into(),
            reclaimed_gb: 3.9,
            outcome: "removed".into(),
        }],
        scan_ticks: 60,
        partial_reason: None,
    }
}

fn pg_acme(blocked: bool) -> PgState {
    let sessions = if blocked {
        vec![
            PgSession {
                pid: 48197,
                user: "alex".into(),
                app: "psql".into(),
                db: "acme".into(),
                client: "127.0.0.1".into(),
                state: "idle in transaction".into(),
                wait: None,
                query_secs: 0,
                txn_secs: 330,
                query: "UPDATE orders SET status = 'settled' WHERE id = 1042".into(),
                blocked_by: None,
            },
            PgSession {
                pid: 48211,
                user: "api".into(),
                app: "acme-api".into(),
                db: "acme".into(),
                client: "172.18.0.4".into(),
                state: "active".into(),
                wait: Some("Lock:tuple".into()),
                query_secs: 252,
                txn_secs: 252,
                query: "UPDATE orders SET status = 'shipped' WHERE id = 1042".into(),
                blocked_by: Some(48197),
            },
            PgSession {
                pid: 48230,
                user: "worker".into(),
                app: "acme-worker".into(),
                db: "acme".into(),
                client: "172.18.0.5".into(),
                state: "active".into(),
                wait: Some("Lock:tuple".into()),
                query_secs: 238,
                txn_secs: 238,
                query: "UPDATE orders SET status = 'settled' WHERE id = 1042".into(),
                blocked_by: Some(48211),
            },
            PgSession {
                pid: 48240,
                user: "api".into(),
                app: "acme-api".into(),
                db: "acme".into(),
                client: "172.18.0.4".into(),
                state: "active".into(),
                wait: None,
                query_secs: 1,
                txn_secs: 1,
                query: "SELECT * FROM orders WHERE customer_id = $1".into(),
                blocked_by: None,
            },
        ]
    } else {
        vec![PgSession {
            pid: 48240,
            user: "api".into(),
            app: "acme-api".into(),
            db: "acme".into(),
            client: "172.18.0.4".into(),
            state: "active".into(),
            wait: None,
            query_secs: 1,
            txn_secs: 1,
            query: "SELECT * FROM orders WHERE customer_id = $1".into(),
            blocked_by: None,
        }]
    };
    PgState {
        label: "acme@localhost:5432/acme".into(),
        source: "DATABASE_URL in .env · password never shown".into(),
        host: "localhost".into(),
        port: 5432,
        user: "acme".into(),
        db: "acme".into(),
        max_connections: 100,
        connections: if blocked { 24 } else { 6 },
        sessions,
        pg_activity_installed: true,
        deadlocks_24h: if blocked { 2 } else { 0 },
        cache_hit_pct: 98,
        replication_lag: None,
    }
}

pub fn mise_holla_pub(trusted: bool) -> MiseState {
    mise_holla(trusted)
}

fn mise_holla(trusted: bool) -> MiseState {
    MiseState {
        installed: true,
        version: "2026.9.3".into(),
        monorepo_root: None,
        global_tools: vec![
            tool("node", "26", "26.0.1", "26.1.0", true),
            tool("python", "3.13", "3.13.5", "3.13.7", true),
        ],
        configs: vec![MiseConfig {
            path: format!("{HOME}/work/holla/mise.toml"),
            trusted,
            env: vec!["RUST_LOG=holla=debug".into()],
            tools: vec![
                tool("rust", "1.90", "1.90.0", "1.90.0", false),
                MiseTool {
                    name: "cargo-nextest".into(),
                    requested: "0.9".into(),
                    active: Some("0.9.101".into()),
                    latest: "0.9.104".into(),
                    installed: true,
                    global: false,
                },
            ],
            tasks: vec![
                task(
                    "test",
                    "test",
                    &format!("{HOME}/work/holla"),
                    "cargo nextest run",
                    &[],
                    "run the test suite with nextest",
                ),
                task(
                    "lint",
                    "lint",
                    &format!("{HOME}/work/holla"),
                    "cargo clippy --all-targets -- -D warnings",
                    &[],
                    "clippy with warnings denied",
                ),
                task(
                    "fmt",
                    "fmt",
                    &format!("{HOME}/work/holla"),
                    "cargo fmt --all -- --check",
                    &[],
                    "check formatting",
                ),
                task(
                    "check",
                    "check",
                    &format!("{HOME}/work/holla"),
                    "cargo check --workspace",
                    &[],
                    "type-check every target",
                ),
                task(
                    "shots",
                    "shots",
                    &format!("{HOME}/work/holla"),
                    "tools/capture.sh start 120 40",
                    &["check"],
                    "capture the review frames",
                ),
            ],
        }],
    }
}

fn mise_acme(child_trusted: bool) -> MiseState {
    let root = format!("{HOME}/work/acme");
    MiseState {
        installed: true,
        version: "2026.9.3".into(),
        monorepo_root: Some(root.clone()),
        global_tools: vec![
            tool("node", "26", "26.0.1", "26.1.0", true),
            tool("python", "3.13", "3.13.5", "3.13.7", true),
            tool("go", "1.24", "1.24.6", "1.25.1", true),
        ],
        configs: vec![
            MiseConfig {
                path: format!("{root}/mise.toml"),
                trusted: true,
                env: vec![
                    "COMPOSE_PROJECT_NAME=acme".into(),
                    "DATABASE_URL=postgres://acme@localhost:5432/acme".into(),
                ],
                tools: vec![
                    tool("node", "26", "26.0.1", "26.1.0", false),
                    tool("pnpm", "10", "10.4.0", "10.6.2", false),
                    tool("rust", "1.90", "1.90.0", "1.90.0", false),
                    MiseTool {
                        name: "sqlx-cli".into(),
                        requested: "0.8".into(),
                        active: None,
                        latest: "0.8.6".into(),
                        installed: false,
                        global: false,
                    },
                ],
                tasks: vec![
                    task(
                        "setup",
                        "setup",
                        &root,
                        "mise install && pnpm install --frozen-lockfile",
                        &[],
                        "install tools and dependencies for every project",
                    ),
                    task(
                        "up",
                        "up",
                        &root,
                        "docker compose up -d db redis",
                        &[],
                        "start the containers every service needs",
                    ),
                    task(
                        "dev",
                        "dev",
                        &root,
                        "mise run --jobs 2 //apps/frontend:dev //services/api:dev",
                        &["up"],
                        "start the whole development ecosystem",
                    ),
                    task(
                        "test",
                        "test",
                        &root,
                        "mise run //apps/frontend:test //services/api:test //tools/worker:test",
                        &[],
                        "run every project's tests",
                    ),
                ],
            },
            MiseConfig {
                path: format!("{root}/apps/frontend/mise.toml"),
                trusted: child_trusted,
                env: vec!["VITE_API_URL=http://localhost:8080".into()],
                tools: vec![
                    tool("node", "26", "26.0.1", "26.1.0", false),
                    tool("pnpm", "10", "10.4.0", "10.6.2", false),
                ],
                tasks: vec![
                    task(
                        "dev",
                        "//apps/frontend:dev",
                        &format!("{root}/apps/frontend"),
                        "pnpm dev",
                        &[],
                        "vite development server",
                    ),
                    task(
                        "build",
                        "//apps/frontend:build",
                        &format!("{root}/apps/frontend"),
                        "pnpm build",
                        &[],
                        "production build",
                    ),
                    task(
                        "test",
                        "//apps/frontend:test",
                        &format!("{root}/apps/frontend"),
                        "pnpm test",
                        &["//apps/frontend:build"],
                        "vitest after a build",
                    ),
                    task(
                        "lint",
                        "//apps/frontend:lint",
                        &format!("{root}/apps/frontend"),
                        "pnpm lint",
                        &[],
                        "eslint",
                    ),
                ],
            },
            MiseConfig {
                path: format!("{root}/services/api/mise.toml"),
                trusted: true,
                env: vec![],
                tools: vec![tool("rust", "1.90", "1.90.0", "1.90.0", false)],
                tasks: vec![
                    task(
                        "dev",
                        "//services/api:dev",
                        &format!("{root}/services/api"),
                        "cargo watch -x 'run -p api'",
                        &[],
                        "api with auto-reload",
                    ),
                    task(
                        "test",
                        "//services/api:test",
                        &format!("{root}/services/api"),
                        "cargo nextest run -p api",
                        &[],
                        "api tests",
                    ),
                    task(
                        "migrate",
                        "//services/api:migrate",
                        &format!("{root}/services/api"),
                        "sqlx migrate run",
                        &[],
                        "apply pending migrations",
                    ),
                ],
            },
            MiseConfig {
                path: format!("{root}/tools/worker/mise.toml"),
                trusted: true,
                env: vec![],
                tools: vec![tool("python", "3.13", "3.13.5", "3.13.7", false)],
                tasks: vec![
                    task(
                        "dev",
                        "//tools/worker:dev",
                        &format!("{root}/tools/worker"),
                        "uv run worker --reload",
                        &[],
                        "queue worker with reload",
                    ),
                    task(
                        "test",
                        "//tools/worker:test",
                        &format!("{root}/tools/worker"),
                        "uv run pytest -q",
                        &[],
                        "pytest",
                    ),
                ],
            },
        ],
    }
}

pub fn docker_acme_pub() -> DockerState {
    docker_acme()
}

fn docker_acme() -> DockerState {
    DockerState {
        daemon: Ok(()),
        containers: vec![
            container(
                "acme-db-1",
                "postgres:17.2",
                true,
                Some("healthy"),
                "5432→5432",
                Some("acme"),
            ),
            container(
                "acme-redis-1",
                "redis:7.4-alpine",
                true,
                Some("healthy"),
                "6379→6379",
                Some("acme"),
            ),
            container(
                "acme-api-1",
                "acme/api:dev",
                true,
                Some("unhealthy"),
                "8080→8080",
                Some("acme"),
            ),
            container(
                "acme-worker-1",
                "acme/worker:dev",
                true,
                None,
                "",
                Some("acme"),
            ),
            container(
                "acme-scheduler-1",
                "acme/scheduler:dev",
                true,
                None,
                "",
                Some("acme"),
            ),
            container("pgadmin", "dpage/pgadmin4:9", false, None, "", None),
        ],
        images: 23,
        images_gb: 6.4,
        dangling_images: 5,
        volumes: vec![
            Volume {
                name: "acme_pgdata".into(),
                gb: 2.1,
                anonymous: false,
                used_by: Some("acme-db-1".into()),
            },
            Volume {
                name: "acme_redisdata".into(),
                gb: 0.1,
                anonymous: false,
                used_by: Some("acme-redis-1".into()),
            },
            Volume {
                name: "pgadmin_data".into(),
                gb: 0.2,
                anonymous: false,
                used_by: None,
            },
        ],
        networks: 2,
        builder_cache_gb: 2.9,
        reclaimable_gb: 7.1,
        compose: Some(Compose {
            file: format!("{HOME}/work/acme/docker-compose.yml"),
            dir: format!("{HOME}/work/acme"),
            name: "acme".into(),
            services: vec![
                "db".into(),
                "redis".into(),
                "api".into(),
                "worker".into(),
                "scheduler".into(),
            ],
        }),
        cleanup_uses: 2,
        compose_plugin: true,
        fail_stage: None,
    }
}

fn docker_devbox() -> DockerState {
    DockerState {
        daemon: Ok(()),
        containers: vec![
            container(
                "acme-api-1",
                "acme/api:dev",
                true,
                Some("healthy"),
                "8080→8080",
                Some("acme"),
            ),
            container(
                "acme-worker-1",
                "acme/worker:dev",
                true,
                None,
                "",
                Some("acme"),
            ),
            container(
                "acme-scheduler-1",
                "acme/scheduler:dev",
                true,
                None,
                "",
                Some("acme"),
            ),
            container(
                "acme-db-1",
                "postgres:17.2",
                true,
                Some("healthy"),
                "5432→5432",
                Some("acme"),
            ),
            container(
                "acme-redis-1",
                "redis:7.4-alpine",
                true,
                Some("healthy"),
                "6379→6379",
                Some("acme"),
            ),
            container(
                "build-cache-7f2",
                "moby/buildkit:v0.18",
                false,
                None,
                "",
                None,
            ),
            container(
                "registry-mirror",
                "registry:2",
                false,
                None,
                "5000→5000",
                None,
            ),
            container("tmp-shell", "debian:13", false, None, "", None),
            container("tmp-shell-2", "debian:13", false, None, "", None),
            container("pgadmin", "dpage/pgadmin4:9", false, None, "", None),
            container(
                "grafana",
                "grafana/grafana:11",
                false,
                None,
                "3000→3000",
                Some("monitoring"),
            ),
            container(
                "loki",
                "grafana/loki:3",
                false,
                None,
                "",
                Some("monitoring"),
            ),
        ],
        images: 41,
        images_gb: 9.8,
        dangling_images: 12,
        volumes: vec![
            Volume {
                name: "acme_pgdata".into(),
                gb: 2.4,
                anonymous: false,
                used_by: Some("acme-db-1".into()),
            },
            Volume {
                name: "acme_redisdata".into(),
                gb: 0.1,
                anonymous: false,
                used_by: Some("acme-redis-1".into()),
            },
            Volume {
                name: "grafana_data".into(),
                gb: 0.4,
                anonymous: false,
                used_by: None,
            },
            Volume {
                name: "loki_data".into(),
                gb: 0.9,
                anonymous: false,
                used_by: None,
            },
            Volume {
                name: "pgadmin_data".into(),
                gb: 0.2,
                anonymous: false,
                used_by: None,
            },
            Volume {
                name: "3a9c…e01f".into(),
                gb: 0.05,
                anonymous: true,
                used_by: None,
            },
            Volume {
                name: "b71d…44c0".into(),
                gb: 0.05,
                anonymous: true,
                used_by: None,
            },
        ],
        networks: 3,
        builder_cache_gb: 6.1,
        reclaimable_gb: 18.4,
        compose: None,
        cleanup_uses: 9,
        compose_plugin: true,
        fail_stage: None,
    }
}

fn memory_common() -> RankingMemory {
    let now = crate::scenario::EPOCH_SECS;
    let mut usage = UsageStore::default();
    let holla = format!("{HOME}/work/holla");
    let acme = format!("{HOME}/work/acme");
    usage.seed("mise.task.test", Some(&holla), "mbp", 6, now - 3_600);
    usage.seed("mise.task.lint", Some(&holla), "mbp", 3, now - 86_400);
    usage.seed("git.pull", None, "mbp", 41, now - 7_200);
    usage.seed("git.pull", Some(&holla), "mbp", 9, now - 7_200);
    usage.seed("docker.cleanup", None, "devbox", 9, now - 5 * 86_400);
    usage.seed("docker.logs.api", Some(&acme), "mbp", 4, now - 86_400);
    usage.seed(
        "mise.task.//apps/frontend:dev",
        Some(&acme),
        "mbp",
        12,
        now - 1_800,
    );
    usage.seed("deploy.preview", Some(&acme), "mbp", 7, now - 2 * 86_400);
    usage.seed("system.resources", None, "mbp", 2, now - 3 * 86_400);
    usage.learn_query("docker clean", "docker.cleanup", "devbox", now - 5 * 86_400);
    RankingMemory {
        pins: vec![],
        aliases: vec![
            ("gp".into(), "git.pull".into()),
            ("du".into(), "disk.usage".into()),
            ("d".into(), "explore.disk".into()),
            ("dc".into(), "docker.cleanup".into()),
        ],
        hidden: vec![],
        usage,
        personalization: true,
        preferred_tools: vec![("system.monitor".into(), "btm".into())],
    }
}

fn platform_for(os: Os, home: &str) -> Platform {
    match os {
        Os::MacOs => Platform {
            trash: TrashBackend::MacNative,
            opener: Some("open".into()),
            opener_fails: None,
            spotlight: Spotlight::Empty,
            osc52: true,
            osc52_limit: 100_000,
            xdg_config_home: format!("{home}/.config"),
            xdg_cache_home: format!("{home}/.cache"),
            subreaper: false,
            process_probe: Ok(()),
            dataless_failure: None,
        },
        Os::Debian => Platform {
            trash: TrashBackend::FreeDesktop,
            opener: Some("xdg-open".into()),
            opener_fails: None,
            spotlight: Spotlight::Unavailable("Spotlight is macOS only · use the tree scan".into()),
            osc52: true,
            osc52_limit: 100_000,
            xdg_config_home: format!("{home}/.config"),
            xdg_cache_home: format!("{home}/.cache"),
            subreaper: true,
            process_probe: Ok(()),
            dataless_failure: None,
        },
    }
}

const COMMON_TOOLS: [&str; 12] = [
    "git", "ssh", "kill", "ps", "lsof", "less", "find", "sh", "lazygit", "trash", "mise", "psql",
];

pub fn base_world(scenario: Scenario, motion: Motion, host: Host, location: Location) -> World {
    base(scenario, motion, host, location)
}

fn base(scenario: Scenario, motion: Motion, host: Host, location: Location) -> World {
    let mut clock = crate::scenario::CLOCK;
    clock.running = motion != Motion::Paused;
    let platform = platform_for(host.os, &location.home);
    let tools: BTreeSet<String> = COMMON_TOOLS.iter().map(|s| s.to_string()).collect();
    World {
        fs: Fs::new(),
        tools,
        cargo: CargoState::default(),
        brew: None,
        gradle: GradleState::default(),
        upgrade: UpgradeState::default(),
        platform,
        outputs: ToolOutputs::default(),
        custom_global: None,
        custom_project: None,
        trust: crate::domain::custom::TrustStore::default(),
        sudo_cached: true,
        service_failures: vec![],
        explore_tiles: crate::domain::catalog::default_explore_tiles(),
        task_failures: vec![],
        ops_log: crate::domain::cleanup::OpsLog {
            path: format!("{}/.cache/holla/ops.log", location.home),
            ..Default::default()
        },
        size_cache: crate::domain::cleanup::SizeCache::default(),
        persisted: Persisted::default(),
        reports: vec![],
        scan: None,
        fs_latency: vec![],
        cleanup_job: None,
        scenario,
        clock,
        tick: 0,
        host,
        location,
        git: vec![],
        github: github(),
        docker: DockerState::unavailable("Docker Desktop is not running"),
        mise: MiseState {
            installed: false,
            version: String::new(),
            configs: vec![],
            monorepo_root: None,
            global_tools: vec![],
        },
        system: system_quiet(),
        disk: disk_quiet(),
        pg: None,
        ssh: ssh_personal(),
        apt: None,
        memory: memory_common(),
        activities: vec![],
        batches: vec![],
        plans: vec![],
        sources: vec![],
        scripts: scripts::scripts(),
        step_output: scripts::step_outputs(),
        clipboard: None,
        next_activity: 0,
        trusted_now: vec![],
        healed_units: vec![],
    }
}

fn sources_for(motion: Motion, list: &[(&str, u64)], fails: &[(&str, u64, &str)]) -> Vec<Source> {
    let scale = |t: u64| if motion == Motion::Reduced { 0 } else { t };
    let mut v: Vec<Source> = list.iter().map(|(n, t)| source(n, scale(*t))).collect();
    v.extend(fails.iter().map(|(n, t, w)| failed(n, scale(*t), w)));
    v
}

pub fn world_for(scenario: Scenario, motion: Motion) -> World {
    let mut w = match scenario {
        Scenario::FirstUse => first_use(motion),
        Scenario::RustDirty => rust_dirty(motion),
        Scenario::MonorepoRoot => monorepo(motion, false),
        Scenario::MonorepoChild => monorepo(motion, true),
        Scenario::DockerCleanup => docker_cleanup(motion),
        Scenario::DiskCleanup => disk_cleanup(motion),
        Scenario::UpgradePlan => upgrade_plan(motion),
        Scenario::ActivitiesMulti => activities_multi(motion),
        Scenario::RemoteHost => remote_host(motion),
        Scenario::LaunchFailure => launch_failure(motion),
        Scenario::HardCases => hard_cases(motion),
        other => crate::domain::parity::world_for(other, motion),
    };
    seed_world(&mut w);
    w
}

fn seed_custom(
    w: &mut World,
    root: &str,
    actions: &[(&str, &str, &[&str], &str, &str)],
    trusted: bool,
) {
    let mut text = String::from("# project actions\n");
    for (id, label, argv, danger, desc) in actions {
        let args: Vec<String> = argv.iter().map(|a| format!("\"{a}\"")).collect();
        text.push_str(&format!(
            "[[action]]\nid = \"{id}\"\nlabel = \"{label}\"\ncommand = [{}]\ndanger = \"{danger}\"\ndescription = \"{desc}\"\n\n",
            args.join(", ")
        ));
    }
    let path = format!("{root}/.holla.toml");
    w.fs.text(&path, &text, 3);
    let cfg = crate::domain::custom::parse_config(
        &path,
        crate::domain::custom::Origin::Project,
        &text,
        &["git.pull"],
        &[],
    );
    if trusted {
        let _ = w.trust.approve(&cfg.digest, &path, root);
    }
    for (_, _, argv, _, _) in actions {
        w.tools.insert(argv[0].to_string());
    }
    w.custom_project = Some(cfg);
}

pub fn seed_world(w: &mut World) {
    seed_tools(w);
    seed_fs(w);

    if w.persisted.frecency.is_none() {
        w.persisted.frecency = Some(w.memory.usage.serialize());
    }
    w.persisted.trust = Some(w.trust.serialize());
}

fn seed_tools(w: &mut World) {
    if w.mise.installed {
        w.tools.insert("mise".into());
    } else {
        w.tools.remove("mise");
    }
    match &w.docker.daemon {
        Err(r) if r.contains("not installed") => {
            w.tools.remove("docker");
        }
        _ => {
            w.tools.insert("docker".into());
        }
    }
    if w.location
        .project
        .as_ref()
        .is_some_and(|p| matches!(p.kind, ProjectKind::Rust { .. }))
        || w.location
            .children
            .iter()
            .any(|c| matches!(c.kind, ProjectKind::Rust { .. }))
    {
        w.tools.insert("cargo".into());
    }
    if w.github.logged_in
        || matches!(
            w.source_state("github"),
            crate::sim::world::SourceState::Failed(_)
        )
    {
        w.tools.insert("gh".into());
    }
    if w.system.btm_installed {
        w.tools.insert("btm".into());
    }
    if w.pg.as_ref().is_some_and(|p| p.pg_activity_installed) {
        w.tools.insert("pg_activity".into());
    }
    if w.host.role == HostRole::Production {
        w.tools.insert("systemctl".into());
        w.tools.insert("journalctl".into());
        w.tools.insert("sudo".into());
    }
    if w.apt.is_some() {
        w.tools.insert("apt-get".into());
        w.tools.insert("apt".into());
        w.tools.insert("sudo".into());
    }
    if w.host.os == Os::MacOs {
        w.tools.insert("open".into());
        w.tools.insert("mdfind".into());
    } else {
        w.tools.insert("xdg-open".into());
    }
    if w.brew.is_some() {
        w.tools.insert("brew".into());
    }
    if w.upgrade.amp {
        w.tools.insert("amp".into());
    }
}

fn sized(fs: &mut Fs, path: &str, bytes: u64, age_days: i64) {
    fs.file(path, bytes.max(1), age_days);
}

fn gb(gb: f32) -> u64 {
    (gb as f64 * 1024.0 * 1024.0 * 1024.0) as u64 / BLOCK * BLOCK
}

struct SeedFs(Fs);

impl std::ops::Deref for SeedFs {
    type Target = Fs;
    fn deref(&self) -> &Fs {
        &self.0
    }
}

impl std::ops::DerefMut for SeedFs {
    fn deref_mut(&mut self) -> &mut Fs {
        &mut self.0
    }
}

impl SeedFs {
    fn text(&mut self, path: &str, text: &str, age_days: i64) -> &mut Self {
        if !self.0.exists(path) {
            self.0.text(path, text, age_days);
        }
        self
    }
    fn dir(&mut self, path: &str, age_days: i64) -> &mut Self {
        if !self.0.exists(path) {
            self.0.dir(path, age_days);
        }
        self
    }
    fn device(&mut self, path: &str) -> &mut Self {
        if !self.0.exists(path) {
            self.0.device(path);
        }
        self
    }
}

fn seed_fs(w: &mut World) {
    let home = w.location.home.clone();
    let cwd = w.location.cwd.clone();
    let now = w.now_secs();
    let mut fs = SeedFs(std::mem::take(&mut w.fs));

    fs.device("/dev/null");
    for f in &w.disk.filesystems {
        if !fs.volumes.iter().any(|v| v.mount == f.mount) {
            fs.volume(&f.mount, gb(f.total_gb as f32), gb(f.used_gb as f32));
        }
    }
    if fs.volumes.is_empty() {
        fs.volume("/", gb(500.0), gb(200.0));
    }
    fs.dir(&home, 400);
    fs.dir(&cwd, 3);
    fs.dir(&format!("{home}/.Trash"), 1);

    for (d, age) in [
        ("Documents", 12),
        ("Downloads", 1),
        ("Desktop", 2),
        ("Pictures", 90),
        ("Library", 30),
        ("work", 0),
    ] {
        fs.dir(&format!("{home}/{d}"), age);
    }
    fs.text(
        &format!("{home}/Documents/notes.md"),
        "# Notes\n\n- review the release\n- café ☕ 東京\n",
        4,
    );
    fs.text(&format!("{home}/Documents/README.md"), "readme\n", 40);
    fs.text(
        &format!("{home}/Downloads/report-2026-08.pdf.txt"),
        "report\n",
        20,
    );
    sized(
        &mut fs,
        &format!("{home}/Downloads/installer.dmg"),
        2 * 1024 * 1024 * 1024,
        30,
    );
    fs.dir(&format!("{home}/Library/Caches/com.apple.dt.Xcode"), 45);
    sized(
        &mut fs,
        &format!("{home}/Library/Caches/com.apple.dt.Xcode/index.db"),
        900 * 1024 * 1024,
        45,
    );
    fs.dir(&format!("{home}/Library/Logs/DiagnosticReports"), 20);
    sized(
        &mut fs,
        &format!("{home}/Library/Logs/DiagnosticReports/crash.ips"),
        40_000,
        20,
    );
    fs.dir(
        &format!("{home}/Library/Mobile Documents/com~apple~CloudDocs"),
        1,
    );
    fs.text(
        &format!("{home}/Library/Mobile Documents/com~apple~CloudDocs/cloud.txt"),
        "cloud\n",
        1,
    );
    fs.dir(&format!("{home}/Library/Keychains"), 200);
    sized(
        &mut fs,
        &format!("{home}/Library/Keychains/login.keychain-db"),
        100_000,
        2,
    );

    let mut roots: Vec<(String, ProjectKind)> = vec![];
    if let Some(p) = &w.location.project {
        roots.push((p.root.clone(), p.kind.clone()));
    }
    if let Some(p) = &w.location.workspace {
        roots.push((p.root.clone(), p.kind.clone()));
    }
    for c in &w.location.children {
        roots.push((c.root.clone(), c.kind.clone()));
    }
    for (root, kind) in &roots {
        fs.dir(root, 2);
        match kind {
            ProjectKind::Rust { .. } => {
                fs.text(
                    &format!("{root}/Cargo.toml"),
                    "[package]\nname = \"crate\"\n",
                    5,
                );
                fs.text(&format!("{root}/src/main.rs"), "fn main() {}\n", 1);
            }
            ProjectKind::Node { manager } => {
                fs.text(&format!("{root}/package.json"), "{\"name\":\"frontend\",\"scripts\":{\"dev\":\"vite --host\",\"build\":\"vite build\",\"test\":\"vitest run\",\"lint\":\"eslint .\"}}\n", 5);
                let lock = match *manager {
                    "pnpm" => "pnpm-lock.yaml",
                    "yarn" => "yarn.lock",
                    "bun" => "bun.lockb",
                    _ => "package-lock.json",
                };
                fs.text(&format!("{root}/{lock}"), "lockfileVersion: 9\n", 5);
            }
            ProjectKind::Gradle => {
                fs.text(
                    &format!("{root}/build.gradle.kts"),
                    "plugins { id(\"com.android.application\") }\n",
                    5,
                );
                fs.text(&format!("{root}/gradlew"), "#!/bin/sh\n", 5);
                fs.text(
                    &format!("{root}/settings.gradle.kts"),
                    "rootProject.name = \"android\"\n",
                    5,
                );
            }
            ProjectKind::Python => {
                fs.text(
                    &format!("{root}/pyproject.toml"),
                    "[project]\nname = \"worker\"\n",
                    5,
                );
            }
            ProjectKind::MiseMonorepo | ProjectKind::Collection | ProjectKind::Service => {}
        }
    }
    for g in &w.git {
        fs.dir(&g.path, 2);
        if g.git_file {
            fs.text(
                &format!("{}/.git", g.path),
                "gitdir: ../.git/worktrees/x\n",
                2,
            );
        } else {
            fs.dir(&format!("{}/.git", g.path), 2);
            fs.text(
                &format!("{}/.git/HEAD", g.path),
                &format!(
                    "ref: refs/heads/{}\n",
                    g.branch.clone().unwrap_or("main".into())
                ),
                1,
            );
        }
        for m in &g.modified {
            fs.text(&format!("{}/{m}", g.path), "modified\n", 0);
        }
    }
    for cfg in &w.mise.configs {
        let mut text = String::new();
        for t in &cfg.tasks {
            text.push_str(&format!(
                "[tasks.{}]\nrun = \"{}\"\ndescription = \"{}\"\n\n",
                t.name, t.run, t.description
            ));
        }
        fs.text(&cfg.path, &text, 3);
    }
    if let Some(c) = &w.docker.compose {
        fs.text(
            &c.file,
            &format!(
                "services:\n{}",
                c.services
                    .iter()
                    .map(|s| format!("  {s}: {{}}\n"))
                    .collect::<String>()
            ),
            10,
        );
    }

    let cands = w.disk.candidates.clone();
    for c in &cands {
        let age = i64::from(c.inactive_days.unwrap_or(0));
        let bytes = gb(c.gb);
        fs.dir(&c.path, age);
        let per = (bytes / 3).max(BLOCK);
        sized(&mut fs, &format!("{}/a.bin", c.path), per, age);
        sized(&mut fs, &format!("{}/b.bin", c.path), per, age);
        sized(
            &mut fs,
            &format!("{}/c/d.bin", c.path),
            bytes.saturating_sub(2 * per).max(BLOCK),
            age,
        );
        if c.inactive_days.is_none() {
            fs.touch(&c.path, -1);
        }
    }
    for l in &w.disk.large.clone() {
        if !fs.exists(&l.path) {
            if l.kind == "large file" || l.kind == "container disk" {
                sized(&mut fs, &l.path, gb(l.gb), 20);
            } else {
                fs.dir(&l.path, 10);
                sized(&mut fs, &format!("{}/blob.bin", l.path), gb(l.gb), 10);
            }
        }
    }
    for p in &w.disk.protected.clone() {
        fs.dir(p, 100);
    }
    if let Some(p) = &w.location.project
        && let ProjectKind::Rust { .. } = p.kind
    {
        let target = format!("{}/target", p.root);
        if fs.is_dir(&target) {
            w.cargo.target = Some(target.clone());
            w.cargo.target_bytes = fs.size_of(&target);
            w.cargo.target_files = fs.subtree(&target).len() as u64;
        }
    }

    if let Some(z) = &w.upgrade.zsh_env {
        fs.dir(z, 30);
        fs.text(&format!("{z}/tools/upgrade.sh"), "#!/bin/sh\n", 30);
    }
    let omz = format!("{home}/.oh-my-zsh");
    if w.tools.contains("brew") && !fs.exists(&omz) && w.host.os == Os::MacOs {
        fs.dir(&omz, 200);
        fs.text(&format!("{omz}/tools/upgrade.sh"), "#!/bin/sh\n", 200);
    }
    let _ = now;
    w.fs = fs.0;
}

fn first_use(motion: Motion) -> World {
    let loc = Location {
        cwd: format!("{HOME}/scratch"),
        home: HOME.into(),
        project: None,
        workspace: None,
        children: vec![],
    };
    let mut w = base(Scenario::FirstUse, motion, host_mbp(), loc);
    w.system = system_pressure();
    w.disk = disk_full();
    w.memory.usage.actions.retain(|u| u.path.is_none());
    w.mise.installed = true;
    w.mise.version = "2026.9.3".into();
    w.mise.global_tools = vec![tool("node", "26", "26.0.1", "26.1.0", true)];
    w.sources = sources_for(
        motion,
        &[
            ("filesystem", 3),
            ("git", 6),
            ("mise", 9),
            ("system", 12),
            ("ssh", 16),
            ("github", 40),
        ],
        &[],
    );
    w
}

fn rust_dirty(motion: Motion) -> World {
    let root = format!("{HOME}/work/holla");
    let loc = Location {
        cwd: root.clone(),
        home: HOME.into(),
        project: Some(Project {
            name: "holla".into(),
            root: root.clone(),
            kind: ProjectKind::Rust {
                workspace: false,
                member: None,
            },
        }),
        workspace: None,
        children: vec![],
    };
    let mut w = base(Scenario::RustDirty, motion, host_mbp(), loc);
    let mut g = GitState::clean(&root, "main", "main");
    g.behind = 3;
    g.unstaged = 4;
    g.untracked = 1;
    g.stash = 1;
    g.head_short = "9f2c1aa".into();
    g.modified = vec![
        "src/bin/holla/screens/here.rs".into(),
        "src/bin/holla/app.rs".into(),
        "DESIGN.md".into(),
        "holla-project/notes/00-decisions.md".into(),
    ];
    w.git = vec![g];
    w.mise = mise_holla(true);
    w.docker = DockerState {
        daemon: Ok(()),
        containers: vec![container(
            "holla-db-1",
            "postgres:17.2",
            true,
            Some("healthy"),
            "5432→5432",
            Some("holla"),
        )],
        images: 6,
        images_gb: 1.9,
        dangling_images: 1,
        volumes: vec![Volume {
            name: "holla_pgdata".into(),
            gb: 0.6,
            anonymous: false,
            used_by: Some("holla-db-1".into()),
        }],
        networks: 1,
        builder_cache_gb: 0.8,
        reclaimable_gb: 1.1,
        compose: Some(Compose {
            file: format!("{root}/docker-compose.yml"),
            dir: root.clone(),
            name: "holla".into(),
            services: vec!["db".into()],
        }),
        cleanup_uses: 0,
        compose_plugin: true,
        fail_stage: None,
    };
    w.pg = Some({
        let mut p = pg_acme(true);
        p.label = "holla@localhost:5432/holla_dev".into();
        p.db = "holla_dev".into();
        p.user = "holla".into();
        for s in &mut p.sessions {
            s.db = "holla_dev".into();
        }
        p
    });
    w.disk = {
        let mut d = disk_quiet();
        d.candidates = vec![Candidate {
            path: format!("{root}/target"),
            family: Family::ProjectArtifacts,
            project: Some("holla".into()),
            gb: 12.3,
            items: 51_204,
            inactive_days: Some(0),
            why: "Cargo target directory · debug and test profiles".into(),
            regenerate: "cargo build".into(),
            active_process: None,
            privilege: None,
            method: Method::Tool("cargo clean".into()),
            confidence: Confidence::High,
            shares_with: None,
            found_at: 6,
            protected: false,
        }];
        d.large = vec![
            LargeEntry {
                path: format!("{root}/target"),
                gb: 12.3,
                kind: "rebuildable",
                found_at: 6,
            },
            LargeEntry {
                path: format!("{root}/shots"),
                gb: 0.9,
                kind: "captures",
                found_at: 9,
            },
        ];
        d
    };
    seed_custom(
        &mut w,
        &root,
        &[(
            "deploy.preview",
            "Deploy preview",
            &["tools/deploy-preview.sh"],
            "mutating",
            "push the current branch to the preview stack",
        )],
        true,
    );
    w.sources = sources_for(
        motion,
        &[
            ("filesystem", 2),
            ("git", 4),
            ("mise", 7),
            ("cargo", 10),
            ("docker", 14),
            ("postgres", 18),
            ("system", 6),
            ("ssh", 8),
            ("github", 30),
        ],
        &[],
    );
    w
}

fn acme_location(child: bool) -> Location {
    let root = format!("{HOME}/work/acme");
    let children = vec![
        Project {
            name: "frontend".into(),
            root: format!("{root}/apps/frontend"),
            kind: ProjectKind::Node { manager: "pnpm" },
        },
        Project {
            name: "api".into(),
            root: format!("{root}/services/api"),
            kind: ProjectKind::Rust {
                workspace: false,
                member: None,
            },
        },
        Project {
            name: "worker".into(),
            root: format!("{root}/tools/worker"),
            kind: ProjectKind::Python,
        },
    ];
    let ws = Project {
        name: "acme".into(),
        root: root.clone(),
        kind: ProjectKind::MiseMonorepo,
    };
    if child {
        Location {
            cwd: format!("{root}/apps/frontend"),
            home: HOME.into(),
            project: Some(children[0].clone()),
            workspace: Some(ws),
            children: vec![],
        }
    } else {
        Location {
            cwd: root,
            home: HOME.into(),
            project: Some(ws),
            workspace: None,
            children,
        }
    }
}

fn monorepo(motion: Motion, child: bool) -> World {
    let scenario = if child {
        Scenario::MonorepoChild
    } else {
        Scenario::MonorepoRoot
    };
    let mut w = base(scenario, motion, host_mbp(), acme_location(child));
    let root = format!("{HOME}/work/acme");
    let mut g = GitState::clean(&root, "main", "main");
    g.head_short = "c81e4d0".into();
    g.ahead = 1;
    if child {
        g.unstaged = 2;
        g.modified = vec![
            "apps/frontend/src/routes/orders.tsx".into(),
            "apps/frontend/src/lib/api.ts".into(),
        ];
    }
    w.git = vec![g];
    w.mise = mise_acme(false);
    w.docker = docker_acme();
    w.pg = Some(pg_acme(false));
    w.disk = {
        let mut d = disk_quiet();
        d.candidates = vec![
            Candidate {
                path: format!("{root}/apps/frontend/node_modules"),
                family: Family::ProjectArtifacts,
                project: Some("frontend".into()),
                gb: 2.8,
                items: 61_002,
                inactive_days: Some(0),
                why: "pnpm dependency tree".into(),
                regenerate: "pnpm install --frozen-lockfile".into(),
                active_process: Some("node (vite)".into()),
                privilege: None,
                method: Method::Trash,
                confidence: Confidence::High,
                shares_with: None,
                found_at: 5,
                protected: false,
            },
            Candidate {
                path: format!("{root}/services/api/target"),
                family: Family::ProjectArtifacts,
                project: Some("api".into()),
                gb: 6.9,
                items: 30_112,
                inactive_days: Some(2),
                why: "Cargo target directory".into(),
                regenerate: "cargo build".into(),
                active_process: None,
                privilege: None,
                method: Method::Tool("cargo clean".into()),
                confidence: Confidence::High,
                shares_with: None,
                found_at: 9,
                protected: false,
            },
        ];
        d
    };
    seed_custom(
        &mut w,
        &root,
        &[
            (
                "deploy.preview",
                "Deploy preview",
                &["tools/deploy-preview.sh", "--env", "preview"],
                "mutating",
                "push the current branch to the preview stack",
            ),
            (
                "rotate.secrets",
                "Rotate local secrets",
                &["tools/rotate-secrets.sh"],
                "destructive",
                "regenerate every local development secret",
            ),
        ],
        false,
    );
    w.sources = sources_for(
        motion,
        &[
            ("filesystem", 2),
            ("git", 4),
            ("mise", 6),
            ("children", 11),
            ("docker", 15),
            ("postgres", 19),
            ("system", 5),
            ("ssh", 7),
            ("github", 28),
        ],
        &[],
    );
    w
}

fn docker_cleanup(motion: Motion) -> World {
    let loc = Location {
        cwd: "/work/scratch".into(),
        home: "/home/alex".into(),
        project: None,
        workspace: None,
        children: vec![],
    };
    let mut w = base(Scenario::DockerCleanup, motion, host_devbox(), loc);
    w.docker = docker_devbox();
    w.mise.installed = true;
    w.mise.version = "2026.9.3".into();
    w.mise.global_tools = vec![
        tool("node", "26", "26.0.1", "26.1.0", true),
        tool("go", "1.24", "1.24.6", "1.25.1", true),
    ];
    w.system = system_quiet();
    w.disk = {
        let mut d = disk_quiet();
        d.filesystems = vec![Filesystem {
            mount: "/".into(),
            total_gb: 240,
            used_gb: 198,
        }];
        d.candidates = vec![Candidate {
            path: "/work/scratch".into(),
            family: Family::Temp,
            project: None,
            gb: 3.6,
            items: 4_812,
            inactive_days: Some(9),
            why: "scratch folder · 4,812 items including hidden files and 3 nested folders".into(),
            regenerate: "n/a".into(),
            active_process: None,
            privilege: None,
            method: Method::Permanent,
            confidence: Confidence::High,
            shares_with: None,
            found_at: 3,
            protected: false,
        }];
        d.large = vec![LargeEntry {
            path: "/var/lib/docker".into(),
            gb: 22.5,
            kind: "container data",
            found_at: 4,
        }];
        d
    };
    w.plans = vec![plan_docker_cleanup(&w)];
    w.memory.usage.seed(
        "docker.stop_all",
        None,
        "devbox",
        5,
        crate::scenario::EPOCH_SECS - 86_400,
    );
    w.sources = sources_for(
        motion,
        &[
            ("filesystem", 2),
            ("docker", 5),
            ("mise", 7),
            ("system", 9),
            ("ssh", 4),
            ("github", 30),
        ],
        &[],
    );
    w
}

fn work_collection() -> Location {
    let root = format!("{HOME}/work");
    Location {
        cwd: root.clone(),
        home: HOME.into(),
        project: Some(Project {
            name: "work".into(),
            root: root.clone(),
            kind: ProjectKind::Collection,
        }),
        workspace: None,
        children: vec![
            Project {
                name: "frontend".into(),
                root: format!("{root}/frontend"),
                kind: ProjectKind::Node { manager: "pnpm" },
            },
            Project {
                name: "backend".into(),
                root: format!("{root}/backend"),
                kind: ProjectKind::Rust {
                    workspace: true,
                    member: None,
                },
            },
            Project {
                name: "backend-wt".into(),
                root: format!("{root}/backend-wt"),
                kind: ProjectKind::Rust {
                    workspace: true,
                    member: None,
                },
            },
            Project {
                name: "android".into(),
                root: format!("{root}/android"),
                kind: ProjectKind::Gradle,
            },
            Project {
                name: "shared-libs".into(),
                root: format!("{root}/backend/vendor/shared-libs"),
                kind: ProjectKind::Rust {
                    workspace: false,
                    member: None,
                },
            },
        ],
    }
}

fn collection_git() -> Vec<GitState> {
    let root = format!("{HOME}/work");
    let mut frontend = GitState::clean(&format!("{root}/frontend"), "main", "main");
    frontend.behind = 2;
    frontend.head_short = "3d1e0f2".into();
    let mut backend = GitState::clean(&format!("{root}/backend"), "master", "master");
    backend.unstaged = 3;
    backend.modified = vec![
        "src/settlement.rs".into(),
        "src/lib.rs".into(),
        "Cargo.lock".into(),
    ];
    let mut wt = GitState::clean(&format!("{root}/backend-wt"), "feature/ledger", "master");
    wt.worktree_of = Some(format!("{root}/backend"));
    wt.ahead = 4;
    let mut android = GitState::clean(&format!("{root}/android"), "develop", "develop");
    android.head_short = "77a1c0e".into();
    let mut shared = GitState::clean(
        &format!("{root}/backend/vendor/shared-libs"),
        "main",
        "main",
    );
    shared.submodule = true;
    shared.diverged = true;
    shared.ahead = 1;
    shared.behind = 2;
    vec![frontend, backend, wt, android, shared]
}

fn disk_cleanup(motion: Motion) -> World {
    let mut w = base(Scenario::DiskCleanup, motion, host_mbp(), work_collection());
    w.git = collection_git();
    w.disk = disk_full();
    w.system = system_quiet();
    w.mise.installed = true;
    w.mise.version = "2026.9.3".into();
    w.docker = docker_acme();
    w.docker.compose = None;
    w.plans = vec![plan_git_pull_all(&w), plan_git_switch_primary(&w)];
    w.sources = sources_for(
        motion,
        &[
            ("filesystem", 2),
            ("git", 5),
            ("children", 9),
            ("mise", 7),
            ("docker", 12),
            ("system", 4),
            ("ssh", 6),
            ("github", 24),
        ],
        &[],
    );
    w
}

fn upgrade_plan(motion: Motion) -> World {
    let loc = Location {
        cwd: "/home/alex".into(),
        home: "/home/alex".into(),
        project: None,
        workspace: None,
        children: vec![],
    };
    let mut w = base(Scenario::UpgradePlan, motion, host_devbox(), loc);
    w.docker = docker_devbox();
    w.mise.installed = true;
    w.mise.version = "2026.9.3".into();
    w.mise.global_tools = vec![
        tool("node", "26", "26.0.1", "26.1.0", true),
        tool("python", "3.13", "3.13.5", "3.13.7", true),
        tool("go", "1.24", "1.24.6", "1.25.1", true),
        tool("pnpm", "10", "10.4.0", "10.6.2", true),
    ];
    w.apt = Some(AptState {
        upgradable: vec![
            AptPackage {
                name: "libssl3".into(),
                from: "3.0.12-1".into(),
                to: "3.0.13-1".into(),
                security: true,
            },
            AptPackage {
                name: "openssl".into(),
                from: "3.0.12-1".into(),
                to: "3.0.13-1".into(),
                security: true,
            },
            AptPackage {
                name: "curl".into(),
                from: "8.5.0-1".into(),
                to: "8.5.0-2".into(),
                security: false,
            },
            AptPackage {
                name: "libcurl4".into(),
                from: "8.5.0-1".into(),
                to: "8.5.0-2".into(),
                security: false,
            },
            AptPackage {
                name: "git".into(),
                from: "2.47.0-1".into(),
                to: "2.47.1-1".into(),
                security: false,
            },
            AptPackage {
                name: "tmux".into(),
                from: "3.5a-1".into(),
                to: "3.5a-2".into(),
                security: false,
            },
            AptPackage {
                name: "vim".into(),
                from: "9.1.1230-1".into(),
                to: "9.1.1320-1".into(),
                security: false,
            },
            AptPackage {
                name: "linux-image-amd64".into(),
                from: "6.12.9-1".into(),
                to: "6.12.12-1".into(),
                security: false,
            },
            AptPackage {
                name: "systemd".into(),
                from: "257.3-1".into(),
                to: "257.4-1".into(),
                security: false,
            },
            AptPackage {
                name: "libsystemd0".into(),
                from: "257.3-1".into(),
                to: "257.4-1".into(),
                security: false,
            },
            AptPackage {
                name: "udev".into(),
                from: "257.3-1".into(),
                to: "257.4-1".into(),
                security: false,
            },
            AptPackage {
                name: "ca-certificates".into(),
                from: "20250311".into(),
                to: "20250801".into(),
                security: false,
            },
            AptPackage {
                name: "docker-ce".into(),
                from: "28.3.2".into(),
                to: "28.4.0".into(),
                security: false,
            },
            AptPackage {
                name: "containerd.io".into(),
                from: "1.7.27".into(),
                to: "1.7.28".into(),
                security: false,
            },
        ],
        pending_reboot: false,
        lock_held_by: None,
        free_gb: 41.2,
        sudo_cached: true,
    });
    w.system = system_quiet();
    w.disk = {
        let mut d = disk_quiet();
        d.filesystems = vec![Filesystem {
            mount: "/".into(),
            total_gb: 240,
            used_gb: 198,
        }];
        d
    };
    w.plans = vec![plan_upgrade(&w)];
    w.sources = sources_for(
        motion,
        &[
            ("filesystem", 2),
            ("apt", 6),
            ("mise", 8),
            ("docker", 10),
            ("system", 4),
            ("ssh", 3),
            ("github", 26),
        ],
        &[],
    );
    w
}

fn activities_multi(motion: Motion) -> World {
    let mut w = monorepo(motion, false);
    w.scenario = Scenario::ActivitiesMulti;
    let root = format!("{HOME}/work/acme");
    let fe = ScopeTag::child(&format!("{root}/apps/frontend"));
    let api = ScopeTag::child(&format!("{root}/services/api"));
    let here = ScopeTag::here(&root);
    let host = ScopeTag::host("mbp");
    let id = w.start_scripted(
        "task:frontend-dev",
        "frontend dev",
        "Start frontend dev",
        ActivityKind::Task,
        fe,
    );
    if let Some(a) = w.activity_mut(&id) {
        a.insights = vec![
            ("port".into(), "5173".into()),
            ("url".into(), "http://localhost:5173/".into()),
            ("pid".into(), "7731".into()),
        ];
        a.started_tick = 0;
    }
    let id = w.start_scripted(
        "task:api-dev",
        "api dev",
        "Start api dev",
        ActivityKind::Task,
        api,
    );
    if let Some(a) = w.activity_mut(&id) {
        a.insights = vec![
            ("port".into(), "8080".into()),
            ("pid".into(), "9120".into()),
        ];
    }
    let id = w.start_scripted(
        "task:tests",
        "api tests",
        "Run api tests",
        ActivityKind::Task,
        w.location
            .project
            .as_ref()
            .map(|p| ScopeTag::project(&p.root, &p.root))
            .unwrap_or(here.clone()),
    );
    if let Some(a) = w.activity_mut(&id) {
        a.follow_ups = vec![
            "Open the failing test".into(),
            "Run tests again".into(),
            "Review 4 modified files".into(),
        ];
    }
    let id = w.start_scripted(
        "logs:acme",
        "logs · api worker scheduler",
        "Follow logs from api, worker and scheduler",
        ActivityKind::Logs {
            services: vec!["api".into(), "worker".into(), "scheduler".into()],
        },
        here,
    );
    if let Some(a) = w.activity_mut(&id) {
        a.insights = vec![
            ("compose".into(), "acme".into()),
            ("errors".into(), "3 in 2 min".into()),
        ];
    }
    let id = w.start_scripted(
        "monitor:btm",
        "btm",
        "Open system monitor",
        ActivityKind::Monitor { tool: "btm".into() },
        host,
    );
    if let Some(a) = w.activity_mut(&id) {
        a.state = crate::domain::activity::ActivityState::Detached;
    }

    for _ in 0..48 {
        w.tick();
    }
    w
}

fn remote_host(motion: Motion) -> World {
    let loc = Location {
        cwd: "/srv/payments".into(),
        home: "/home/deploy".into(),
        project: Some(Project {
            name: "payments".into(),
            root: "/srv/payments".into(),
            kind: ProjectKind::Service,
        }),
        workspace: None,
        children: vec![],
    };
    let mut w = base(Scenario::RemoteHost, motion, host_prod(), loc);
    let mut g = GitState::clean("/srv/payments", "master", "master");
    g.branch = None;
    g.head_short = "v2.14.3".into();
    g.upstream = None;
    w.git = vec![g];
    w.system = SystemSnapshot {
        cpu_pct: 34,
        cores: vec![40, 31, 36, 29],
        load: "2.8 2.4 2.1".into(),
        mem_used_gb: 14.1,
        mem_total_gb: 16.0,
        swap_used_gb: 2.3,
        pressure: Some((18, 4)),
        pressure_minutes: 11,
        disk_io: vec![("nvme0n1".into(), "2.1 MB/s read · 9.4 MB/s write".into())],
        net: vec![("eth0".into(), "rx 4.1 MB/s · tx 1.9 MB/s".into())],
        top: vec![
            Proc {
                pid: 41903,
                name: "payments-worker".into(),
                cpu_pct: 21,
                mem_mb: 5100,
                state: "running".into(),
                port: None,
            },
            Proc {
                pid: 1288,
                name: "payments".into(),
                cpu_pct: 9,
                mem_mb: 3900,
                state: "running".into(),
                port: Some(8443),
            },
            Proc {
                pid: 812,
                name: "postgres".into(),
                cpu_pct: 3,
                mem_mb: 2200,
                state: "sleeping".into(),
                port: Some(5432),
            },
        ],
        btm_installed: true,
        temp_c: None,
    };
    w.docker = DockerState::unavailable("docker is not installed on prod-eu-1");
    w.pg = Some({
        let mut p = pg_acme(true);
        p.label = "payments@localhost:5432/payments".into();
        p.source = "PGSERVICE=payments in /etc/payments/env · password never shown".into();
        p.db = "payments".into();
        p.user = "payments".into();
        p.connections = 91;
        p.replication_lag = Some("replay lag 1.4 s on replica-1".into());
        for s in &mut p.sessions {
            s.db = "payments".into();
        }
        p
    });
    w.disk = {
        let mut d = disk_quiet();
        d.filesystems = vec![
            Filesystem {
                mount: "/".into(),
                total_gb: 120,
                used_gb: 96,
            },
            Filesystem {
                mount: "/var/lib/postgresql".into(),
                total_gb: 500,
                used_gb: 388,
            },
        ];
        d.candidates = vec![Candidate {
            path: "/var/log/payments".into(),
            family: Family::Logs,
            project: Some("payments".into()),
            gb: 4.2,
            items: 1_204,
            inactive_days: Some(30),
            why: "rotated logs older than 30 days".into(),
            regenerate: "n/a".into(),
            active_process: None,
            privilege: Some("sudo".into()),
            method: Method::Permanent,
            confidence: Confidence::High,
            shares_with: None,
            found_at: 6,
            protected: false,
        }];
        d
    };
    w.memory.usage.seed(
        "service.journal.payments-worker",
        Some("/srv/payments"),
        "prod-eu-1",
        5,
        crate::scenario::EPOCH_SECS - 3_600,
    );
    w.sources = sources_for(
        motion,
        &[
            ("filesystem", 2),
            ("git", 5),
            ("systemd", 4),
            ("postgres", 9),
            ("system", 3),
            ("ssh", 6),
        ],
        &[],
    );
    w
}

fn launch_failure(motion: Motion) -> World {
    let mut w = monorepo(motion, true);
    w.scenario = Scenario::LaunchFailure;
    let root = format!("{HOME}/work/acme");
    for c in &mut w.mise.configs {
        c.trusted = true;
    }
    w.system.top.push(Proc {
        pid: 5120,
        name: "node (stale vite)".into(),
        cpu_pct: 0,
        mem_mb: 310,
        state: "sleeping".into(),
        port: Some(5173),
    });
    let id = w.start_scripted(
        "task:frontend-dev-fail",
        "frontend dev",
        "Start frontend dev",
        ActivityKind::Task,
        ScopeTag::here(&format!("{root}/apps/frontend")),
    );
    if let Some(a) = w.activity_mut(&id) {
        a.follow_ups = vec![
            "Find the process on port 5173".into(),
            "Retry frontend dev".into(),
            "Open the mise task definition".into(),
        ];
    }
    w
}

fn hard_cases(motion: Motion) -> World {
    let root = format!("{HOME}/work/clients/northwind-traders-platform-migration");
    let svc = format!("{root}/services/inventory-reconciliation-service");
    let loc = Location {
        cwd: svc.clone(),
        home: HOME.into(),
        project: Some(Project {
            name: "inventory-reconciliation-service".into(),
            root: svc.clone(),
            kind: ProjectKind::Rust {
                workspace: true,
                member: Some("inventory-reconciliation-core".into()),
            },
        }),
        workspace: Some(Project {
            name: "northwind-traders-platform-migration".into(),
            root: root.clone(),
            kind: ProjectKind::MiseMonorepo,
        }),
        children: (0..14)
            .map(|i| Project {
                name: format!(
                    "crate-{:02}-{}",
                    i,
                    [
                        "ledger",
                        "reconciliation",
                        "adapters",
                        "fixtures",
                        "cli",
                        "warehouse-sync",
                        "notifications",
                        "reporting",
                        "audit",
                        "migrations",
                        "bench",
                        "proto",
                        "sdk",
                        "legacy-compat"
                    ][i]
                ),
                root: format!("{svc}/crates/{i:02}"),
                kind: ProjectKind::Rust {
                    workspace: false,
                    member: None,
                },
            })
            .collect(),
    };
    let mut w = base(Scenario::HardCases, motion, host_mbp(), loc);
    let mut g = GitState::clean(
        &svc,
        "feature/NWT-4821-reconcile-partial-shipments-with-backdated-invoices",
        "develop",
    );
    g.in_progress = Some("rebase".into());
    g.conflicts = 2;
    g.unstaged = 7;
    g.behind = 14;
    g.ahead = 3;
    g.diverged = true;
    g.head_short = "e4d1c0b".into();
    w.git = vec![g];
    let mut m = mise_holla(false);
    m.configs[0].path = format!("{svc}/mise.toml");
    m.configs[0].tasks = (0..26)
        .map(|i| task(&format!("task-{i:02}"), &format!("task-{i:02}"), &svc, &format!("cargo run -p inventory-reconciliation-cli -- step-{i:02} --very-long-flag-name-that-keeps-going"), &[], "generated task with an intentionally long description that will need truncation in every column"))
        .collect();
    m.configs[0].tasks.insert(0, task("reconcile-partial-shipments-against-backdated-invoices-and-emit-audit-report", "reconcile-partial-shipments-against-backdated-invoices-and-emit-audit-report", &svc, "cargo run -p inventory-reconciliation-cli -- reconcile --partial --backdated --emit-audit", &[], "the task with the longest name in the fixture"));
    w.mise = m;
    w.docker = DockerState::unavailable(
        "Cannot connect to the Docker daemon at unix:///var/run/docker.sock",
    );
    w.system = system_pressure();
    w.disk = {
        let mut d = disk_full();
        d.partial_reason = Some("2 folders unreadable · permission denied".into());
        d
    };
    w.sources = sources_for(
        motion,
        &[
            ("filesystem", 2),
            ("git", 5),
            ("mise", 8),
            ("cargo", 12),
            ("system", 4),
            ("ssh", 6),
            ("children", 14),
        ],
        &[
            (
                "docker",
                9,
                "Cannot connect to the Docker daemon at unix:///var/run/docker.sock",
            ),
            ("github", 20, "gh: not logged in · run gh auth login"),
        ],
    );
    w
}

pub fn plan_docker_cleanup(w: &World) -> Plan {
    let d = &w.docker;
    let host = w.host.name.clone();
    let running: Vec<&str> = d
        .containers
        .iter()
        .filter(|c| c.running)
        .map(|c| c.name.as_str())
        .collect();
    let all: Vec<&str> = d.containers.iter().map(|c| c.name.as_str()).collect();
    let stop_cmd = format!("docker stop {}", running.join(" "));
    let rm_cmd = format!("docker rm {}", all.join(" "));
    let steps = vec![
        Step::new("stop", "Stop running containers", &[&stop_cmd], "/")
            .exclusive("docker daemon")
            .effects(&[&format!(
                "{} running containers stop gracefully (10 s timeout)",
                running.len()
            )])
            .ticks(40),
        Step::new("rm", "Remove containers", &[&rm_cmd], "/")
            .after(&[0])
            .exclusive("docker daemon")
            .effects(&[&format!(
                "{} containers removed · writable layers lost",
                all.len()
            )])
            .ticks(28),
        Step::new(
            "images",
            "Remove unused images",
            &["docker image prune -a --force"],
            "/",
        )
        .after(&[1])
        .effects(&[&format!("{} images · {:.1} GB", d.images, d.images_gb)])
        .ticks(64),
        Step::new(
            "networks",
            "Prune networks",
            &["docker network prune --force"],
            "/",
        )
        .after(&[1])
        .effects(&[&format!("{} custom networks", d.networks)])
        .ticks(16),
        Step::new(
            "volumes",
            "Prune volumes",
            &["docker volume prune -a --force"],
            "/",
        )
        .after(&[1])
        .effects(&[&format!(
            "{} named + {} anonymous volumes · {:.1} GB · permanent",
            d.named_volumes(),
            d.volumes.len() - d.named_volumes(),
            d.volumes_gb()
        )])
        .note("named volumes hold durable data; a generic system prune would keep them")
        .ticks(34),
        Step::new(
            "builder",
            "Prune builder and Buildx caches",
            &[
                "docker builder prune -a --force",
                "docker buildx prune -a --force",
            ],
            "/",
        )
        .after(&[1])
        .effects(&[&format!("{:.1} GB of build cache", d.builder_cache_gb)])
        .ticks(46),
        Step::new("verify", "Rescan and report", &["docker system df"], "/")
            .after(&[2, 3, 4, 5])
            .effects(&["reclaimed and remaining space reported"])
            .ticks(14),
    ];
    Plan::new("docker-cleanup", "Clean Docker completely", &format!("remove all user-removable Docker state on {host}"), &host, steps)
        .phrase(&format!("REMOVE ALL DOCKER DATA ON {host}"), true)
        .fact("Docker accounting", &format!("{} containers ({} running) · {} images {:.1} GB · {} volumes {:.1} GB · build cache {:.1} GB", d.containers.len(), d.running(), d.images, d.images_gb, d.volumes.len(), d.volumes_gb(), d.builder_cache_gb))
        .fact("Reclaimable", &format!("~{:.1} GB", d.reclaimable_gb))
        .fact("Recoverable", "no · images, containers and volumes are permanent")
        .fact("Not touched", "the Docker daemon itself · its configuration")
        .drift("a new container (tmp-shell-3) appeared")
        .follow_up("Docker disk usage", "docker.df")
}

pub fn plan_upgrade(w: &World) -> Plan {
    let host = w.host.name.clone();
    let apt = w.apt.as_ref();
    let n = apt.map(|a| a.upgradable.len()).unwrap_or(0);
    let sec = apt
        .map(|a| a.upgradable.iter().filter(|p| p.security).count())
        .unwrap_or(0);
    let steps = vec![
        Step::new(
            "preflight",
            "Preflight",
            &[
                "cat /etc/os-release",
                "sudo -n true",
                "fuser /var/lib/dpkg/lock-frontend",
                "df -h /",
                "test -f /var/run/reboot-required",
            ],
            "/",
        )
        .effects(&["checks OS, privileges, package lock, network, free space, pending reboot"])
        .privilege("sudo")
        .ticks(22),
        Step::new(
            "apt-update",
            "Refresh Debian metadata",
            &["sudo apt-get update"],
            "/",
        )
        .after(&[0])
        .exclusive("apt lock")
        .privilege("sudo")
        .effects(&["package lists refreshed · nothing installed"])
        .ticks(34),
        Step::new(
            "apt-review",
            "Review Debian upgrades",
            &["apt list --upgradable"],
            "/",
        )
        .after(&[1])
        .effects(&[&format!("{n} upgradable · {sec} security")])
        .ticks(16),
        Step::new(
            "apt-apply",
            "Apply Debian upgrades",
            &["sudo apt-get upgrade -y"],
            "/",
        )
        .after(&[2])
        .exclusive("apt lock")
        .privilege("sudo")
        .effects(&[&format!("{n} packages upgraded · 38.4 MB downloaded")])
        .note("linux-image-amd64 is included: a reboot will be required")
        .ticks(110),
        Step::new(
            "mise-inspect",
            "Inspect global mise tools",
            &["mise outdated --json"],
            "/",
        )
        .after(&[0])
        .effects(&["4 outdated tools listed"])
        .ticks(28),
        Step::new(
            "mise-upgrade",
            "Upgrade selected mise tools",
            &["mise upgrade --exclude go"],
            "/",
        )
        .after(&[4])
        .optional()
        .effects(&[
            "node 26.0.1 → 26.1.0 · python 3.13.5 → 3.13.7 · pnpm 10.4.0 → 10.6.2 · go excluded",
        ])
        .ticks(70)
        .fails(),
        Step::new(
            "cleanup",
            "Package cleanup",
            &["sudo apt-get autoremove --purge -y"],
            "/",
        )
        .after(&[3])
        .optional()
        .exclusive("apt lock")
        .privilege("sudo")
        .effects(&["2 packages removed"])
        .ticks(26),
        Step::new(
            "verify",
            "Final verification",
            &[
                "apt list --upgradable",
                "mise ls --json",
                "systemctl --failed",
                "test -f /var/run/reboot-required",
            ],
            "/",
        )
        .after(&[3, 5])
        .effects(&["package state, tool versions, services and reboot need reported"])
        .ticks(20),
    ];
    Plan::new(
        "upgrade",
        "Upgrade everything",
        &format!("upgrade Debian packages and global mise tools on {host}"),
        &host,
        steps,
    )
    .fact("Privilege", "sudo · cached for this session")
    .fact("Debian", &format!("{n} upgradable · {sec} security"))
    .fact("mise", "4 outdated global tools · go excluded")
    .fact("Reboot", "required after the kernel upgrade")
    .follow_up("Outdated tools", "mise.outdated")
}

pub fn plan_upgrade_all(w: &World) -> Plan {
    let host = w.host.name.clone();
    let fails = |stage: &str| w.upgrade.failing.iter().any(|f| f == stage);
    let mut steps = vec![];
    let mut brew_tail: Option<usize> = None;
    let mut managers = vec![];
    if w.brew.is_some() {
        managers.push("Homebrew");
        let n = w.upgrade.brew_outdated.len();
        let mut s = Step::new("brew-update", "brew update", &["brew update"], "/")
            .effects(&["formulae index refreshed"])
            .ticks(18);
        if fails("brew update") {
            s = s.fails();
        }
        steps.push(s);
        steps.push(
            Step::new(
                "brew-upgrade",
                "brew upgrade",
                &["brew upgrade --greedy --yes"],
                "/",
            )
            .after(&[0])
            .effects(&[&format!("{n} formulae upgraded")])
            .ticks(60),
        );
        steps.push(
            Step::new(
                "brew-cleanup",
                "brew cleanup",
                &["brew cleanup", "brew autoremove"],
                "/",
            )
            .after(&[1])
            .optional()
            .effects(&["old kegs and orphans removed"])
            .ticks(14),
        );
        let mut d = Step::new("brew-doctor", "brew doctor", &["brew doctor"], "/")
            .after(&[2])
            .optional()
            .effects(&["warnings reported, never fixed"])
            .ticks(10);
        if fails("brew doctor") {
            d = d.fails();
        }
        steps.push(d);
        brew_tail = Some(3);
        if w.host.os == Os::MacOs {
            let c = w.upgrade.casks_outdated.len();
            let mut s = Step::new(
                "brew-casks",
                "brew upgrade --cask",
                &["brew upgrade --cask --greedy --yes"],
                "/",
            )
            .after(&[0])
            .effects(&[&format!("{c} casks upgraded")])
            .ticks(50);
            if fails("brew upgrade --cask") {
                s = s.fails();
            }
            steps.push(s);
        }
    }
    if w.mise.installed {
        managers.push("mise");
        let n = w.mise.outdated().len();
        let mut s = Step::new("mise-upgrade", "mise upgrade", &["mise upgrade"], "/")
            .effects(&[&format!("{n} tools moved to their latest versions")])
            .ticks(40);
        if fails("mise upgrade") {
            s = s.fails();
        }
        steps.push(s);
    }
    if w.upgrade.amp {
        managers.push("amp");
        let mut s = Step::new("amp-update", "amp update", &["amp update"], "/")
            .effects(&["amp binary replaced in place"])
            .ticks(12);
        if fails("amp update") {
            s = s.fails();
        }
        steps.push(s);
    }
    if let Some(dir) = crate::domain::catalog::omz_dir(w) {
        managers.push("Oh My Zsh");
        let mut s = Step::new("omz-upgrade", "omz update", &["zsh -ic 'omz update'"], &dir)
            .effects(&["framework pulled to its latest commit"])
            .ticks(16);
        if fails("omz") {
            s = s.fails();
        }
        steps.push(s);
    }
    let n = steps.len();
    steps.push(
        Step::new(
            "verify",
            "Re-probe managers",
            &["brew outdated", "mise outdated", "amp --version"],
            "/",
        )
        .after(
            &(0..n)
                .filter(|i| brew_tail.is_none_or(|t| *i >= t || *i == 0))
                .collect::<Vec<_>>(),
        )
        .effects(&["every manager reports its post-upgrade state"])
        .ticks(12),
    );
    Plan::new(
        "upgrade-all",
        "Upgrade everything",
        &format!("upgrade every detected manager on {host}: {}", managers.join(", ")),
        &host,
        steps,
    )
    .fact("Managers", &managers.join(", "))
    .fact("Order", "Homebrew stages chain · the others run beside them · a failed stage stops only its own chain")
    .fact("Availability", "re-probed right before execution · a manager that vanished is skipped, never faked")
    .follow_up("Outdated tools", "mise.outdated")
}

pub fn plan_cleanup_work(w: &World, selected: &[String]) -> Plan {
    let root = w.location.cwd.clone();
    let host = w.host.name.clone();
    let mut steps = vec![];
    let mut idx = BTreeMap::new();
    for c in &w.disk.candidates {
        if !selected.contains(&c.path) || c.skip_reason().is_some() {
            continue;
        }
        let id = c
            .path
            .trim_start_matches(&format!("{HOME}/"))
            .replace(['/', '.'], "-");
        let id = match id.as_str() {
            "work-frontend-node_modules" => "frontend-node_modules".to_owned(),
            "work-backend-target" => "backend-target".to_owned(),
            "work-android-build" => "android-build".to_owned(),
            "Library-pnpm-store-v3" => "pnpm-store".to_owned(),
            other => other.to_owned(),
        };
        let leaf = c.path.rsplit('/').next().unwrap_or("").to_owned();
        let owner = c
            .project
            .clone()
            .unwrap_or_else(|| w.location.short(&c.path));
        let target = match &c.project {
            Some(p) if !p.ends_with(leaf.as_str()) => format!("{p} › {leaf}"),
            Some(p) => p.clone(),
            None => w.location.short(&c.path),
        };
        let (label, cmd) = match &c.method {
            Method::Trash => (format!("Trash {target}"), format!("trash {}", c.path)),
            Method::Permanent => (format!("Delete {target}"), format!("rm -rf {}", c.path)),
            Method::Tool(t) => (
                format!("{t} · {owner}"),
                format!(
                    "{t}  (in {})",
                    c.project
                        .as_ref()
                        .map(|p| format!("{root}/{p}"))
                        .unwrap_or(c.path.clone())
                ),
            ),
        };
        let mut s = Step::new(
            &id,
            &label,
            &[&cmd],
            c.project
                .as_ref()
                .map(|p| format!("{root}/{p}"))
                .unwrap_or(root.clone())
                .as_str(),
        )
        .effects(&[&format!(
            "{:.1} GB · {} items · {}",
            c.gb,
            termrock::ui::text::thousands(c.items as usize),
            c.method.label()
        )])
        .optional()
        .target(&c.path)
        .ticks(20 + (c.gb * 4.0) as u64);
        if let Some(shared) = &c.shares_with {
            s = s.exclusive(&format!("cargo target shared with {shared}"));
        }
        idx.insert(c.path.clone(), steps.len());
        steps.push(s);
    }
    let deps: Vec<usize> = (0..steps.len()).collect();
    steps.push(
        Step::new(
            "verify",
            "Rescan and report",
            &[&format!("holla disk rescan {root}")],
            &root,
        )
        .after(&deps)
        .effects(&["reclaimed, skipped and failed totals · audit history entry"])
        .ticks(46),
    );
    let total: f32 = w
        .disk
        .candidates
        .iter()
        .filter(|c| selected.contains(&c.path) && c.skip_reason().is_none())
        .map(|c| c.gb)
        .sum();
    Plan::new(
        "cleanup-work",
        &format!("Clean developer artifacts under {}", w.location.cwd_short()),
        &format!("remove selected build artifacts and caches under {root}"),
        &host,
        steps,
    )
    .phrase(&format!("REMOVE ALL BUILD ARTIFACTS UNDER {root}"), false)
    .fact(
        "Targets",
        &format!("{} candidates selected · {total:.1} GB", idx.len()),
    )
    .fact(
        "Recovery",
        "Trash where possible · tool-managed cleanups regenerate on the next build",
    )
    .fact(
        "Not touched",
        "recent artifacts, unverifiable activity, protected paths",
    )
    .follow_up("Disk usage", "disk.usage")
    .follow_up("Cleanup history", "disk.history")
}

pub fn plan_git_pull_all(w: &World) -> Plan {
    let host = w.host.name.clone();
    let mut steps = vec![];
    let mut done = std::collections::BTreeSet::new();
    for g in &w.git {
        if g.worktree_of.is_some() {
            continue;
        }
        let name = g.path.rsplit('/').next().unwrap_or("project").to_owned();
        if !done.insert(name.clone()) {
            continue;
        }
        let cmd = format!("git -C {} pull --ff-only", g.path);
        let mut s = Step::new(
            &name,
            &format!(
                "Pull {name} · {}",
                g.branch.clone().unwrap_or("detached".into())
            ),
            &[&cmd],
            &g.path,
        )
        .optional()
        .ticks(18 + g.behind as u64 * 6);
        if let Some(reason) = g.block_reason() {
            s = s.excluded().note(&format!("blocked · {reason}"));
        } else if g.behind == 0 {
            s = s.effects(&["already up to date"]);
        } else {
            s = s.effects(&[&format!("fast-forward {} commits", g.behind)]);
        }
        if g.submodule {
            s = s.note("submodule · pulled inside its parent");
        }
        steps.push(s);
    }
    let deps: Vec<usize> = (0..steps.len()).collect();
    steps.push(
        Step::new(
            "verify",
            "Per-project results",
            &["git -C <each> status --porcelain=v2 --branch"],
            &w.location.cwd,
        )
        .after(&deps)
        .ticks(12),
    );
    Plan::new(
        "git-pull-all",
        "Pull every child project",
        "fast-forward-only pulls across the discovered Git projects",
        &host,
        steps,
    )
    .fact("Policy", "fast-forward only · never reset, never stash")
    .fact(
        "Discovery",
        &format!(
            "{} projects · 1 worktree deduplicated · 1 submodule",
            w.git.len()
        ),
    )
}

pub fn plan_git_switch_primary(w: &World) -> Plan {
    let host = w.host.name.clone();
    let mut steps = vec![];
    for g in &w.git {
        if g.worktree_of.is_some() {
            continue;
        }
        let name = g.path.rsplit('/').next().unwrap_or("project").to_owned();
        let cmd = format!("git -C {} switch {}", g.path, g.primary);
        let mut s = Step::new(
            &name,
            &format!("Switch {name} to {}", g.primary),
            &[&cmd],
            &g.path,
        )
        .optional()
        .ticks(16);
        if let Some(reason) = g.block_reason() {
            s = s.excluded().note(&format!("blocked · {reason}"));
        } else if g.branch.as_deref() == Some(g.primary.as_str()) {
            s = s.effects(&["already on the primary branch"]);
        }
        steps.push(s);
    }
    Plan::new("git-switch-primary", "Switch every project to its primary branch", "resolve each project's primary branch and switch to it · main, master or develop as each project defines", &host, steps)
        .fact("Resolution", "primary branch read from origin/HEAD per project · never hard-coded")
        .fact("Blocked", "dirty, detached, diverged or in-progress projects are skipped without discarding work")
}
