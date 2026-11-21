use std::collections::BTreeMap;

use crate::arbiter::DiscoveryError;
use crate::backend::OpSession;
use crate::domain::account::{
    Account, AccountId, AccountIdentity, AccountRegistry, Confidence, CredentialSource,
    DetectedKind, IdentitySubject, IssueCode, Lifecycle, Recoverability, RecoverableIssue,
    ValidationLevel, ValidationState,
};
use crate::domain::agent::{Agent, AuthMode, Provider};
use crate::domain::instance::{
    DaemonSnapshot, Instance, InstanceEntry, InstanceId, InstanceStatus, ManifestError,
    SessionRecord, SessionStatus,
};
use crate::domain::onepassword::OpReference;
use crate::domain::reason::{IssueReason, UsageReason};
use crate::domain::surfaces::Surfaces;
use crate::domain::usage::{
    AccountUsage, FreshnessInfo, QuotaStatus, QuotaWindow, WindowCategory, WindowUnit,
};
use crate::domain::workspace::{
    AllowedRoles, EnvVar, Isolation, Mount, MountScope, RoleEntry, RolePolicy, RoleSource,
    Workspace,
};
use crate::panes::{Daemon, PaneBody, PaneRequest, PaneSpawn};
use crate::sim::SimBackend;
use crate::sim::onepassword::SimOnePassword;
use crate::sim::pane::ScriptPane;
use crate::sim::scripts::SimScripts;
use crate::world::{
    DaemonHealth, FsEntry, GithubRepo, GlobalConfig, TrustKind, TrustRow, World, WorldParts,
    refresh_snapshots,
};
use termrock::ui::layout::SplitDir;

pub mod scenario;
pub mod surfaces;
pub mod terminal;
pub mod vocabulary;

pub use scenario::{
    CLOCK, EPOCH_SECS, Scenario, UTC_OFFSET_SECS, app_for, app_with_world, entry_for,
};

pub const HOME: &str = "/Users/alexey";

fn op_ref(account: &str, vault: (&str, &str), item: (&str, &str), field: &str) -> OpReference {
    OpReference {
        account: account.into(),
        vault_id: vault.0.into(),
        vault_name: vault.1.into(),
        item_id: item.0.into(),
        item_title: item.1.into(),
        section: None,
        field_id: field.into(),
        field_label: field.into(),
    }
}

fn handle(s: &str) -> Option<IdentitySubject> {
    Some(IdentitySubject::Handle(s.into()))
}

fn roles() -> Vec<RoleEntry> {
    let git = |name: &str, desc: &str, trusted: bool| RoleEntry {
        namespace: "chainargos".into(),
        name: name.into(),
        source: RoleSource::Git {
            url: "github.com/chainargos/roles".into(),
            branch: Some("main".into()),
        },
        trusted,
        in_registry: true,
        description: desc.into(),
        load_error: None,
        fetch_error: None,
    };
    vec![
        git(
            "the-architect",
            "Full-stack design and refactoring; Claude Code default",
            true,
        ),
        git("backend", "Rust and Postgres services", true),
        git(
            "reviewer",
            "Read-mostly code review with limited write scope",
            true,
        ),
        git("sre", "Infrastructure, Terraform, Kubernetes", true),
        RoleEntry {
            namespace: "acme-labs".into(),
            name: "data-eng".into(),
            source: RoleSource::Git {
                url: "github.com/acme-labs/roles-experimental".into(),
                branch: Some("next".into()),
            },
            trusted: false,
            in_registry: true,
            description: "Notebook and pipeline tooling (untrusted source)".into(),
            load_error: Some("trust required".into()),
            fetch_error: None,
        },
        RoleEntry {
            namespace: "local".into(),
            name: "writer".into(),
            source: RoleSource::Local {
                path: "~/roles/writer".into(),
            },
            trusted: true,
            in_registry: false,
            description: "Docs and release notes".into(),
            load_error: None,
            fetch_error: None,
        },
    ]
}

fn generated_roles(n: usize) -> Vec<RoleEntry> {
    const AREAS: [&str; 6] = ["billing", "ledger", "search", "notify", "ingest", "auth"];
    (1..=n)
        .map(|i| RoleEntry {
            namespace: "chainargos".into(),
            name: format!("svc-{i:03}"),
            source: RoleSource::Git {
                url: "github.com/chainargos/roles".into(),
                branch: Some("main".into()),
            },
            trusted: true,
            in_registry: true,
            description: format!("{} service agent #{i}", AREAS[i % AREAS.len()]),
            load_error: None,
            fetch_error: None,
        })
        .collect()
}

fn workspaces(rich: bool) -> Vec<Workspace> {
    let mut v = vec![];
    let mut w = Workspace::new(1, "payments-platform", "/workspace/payments-platform");
    w.mounts = vec![
        Mount::host("~/src/payments-platform", "/workspace/payments-platform")
            .repository()
            .isolation(Isolation::Worktree),
        Mount::host("~/src/shared-libs", "/workspace/libs").readonly(true),
    ];
    w.roles = RolePolicy {
        allowed: AllowedRoles::Custom(vec![
            "the-architect".into(),
            "backend".into(),
            "reviewer".into(),
        ]),
        default: Some("the-architect".into()),
        last: Some("the-architect".into()),
    };
    w.env = vec![
        EnvVar::plain(
            "DATABASE_URL",
            "postgres://payments:pw-fixture-only@db.internal:5432/payments",
        ),
        EnvVar::op(
            "STRIPE_KEY",
            op_ref(
                "chainargos.1password.com",
                ("v_eng01", "Engineering"),
                ("it_str01", "Stripe · sandbox"),
                "credential",
            ),
        ),
        EnvVar::plain("LOG_LEVEL", "debug"),
        EnvVar::host("GH_TOKEN", "GH_TOKEN"),
    ];
    w.role_env.insert(
        "backend".into(),
        vec![EnvVar::op(
            "OPENAI_API_KEY",
            op_ref(
                "chainargos.1password.com",
                ("v_eng01", "Engineering"),
                ("it_cdx01", "OpenAI · Codex Primary"),
                "credential",
            ),
        )],
    );

    w.accounts.enabled.insert("acct-claude-work".into());
    w.accounts.role_preferred.insert(
        ("reviewer".into(), Provider::Anthropic),
        "acct-claude-work".into(),
    );
    w.keep_awake = true;
    v.push(w);

    let mut w = Workspace::new(2, "infra-control-plane", "/workspace/infra-control-plane");
    w.mounts = vec![
        Mount::host(
            "~/src/infra-control-plane",
            "/workspace/infra-control-plane",
        )
        .repository(),
    ];
    w.roles = RolePolicy {
        allowed: AllowedRoles::All,
        default: Some("sre".into()),
        last: Some("sre".into()),
    };
    w.env = vec![
        EnvVar::op(
            "CLOUDFLARE_API_TOKEN",
            op_ref(
                "chainargos.1password.com",
                ("v_eng01", "Engineering"),
                ("it_cf01", "Cloudflare · infra"),
                "credential",
            ),
        ),
        EnvVar::plain("TF_LOG", "WARN"),
    ];

    w.accounts
        .disabled_defaults
        .insert("acct-codex-primary".into());
    w.accounts.enabled.insert("acct-codex-experiments".into());
    w.accounts
        .preferred
        .insert(Provider::OpenAi, "acct-codex-experiments".into());
    v.push(w);

    let mut w = Workspace::new(3, "release-automation", "/workspace/release-automation");
    w.mounts = vec![Mount::git(
        "github.com/chainargos/release-automation",
        "/workspace/release-automation",
    )];
    w.roles = RolePolicy {
        allowed: AllowedRoles::Custom(vec!["backend".into()]),
        default: Some("backend".into()),
        last: None,
    };
    w.git_pull = false;
    v.push(w);

    let mut w = Workspace::new(4, "customer-portal", "/workspace/customer-portal");
    w.mounts = vec![
        Mount::host("~/src/customer-portal", "/workspace/customer-portal").repository(),
        Mount::host("~/design/portal-assets", "/workspace/assets").readonly(true),
    ];
    w.roles = RolePolicy {
        allowed: AllowedRoles::All,
        default: None,
        last: Some("writer".into()),
    };
    w.env = vec![EnvVar::plain("NEXT_PUBLIC_API", "https://api.portal.local")];
    v.push(w);

    if rich {
        let names = [
            "data-pipeline",
            "docs-site",
            "auth-service",
            "gateway",
            "mobile-app",
            "ml-notebooks",
            "search-indexer",
            "staging-env",
            "billing-reconciliation-service-with-a-very-long-name-for-truncation",
            "legacy-monolith",
        ];
        for (i, n) in names.iter().enumerate() {
            let mut w = Workspace::new(10 + i as u32, n, &format!("/workspace/{n}"));
            let src = if i == 8 {
                "~/src/enterprise/platform/services/billing/reconciliation/billing-reconciliation-service-with-a-very-long-name-for-truncation".to_owned()
            } else {
                format!("~/src/{n}")
            };
            w.mounts = vec![Mount::host(&src, &format!("/workspace/{n}")).repository()];
            v.push(w);
        }
    }
    v
}

#[allow(clippy::too_many_arguments)]
fn instance(
    id: &str,
    ws: Option<u32>,
    wsname: &str,
    role: &str,
    agent: Agent,
    status: InstanceStatus,
    created: i64,
    last_seen: i64,
) -> Instance {
    Instance {
        id: crate::domain::instance::InstanceId::simulated(id),
        container: format!("jackin-{wsname}"),
        workspace: ws,
        workdir: Some(format!("/workspace/{wsname}")),
        role: Some(role.into()),
        agent,
        status,
        created_secs: created,
        last_seen_secs: last_seen,
        run_id: format!("run-{}", id.trim_start_matches("jk-")),
        sessions: Ok(vec![]),
        daemon: DaemonSnapshot::Unavailable,
        branch: None,
        pr: None,
        default_branch: "main".into(),
        github: github()
            .iter()
            .find(|r| r.full_name.ends_with(wsname))
            .map(Into::into),
        uncommitted: 0,
        unpushed: 0,
        account: None,
        accounts: vec![],
        agents: vec![crate::domain::instance::AgentInstanceView {
            agent,
            provider: agent.provider(),
            account: None,
            model: String::new(),
            branch: None,
            state: crate::domain::instance::AgentState::Idle,
        }],
        task: None,
    }
}

fn global(rich: bool) -> GlobalConfig {
    GlobalConfig {
        coauthor_trailer: true,
        dco_signoff: true,
        mounts: if rich {
            vec![
                Mount::host("~/.gitconfig", "/home/agent/.gitconfig")
                    .readonly(true)
                    .scope(MountScope::Global),
                Mount::host("~/.cache/cargo-registry", "/home/agent/.cargo/registry")
                    .scope(MountScope::Global),
                Mount::host("~/roles/sre-kube", "/home/agent/.kube")
                    .readonly(true)
                    .scope(MountScope::Role("sre".into())),
            ]
        } else {
            vec![]
        },
        env: if rich {
            vec![
                EnvVar::op(
                    "GH_TOKEN",
                    op_ref(
                        "chainargos.1password.com",
                        ("v_eng01", "Engineering"),
                        ("it_gh01", "GitHub · CLI token"),
                        "credential",
                    ),
                ),
                EnvVar::plain("EDITOR", "nvim"),
                EnvVar::plain("CARGO_NET_GIT_FETCH_WITH_CLI", "true"),
            ]
        } else {
            vec![]
        },
        role_env: if rich {
            let mut m = BTreeMap::new();
            m.insert(
                "sre".to_owned(),
                vec![EnvVar::plain("KUBECONFIG", "/home/agent/.kube/config")],
            );
            m
        } else {
            BTreeMap::new()
        },
        agent_modes: if rich {
            [
                (Agent::ClaudeCode, AuthMode::Sync),
                (Agent::Codex, AuthMode::ApiKey),
                (Agent::GrokBuild, AuthMode::ApiKey),
                (Agent::OpenCode, AuthMode::Sync),
                (Agent::Amp, AuthMode::Sync),
                (Agent::KimiCode, AuthMode::Ignore),
            ]
            .into_iter()
            .collect()
        } else {
            BTreeMap::new()
        },
        trust: vec![
            TrustRow {
                source: "github.com/chainargos/roles".into(),
                kind: TrustKind::Git,
                trusted: true,
                roles: 4,
            },
            TrustRow {
                source: "github.com/acme-labs/roles-experimental".into(),
                kind: TrustKind::Git,
                trusted: false,
                roles: 1,
            },
            TrustRow {
                source: "~/roles".into(),
                kind: TrustKind::Path,
                trusted: true,
                roles: 1,
            },
            TrustRow {
                source: "git@corp:infra/roles".into(),
                kind: TrustKind::Git,
                trusted: true,
                roles: 2,
            },
        ],
    }
}

fn fs() -> Vec<FsEntry> {
    let d = |p: &str, git: Option<&str>, meta: &str| FsEntry {
        path: p.into(),
        dir: true,
        git: git.map(str::to_owned),
        meta: meta.into(),
    };
    let f = |p: &str, meta: &str| FsEntry {
        path: p.into(),
        dir: false,
        git: None,
        meta: meta.into(),
    };
    vec![
        d("/Users/alexey", None, ""),
        d("/Users/alexey/src", None, "12 items"),
        d("/Users/alexey/design", None, "2 items"),
        d("/Users/alexey/roles", None, "1 item"),
        d("/Users/alexey/notes", None, "9 items"),
        d("/Users/alexey/.claude", None, "profile"),
        d("/Users/alexey/.codex", None, "profile"),
        d("/Users/alexey/.local/share/opencode", None, "profile"),
        d("/Users/alexey/.grok", None, "empty"),
        d("/Users/alexey/.codex-locked", None, "no access"),
        d("/Users/alexey/.opencode-broken", None, "profile"),
        f("/Users/alexey/notes.md", "2 d"),
        d(
            "/Users/alexey/src/payments-platform",
            Some("feature/settlement-backoff"),
            "git",
        ),
        d(
            "/Users/alexey/src/payments-platform/crates",
            None,
            "6 items",
        ),
        d(
            "/Users/alexey/src/payments-platform/crates/settlement",
            None,
            "rust",
        ),
        d(
            "/Users/alexey/src/payments-platform/crates/ledger",
            None,
            "rust",
        ),
        d("/Users/alexey/src/payments-platform/docs", None, "adr"),
        d(
            "/Users/alexey/src/payments-platform/scripts",
            None,
            "3 items",
        ),
        f("/Users/alexey/src/payments-platform/Cargo.toml", "1 h"),
        f("/Users/alexey/src/payments-platform/README.md", "3 d"),
        d("/Users/alexey/src/infra-control-plane", Some("main"), "git"),
        d(
            "/Users/alexey/src/infra-control-plane/modules",
            None,
            "terraform",
        ),
        d("/Users/alexey/src/infra-control-plane/kube", None, "go"),
        d("/Users/alexey/src/release-automation", Some("main"), "git"),
        d("/Users/alexey/src/customer-portal", Some("develop"), "git"),
        d("/Users/alexey/src/customer-portal/web", None, "next.js"),
        d(
            "/Users/alexey/src/customer-portal/services",
            None,
            "5 items",
        ),
        d("/Users/alexey/src/shared-libs", Some("main"), "git"),
        d("/Users/alexey/src/data-pipeline", Some("main"), "git"),
        d("/Users/alexey/src/docs-site", Some("gh-pages"), "git"),
        d("/Users/alexey/src/scratch", None, "4 items"),
        d("/Users/alexey/src/enterprise", None, "1 item"),
        d("/Users/alexey/design/portal-assets", None, "assets"),
        d("/Users/alexey/design/brand", None, "assets"),
        d("/Users/alexey/roles/writer", None, "manifest"),
    ]
}

fn github() -> Vec<GithubRepo> {
    let r = |n: &str, b: &str, extra: &[&str], upd: &str| GithubRepo {
        full_name: n.into(),
        default_branch: b.into(),
        branches: std::iter::once(b.to_owned())
            .chain(extra.iter().map(|s| (*s).to_owned()))
            .collect(),
        updated: upd.into(),
        url: format!("https://github.com/{n}"),
    };
    vec![
        r(
            "chainargos/payments-platform",
            "main",
            &["feature/settlement-backoff", "release/2026.09"],
            "1 h ago",
        ),
        r(
            "chainargos/infra-control-plane",
            "main",
            &["sre/node-pools"],
            "3 h ago",
        ),
        r(
            "chainargos/release-automation",
            "main",
            &["node-22"],
            "2 d ago",
        ),
        r(
            "chainargos/customer-portal",
            "develop",
            &["main", "feature/skeletons"],
            "5 h ago",
        ),
        r("chainargos/roles", "main", &[], "6 d ago"),
        r("chainargos/docs", "main", &["gh-pages"], "3 d ago"),
        r("acme-labs/roles-experimental", "next", &[], "2 mo ago"),
    ]
}

fn accounts_mixed(now: i64) -> AccountRegistry {
    let h = 3600;
    let d = 86_400;
    let mut r = AccountRegistry::default();

    let mut a = Account::registered(
        "acct-claude-personal",
        "Personal",
        Provider::Anthropic,
        CredentialSource::LocalFolder {
            path: "~/.claude".into(),
            detected: DetectedKind::ClaudeOAuthProfile,
        },
    );
    a.identity = AccountIdentity {
        subject: handle("alexey@donbeave.dev"),
        plan: Some("Max 5x".into()),
    };
    a.confidence = Confidence::Authoritative;
    a.lifecycle = Lifecycle::Available;
    a.purpose = Some("personal".into());
    a.default_for_provider = true;
    a.validation = ValidationState::Valid(ValidationLevel::QuotaReadable);
    a.last_refresh_secs = Some(now - 4 * 60);
    a.usage = AccountUsage {
        freshness: FreshnessInfo::current(now - 4 * 60),
        windows: vec![
            QuotaWindow::pct("session", "Session · 5-hour", WindowCategory::Session, 38)
                .reset(now + 3 * h + 12 * 60),
            QuotaWindow::pct(
                "weekly",
                "Weekly · all models",
                WindowCategory::LongRange,
                33,
            )
            .reset(now + 3 * d + 10 * h),
            QuotaWindow::pct(
                "weekly_sonnet",
                "Weekly · Sonnet",
                WindowCategory::Model,
                21,
            )
            .reset(now + 3 * d + 10 * h),
            QuotaWindow::pct("weekly_opus", "Weekly · Opus", WindowCategory::Model, 54)
                .reset(now + 3 * d + 10 * h),
        ],
    };
    r.insert(a);

    let mut a = Account::registered(
        "acct-claude-work",
        "Work",
        Provider::Anthropic,
        CredentialSource::OnePassword(op_ref(
            "chainargos.1password.com",
            ("v_eng01", "Engineering"),
            ("it_ant01", "Anthropic · Work"),
            "credential",
        )),
    );
    a.identity = AccountIdentity {
        subject: handle("alexey@chainargos.com"),
        plan: Some("Team".into()),
    };
    a.confidence = Confidence::Authoritative;
    a.lifecycle = Lifecycle::Available;
    a.purpose = Some("work".into());
    a.validation = ValidationState::Valid(ValidationLevel::QuotaReadable);
    a.last_refresh_secs = Some(now - 47 * 60);
    a.issue = Some(
        RecoverableIssue::new(
            IssueCode::Stale,
            IssueReason::Stale { age_secs: 47 * 60 },
            Recoverability::Retryable,
        )
        .retry(now + 13 * 60),
    );
    a.usage = AccountUsage {
        freshness: FreshnessInfo::stale(now - 47 * 60, now + 13 * 60),
        windows: vec![
            QuotaWindow::pct("session", "Session · 5-hour", WindowCategory::Session, 76)
                .reset(now + h + 5 * 60),
            QuotaWindow::pct(
                "weekly",
                "Weekly · all models",
                WindowCategory::LongRange,
                88,
            )
            .reset(now + 3 * d + 10 * h),
            QuotaWindow::pct("weekly_opus", "Weekly · Opus", WindowCategory::Model, 100)
                .reset(now + 3 * d + 10 * h),
            QuotaWindow::counted(
                "credits",
                "Extra usage credits",
                WindowCategory::Other,
                WindowUnit::Usd,
                1_420,
                5_000,
            )
            .spend("$14.20 of $50.00"),
        ],
    };
    r.insert(a);

    let mut a = Account::registered(
        "acct-codex-primary",
        "Primary",
        Provider::OpenAi,
        CredentialSource::OnePassword(op_ref(
            "chainargos.1password.com",
            ("v_eng01", "Engineering"),
            ("it_cdx01", "OpenAI · Codex Primary"),
            "credential",
        )),
    );
    a.identity = AccountIdentity {
        subject: handle("ChatGPT account org_7Hq2"),
        plan: Some("Pro 20x".into()),
    };
    a.confidence = Confidence::Authoritative;
    a.lifecycle = Lifecycle::Available;
    a.purpose = Some("work".into());
    a.default_for_provider = true;
    a.validation = ValidationState::Valid(ValidationLevel::QuotaReadable);
    a.last_refresh_secs = Some(now - 2 * 60);
    a.usage = AccountUsage {
        freshness: FreshnessInfo::current(now - 2 * 60),
        windows: vec![
            QuotaWindow::pct("session", "Session · 5-hour", WindowCategory::Session, 12)
                .reset(now + 4 * h + 40 * 60),
            QuotaWindow::pct("weekly", "Weekly · 7-day", WindowCategory::LongRange, 59)
                .reset(now + 2 * d + 19 * h),
            QuotaWindow::not_started("spark", "Codex Spark · 5-hour", WindowCategory::Model),
            QuotaWindow::counted(
                "credits",
                "Credits",
                WindowCategory::Other,
                WindowUnit::Credits,
                1_240,
                5_000,
            ),
        ],
    };
    r.insert(a);

    let mut a = Account::registered(
        "acct-codex-experiments",
        "Experiments",
        Provider::OpenAi,
        CredentialSource::PlainApiKey {
            fingerprint: "7f3a91c2".into(),
            tail: "k7Qz".into(),
        },
    );
    a.identity = AccountIdentity {
        subject: handle("ChatGPT account org_7Hq2"),
        plan: Some("Plus".into()),
    };
    a.confidence = Confidence::Estimated;
    a.lifecycle = Lifecycle::Available;
    a.purpose = Some("experiments".into());
    a.validation = ValidationState::Valid(ValidationLevel::IdentityAuthenticated);
    a.last_refresh_secs = Some(now - 20 * 60);
    a.issue = Some(RecoverableIssue::new(
        IssueCode::QuotaUnsupported,
        IssueReason::QuotaHidden {
            provider: Provider::OpenAi,
        },
        Recoverability::Unsupported,
    ));
    a.usage = AccountUsage {
        freshness: FreshnessInfo::refreshing(Some(now - 20 * 60)),
        windows: vec![
            QuotaWindow::pct("session", "Session · 5-hour", WindowCategory::Session, 4)
                .reset(now + 4 * h),
            QuotaWindow::pct("weekly", "Weekly · 7-day", WindowCategory::LongRange, 12)
                .reset(now + 2 * d + 19 * h),
            QuotaWindow::counted(
                "credits",
                "Credits",
                WindowCategory::Other,
                WindowUnit::Credits,
                90,
                500,
            ),
            QuotaWindow::sentinel(
                "quota",
                "Quota",
                QuotaStatus::Unsupported,
                "Quota not visible for API keys",
            ),
        ],
    };
    r.insert(a);

    let mut a = Account::registered(
        "acct-grok-team",
        "Team",
        Provider::XAi,
        CredentialSource::OnePassword(op_ref(
            "chainargos.1password.com",
            ("v_eng01", "Engineering"),
            ("it_grk01", "xAI · Grok Team"),
            "credential",
        )),
    )
    .with_endpoint("Grok Build (default)", "api.x.ai");
    a.identity = AccountIdentity {
        subject: handle("team_chainargos"),
        plan: Some("Team · prepaid".into()),
    };
    a.confidence = Confidence::Authoritative;
    a.lifecycle = Lifecycle::Available;
    a.purpose = Some("shared".into());
    a.default_for_provider = true;
    a.validation = ValidationState::Valid(ValidationLevel::QuotaReadable);
    a.last_refresh_secs = Some(now - 9 * 60);
    a.usage = AccountUsage {
        freshness: FreshnessInfo::current(now - 9 * 60),
        windows: vec![
            QuotaWindow::pct("monthly", "Monthly", WindowCategory::LongRange, 31)
                .reset(now + 27 * d + 10 * h),
            QuotaWindow::pct("weekly", "Weekly", WindowCategory::LongRange, 68)
                .reset(now + 3 * d + 10 * h),
            QuotaWindow::counted(
                "credits",
                "Credits",
                WindowCategory::Other,
                WindowUnit::Usd,
                3_140,
                10_000,
            )
            .spend("$68.60 remaining of $100.00"),
            QuotaWindow::counted(
                "ondemand",
                "On-demand usage",
                WindowCategory::Other,
                WindowUnit::Usd,
                315,
                315,
            )
            .status(QuotaStatus::Available)
            .spend("$3.15 this month"),
        ],
    };
    r.insert(a);

    let mut a = Account::registered(
        "acct-opencode-go",
        "Go subscription",
        Provider::OpenCode,
        CredentialSource::PlainApiKey {
            fingerprint: "c41d0be9".into(),
            tail: "m2Xa".into(),
        },
    );
    a.identity = AccountIdentity {
        subject: handle("donbeave"),
        plan: Some("OpenCode Go".into()),
    };
    a.confidence = Confidence::Authoritative;
    a.lifecycle = Lifecycle::Available;
    a.default_for_provider = true;
    a.validation = ValidationState::Valid(ValidationLevel::IdentityAuthenticated);
    a.last_refresh_secs = Some(now - 3 * h - 2 * 60);
    a.issue = Some(
        RecoverableIssue::new(
            IssueCode::RateLimited,
            IssueReason::Usage(UsageReason::RateLimited {
                retry_in_secs: Some(25 * 60),
            }),
            Recoverability::Retryable,
        )
        .detail("Last-good data kept from 3 h ago")
        .retry(now + 25 * 60),
    );
    a.usage = AccountUsage {
        freshness: FreshnessInfo::failed(Some(now - 3 * h - 2 * 60), Some(now + 25 * 60)),
        windows: vec![
            QuotaWindow::pct("rolling", "Rolling", WindowCategory::Session, 57)
                .reset(now + h + 50 * 60),
            QuotaWindow::pct("weekly", "Weekly", WindowCategory::LongRange, 45).reset(now + 3 * d),
            QuotaWindow::pct("monthly", "Monthly", WindowCategory::LongRange, 22)
                .reset(now + 27 * d),
        ],
    };
    r.insert(a);

    let mut a = Account::registered(
        "acct-claude-archive",
        "Archived contractor laptop profile — do not use for production launches",
        Provider::Anthropic,
        CredentialSource::LocalFolder {
            path: "~/Library/Application Support/jackin/profiles/claude-contractor-2025-archive"
                .into(),
            detected: DetectedKind::ClaudeApiKeyEnv,
        },
    );
    a.confidence = Confidence::PresenceOnly;
    a.lifecycle = Lifecycle::NeedsLogin;
    a.enabled = false;
    a.validation = ValidationState::Valid(ValidationLevel::MaterialDiscovered);
    a.last_refresh_secs = Some(now - 30 * d);
    a.issue = Some(RecoverableIssue::new(
        IssueCode::IdentityUnresolved,
        IssueReason::IdentityUnresolved,
        Recoverability::Unsupported,
    ));
    a.usage = AccountUsage {
        freshness: FreshnessInfo::current(now - 30 * d),
        windows: vec![],
    };
    r.insert(a);

    let mut a = Account::discovered(
        "disc-amp",
        "discovered",
        Provider::Amp,
        CredentialSource::LocalFolder {
            path: "~/.config/amp/secrets.json".into(),
            detected: DetectedKind::AmpSecrets,
        },
    );
    a.identity = AccountIdentity {
        subject: None,
        plan: Some("Free".into()),
    };
    a.confidence = Confidence::PresenceOnly;
    a.lifecycle = Lifecycle::Available;
    a.validation = ValidationState::Valid(ValidationLevel::QuotaReadable);
    a.last_refresh_secs = Some(now - 60);
    a.usage = AccountUsage {
        freshness: FreshnessInfo::current(now - 60),
        windows: vec![
            QuotaWindow::pct(
                "daily_free",
                "Amp Free · daily",
                WindowCategory::Session,
                91,
            )
            .reset(now + 10 * h),
            QuotaWindow::counted(
                "credits",
                "Individual credits",
                WindowCategory::Other,
                WindowUnit::Usd,
                0,
                0,
            )
            .spend("$0.00"),
        ],
    };
    r.insert(a);

    let mut a = Account::discovered(
        "disc-zai",
        "discovered",
        Provider::Zai,
        CredentialSource::HostEnv {
            var: "ZAI_API_KEY".into(),
            detected: DetectedKind::ZaiApiKeyEnv,
        },
    );
    a.identity = AccountIdentity {
        subject: handle("zai_9f1c"),
        plan: Some("GLM Coding Plan Pro".into()),
    };
    a.confidence = Confidence::Authoritative;
    a.lifecycle = Lifecycle::Available;
    a.validation = ValidationState::Valid(ValidationLevel::QuotaReadable);
    a.last_refresh_secs = Some(now - 2 * h - 15 * 60);
    a.issue = Some(RecoverableIssue::new(
        IssueCode::Stale,
        IssueReason::Stale {
            age_secs: 2 * h + 15 * 60,
        },
        Recoverability::Retryable,
    ));
    a.usage = AccountUsage {
        freshness: FreshnessInfo::stale(now - 2 * h - 15 * 60, now + 10 * 60),
        windows: vec![
            QuotaWindow::counted(
                "session",
                "Session",
                WindowCategory::Session,
                WindowUnit::Tokens,
                4_200_000,
                10_000_000,
            )
            .reset(now + 2 * h),
            QuotaWindow::counted(
                "weekly",
                "Weekly",
                WindowCategory::LongRange,
                WindowUnit::Tokens,
                61_000_000,
                80_000_000,
            )
            .reset(now + 4 * d),
            QuotaWindow::counted(
                "credits",
                "Credits",
                WindowCategory::Other,
                WindowUnit::Credits,
                310,
                1_000,
            ),
        ],
    };
    r.insert(a);

    let mut a = Account::discovered(
        "disc-kimi",
        "discovered",
        Provider::Moonshot,
        CredentialSource::LocalFolder {
            path: "~/.kimi".into(),
            detected: DetectedKind::KimiApiKeyEnv,
        },
    );
    a.identity = AccountIdentity {
        subject: None,
        plan: Some("Kimi Code".into()),
    };
    a.confidence = Confidence::PresenceOnly;
    a.lifecycle = Lifecycle::NeedsSecret;
    a.validation = ValidationState::Valid(ValidationLevel::MaterialDiscovered);
    a.issue = Some(RecoverableIssue::new(
        IssueCode::CredentialFileMissing,
        IssueReason::LoginMissing {
            path: "~/.kimi".into(),
            file: "api key".into(),
        },
        Recoverability::ActionRequired,
    ));
    a.usage = AccountUsage {
        freshness: FreshnessInfo::failed(None, None),
        windows: vec![QuotaWindow::sentinel(
            "quota",
            "Quota",
            QuotaStatus::Unavailable,
            "Quota unavailable until a key is present",
        )],
    };
    r.insert(a);

    let mut a = Account::discovered(
        "disc-minimax",
        "discovered",
        Provider::MiniMax,
        CredentialSource::HostEnv {
            var: "MINIMAX_API_TOKEN".into(),
            detected: DetectedKind::MinimaxTokenEnv,
        },
    );
    a.identity = AccountIdentity {
        subject: handle("mm_4471"),
        plan: Some("Coding Plan".into()),
    };
    a.confidence = Confidence::Estimated;
    a.lifecycle = Lifecycle::Unavailable;
    a.validation = ValidationState::Valid(ValidationLevel::IdentityAuthenticated);
    a.last_refresh_secs = Some(now - 6 * h);
    a.issue = Some(
        RecoverableIssue::new(
            IssueCode::ProviderUnavailable,
            IssueReason::ProviderDown {
                provider: Provider::MiniMax,
                detail: "Timed out after 8 s".into(),
            },
            Recoverability::Retryable,
        )
        .detail("Timed out after 8 s"),
    );
    a.usage = AccountUsage {
        freshness: FreshnessInfo::failed(Some(now - 6 * h), Some(now + 15 * 60)),
        windows: vec![
            QuotaWindow::pct(
                "general_session",
                "General · Session",
                WindowCategory::Session,
                8,
            )
            .reset(now + 3 * h),
            QuotaWindow::pct(
                "general_weekly",
                "General · Weekly",
                WindowCategory::LongRange,
                34,
            )
            .reset(now + 5 * d),
            QuotaWindow::pct(
                "m2_weekly",
                "MiniMax-M2 · Weekly",
                WindowCategory::Model,
                47,
            )
            .reset(now + 5 * d),
        ],
    };
    r.insert(a);

    let mut a = Account::discovered(
        "disc-opencode-ci",
        "ci-bot",
        Provider::OpenCode,
        CredentialSource::LocalFolder {
            path: "~/.local/share/opencode/auth.json".into(),
            detected: DetectedKind::OpenCodeGoAuthJson,
        },
    );
    a.identity = AccountIdentity {
        subject: handle("ci-bot"),
        plan: None,
    };
    a.confidence = Confidence::Authoritative;
    a.lifecycle = Lifecycle::Unsupported;
    a.validation = ValidationState::Valid(ValidationLevel::IdentityAuthenticated);
    a.last_refresh_secs = Some(now - 5 * 60);
    a.issue = Some(RecoverableIssue::new(
        IssueCode::QuotaUnsupported,
        IssueReason::QuotaDenied {
            provider: Provider::OpenCode,
            status: 403,
        },
        Recoverability::Unsupported,
    ));
    a.usage = AccountUsage {
        freshness: FreshnessInfo::current(now - 5 * 60),
        windows: vec![QuotaWindow::sentinel(
            "quota",
            "Quota",
            QuotaStatus::Unsupported,
            "Quota not visible: OpenCode returned 403",
        )],
    };
    r.insert(a);
    r
}

fn accounts_hard(now: i64) -> AccountRegistry {
    let mut r = accounts_mixed(now);
    let mut a = Account::registered(
        "acct-grok-revoked",
        "Revoked key",
        Provider::XAi,
        CredentialSource::OnePassword(op_ref(
            "chainargos.1password.com",
            ("v_eng01", "Engineering"),
            ("it_leg01", "Legacy · Rotated key"),
            "credential",
        )),
    );
    a.lifecycle = Lifecycle::NeedsLogin;
    a.validation = ValidationState::Invalid(RecoverableIssue::new(
        IssueCode::Unauthorized,
        IssueReason::Rejected {
            provider: Provider::XAi,
        },
        Recoverability::ActionRequired,
    ));
    a.issue = Some(
        RecoverableIssue::new(
            IssueCode::Unauthorized,
            IssueReason::Rejected {
                provider: Provider::XAi,
            },
            Recoverability::ActionRequired,
        )
        .detail("HTTP 401 · re-login required"),
    );
    a.last_refresh_secs = Some(now - 40 * 60);
    a.usage = AccountUsage {
        freshness: FreshnessInfo::failed(None, None),
        windows: vec![],
    };
    r.insert(a);

    if let Some(w) = r.get_mut("acct-claude-work")
        && let Some(win) = w.usage.windows.iter_mut().find(|w| w.id == "weekly")
    {
        win.label =
            "Weekly · all models · includes Claude Code, desktop and API usage on the Team plan"
                .into();
    }
    r
}

pub(super) fn about_rows(scenario: Scenario) -> Vec<(String, String)> {
    [
        ("Product", "jackin-preview · deterministic redesign preview"),
        ("Scenario", scenario.name()),
        (
            "Construct",
            "simulated · nothing here touches a real container",
        ),
        (
            "Design system",
            "Junie-inspired Ratatui components (termrock)",
        ),
    ]
    .into_iter()
    .map(|(label, value)| (label.to_owned(), value.to_owned()))
    .collect()
}

fn base_world(scenario: Scenario) -> World {
    let clock = CLOCK;
    let now = clock.now_secs();
    World::new(WorldParts {
        roles: roles(),
        global: global(false),
        fs: fs(),
        github: github(),
        last_refresh_secs: now - 3,
        ..WorldParts::new(
            clock,
            HOME.into(),
            format!("{HOME}/src/payments-platform"),
            Box::new(SimOnePassword::fixture(now)),
            SimBackend::shared(),
            Surfaces::demo(scenario),
        )
    })
}

fn populated(scenario: Scenario, rich: bool) -> World {
    let mut w = base_world(scenario);
    let now = w.now_secs();
    let h = 3600;
    let d = 86_400;
    w.workspaces = workspaces(rich);
    w.global = global(true);
    w.accounts = if rich {
        accounts_hard(now)
    } else {
        accounts_mixed(now)
    };
    w.adopt_refreshing();

    let mut i1 = instance(
        "jk-7f3a",
        Some(1),
        "payments-platform",
        "the-architect",
        Agent::ClaudeCode,
        InstanceStatus::Running,
        now - 2 * h - 14 * 60,
        now - 3,
    );
    i1.branch = Some("feature/settlement-backoff".into());
    i1.account = Some("acct-claude-work".into());
    i1.pr = Some((482, "Settlement retry backoff".into()));
    i1.uncommitted = 2;
    i1.unpushed = 1;
    i1.sessions = Ok(vec![
        SessionRecord {
            id: "s-01".into(),
            agent: Some(Agent::ClaudeCode),
            label: "claude · the-architect".into(),
            status: SessionStatus::Active,
            started_secs: now - 2 * h - 14 * 60,
        },
        SessionRecord {
            id: "s-02".into(),
            agent: Some(Agent::Codex),
            label: "codex · ledger tests".into(),
            status: SessionStatus::Exited(0),
            started_secs: now - d,
        },
    ]);
    let mut i2 = instance(
        "jk-c41e",
        Some(1),
        "payments-platform",
        "reviewer",
        Agent::Codex,
        InstanceStatus::PreservedDirty,
        now - d - 3 * h,
        now - d,
    );
    i2.uncommitted = 3;
    i2.sessions = Ok(vec![SessionRecord {
        id: "s-03".into(),
        agent: Some(Agent::Codex),
        label: "codex · review".into(),
        status: SessionStatus::Exited(0),
        started_secs: now - d - 3 * h,
    }]);
    let mut i3 = instance(
        "jk-9b02",
        Some(2),
        "infra-control-plane",
        "sre",
        Agent::Codex,
        InstanceStatus::Running,
        now - 40 * 60,
        now - 2,
    );
    i3.branch = Some("sre/node-pools".into());
    i3.account = Some("acct-codex-experiments".into());
    i3.sessions = Ok(vec![SessionRecord {
        id: "s-04".into(),
        agent: Some(Agent::Codex),
        label: "codex · sre".into(),
        status: SessionStatus::Active,
        started_secs: now - 40 * 60,
    }]);
    let mut i4 = instance(
        "jk-12ee",
        Some(4),
        "customer-portal",
        "writer",
        Agent::Amp,
        InstanceStatus::Crashed,
        now - 5 * h,
        now - 4 * h,
    );
    i4.sessions = Ok(vec![SessionRecord {
        id: "s-05".into(),
        agent: Some(Agent::Amp),
        label: "amp · docs".into(),
        status: SessionStatus::Crashed,
        started_secs: now - 5 * h,
    }]);
    let i5 = instance(
        "jk-a1c0",
        Some(3),
        "release-automation",
        "backend",
        Agent::OpenCode,
        InstanceStatus::RestoreAvailable,
        now - 3 * d,
        now - 3 * d,
    );
    let mut i6 = instance(
        "jk-04d7",
        Some(2),
        "infra-control-plane",
        "sre",
        Agent::GrokBuild,
        InstanceStatus::PreservedUnpushed,
        now - 2 * d,
        now - 2 * d,
    );
    i6.unpushed = 2;
    let i7 = instance(
        "jk-77aa",
        Some(1),
        "payments-platform",
        "backend",
        Agent::Codex,
        InstanceStatus::Superseded,
        now - 6 * d,
        now - 6 * d,
    );
    let i8 = instance(
        "jk-88bb",
        Some(4),
        "customer-portal",
        "writer",
        Agent::KimiCode,
        InstanceStatus::Purged,
        now - 9 * d,
        now - 9 * d,
    );
    let mut i9 = instance(
        "jk-5e5e",
        Some(3),
        "release-automation",
        "backend",
        Agent::Codex,
        InstanceStatus::FailedSetup,
        now - 30 * 60,
        now - 30 * 60,
    );
    i9.sessions = Ok(vec![]);
    w.instances = vec![i1, i2, i3, i4, i5, i6, i7, i8, i9]
        .into_iter()
        .map(InstanceEntry::available)
        .collect();
    if rich {
        let mut i10 = instance(
            "jk-e0e0",
            Some(10),
            "data-pipeline",
            "backend",
            Agent::ClaudeCode,
            InstanceStatus::Running,
            now - 10 * 60,
            now - 90,
        );
        i10.sessions = Err(ManifestError::ReadError);
        w.instances.push(InstanceEntry::available(i10));
        let i11 = instance(
            "jk-f1f1",
            Some(11),
            "docs-site",
            "writer",
            Agent::KimiCode,
            InstanceStatus::CleanExited,
            now - 4 * d,
            now - 4 * d,
        );
        w.instances.push(InstanceEntry::available(i11));
        for k in 0..6 {
            let id = format!("jk-b{k}{k}{k}");
            let st = if k % 2 == 0 {
                InstanceStatus::CleanExited
            } else {
                InstanceStatus::RestoreAvailable
            };
            w.instances.push(InstanceEntry::available(instance(
                &id,
                Some(1),
                "payments-platform",
                "reviewer",
                Agent::Codex,
                st,
                now - (k + 2) * d,
                now - (k + 2) * d,
            )));
        }
    }

    for ws in &mut w.workspaces {
        ws.github = w
            .instances
            .iter()
            .filter_map(InstanceEntry::as_available)
            .filter(|i| i.workspace == Some(ws.id))
            .find_map(|i| i.github.clone());
    }

    let now_ms = w.now_ms();
    let mut d1 = Daemon::new("payments-platform");
    let payments_instance = InstanceId::simulated("jk-7f3a");
    let claude = PaneRequest::new(
        &payments_instance,
        Some(Agent::ClaudeCode),
        Some("acct-claude-work".into()),
    );
    d1.new_tab(&claude, now_ms - 60_000, scripted(&claude));
    let codex = PaneRequest::new(
        &payments_instance,
        Some(Agent::Codex),
        Some("acct-codex-primary".into()),
    );
    d1.split(
        SplitDir::Horizontal,
        false,
        &codex,
        now_ms - 50_000,
        scripted(&codex),
    );
    let shell = PaneRequest::new(&payments_instance, None, None);
    d1.split(
        SplitDir::Vertical,
        false,
        &shell,
        now_ms - 40_000,
        |spawn| {
            let body = ScriptPane::new(&shell, &spawn, SimScripts::shared())
                .booted()
                .with_history_above(settlement_history());
            Box::new(body)
        },
    );
    if let Some(t) = d1.active_tab_mut() {
        t.focused = 1;
    }
    d1.new_tab(&shell, now_ms - 30_000, scripted(&shell));
    let amp = PaneRequest::new(&payments_instance, Some(Agent::Amp), None);
    d1.new_tab(&amp, now_ms - 20_000, scripted(&amp));
    if let Some(t) = d1.tabs.get_mut(2) {
        t.custom_label = Some("docs".into());
    }
    d1.active = 0;

    w.daemons.insert(payments_instance, d1);
    let infra_instance = InstanceId::simulated("jk-9b02");
    let mut d3 = Daemon::new("infra-control-plane");
    let experiments = PaneRequest::new(
        &infra_instance,
        Some(Agent::Codex),
        Some("acct-codex-experiments".into()),
    );
    d3.new_tab(&experiments, now_ms - 30_000, scripted(&experiments));
    w.daemons.insert(infra_instance, d3);
    if rich {
        let d10 = Daemon::new("data-pipeline");
        let data_instance = InstanceId::simulated("jk-e0e0");
        w.daemons.insert(data_instance.clone(), d10);

        if let Some(i) = w.instance_mut(&data_instance) {
            i.daemon = DaemonSnapshot::Unavailable;
        }
        w.daemons.remove(&data_instance);
    }

    w.roles.extend(generated_roles(if rich { 120 } else { 40 }));

    let registry = w.accounts.clone();
    let sets: Vec<(u32, Vec<AccountId>)> = w
        .workspaces
        .iter()
        .map(|ws| {
            (
                ws.id,
                ws.effective_accounts(&registry)
                    .into_iter()
                    .map(|e| e.id)
                    .collect(),
            )
        })
        .collect();
    for i in w.instances.iter_mut() {
        if let Some(i) = i.as_available_mut()
            && let Some((_, set)) = i
                .workspace
                .and_then(|id| sets.iter().find(|(w, _)| *w == id))
        {
            i.accounts = set.clone();
        }
    }
    refresh_snapshots(&mut w);
    w.sync_arbiter();
    w
}

fn scripted(request: &PaneRequest) -> impl FnOnce(PaneSpawn) -> Box<dyn PaneBody> + '_ {
    |spawn| Box::new(ScriptPane::new(request, &spawn, SimScripts::shared()).booted())
}

fn settlement_history() -> Vec<termrock::widgets::viewport::Line> {
    let mut history: Vec<termrock::widgets::viewport::Line> = vec![];
    for n in 0..1_960u32 {
        let status = match n % 7 {
            0 => "retrying attempt=2",
            3 => "retrying attempt=1",
            _ => "settled",
        };
        let items = 9 + (n * 37) % 50;
        let l = if n % 60 == 0 {
            vec![termrock::widgets::viewport::Span::muted(format!(
                "==== settlement.batch.2026-09-03T{:02}:{:02}Z ====",
                9 + n / 360,
                (n / 6) % 60
            ))]
        } else if n % 23 == 0 {
            vec![termrock::widgets::viewport::Span::new(
                format!("warn: batch {} backoff 500 ms", 4000 + n),
                termrock::theme::Tone::Warning,
            )]
        } else if n % 97 == 0 {
            vec![termrock::widgets::viewport::Span::new(
                format!("error: batch {} gave up after 3 attempts", 4000 + n),
                termrock::theme::Tone::Error,
            )]
        } else {
            vec![termrock::widgets::viewport::Span::plain(format!(
                "batch {}  {:>2} items   status={status}",
                4000 + n,
                items
            ))]
        };
        history.insert(0, l);
    }
    history
}

pub fn world_for(scenario: Scenario) -> World {
    match scenario {
        Scenario::FirstUse => {
            let mut w = base_world(scenario);
            w.discovery_pending = true;
            w
        }
        Scenario::DockerSetting | Scenario::DockerNoEngine => {
            let mut w = populated(scenario, false);
            surfaces::docker(&mut w, scenario == Scenario::DockerNoEngine);
            w
        }
        Scenario::RoleEditor => {
            let mut w = populated(scenario, false);
            surfaces::roles(&mut w, vec![Agent::Codex]);
            surfaces::docker(&mut w, false);
            w
        }
        Scenario::RoleBinding => {
            let mut w = populated(scenario, false);
            surfaces::roles(&mut w, vec![Agent::Codex, Agent::ClaudeCode]);
            w.backend = SimBackend::failing_first_bind();
            w
        }
        Scenario::Doctor => {
            let mut w = populated(scenario, false);
            surfaces::doctor(&mut w);
            w
        }
        Scenario::LaunchOpenshell => {
            let mut w = populated(scenario, false);
            w.backend = SimBackend::openshell();
            w
        }
        Scenario::OpenshellFixed => {
            let mut w = populated(scenario, false);
            surfaces::fixed(&mut w);
            w
        }
        Scenario::LaunchFailure => {
            let mut w = populated(scenario, false);
            w.backend = SimBackend::failing_launch();
            w
        }
        Scenario::CapsuleTerminal => {
            let mut w = populated(scenario, false);
            terminal::capsule_terminal(&mut w);
            w
        }
        Scenario::LaunchChecks => {
            let mut w = populated(scenario, false);
            surfaces::roles(&mut w, vec![Agent::Codex, Agent::ClaudeCode]);
            surfaces::launch_checks(&mut w);
            w.backend = SimBackend::guarded();
            w
        }
        Scenario::Returning
        | Scenario::AccountsMixed
        | Scenario::LaunchRunning
        | Scenario::CapsuleMulti => populated(scenario, false),
        Scenario::OutroLast => {
            let mut w = populated(scenario, false);

            for i in w.instances.iter_mut() {
                if i.id() == &InstanceId::simulated("jk-9b02")
                    && let Some(instance) = i.as_available_mut()
                {
                    instance.status = InstanceStatus::CleanExited;
                }
            }
            w.daemons.remove(&InstanceId::simulated("jk-9b02"));
            refresh_snapshots(&mut w);
            w.sync_arbiter();
            w.arbiter.entered_at_ms = Some(-8_040_000);
            w
        }
        Scenario::HardCases => {
            let mut w = populated(scenario, true);
            w.op.set_session(OpSession::Locked);
            w.daemon_health = DaemonHealth::Stale;
            w.backend = SimBackend::hard_cases();

            if let Some(r) = w.roles.iter_mut().find(|r| r.name == "writer") {
                r.fetch_error = Some(
                    "~/roles/writer/role.toml: parse error at line 4: unknown key `toolz`".into(),
                );
            }
            w.arbiter.discovery = Err(DiscoveryError::IndexUnreadable);
            w.arbiter.entered_at_ms = None;
            w
        }
    }
}
