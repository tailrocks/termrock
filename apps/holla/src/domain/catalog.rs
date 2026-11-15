use std::collections::BTreeSet;

use crate::domain::action::{
    ArgSpec, Confirmation, Freshness, Item, Kind, Launch, ResultType, Risk, Signal,
};
use crate::domain::cleanup::{self, Eligibility, ProcessObservation};
use crate::domain::context::{Os, Scope, ScopeTag};
use crate::domain::custom::{Danger, TrustStatus};
use crate::domain::effect::Effect;
use crate::domain::exec::Command;
use crate::domain::manifest;
use crate::sim::world::{BatchMode, SourceState, World};

fn tag_for_dir(w: &World, dir: &str) -> ScopeTag {
    let cwd = &w.location.cwd;
    if dir == cwd {
        ScopeTag::here(cwd)
    } else if cwd.starts_with(&format!("{dir}/")) {
        ScopeTag::parent(dir)
    } else if dir.starts_with(&format!("{cwd}/")) {
        ScopeTag::child(dir)
    } else if let Some(ws) = &w.location.workspace
        && dir.starts_with(&format!("{}/", ws.root))
    {
        ScopeTag {
            direction: Scope::Parent,
            defined_at: dir.to_owned(),
            runs_in: dir.to_owned(),
            word: "sibling".into(),
        }
    } else {
        ScopeTag::host(&w.host.name)
    }
}

fn host_tag(w: &World) -> ScopeTag {
    if w.host.remote {
        ScopeTag::remote(&w.host.name)
    } else {
        ScopeTag::host(&w.host.name)
    }
}

fn freshness(w: &World, source: &str) -> Freshness {
    match w.source_state(source) {
        SourceState::Loading => Freshness::Loading,
        SourceState::Failed(r) => Freshness::Unavailable(r),
        SourceState::Done => {
            let done_at = w
                .sources
                .iter()
                .find(|s| s.name == source)
                .map(|s| s.done_at)
                .unwrap_or(0);
            let age_ms = (w.tick.saturating_sub(done_at) as i64) * crate::sim::world::TICK_MS;
            match source {
                "github" => Freshness::Cached {
                    age_ms: age_ms + 40_000,
                },

                "children" if w.disk.partial_reason.is_some() => Freshness::Partial {
                    loaded: w.location.children.len(),
                    total: w.location.children.len() + 2,
                },
                _ => Freshness::Live { age_ms },
            }
        }
    }
}

fn source_ready(w: &World, source: &str) -> bool {
    !matches!(w.source_state(source), SourceState::Loading)
}

fn cmd(w: &World, program: &str, args: &[&str], cwd: &str) -> Command {
    Command::argv(program, args, cwd, &w.host.name)
}

fn script_for_task(w: &World, name: &str, dir: &str) -> Option<String> {
    let fail = w.scenario == crate::scenario::Scenario::LaunchFailure;
    let holla = dir.ends_with("/holla");
    let key = match name {
        "dev" if dir.ends_with("apps/frontend") => {
            if fail {
                "task:frontend-dev-fail"
            } else {
                "task:frontend-dev"
            }
        }
        "dev" if dir.ends_with("services/api") => "task:api-dev",
        "test" if dir.ends_with("apps/frontend") => "task:tests-frontend",
        "migrate" => "task:migrate",
        "test" if holla || dir.ends_with("services/api") => "task:tests",
        "lint" if holla => "task:clippy",
        "setup" => "task:setup",
        "up" => "task:up",
        "dev" if dir.ends_with("/acme") => "task:eco",
        _ => return None,
    };
    Some(key.into())
}

fn apply_memory(w: &World, mut item: Item) -> Item {
    let cwd = &w.location.cwd;
    let project_root = w.location.project.as_ref().map(|p| p.root.as_str());
    let host = &w.host.name;
    let now = w.now_secs();
    for u in &w.memory.usage.actions {
        if u.item != item.id || u.host != *host {
            continue;
        }
        match &u.path {
            Some(p) if p == cwd || Some(p.as_str()) == project_root => {
                item.used_here += u.count();
                item.last_used_secs = Some(item.last_used_secs.unwrap_or(0).max(u.last_secs()));
            }
            Some(_) => {}
            None => item.used_anywhere += u.count(),
        }
    }
    item.frecency = w.memory.usage.frecency_any(&item.id, cwd, host, now);
    if item.used_here > 0 {
        let text = if item.used_here == 1 {
            "used once here".to_owned()
        } else {
            format!("used {} times here", item.used_here)
        };
        item.reasons
            .push(crate::domain::action::Reason::new(Signal::LocalUse, text));
    } else if item.used_anywhere > 0 {
        item.reasons.push(crate::domain::action::Reason::new(
            Signal::GlobalUse,
            format!("used {} times on {}", item.used_anywhere, w.host.name),
        ));
    }
    for (a, id) in &w.memory.aliases {
        if id == &item.id && !item.aliases.contains(a) {
            item.aliases.push(a.clone());
        }
    }
    item.pinned = w
        .memory
        .pins
        .iter()
        .any(|(p, id)| p == cwd && id == &item.id);
    item.hidden = w
        .memory
        .hidden
        .iter()
        .any(|(p, id)| p == cwd && id == &item.id);
    if item.pinned {
        item.reasons.push(crate::domain::action::Reason::new(
            Signal::Pin,
            "pinned here",
        ));
    }
    if let Some((_, tool)) = w
        .memory
        .preferred_tools
        .iter()
        .find(|(id, _)| id == &item.id)
    {
        item.preferred_tool = Some(tool.clone());
    }
    item
}

pub fn builtin_ids(w: &World) -> Vec<String> {
    let mut items = vec![];
    contribute(w, &mut items, false);
    items.into_iter().map(|i| i.id).collect()
}

pub fn build(w: &World) -> Vec<Item> {
    build_with_warnings(w).0
}

pub fn build_with_warnings(w: &World) -> (Vec<Item>, Vec<String>) {
    let mut items: Vec<Item> = vec![];
    contribute(w, &mut items, true);
    let mut warnings = vec![];
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut merged = Vec::with_capacity(items.len());
    for it in items {
        if !seen.insert(it.id.clone()) {
            warnings.push(format!(
                "duplicate action id {} from {} · the earlier definition wins",
                it.id, it.provenance
            ));
            continue;
        }
        merged.push(it);
    }
    for cfg in [&w.custom_global, &w.custom_project].into_iter().flatten() {
        for d in &cfg.diagnostics {
            warnings.push(d.text());
        }
    }
    for (name, why) in w.failed_sources() {
        warnings.push(format!("{name}: {why}"));
    }
    (
        merged.into_iter().map(|i| apply_memory(w, i)).collect(),
        warnings,
    )
}

fn contribute(w: &World, items: &mut Vec<Item>, with_custom: bool) {
    explore(w, items);
    file_items(w, items);
    disk_items(w, items);
    cleanup_items(w, items);
    mise_items(w, items);
    native_task_items(w, items);
    rust_items(w, items);
    git_items(w, items);
    system_items(w, items);
    upgrade_items(w, items);
    brew_items(w, items);
    docker_items(w, items);
    gradle_items(w, items);
    idea_items(w, items);
    pg_items(w, items);
    ssh_items(w, items);
    service_items(w, items);
    if with_custom {
        custom_items(w, items);
    }
    activity_items(w, items);
}

/// One Explore tile on the Finder's empty Here view. The world owns the list so
/// the tile set is data, not a constant baked into the catalog builder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExploreTile {
    pub id: &'static str,
    pub label: &'static str,
    pub group: &'static str,
    pub aliases: &'static [&'static str],
    pub summary: &'static str,
}

pub fn default_explore_tiles() -> Vec<ExploreTile> {
    let tile = |id, label, group, aliases, summary| ExploreTile {
        id,
        label,
        group,
        aliases,
        summary,
    };
    vec![
        tile(
            "explore.tasks",
            "Tasks",
            "Tasks",
            &["t"],
            "project tasks from mise and the build system, here and around",
        ),
        tile(
            "explore.git",
            "Git",
            "Git",
            &["g"],
            "worktree state, sync, branches, bulk operations across children",
        ),
        tile(
            "explore.files",
            "Files",
            "Files",
            &["f"],
            "browse this folder, find files under home, preview safely",
        ),
        tile(
            "explore.disk",
            "Disk",
            "Disk",
            &["d"],
            "usage analysis and stack-aware cleanup",
        ),
        tile(
            "explore.services",
            "Services",
            "Services",
            &["s"],
            "containers, Compose, databases and system services",
        ),
        tile(
            "explore.system",
            "System",
            "System",
            &["y"],
            "resources, processes, monitors, upgrades",
        ),
    ]
}

fn explore(w: &World, out: &mut Vec<Item>) {
    let here = ScopeTag::here(&w.location.cwd);
    for t in &w.explore_tiles {
        out.push(
            Item::new(t.id, t.label, Kind::Explore, ResultType::Flow, here.clone())
                .summary(t.summary)
                .aliases(t.aliases)
                .group("Explore")
                .launch(Launch::Group(t.group))
                .reason(Signal::Default, "stable entry point"),
        );
    }

    if let Some(p) = &w.location.workspace {
        out.push(
            Item::new(
                "scope.parent",
                &format!("Parent · {} ({})", p.name, p.kind.label()),
                Kind::Explore,
                ResultType::Resource,
                ScopeTag::parent(&p.root),
            )
            .summary(&format!(
                "show what the workspace root at {} defines for this folder",
                w.location.short(&p.root)
            ))
            .keywords(&["parent", "ancestor", "workspace", "monorepo", "root"])
            .group("Explore")
            .launch(Launch::Scope(Scope::Parent))
            .reason(Signal::Context, "workspace root above this folder"),
        );
    } else if let Some(p) = &w.location.project
        && p.root != w.location.cwd
    {
        out.push(
            Item::new(
                "scope.parent",
                &format!("Parent · {} ({})", p.name, p.kind.label()),
                Kind::Explore,
                ResultType::Resource,
                ScopeTag::parent(&p.root),
            )
            .summary(&format!(
                "show what the project root at {} defines",
                w.location.short(&p.root)
            ))
            .keywords(&["parent", "ancestor", "project root"])
            .group("Explore")
            .launch(Launch::Scope(Scope::Parent))
            .reason(Signal::Context, "project root above this folder"),
        );
    }
    if !w.location.children.is_empty() {
        let n = w.location.children.len();
        let names: Vec<&str> = w
            .location
            .children
            .iter()
            .take(3)
            .map(|c| c.name.as_str())
            .collect();
        out.push(
            Item::new(
                "scope.children",
                &format!("Children · {n} projects"),
                Kind::Explore,
                ResultType::Resource,
                ScopeTag::child(&w.location.cwd),
            )
            .summary(&format!(
                "bounded child projects below this folder: {}{}",
                names.join(", "),
                if n > 3 { ", …" } else { "" }
            ))
            .keywords(&["children", "child", "descendants", "projects"])
            .group("Explore")
            .launch(Launch::Scope(Scope::Children))
            .freshness(freshness(w, "children"))
            .reason(Signal::Context, &format!("{n} child projects discovered")),
        );
    }
    out.push(
        Item::new(
            "scope.system",
            &format!("System · {}", w.host.name),
            Kind::Explore,
            ResultType::Resource,
            host_tag(w),
        )
        .summary(&format!(
            "host-wide capabilities on {} · {} · {}",
            w.host.name,
            w.host.os.label(),
            w.host.role.label()
        ))
        .keywords(&["system", "host", "machine", "everything"])
        .group("Explore")
        .launch(Launch::Scope(Scope::System))
        .reason(Signal::Default, "available on this host"),
    );
}

fn mise_items(w: &World, out: &mut Vec<Item>) {
    if !w.mise.installed || !w.tools.contains("mise") || !source_ready(w, "mise") {
        return;
    }
    let git = w.git_here();
    for cfg in &w.mise.configs {
        let trusted = cfg.trusted || w.trusted_now.contains(&cfg.path);
        for t in &cfg.tasks {
            let tag = tag_for_dir(w, &t.dir);
            let id = format!("mise.task.{}", t.namespaced);
            let label = match tag.direction {
                Scope::Here => format!("Run {}", t.name),
                _ => format!("Run {}", t.namespaced),
            };
            let label = match t.name.as_str() {
                "dev" if tag.direction == Scope::Parent => "Start development ecosystem".to_owned(),
                "up" if tag.direction == Scope::Parent => "Start required containers".to_owned(),
                "setup" if tag.direction == Scope::Parent => "Run workspace setup".to_owned(),
                "test" if tag.direction == Scope::Here => "Run tests".to_owned(),
                "dev" if tag.direction == Scope::Here => "Start dev server".to_owned(),
                "lint" if tag.direction == Scope::Here => "Run lint".to_owned(),
                _ => label,
            };
            let mut item = Item::new(&id, &label, Kind::Mise, ResultType::Action, tag.clone())
                .summary(&format!(
                    "{} · runs in {}",
                    t.description,
                    w.location.short(&t.dir)
                ))
                .exec(vec![cmd(w, "mise", &["run", &t.namespaced], &t.dir)])
                .effects(&[&format!("runs `{}` in {}", t.run, w.location.short(&t.dir))])
                .keywords(&[&t.name, &t.namespaced, "task", "mise", &t.description])
                .group("Tasks")
                .freshness(freshness(w, "mise"))
                .provenance(&format!("mise task · {}", w.location.short(&cfg.path)))
                .launch(Launch::Activity {
                    script: script_for_task(w, &t.name, &t.dir),
                })
                .reason(
                    Signal::Context,
                    &format!("defined by {}", w.location.short(&cfg.path)),
                )
                .alt("Show the task definition", "file.mise")
                .alt("Dry run", &format!("{id}.dry"));
            if !t.depends.is_empty() {
                item = item.effects(&[
                    &format!("runs `{}` in {}", t.run, w.location.short(&t.dir)),
                    &format!("after {}", t.depends.join(", ")),
                ]);
            }
            if t.name == "test" && tag.direction == Scope::Here {
                item = item.aliases(&["test"]);
            }

            item = item.risk(Risk::Mutating);
            if !trusted {
                item = item.confirm(Confirmation::Trust {
                    config: w.location.short(&cfg.path),
                });
            }
            if t.name == "test" && tag.direction == Scope::Here && git.is_some_and(|g| g.dirty()) {
                let n = git.map(|g| g.changed() + g.untracked).unwrap_or(0);
                item = item.reason(
                    Signal::Urgency,
                    &format!("{n} files changed since the last run"),
                );
                item = item.args(vec![
                    ArgSpec::text("filter", "test name substring", "")
                        .help("passes through to nextest"),
                    ArgSpec::choice("profile", &["default", "ci"], 0),
                ]);
                item = item.alt("Run with cargo test instead", "cargo.test");
            }
            if t.name == "dev" {
                item = item.effects(&[
                    &format!(
                        "starts a long-running server in {}",
                        w.location.short(&t.dir)
                    ),
                    "becomes a named activity",
                ]);
            }
            if t.name == "setup" {
                item = item.effects(&["installs tools and dependencies for every project"]);
            }
            out.push(item);
        }
    }

    let outdated = w.mise.outdated();
    if !outdated.is_empty() {
        let names: Vec<String> = outdated
            .iter()
            .map(|t| {
                format!(
                    "{} {} → {}",
                    t.name,
                    t.active.clone().unwrap_or_default(),
                    t.latest
                )
            })
            .collect();
        out.push(
            Item::new(
                "mise.outdated",
                &format!("Upgrade {} outdated mise tools", outdated.len()),
                Kind::Mise,
                ResultType::Recommendation,
                host_tag(w),
            )
            .summary("show current, requested and latest versions, then upgrade the selected tools")
            .exec(vec![cmd(
                w,
                "mise",
                &["outdated", "--json"],
                &w.location.cwd,
            )])
            .effects(&names.iter().map(|s| s.as_str()).collect::<Vec<_>>())
            .keywords(&[
                "mise", "tools", "outdated", "upgrade", "versions", "node", "python",
            ])
            .group("System")
            .freshness(freshness(w, "mise"))
            .launch(Launch::Snapshot {
                snapshot: "mise-outdated".into(),
            })
            .alt("Upgrade now (mise upgrade)", "upgrade.mise")
            .reason(
                Signal::Context,
                &format!("{} tools have newer versions", outdated.len()),
            ),
        );
    }
    let missing = w.mise.missing();
    if !missing.is_empty() {
        let names: Vec<&str> = missing.iter().map(|t| t.name.as_str()).collect();
        let root = w
            .location
            .project
            .as_ref()
            .map(|p| p.root.clone())
            .unwrap_or(w.location.cwd.clone());
        out.push(
            Item::new(
                "mise.install",
                &format!("Install missing tools · {}", names.join(", ")),
                Kind::Mise,
                ResultType::Recommendation,
                tag_for_dir(w, &root),
            )
            .summary("install the tools this configuration requests but the host lacks")
            .exec(vec![cmd(
                w,
                "mise",
                &["install", "--include-task-tools"],
                &root,
            )])
            .effect(Effect::MiseInstall)
            .keywords(&["mise", "install", "missing", "tools"])
            .group("Tasks")
            .risk(Risk::Mutating)
            .confirm(Confirmation::One)
            .launch(Launch::Activity { script: None })
            .reason(
                Signal::Urgency,
                &termrock::ui::text::plural(names.len(), "tool missing", "tools missing"),
            ),
        );
    }
    out.push(
        Item::new("mise.status", "Show mise configuration", Kind::Mise, ResultType::Resource, ScopeTag::here(&w.location.cwd))
            .summary("effective configuration from this folder through its ancestors: tools, tasks, environment, trust")
            .exec(vec![cmd(w, "mise", &["ls", "--json"], &w.location.cwd), cmd(w, "mise", &["tasks", "ls", "--all", "--json"], &w.location.cwd), cmd(w, "mise", &["trust", "--show"], &w.location.cwd)])
            .keywords(&["mise", "config", "tools", "trust", "env"])
            .group("Tasks")
            .freshness(freshness(w, "mise"))
            .launch(Launch::Snapshot { snapshot: "mise".into() })
            .reason(Signal::Default, &format!("{} configuration files in effect", w.mise.configs.len())),
    );
    if let Some(Err(e)) = &w.outputs.mise_tasks {
        out.push(
            Item::new(
                "mise.discovery",
                "mise tasks",
                Kind::Mise,
                ResultType::Resource,
                ScopeTag::here(&w.location.cwd),
            )
            .summary(&format!(
                "mise tasks ls failed · {e} · its output was not used"
            ))
            .keywords(&["mise", "tasks"])
            .group("Tasks")
            .freshness(Freshness::Unavailable(e.clone()))
            .launch(Launch::Snapshot {
                snapshot: "mise".into(),
            })
            .reason(Signal::Default, "discovery failed"),
        );
    }
}

fn native_task_items(w: &World, out: &mut Vec<Item>) {
    let cwd = w.location.cwd.clone();
    let here = ScopeTag::here(&cwd);
    let read = |name: &str| -> Option<String> {
        match &w.fs.get(&format!("{cwd}/{name}"))?.content {
            crate::sim::fs::Content::Text(t) => Some(t.clone()),
            _ => Some(String::new()),
        }
    };
    let mut discoveries: Vec<(manifest::Discovery, Kind, &'static str, &'static str)> = vec![];

    if w.fs.exists(&format!("{cwd}/package.json")) {
        let locks: Vec<&str> = ["pnpm-lock.yaml", "yarn.lock", "bun.lock", "bun.lockb"]
            .into_iter()
            .filter(|l| w.fs.exists(&format!("{cwd}/{l}")))
            .collect();
        let pj = read("package.json");
        discoveries.push((
            manifest::node_scripts(&cwd, pj.as_deref(), &locks),
            Kind::Node,
            "Node scripts",
            "package.json",
        ));
    }
    if w.tools.contains("just")
        && let Some(file) = ["justfile", "Justfile", ".justfile"]
            .into_iter()
            .find(|f| w.fs.exists(&format!("{cwd}/{f}")))
    {
        let summary = w
            .outputs
            .just_summary
            .clone()
            .unwrap_or(Err("just --summary was not run".into()));
        discoveries.push((
            manifest::just_recipes(&cwd, file, summary.as_deref().map_err(String::as_str)),
            Kind::Just,
            "Just",
            file,
        ));
    }
    if w.tools.contains("make") && w.fs.exists(&format!("{cwd}/Makefile")) {
        discoveries.push((
            manifest::make_discovery(&cwd, read("Makefile").as_deref()),
            Kind::Make,
            "Make",
            "Makefile",
        ));
    }
    if w.tools.contains("task")
        && let Some(file) = ["Taskfile.yml", "Taskfile.yaml"]
            .into_iter()
            .find(|f| w.fs.exists(&format!("{cwd}/{f}")))
    {
        let listing = w
            .outputs
            .task_list
            .clone()
            .unwrap_or(Err("task --list --json was not run".into()));
        discoveries.push((
            manifest::taskfile_tasks(&cwd, file, listing.as_deref().map_err(String::as_str)),
            Kind::Taskfile,
            "Taskfile",
            file,
        ));
    }

    if w.tools.contains("mise")
        && let Some(Ok(text)) = &w.outputs.mise_tasks
    {
        let mut d = manifest::mise_tasks(&cwd, Ok(text));
        let known: std::collections::BTreeSet<String> = w
            .mise
            .configs
            .iter()
            .flat_map(|c| {
                c.tasks
                    .iter()
                    .map(|t| format!("mise.task.{}", t.namespaced))
            })
            .collect();
        d.tasks.retain(|t| {
            !known.contains(&t.id) && !known.contains(&format!("mise.task.//:{}", t.name))
        });
        if !d.tasks.is_empty() {
            discoveries.push((d, Kind::Mise, "mise", "mise tasks ls"));
        }
    }
    for (d, kind, label, file) in discoveries {
        let runner_present = w.tools.contains(&d.runner);
        for t in &d.tasks {
            let argv: Vec<&str> = t.argv.iter().skip(1).map(String::as_str).collect();
            let mut item = Item::new(
                &t.id,
                &format!("{} {}", d.runner, t.name),
                kind,
                ResultType::Action,
                here.clone(),
            )
            .summary(&if t.description.is_empty() {
                format!(
                    "{} {} from {file} · the recipe runs through {}",
                    label.trim_end_matches('s'),
                    t.name,
                    d.runner
                )
            } else {
                format!("{} · {}", t.description, file)
            })
            .exec(vec![cmd(w, &t.argv[0], &argv, &cwd)])
            .keywords(&[&t.name, label, file, &d.runner, "task", "script"])
            .group("Tasks")
            .risk(Risk::Mutating)
            .provenance(&format!("{file} · {}", d.title(label)))
            .launch(Launch::Activity { script: None })
            .reason(Signal::Context, &format!("{file} here"));
            if !runner_present {
                item = item.freshness(Freshness::Unavailable(format!(
                    "{} is not on PATH",
                    d.runner
                )));
            }
            if d.capped() {
                item.cap_note = Some(format!(
                    "{} · first {} of {}",
                    d.title(label),
                    d.tasks.len(),
                    d.total
                ));
            }
            out.push(item);
        }
        if let Some(diag) = &d.diagnostic {
            out.push(
                Item::new(
                    &format!("{}.discovery", d.source),
                    &format!("{label} · {file}"),
                    kind,
                    ResultType::Resource,
                    here.clone(),
                )
                .summary(&format!("{diag} · no task was invented"))
                .keywords(&[label, file, "discovery"])
                .group("Tasks")
                .freshness(Freshness::Unavailable(diag.clone()))
                .launch(Launch::Files { path: cwd.clone() })
                .reason(Signal::Default, "discovery diagnostic"),
            );
        }
    }
}

fn git_items(w: &World, out: &mut Vec<Item>) {
    if !w.tools.contains("git") || !source_ready(w, "git") {
        return;
    }
    if let Some(g) = w.git_here() {
        git_here_items(w, g, out);
    }
    git_children_items(w, out);
    git_sibling_items(w, out);
    github_items(w, out);
}

fn git_here_items(w: &World, g: &crate::domain::stack::GitState, out: &mut Vec<Item>) {
    let tag = tag_for_dir(w, &g.path);
    let root_short = w.location.short(&g.path);
    let fresh = freshness(w, "git");
    let branch = g
        .branch
        .clone()
        .unwrap_or_else(|| format!("detached @ {}", g.head_short));
    let prov = format!(
        "built-in · {} in {}",
        if g.git_file {
            ".git file"
        } else {
            ".git directory"
        },
        root_short
    );
    out.push(
        Item::new(
            "git.status",
            "Show git status",
            Kind::Git,
            ResultType::Resource,
            tag.clone(),
        )
        .summary(&format!(
            "full git status of {root_short}: branch, upstream, staged, unstaged, untracked, stash"
        ))
        .exec(vec![cmd(w, "git", &["-C", &g.path, "status"], &g.path)])
        .keywords(&["git", "status", "branch", "worktree", "changes"])
        .group("Git")
        .freshness(fresh.clone())
        .provenance(&prov)
        .launch(Launch::Snapshot {
            snapshot: "git".into(),
        })
        .alt("Short status", "git.status_short")
        .reason(Signal::Context, &format!("on {branch}")),
    );
    out.push(
        Item::new(
            "git.status_short",
            "Show short status",
            Kind::Git,
            ResultType::Resource,
            tag.clone(),
        )
        .summary("git status --short as an activity")
        .exec(vec![cmd(
            w,
            "git",
            &["-C", &g.path, "status", "--short"],
            &g.path,
        )])
        .keywords(&["git", "status", "short"])
        .group("Git")
        .freshness(fresh.clone())
        .provenance(&prov)
        .launch(Launch::Activity { script: None })
        .reason(Signal::Default, "available"),
    );
    if g.dirty() {
        let n = g.changed() + g.untracked;
        out.push(
            Item::new(
                "git.review",
                &format!("Review {n} modified files"),
                Kind::Git,
                ResultType::Recommendation,
                tag.clone(),
            )
            .summary("show the changed files with their diff stat before committing or syncing")
            .exec(vec![
                cmd(w, "git", &["-C", &g.path, "status", "--short"], &g.path),
                cmd(w, "git", &["-C", &g.path, "diff", "--stat"], &g.path),
            ])
            .keywords(&["git", "diff", "changes", "modified", "review", "dirty"])
            .group("Git")
            .freshness(fresh.clone())
            .launch(Launch::Snapshot {
                snapshot: "git-diff".into(),
            })
            .alt("Open in a Git TUI (lazygit)", "git.tui")
            .reason(
                Signal::Urgency,
                &format!("{} modified · {} untracked", g.changed(), g.untracked),
            ),
        );
    }
    let pull_args: Vec<&str> = if g.pull_config == "ff-only" {
        vec!["-C", &g.path, "pull", "--ff-only"]
    } else {
        vec!["-C", &g.path, "pull"]
    };
    let mut pull = Item::new(
        "git.pull",
        "Pull",
        Kind::Git,
        ResultType::Action,
        tag.clone(),
    )
    .summary(&format!(
        "{} {root_short} from {}",
        if g.pull_config == "ff-only" {
            "fast-forward"
        } else {
            "pull (configured strategy)"
        },
        g.upstream.clone().unwrap_or("its upstream".into())
    ))
    .exec(vec![cmd(w, "git", &pull_args, &g.path)])
    .effect(if g.pull_config == "ff-only" {
        Effect::GitPull(g.path.clone())
    } else {
        Effect::GitPullMerge(g.path.clone())
    })
    .keywords(&["git", "pull", "sync", "fetch", "update", "behind"])
    .group("Git")
    .risk(Risk::Mutating)
    .freshness(fresh.clone())
    .provenance(&prov)
    .launch(Launch::Activity { script: None })
    .alt("Fetch only", "git.fetch")
    .alt("Pull with merge", "git.pull_merge")
    .alt("Pull with rebase", "git.pull_rebase")
    .alt("Push", "git.push");
    if g.behind > 0 && !g.diverged {
        pull = pull
            .reason(
                Signal::Urgency,
                &format!("branch is {} commits behind", g.behind),
            )
            .effects(&[&format!(
                "{} commits arrive · nothing is rewritten",
                g.behind
            )]);
    } else if g.diverged
        && g.pull_config != "ff-only"
        && !g.dirty()
        && g.conflicts == 0
        && g.in_progress.is_none()
    {
        pull = pull
            .effects(&[&format!(
                "{} ahead, {} behind · integrated with the configured {} strategy",
                g.ahead, g.behind, g.pull_config
            )])
            .confirm(Confirmation::One)
            .reason(
                Signal::Context,
                &format!("diverged · {} configured", g.pull_config),
            );
    } else if g.diverged
        && g.pull_config != "ff-only"
        && g.dirty()
        && g.conflicts == 0
        && g.in_progress.is_none()
    {
        let r = format!("{} modified", g.changed() + g.untracked);
        pull = pull
            .effects(&[&format!(
                "blocked · {r} · the command fails without changing anything"
            )])
            .confirm(Confirmation::One)
            .reason(Signal::Context, &format!("blocked · {r}"));
    } else if let Some(r) = g.block_reason() {
        pull = pull
            .effects(&[&format!(
                "blocked · {r} · the command fails without changing anything"
            )])
            .confirm(Confirmation::One)
            .reason(Signal::Context, &format!("blocked · {r}"));
    } else {
        pull = pull
            .effects(&["already up to date"])
            .reason(Signal::Default, "up to date");
    }
    out.push(pull);
    for (id, label, strategy, args) in [
        (
            "git.pull_merge",
            "Pull with merge",
            "merge",
            vec!["-C", g.path.as_str(), "pull", "--no-rebase"],
        ),
        (
            "git.pull_rebase",
            "Pull with rebase",
            "rebase",
            vec!["-C", g.path.as_str(), "pull", "--rebase"],
        ),
    ] {
        out.push(
            Item::new(id, label, Kind::Git, ResultType::Action, tag.clone())
                .summary(&format!("pull {root_short} with the {strategy} strategy · diverged history is integrated, not refused"))
                .exec(vec![cmd(w, "git", &args, &g.path)])
                .effect(Effect::GitPullMerge(g.path.clone()))
                .keywords(&["git", "pull", strategy, "diverged"])
                .group("Git")
                .risk(Risk::Mutating)
                .confirm(Confirmation::One)
                .freshness(fresh.clone())
                .provenance(&prov)
                .launch(Launch::Activity { script: None })
                .effects(&[if strategy == "rebase" { "local commits are replayed on top of the remote" } else { "a merge commit joins both histories" }])
                .reason(Signal::Default, if g.diverged { "history has diverged" } else { "ordinary pull behaviour" }),
        );
    }
    out.push(
        Item::new(
            "git.fetch",
            "Fetch and prune",
            Kind::Git,
            ResultType::Action,
            tag.clone(),
        )
        .summary(&format!(
            "update remote refs of {root_short} and drop deleted branches · nothing local changes"
        ))
        .exec(vec![cmd(
            w,
            "git",
            &["-C", &g.path, "fetch", "--prune"],
            &g.path,
        )])
        .effect(Effect::GitFetch(g.path.clone()))
        .keywords(&["git", "fetch", "prune", "remote"])
        .group("Git")
        .risk(Risk::Mutating)
        .freshness(fresh.clone())
        .provenance(&prov)
        .launch(Launch::Activity { script: None })
        .reason(Signal::Default, "available"),
    );
    let mut push = Item::new(
        "git.push",
        "Push",
        Kind::Git,
        ResultType::Action,
        tag.clone(),
    )
    .summary(&format!(
        "push {branch} of {root_short} to {}",
        g.upstream.clone().unwrap_or("its upstream".into())
    ))
    .exec(vec![cmd(w, "git", &["-C", &g.path, "push"], &g.path)])
    .effect(Effect::GitPush(g.path.clone()))
    .keywords(&["git", "push", "upload", "ahead"])
    .group("Git")
    .risk(Risk::Mutating)
    .confirm(Confirmation::One)
    .freshness(fresh.clone())
    .provenance(&prov)
    .launch(Launch::Activity { script: None })
    .alt("Push dry run", "git.push_dry");
    push = if g.ahead > 0 {
        push.reason(Signal::Context, &format!("{} commits ahead", g.ahead))
            .effects(&[&format!("{} commits reach the remote", g.ahead)])
    } else if g.upstream.is_none() {
        push.reason(Signal::Default, "no upstream · push will fail and say so")
    } else {
        push.reason(Signal::Default, "nothing to push")
    };
    if let Some(r) = &g.push_rejected {
        push = push.effects(&[&format!("the remote rejects it ({r}) · nothing changes")]);
    }
    out.push(push);
    out.push(
        Item::new(
            "git.push_dry",
            "Push dry run",
            Kind::Git,
            ResultType::Action,
            tag.clone(),
        )
        .summary("show what a push would send")
        .exec(vec![cmd(
            w,
            "git",
            &["-C", &g.path, "push", "--dry-run"],
            &g.path,
        )])
        .keywords(&["git", "push", "dry"])
        .group("Git")
        .freshness(fresh.clone())
        .launch(Launch::Activity { script: None })
        .reason(Signal::Default, "read-only"),
    );
    if g.branch.as_deref() != Some(g.primary.as_str()) {
        let mut sw = Item::new(
            "git.switch_primary",
            &format!("Switch to {} (primary branch)", g.primary),
            Kind::Git,
            ResultType::Action,
            tag.clone(),
        )
        .summary(&format!(
            "switch {root_short} to its resolved primary branch {}",
            g.primary
        ))
        .exec(vec![cmd(
            w,
            "git",
            &["-C", &g.path, "switch", &g.primary],
            &g.path,
        )])
        .effect(Effect::GitSwitch(g.path.clone(), g.primary.clone()))
        .keywords(&[
            "git",
            "checkout main",
            "checkout master",
            "switch",
            "primary",
            "default branch",
        ])
        .group("Git")
        .risk(Risk::Mutating)
        .freshness(fresh.clone())
        .provenance(&prov)
        .launch(Launch::Activity { script: None })
        .reason(
            Signal::Context,
            &format!(
                "primary branch resolves to {} ({})",
                g.primary,
                g.default_from.unwrap_or("no default")
            ),
        );
        if let Some(r) = g.block_reason() {
            sw = sw
                .effects(&[&format!("blocked · {r} · work is never discarded")])
                .confirm(Confirmation::One);
        }
        out.push(sw);
    }

    if g.default_from.is_some() && g.command_failure.is_none() {
        let (cands, total) = g.merged_candidates();
        out.push(
            Item::new(
                "git.gc",
                "Garbage-collect the repository",
                Kind::Git,
                ResultType::Action,
                tag.clone(),
            )
            .summary(&format!(
                "git gc in {root_short} · packs objects, no aggressive flag"
            ))
            .exec(vec![cmd(w, "git", &["-C", &g.path, "gc"], &g.path)])
            .effect(Effect::GitGc(g.path.clone()))
            .keywords(&["git", "gc", "hygiene", "pack", "garbage"])
            .group("Git")
            .risk(Risk::Mutating)
            .freshness(fresh.clone())
            .provenance(&prov)
            .launch(Launch::Activity { script: None })
            .reason(
                Signal::Default,
                if g.gc_needed {
                    "loose objects piling up"
                } else {
                    "available"
                },
            ),
        );
        if !cands.is_empty() {
            let mut args: Vec<&str> = vec!["-C", &g.path, "branch", "-d", "--"];
            args.extend(cands.iter().map(String::as_str));
            let title = if total > cands.len() {
                format!("Delete {} of {total} merged branches", cands.len())
            } else {
                format!("Delete {} merged branches", cands.len())
            };
            out.push(
                Item::new("git.delete_merged", &title, Kind::Git, ResultType::Action, tag.clone())
                    .summary(&format!("branches merged into {} · {} · safe deletion only (-d): Git refuses unmerged or checked-out branches", g.primary, cands.join(", ")))
                    .exec(vec![cmd(w, "git", &args, &g.path)])
                    .effect(Effect::GitDeleteBranches(g.path.clone(), cands.clone()))
                    .effects(&[&format!("{} branch refs removed · reflog keeps the commits · never -D", cands.len())])
                    .keywords(&["git", "branch", "delete", "merged", "hygiene", "cleanup"])
                    .group("Git")
                    .risk(Risk::Destructive)
                    .confirm(Confirmation::One)
                    .freshness(fresh.clone())
                    .provenance(&prov)
                    .launch(Launch::Activity { script: None })
                    .reason(Signal::Context, &format!("{total} merged into {}", g.primary)),
            );
        }
    } else if g.command_failure.is_some() {
        out.push(
            Item::new("git.hygiene_unavailable", "Git hygiene", Kind::Git, ResultType::Resource, tag.clone())
                .summary(&format!("git could not resolve the repository ({}) · fetch, gc and branch cleanup are hidden until it can", g.command_failure.clone().unwrap_or_default()))
                .keywords(&["git", "hygiene"])
                .group("Git")
                .freshness(Freshness::Unavailable(g.command_failure.clone().unwrap_or_default()))
                .launch(Launch::Snapshot { snapshot: "git".into() })
                .reason(Signal::Default, "unavailable"),
        );
    } else {
        out.push(
            Item::new("git.hygiene_unavailable", "Git hygiene · no default branch", Kind::Git, ResultType::Resource, tag.clone())
                .summary("origin/HEAD is unset and neither main nor master exists · merged-branch review needs a default branch")
                .keywords(&["git", "hygiene", "default", "branch"])
                .group("Git")
                .freshness(Freshness::Unavailable("no default branch".into()))
                .launch(Launch::Snapshot { snapshot: "git".into() })
                .reason(Signal::Default, "unavailable"),
        );
    }
    if g.in_progress.is_some() || g.conflicts > 0 {
        out.push(
            Item::new("git.tui", "Resolve the rebase in lazygit", Kind::Git, ResultType::Handoff, tag.clone())
                .summary("conflict and history work goes to the specialist Git TUI; holla returns when it exits")
                .exec(vec![cmd(w, "lazygit", &["-p", &g.path], &g.path)])
                .keywords(&["git", "rebase", "conflicts", "lazygit", "resolve", "merge"])
                .group("Git")
                .freshness(fresh.clone())
                .launch(Launch::Handoff { tool: "lazygit".into() })
                .reason(Signal::Urgency, &format!("{} in progress · {} conflicts", g.in_progress.clone().unwrap_or("operation".into()), g.conflicts)),
        );
    } else {
        out.push(
            Item::new(
                "git.tui",
                "Open lazygit",
                Kind::Git,
                ResultType::Handoff,
                tag.clone(),
            )
            .summary("history, staging and rebasing in the specialist Git TUI")
            .exec(vec![cmd(w, "lazygit", &["-p", &g.path], &g.path)])
            .keywords(&["git", "lazygit", "history", "stage", "commit"])
            .group("Git")
            .launch(Launch::Handoff {
                tool: "lazygit".into(),
            })
            .reason(Signal::Default, "installed on this host"),
        );
    }
    if g.stash > 0 {
        out.push(
            Item::new(
                "git.stash",
                &format!("Show stash ({})", g.stash),
                Kind::Git,
                ResultType::Resource,
                tag.clone(),
            )
            .summary("list stashed changes")
            .exec(vec![cmd(
                w,
                "git",
                &["-C", &g.path, "stash", "list"],
                &g.path,
            )])
            .keywords(&["git", "stash"])
            .group("Git")
            .launch(Launch::Snapshot {
                snapshot: "git-stash".into(),
            })
            .reason(Signal::Context, &format!("{} stash entries", g.stash)),
        );
    }
}

fn git_children_items(w: &World, out: &mut Vec<Item>) {
    let here = w.git_here().map(|g| g.path.clone());
    let children: Vec<&crate::domain::stack::GitState> = w
        .git
        .iter()
        .filter(|x| Some(&x.path) != here.as_ref())
        .filter(|x| x.path.starts_with(&format!("{}/", w.location.cwd)))
        .collect();
    if children.is_empty() || w.location.children.is_empty() {
        return;
    }
    let dirty = children.iter().filter(|x| x.dirty()).count();
    let behind = children.iter().filter(|x| x.behind > 0).count();
    let n = children.iter().filter(|x| x.worktree_of.is_none()).count();
    let ctag = ScopeTag::child(&w.location.cwd);
    let statuses: Vec<Command> = children
        .iter()
        .map(|g| {
            cmd(
                w,
                "git",
                &["-C", &g.path, "status", "--porcelain=v2", "--branch"],
                &g.path,
            )
        })
        .collect();
    out.push(
        Item::new(
            "git.status_all",
            &format!("Status across {n} child projects"),
            Kind::Git,
            ResultType::Recommendation,
            ctag.clone(),
        )
        .summary("inspect every discovered Git project · worktrees deduplicated, submodules marked")
        .exec(statuses)
        .keywords(&[
            "git",
            "status",
            "children",
            "projects",
            "all",
            "collection",
            "workspace",
        ])
        .group("Git")
        .freshness(freshness(w, "children"))
        .launch(Launch::Snapshot {
            snapshot: "git-all".into(),
        })
        .reason(Signal::Urgency, &format!("{dirty} dirty · {behind} behind")),
    );
    out.push(
        Item::new("git.pull_all", "Pull every child project", Kind::Plan, ResultType::Flow, ctag.clone())
            .summary("fast-forward-only pulls across the eligible projects, as a reviewable plan with exclusions")
            .exec(children.iter().map(|g| cmd(w, "git", &["-C", &g.path, "pull", "--ff-only"], &g.path)).collect())
            .keywords(&["git", "pull", "sync projects", "all", "children", "synchronize"])
            .group("Git")
            .risk(Risk::Mutating)
            .confirm(Confirmation::One)
            .launch(Launch::Plan { plan: "git-pull-all".into() })
            .reason(Signal::Context, &format!("{behind} projects behind · {dirty} blocked by local changes")),
    );
    out.push(
        Item::new("git.switch_all", "Switch every project to its primary branch", Kind::Plan, ResultType::Flow, ctag)
            .summary("resolve each project's primary branch (main, master, develop…) and switch the eligible ones")
            .exec(children.iter().map(|g| cmd(w, "git", &["-C", &g.path, "switch", &g.primary], &g.path)).collect())
            .keywords(&["git", "checkout main", "checkout master", "switch to default", "switch to primary branch", "children"])
            .group("Git")
            .risk(Risk::Mutating)
            .confirm(Confirmation::One)
            .launch(Launch::Plan { plan: "git-switch-primary".into() })
            .reason(Signal::Default, "primary branch resolved per project, never hard-coded"),
    );
}

fn sibling_repos(w: &World) -> Vec<&crate::domain::stack::GitState> {
    let cwd = &w.location.cwd;
    let mut v: Vec<&crate::domain::stack::GitState> = w
        .git
        .iter()
        .filter(|g| {
            crate::sim::fs::Fs::parent(&g.path).as_deref() == Some(cwd.as_str())
                && (w.fs.exists(&format!("{}/.git", g.path)) || true)
        })
        .collect();
    v.sort_by(|a, b| a.name().cmp(b.name()));
    v
}

fn git_sibling_items(w: &World, out: &mut Vec<Item>) {
    let repos = sibling_repos(w);
    if repos.len() < 2 {
        return;
    }
    let names: Vec<&str> = repos.iter().map(|g| g.name()).collect();
    let ctag = ScopeTag::child(&w.location.cwd);
    let count = repos.len();
    let member =
        |g: &crate::domain::stack::GitState, label: &str, args: &[&str], effect: Option<Effect>| {
            (
                format!("{label} {}", g.name()),
                vec![cmd(w, "git", args, &g.path)],
                effect,
                tag_for_dir(w, &g.path),
            )
        };
    let prov = format!(
        "built-in · {} immediate repositories under {}",
        count,
        w.location.cwd_short()
    );
    out.push(
        Item::new(
            "git.pull-all",
            &format!("Pull {count} sibling repositories"),
            Kind::Git,
            ResultType::Action,
            ctag.clone(),
        )
        .summary(&format!(
            "git pull in parallel in each of {} · each repository keeps its own output and result",
            names.join(", ")
        ))
        .batch(
            BatchMode::Parallel,
            repos
                .iter()
                .map(|g| {
                    member(
                        g,
                        "pull",
                        &["-C", &g.path, "pull"],
                        Some(Effect::GitPull(g.path.clone())),
                    )
                })
                .collect(),
        )
        .keywords(&["git", "pull", "all", "siblings", "repositories", "batch"])
        .group("Git")
        .risk(Risk::Mutating)
        .confirm(Confirmation::One)
        .provenance(&prov)
        .reason(Signal::Context, &format!("{count} repositories here")),
    );
    out.push(
        Item::new("git.push-all", &format!("Push {count} sibling repositories"), Kind::Git, ResultType::Action, ctag.clone())
            .summary(&format!("git push in parallel in each of {} · rejections and missing upstreams are reported per repository", names.join(", ")))
            .batch(BatchMode::Parallel, repos.iter().map(|g| member(g, "push", &["-C", &g.path, "push"], Some(Effect::GitPush(g.path.clone())))).collect())
            .keywords(&["git", "push", "all", "siblings", "batch"])
            .group("Git")
            .risk(Risk::Mutating)
            .confirm(Confirmation::One)
            .provenance(&prov)
            .reason(Signal::Default, "available"),
    );
    out.push(
        Item::new(
            "git.status-all",
            &format!("Short status of {count} sibling repositories"),
            Kind::Git,
            ResultType::Action,
            ctag.clone(),
        )
        .summary("git status --short in each repository, one after another")
        .batch(
            BatchMode::Sequential,
            repos
                .iter()
                .map(|g| member(g, "status", &["-C", &g.path, "status", "--short"], None))
                .collect(),
        )
        .keywords(&["git", "status", "all", "siblings", "batch"])
        .group("Git")
        .provenance(&prov)
        .reason(Signal::Default, "read-only"),
    );
    let mut members = vec![];
    let mut mirrored = 0;
    for g in &repos {
        members.push(member(
            g,
            "push origin ·",
            &["-C", &g.path, "push", "origin"],
            Some(Effect::GitPushRemote(g.path.clone(), "origin".into())),
        ));
        if g.has_remote("gitlab") {
            mirrored += 1;
            members.push(member(
                g,
                "push gitlab ·",
                &["-C", &g.path, "push", "gitlab"],
                Some(Effect::GitPushRemote(g.path.clone(), "gitlab".into())),
            ));
        } else {
            members.push((
                format!("push gitlab · {} (no gitlab remote)", g.name()),
                vec![Command::internal(
                    "skipped: no gitlab remote",
                    &g.path,
                    &w.host.name,
                )],
                None,
                tag_for_dir(w, &g.path),
            ));
        }
    }
    out.push(
        Item::new("git.push-all-remotes", &format!("Push {count} repositories to origin and GitLab"), Kind::Git, ResultType::Action, ctag)
            .summary(&format!("push origin always; push gitlab where `git remote get-url gitlab` succeeds ({mirrored} of {count}) · no other remote is pushed"))
            .batch(BatchMode::Parallel, members)
            .keywords(&["git", "push", "gitlab", "mirror", "remotes", "batch"])
            .group("Git")
            .risk(Risk::Mutating)
            .confirm(Confirmation::One)
            .provenance(&prov)
            .effects(&[&format!("origin in {count} repositories · gitlab in {mirrored}")])
            .reason(Signal::Default, &format!("{mirrored} repositories have a gitlab remote")),
    );
}

fn github_items(w: &World, out: &mut Vec<Item>) {
    if !w.tools.contains("gh") {
        return;
    }
    if w.github.logged_in {
        out.push(
            Item::new("github.clone", "Clone a GitHub repository…", Kind::Git, ResultType::Action, ScopeTag::here(&w.location.cwd))
                .summary(&format!("choose a repository from {} or its organizations, review owner, protocol, destination and primary branch, then clone", w.github.account))
                .exec(vec![cmd(w, "gh", &["repo", "clone", "<owner>/<repository>", "<destination>"], &w.location.cwd)])
                .keywords(&["clone", "github", "gh", "repository", "checkout"])
                .group("Git")
                .risk(Risk::Mutating)
                .confirm(Confirmation::One)
                .freshness(freshness(w, "github"))
                .args(vec![
                    ArgSpec::choice("owner", &{
                        let mut v = vec![w.github.account.as_str()];
                        v.extend(w.github.orgs.iter().map(|s| s.as_str()));
                        v
                    }, 0),
                    ArgSpec::text("repository", "name", "").required().help("searched in the owner's repositories"),
                    ArgSpec::choice("protocol", &["ssh", "https"], 0),
                    ArgSpec::text("destination", "folder", &w.location.cwd_short()).help("never overwrites an existing folder"),
                ])
                .launch(Launch::Activity { script: None })
                .reason(Signal::Default, &format!("logged in as {}", w.github.account)),
        );
    } else if let SourceState::Failed(r) = w.source_state("github") {
        out.push(
            Item::new(
                "github.clone",
                "Clone a GitHub repository…",
                Kind::Git,
                ResultType::Action,
                ScopeTag::here(&w.location.cwd),
            )
            .summary("cloning needs an active gh login")
            .exec(vec![cmd(w, "gh", &["auth", "login"], &w.location.cwd)])
            .keywords(&["clone", "github", "gh"])
            .group("Git")
            .freshness(Freshness::Unavailable(r))
            .launch(Launch::Insert)
            .reason(Signal::Default, "gh is not logged in"),
        );
    }
}

fn rust_items(w: &World, out: &mut Vec<Item>) {
    let cwd = w.location.cwd.clone();

    let manifest_here = w.fs.exists(&format!("{cwd}/Cargo.toml"));
    let Some(p) = &w.location.project else {
        return;
    };
    let crate::domain::context::ProjectKind::Rust { workspace, member } = &p.kind else {
        return;
    };
    if !w.tools.contains("cargo") {
        return;
    }
    let tag = tag_for_dir(w, &p.root);
    let ws = *workspace;
    let fresh = freshness(w, "cargo");
    let run_dir = if manifest_here {
        cwd.clone()
    } else {
        p.root.clone()
    };
    let prov = format!("built-in · Cargo.toml in {}", w.location.short(&run_dir));
    let mut push =
        |id: &str, label: &str, args: &[&str], summary: &str, kw: &[&str], risk: Risk| {
            out.push(
                Item::new(id, label, Kind::Rust, ResultType::Action, tag.clone())
                    .summary(summary)
                    .exec(vec![cmd(w, "cargo", args, &run_dir)])
                    .keywords(kw)
                    .group("Tasks")
                    .risk(risk)
                    .freshness(fresh.clone())
                    .provenance(&prov)
                    .launch(Launch::Activity { script: None })
                    .reason(Signal::Context, "Cargo.toml here"),
            );
        };
    let ws_flag: Vec<&'static str> = if ws { vec!["--workspace"] } else { vec![] };
    let with = |base: &[&'static str]| -> Vec<&'static str> {
        let mut v: Vec<&'static str> = base.to_vec();
        v.extend(ws_flag.iter().copied());
        v
    };
    push(
        "cargo.build",
        "cargo build",
        &with(&["build"]),
        "build the debug profile · compiler diagnostics stay in the activity",
        &["cargo", "build", "compile"],
        Risk::Mutating,
    );
    push(
        "cargo.test",
        "cargo test",
        &with(&["test"]),
        "run the tests with the built-in harness · failures are named",
        &["cargo", "test", "tests"],
        Risk::Mutating,
    );
    push(
        "cargo.clippy",
        "cargo clippy",
        &["clippy", "--all-targets", "--all-features"],
        "lint every target and feature · warnings do not fail the run",
        &["cargo", "clippy", "lint"],
        Risk::Mutating,
    );
    push(
        "cargo.check",
        "cargo check",
        &with(&["check"]),
        "type-check every target without producing binaries",
        &["cargo", "check", "compile"],
        Risk::ReadOnly,
    );
    push(
        "cargo.fmt",
        "cargo fmt --check",
        &["fmt", "--all", "--", "--check"],
        "check formatting",
        &["cargo", "fmt", "format"],
        Risk::ReadOnly,
    );
    if let Some(m) = member {
        push(
            "cargo.run",
            &format!("cargo run -p {m}"),
            &["run", "-p", m],
            "run the member this folder belongs to",
            &["cargo", "run"],
            Risk::Mutating,
        );
    }
    let cg = &w.cargo;
    let mut clean = Item::new("cargo.clean", &match cg.target {
        Some(_) => format!("cargo clean · {}", termrock::ui::text::bytes(cg.target_bytes)),
        None => "cargo clean".into(),
    }, Kind::Cleanup, ResultType::Action, tag.clone())
        .summary(&match &cg.target {
            Some(t) => format!("remove the resolved target directory {} · {} files · tool-native permanent removal, not Trash · rebuilt by the next build", w.location.short(t), cg.target_files),
            None => "no target directory resolved · cargo clean would remove nothing".into(),
        })
        .exec(vec![cmd(w, "cargo", &["clean"], &run_dir)])
        .keywords(&["cargo", "clean", "target", "artifacts", "space"])
        .group("Disk")
        .risk(Risk::Destructive)
        .confirm(Confirmation::One)
        .freshness(fresh.clone())
        .provenance(&prov)
        .launch(Launch::Activity { script: None })
        .alt("Show what clean would remove (dry run)", "cargo.clean_dry")
        .reason(Signal::Context, &match &cg.target {
            Some(_) => format!("{} of generated artifacts", termrock::ui::text::bytes(cg.target_bytes)),
            None => "nothing to remove".into(),
        });
    if let Some(t) = &cg.target {
        clean = clean.effect(Effect::CargoClean(t.clone()));
        let mut effects = vec![format!(
            "{} freed by cargo itself · permanent · next build recompiles everything",
            termrock::ui::text::bytes(cg.target_bytes)
        )];
        if let Some(s) = &cg.target_shared_with {
            effects.push(format!("shared target: {s} loses its artifacts too"));
        }
        if !t.starts_with(&run_dir) {
            effects.push(format!(
                "custom target directory outside the project: {}",
                w.location.short(t)
            ));
        }
        let e: Vec<&str> = effects.iter().map(String::as_str).collect();
        clean = clean.effects(&e);
    }
    out.push(clean);
    out.push(
        Item::new(
            "cargo.clean_dry",
            "cargo clean dry run",
            Kind::Rust,
            ResultType::Action,
            tag,
        )
        .summary("list what cargo clean would remove · nothing is deleted")
        .exec(vec![cmd(
            w,
            "cargo",
            &["clean", "--dry-run", "--verbose"],
            &run_dir,
        )])
        .keywords(&["cargo", "clean", "dry"])
        .group("Disk")
        .freshness(fresh)
        .launch(Launch::Activity { script: None })
        .reason(Signal::Default, "read-only"),
    );
}

fn docker_items(w: &World, out: &mut Vec<Item>) {
    if !w.tools.contains("docker") || !source_ready(w, "docker") {
        return;
    }
    let d = &w.docker;
    let htag = host_tag(w);
    let fresh = freshness(w, "docker");
    let cwd = w.location.cwd.clone();
    let compose_file = [
        "docker-compose.yml",
        "docker-compose.yaml",
        "compose.yml",
        "compose.yaml",
    ]
    .into_iter()
    .find(|f| w.fs.exists(&format!("{cwd}/{f}")))
    .map(|f| format!("{cwd}/{f}"));
    if let Err(reason) = &d.daemon {
        out.push(
            Item::new("docker.daemon", "Docker", Kind::Docker, ResultType::Resource, htag.clone())
                .summary("the Docker daemon could not be reached; container actions are unavailable until it is")
                .exec(vec![cmd(w, "docker", &["info"], &cwd)])
                .keywords(&["docker", "containers", "daemon"])
                .group("Services")
                .freshness(Freshness::Unavailable(reason.clone()))
                .launch(Launch::Snapshot { snapshot: "docker-unavailable".into() })
                .reason(Signal::Default, "capability present, daemon unreachable"),
        );

        docker_system_items(w, out, &htag, &fresh);
        if let Some(f) = compose_file {
            docker_compose_items(w, out, &f);
        }
        return;
    }
    out.push(
        Item::new(
            "docker.ps",
            "List containers",
            Kind::Docker,
            ResultType::Resource,
            htag.clone(),
        )
        .summary(&format!(
            "{} containers · {} running · with image, state, ports, project and size",
            d.containers.len(),
            d.running()
        ))
        .exec(vec![cmd(w, "docker", &["ps", "-a", "--size"], &cwd)])
        .keywords(&["docker", "containers", "ps", "list", "running"])
        .group("Services")
        .freshness(fresh.clone())
        .launch(Launch::Snapshot {
            snapshot: "docker-ps".into(),
        })
        .reason(Signal::Context, &format!("{} running", d.running())),
    );
    out.push(
        Item::new(
            "docker.df",
            "Show Docker disk usage",
            Kind::Docker,
            ResultType::Resource,
            htag.clone(),
        )
        .summary("images, containers, volumes and build cache with reclaimable space")
        .exec(vec![cmd(w, "docker", &["system", "df"], &cwd)])
        .keywords(&["docker", "disk", "space", "usage", "df", "reclaimable"])
        .group("Services")
        .freshness(fresh.clone())
        .launch(Launch::Snapshot {
            snapshot: "docker-df".into(),
        })
        .reason(
            Signal::Context,
            &format!("{:.1} GB reclaimable", d.reclaimable_gb),
        ),
    );
    for c in &d.containers {
        let scope = match (&c.project, &d.compose) {
            (Some(p), Some(comp)) if p == &comp.name => tag_for_dir(w, &comp.dir),
            _ => htag.clone(),
        };
        let mut item = Item::new(
            &format!("docker.logs.{}", c.name),
            &format!("Follow {} logs", c.name),
            Kind::Docker,
            ResultType::Action,
            scope,
        )
        .summary(&format!(
            "stream the last 10 minutes and follow · {} · {}",
            c.image,
            if c.running { "running" } else { "exited" }
        ))
        .exec(vec![cmd(
            w,
            "docker",
            &["logs", "-f", "--since", "10m", &c.name],
            &cwd,
        )])
        .keywords(&[
            "docker",
            "logs",
            "service logs",
            &c.name,
            "follow",
            "container",
        ])
        .group("Services")
        .freshness(fresh.clone())
        .launch(Launch::Activity {
            script: if c.name == "acme-api-1" {
                Some("docker:logs-api".into())
            } else {
                None
            },
        })
        .alt("Restart", &format!("docker.restart.{}", c.name))
        .alt("Stop", &format!("docker.stop.{}", c.name));
        item = match c.health.as_deref() {
            Some("unhealthy") => item.reason(Signal::Urgency, "service is unhealthy"),
            _ if c.running => item.reason(Signal::Context, "running"),
            _ => item.reason(Signal::Default, "exited"),
        };
        out.push(item);
        out.push(
            Item::new(
                &format!("docker.restart.{}", c.name),
                &format!("Restart {}", c.name),
                Kind::Docker,
                ResultType::Action,
                htag.clone(),
            )
            .summary(&format!(
                "docker restart {} · the container comes back and its health is re-observed",
                c.name
            ))
            .exec(vec![cmd(w, "docker", &["restart", &c.name], &cwd)])
            .effect(Effect::DockerRestart(c.name.clone()))
            .keywords(&["docker", "restart", &c.name])
            .group("Services")
            .risk(Risk::Mutating)
            .confirm(Confirmation::One)
            .freshness(fresh.clone())
            .launch(Launch::Activity { script: None })
            .reason(Signal::Default, "container action"),
        );
        out.push(
            Item::new(
                &format!("docker.stop.{}", c.name),
                &format!("Stop {}", c.name),
                Kind::Docker,
                ResultType::Action,
                htag.clone(),
            )
            .summary(&format!(
                "docker stop {} · graceful stop with the default 10 s timeout · nothing is removed",
                c.name
            ))
            .exec(vec![cmd(w, "docker", &["stop", &c.name], &cwd)])
            .effect(Effect::DockerStop(c.name.clone()))
            .keywords(&["docker", "stop", &c.name])
            .group("Services")
            .risk(Risk::Mutating)
            .confirm(Confirmation::One)
            .freshness(fresh.clone())
            .launch(Launch::Activity { script: None })
            .reason(Signal::Default, "container action"),
        );
    }
    if let Some(f) = compose_file {
        docker_compose_items(w, out, &f);
    }
    docker_system_items(w, out, &htag, &fresh);
}

fn docker_compose_items(w: &World, out: &mut Vec<Item>, file: &str) {
    let d = &w.docker;
    let dir = crate::sim::fs::Fs::parent(file).unwrap_or(w.location.cwd.clone());
    let ctag = tag_for_dir(w, &dir);
    let fresh = freshness(w, "docker");
    let name = d
        .compose
        .as_ref()
        .map(|c| c.name.clone())
        .unwrap_or_else(|| dir.rsplit('/').next().unwrap_or("compose").to_owned());
    let prov = format!(
        "built-in · {} · daemon and plugin are not checked at discovery",
        w.location.short(file)
    );
    let services: Vec<String> = d
        .compose
        .as_ref()
        .map(|c| c.services.clone())
        .unwrap_or_default();
    let app_services: Vec<&str> = services
        .iter()
        .filter(|s| !matches!(s.as_str(), "db" | "redis"))
        .map(String::as_str)
        .collect();
    if !app_services.is_empty() {
        let mut args = vec!["compose", "logs", "-f", "--tail", "200"];
        args.extend(app_services.iter().copied());
        out.push(
            Item::new("docker.compose_logs", &format!("Follow logs from {}", app_services.join(", ")), Kind::Docker, ResultType::Action, ctag.clone())
                .summary(&format!("project-aware multi-service logs of Compose project {name} · each stream keeps its identity"))
                .exec(vec![cmd(w, "docker", &args, &dir)])
                .keywords(&["docker", "compose", "logs", "service logs", "follow", "api", "worker", "scheduler"])
                .group("Services")
                .freshness(fresh.clone())
                .provenance(&prov)
                .launch(Launch::Activity { script: Some("logs:acme".into()) })
                .reason(Signal::Context, &format!("Compose project {name} defined here")),
        );
    }
    out.push(
        Item::new(
            "compose.logs",
            "Show recent Compose logs",
            Kind::Docker,
            ResultType::Action,
            ctag.clone(),
        )
        .summary(&format!(
            "docker compose logs --tail 200 for project {name} · a finite snapshot, no follow"
        ))
        .exec(vec![cmd(
            w,
            "docker",
            &["compose", "logs", "--tail", "200"],
            &dir,
        )])
        .keywords(&["docker", "compose", "logs", "recent", "tail"])
        .group("Services")
        .freshness(fresh.clone())
        .provenance(&prov)
        .launch(Launch::Activity { script: None })
        .reason(Signal::Default, "read-only"),
    );
    out.push(
        Item::new(
            "compose.up",
            "Start the Compose project",
            Kind::Docker,
            ResultType::Action,
            ctag.clone(),
        )
        .summary(&format!(
            "docker compose up -d for project {name} · {} services",
            services.len()
        ))
        .exec(vec![cmd(w, "docker", &["compose", "up", "-d"], &dir)])
        .effect(Effect::ComposeUp)
        .keywords(&["docker", "compose", "up", "start"])
        .group("Services")
        .risk(Risk::Mutating)
        .confirm(Confirmation::One)
        .freshness(fresh.clone())
        .provenance(&prov)
        .launch(Launch::Activity { script: None })
        .reason(Signal::Default, "project containers"),
    );
    out.push(
        Item::new("compose.down", "Stop the Compose project", Kind::Docker, ResultType::Action, ctag.clone())
            .summary(&format!("docker compose down for project {name} · containers and the network are removed · volumes are kept (no --volumes)"))
            .exec(vec![cmd(w, "docker", &["compose", "down"], &dir)])
            .effect(Effect::ComposeDown)
            .effects(&["project containers and network removed · named volumes stay"])
            .keywords(&["docker", "compose", "down", "stop"])
            .group("Services")
            .risk(Risk::Mutating)
            .confirm(Confirmation::One)
            .freshness(fresh.clone())
            .provenance(&prov)
            .launch(Launch::Activity { script: None })
            .reason(Signal::Default, "project containers only"),
    );
    if d.compose.is_some() {
        out.push(
            Item::new(
                "docker.compose_ps",
                "Show Compose services",
                Kind::Docker,
                ResultType::Resource,
                ctag,
            )
            .summary(&format!("services of {name} with state and health"))
            .exec(vec![cmd(w, "docker", &["compose", "ps", "--all"], &dir)])
            .keywords(&["docker", "compose", "services", "health"])
            .group("Services")
            .freshness(fresh)
            .launch(Launch::Snapshot {
                snapshot: "compose-ps".into(),
            })
            .reason(
                Signal::Context,
                &format!("{} unhealthy", d.unhealthy().len()),
            ),
        );
    }
}

fn docker_system_items(w: &World, out: &mut Vec<Item>, htag: &ScopeTag, fresh: &Freshness) {
    let d = &w.docker;
    let cwd = w.location.cwd.clone();
    let running: Vec<&str> = d
        .containers
        .iter()
        .filter(|c| c.running)
        .map(|c| c.name.as_str())
        .collect();
    let all: Vec<&str> = d.containers.iter().map(|c| c.name.as_str()).collect();

    let ids = |names: &[&str]| -> Vec<Command> {
        if names.is_empty() {
            return vec![cmd(w, "docker", &["ps", "-q"], &cwd)];
        }
        let mut stop = vec!["stop"];
        stop.extend(names.iter().copied());
        vec![cmd(w, "docker", &stop, &cwd)]
    };
    let prov = "built-in · docker on PATH · IDs captured at review (docker ps -qa)";
    out.push(
        Item::new(
            "docker.stop_all",
            "Stop all containers",
            Kind::Docker,
            ResultType::Action,
            htag.clone(),
        )
        .summary(&format!(
            "stop the {} running containers on {} gracefully · nothing is removed",
            running.len(),
            w.host.name
        ))
        .exec(ids(&running))
        .effect(Effect::DockerStopAll)
        .effects(&[&format!(
            "{} containers stop · data and images stay",
            running.len()
        )])
        .keywords(&["docker", "stop", "all", "containers", "clean", "cleanup"])
        .group("Services")
        .risk(Risk::Mutating)
        .confirm(Confirmation::One)
        .freshness(fresh.clone())
        .provenance(prov)
        .launch(Launch::Activity { script: None })
        .reason(
            Signal::Context,
            &format!("{} running on {}", running.len(), w.host.name),
        ),
    );
    let mut remove_cmds = if all.is_empty() {
        vec![cmd(w, "docker", &["ps", "-qa"], &cwd)]
    } else {
        ids(&running)
    };
    if !all.is_empty() {
        let mut rm = vec!["rm"];
        rm.extend(all.iter().copied());
        remove_cmds.push(cmd(w, "docker", &rm, &cwd));
    }
    out.push(
        Item::new("docker.remove_all", "Stop and remove all containers", Kind::Docker, ResultType::Action, htag.clone())
            .summary(&format!("stop the running containers, then remove every captured container on {} (running and stopped) · images and volumes stay · rm runs only after stop succeeds", w.host.name))
            .exec(remove_cmds)
            .effect(Effect::DockerRemoveAll)
            .effects(&[&format!("{} containers removed · writable layers lost", all.len())])
            .keywords(&["docker", "remove", "rm", "all", "containers", "reset", "clean", "cleanup", "stop-all"])
            .group("Services")
            .risk(Risk::Destructive)
            .confirm(Confirmation::TwoGate { phrase: format!("REMOVE ALL CONTAINERS ON {}", w.host.name), broad: false })
            .freshness(fresh.clone())
            .provenance(prov)
            .launch(Launch::Activity { script: None })
            .alt("Stop only", "docker.stop_all")
            .reason(Signal::Default, &if all.is_empty() { "no containers · a successful no-op".to_owned() } else { format!("{} containers on {}", all.len(), w.host.name) }),
    );
    out.push(
        Item::new(
            "docker.images_prune",
            "Remove all unused images",
            Kind::Docker,
            ResultType::Action,
            htag.clone(),
        )
        .summary(&format!(
            "remove every image not used by a container · {} images · {:.1} GB",
            d.images, d.images_gb
        ))
        .exec(vec![cmd(
            w,
            "docker",
            &["image", "prune", "-a", "--force"],
            &cwd,
        )])
        .effect(Effect::DockerImagesPrune)
        .effects(&[&format!(
            "{:.1} GB freed · images re-pull on next use",
            d.images_gb
        )])
        .keywords(&[
            "docker", "images", "prune", "remove", "rmi", "clean", "cleanup",
        ])
        .group("Services")
        .risk(Risk::Destructive)
        .confirm(Confirmation::TwoGate {
            phrase: format!("REMOVE ALL IMAGES ON {}", w.host.name),
            broad: false,
        })
        .provenance(prov)
        .launch(Launch::Activity { script: None })
        .reason(Signal::Default, &format!("{} dangling", d.dangling_images)),
    );
    out.push(
        Item::new(
            "docker.network_prune",
            "Prune unused networks",
            Kind::Docker,
            ResultType::Action,
            htag.clone(),
        )
        .summary(&format!(
            "remove custom networks no container uses · {}",
            d.networks
        ))
        .exec(vec![cmd(
            w,
            "docker",
            &["network", "prune", "--force"],
            &cwd,
        )])
        .effect(Effect::DockerNetworkPrune)
        .keywords(&["docker", "network", "prune", "clean", "cleanup"])
        .group("Services")
        .risk(Risk::Mutating)
        .confirm(Confirmation::One)
        .provenance(prov)
        .launch(Launch::Activity { script: None })
        .reason(Signal::Default, "available on this host"),
    );
    out.push(
        Item::new("docker.volume_prune", "Prune unused volumes", Kind::Docker, ResultType::Action, htag.clone())
            .summary(&format!("remove every volume no container uses · {} named · {:.1} GB · named volumes hold durable data", d.named_volumes(), d.volumes_gb()))
            .exec(vec![cmd(w, "docker", &["volume", "prune", "-a", "--force"], &cwd)])
            .effect(Effect::DockerVolumePrune)
            .effects(&[&format!("{:.1} GB freed · permanent", d.volumes_gb())])
            .keywords(&["docker", "volumes", "prune", "data", "clean", "cleanup"])
            .group("Services")
            .risk(Risk::Destructive)
            .confirm(Confirmation::TwoGate { phrase: format!("PRUNE ALL VOLUMES ON {}", w.host.name), broad: false })
            .provenance(prov)
            .launch(Launch::Activity { script: None })
            .reason(Signal::Default, &format!("{} volumes", d.volumes.len())),
    );
    out.push(
        Item::new("docker.builder_prune", "Prune builder cache", Kind::Docker, ResultType::Action, htag.clone())
            .summary(&format!("docker builder prune -f · {:.1} GB · independent of the full cleanup · buildx caches are a separate expansion", d.builder_cache_gb))
            .exec(vec![cmd(w, "docker", &["builder", "prune", "-f"], &cwd)])
            .effect(Effect::DockerBuilderPrune)
            .keywords(&["docker", "builder", "buildx", "cache", "prune", "clean", "cleanup"])
            .group("Services")
            .risk(Risk::Destructive)
            .confirm(Confirmation::One)
            .provenance(prov)
            .launch(Launch::Activity { script: None })
            .alt("Prune buildx caches too", "docker.buildx_prune")
            .reason(Signal::Context, &format!("{:.1} GB of build cache", d.builder_cache_gb)),
    );
    out.push(
        Item::new(
            "docker.buildx_prune",
            "Prune buildx caches",
            Kind::Docker,
            ResultType::Action,
            htag.clone(),
        )
        .summary("docker buildx prune -a -f · builder instances beyond the default")
        .exec(vec![cmd(
            w,
            "docker",
            &["buildx", "prune", "-a", "-f"],
            &cwd,
        )])
        .effect(Effect::DockerBuilderPrune)
        .keywords(&["docker", "buildx", "prune"])
        .group("Services")
        .risk(Risk::Destructive)
        .confirm(Confirmation::One)
        .launch(Launch::Activity { script: None })
        .reason(Signal::Default, "expansion beyond the legacy builder prune"),
    );
    let mut all_cmds = ids(&running);
    if !all.is_empty() {
        let mut rm = vec!["rm"];
        rm.extend(all.iter().copied());
        all_cmds.push(cmd(w, "docker", &rm, &cwd));
    }
    all_cmds.push(cmd(w, "docker", &["image", "prune", "-a", "--force"], &cwd));
    all_cmds.push(cmd(w, "docker", &["network", "prune", "--force"], &cwd));
    all_cmds.push(cmd(w, "docker", &["system", "prune", "--force"], &cwd));
    all_cmds.push(cmd(
        w,
        "docker",
        &["volume", "prune", "-a", "--force"],
        &cwd,
    ));
    all_cmds.push(cmd(
        w,
        "docker",
        &["builder", "prune", "-a", "--force"],
        &cwd,
    ));
    all_cmds.push(cmd(w, "docker", &["system", "df"], &cwd));
    out.push(
        Item::new("docker.cleanup", "Clean Docker completely", Kind::Plan, ResultType::Flow, htag.clone())
            .summary(&format!("remove all user-removable Docker state on {}: containers, all images, networks, system data, volumes, build cache · as a reviewable plan", w.host.name))
            .exec(all_cmds)
            .effects(&[&format!("{} containers · {} images · {} volumes · ~{:.1} GB reclaimed · permanent", d.containers.len(), d.images, d.volumes.len(), d.reclaimable_gb)])
            .keywords(&["docker", "docker cleanup", "docker clean", "clean", "prune", "everything", "reset", "remove all", "complete"])
            .group("Services")
            .risk(Risk::Destructive)
            .confirm(Confirmation::TwoGate { phrase: format!("REMOVE ALL DOCKER DATA ON {}", w.host.name), broad: true })
            .freshness(fresh.clone())
            .provenance(prov)
            .launch(Launch::Plan { plan: "docker-cleanup".into() })
            .alt("Stop all containers only", "docker.stop_all")
            .alt("Prune builder cache only", "docker.builder_prune")
            .reason(Signal::Context, &format!("{:.1} GB reclaimable on {}", d.reclaimable_gb, w.host.name)),
    );
}

fn system_items(w: &World, out: &mut Vec<Item>) {
    let s = &w.system;
    let htag = host_tag(w);
    let fresh = freshness(w, "system");
    let cwd = w.location.cwd.clone();
    let mut res = Item::new("system.resources", "Show system resources", Kind::System, ResultType::Resource, htag.clone())
        .summary("CPU, load, memory, swap, pressure, disk and network snapshot; then hand off to btm for continuous monitoring")
        .exec(vec![Command::internal("holla snapshot system", &cwd, &w.host.name)])
        .keywords(&["system", "resources", "cpu", "memory", "load", "pressure", "what uses cpu", "what uses memory", "slow"])
        .group("System")
        .freshness(fresh.clone())
        .launch(Launch::Snapshot { snapshot: "system".into() })
        .alt("Open btm", "system.monitor")
        .alt("Show process tree", "system.processes");
    res = if s.pressure_minutes > 0 {
        res.reason(
            Signal::Urgency,
            &format!("CPU {}% for {} min", s.cpu_pct, s.pressure_minutes),
        )
    } else {
        res.reason(
            Signal::Context,
            &format!(
                "CPU {}% · memory {:.0}%",
                s.cpu_pct,
                s.mem_used_gb / s.mem_total_gb * 100.0
            ),
        )
    };
    out.push(res);
    if s.btm_installed && w.tools.contains("btm") {
        out.push(
            Item::new("system.monitor", "Open btm", Kind::System, ResultType::Handoff, htag.clone())
                .summary("persistent deep monitoring in the specialist tool; the activity keeps running while you return to holla")
                .exec(vec![cmd(w, "btm", &[], &cwd)])
                .keywords(&["btm", "bottom", "monitor", "top", "htop", "processes", "system monitor"])
                .group("System")
                .freshness(fresh.clone())
                .launch(Launch::Handoff { tool: "btm".into() })
                .reason(if s.pressure_minutes > 0 { Signal::Urgency } else { Signal::Default }, if s.pressure_minutes > 0 { "sustained pressure · continuous view fits" } else { "installed on this host" }),
        );
    }
    out.push(
        Item::new(
            "system.processes",
            "Show process tree",
            Kind::System,
            ResultType::Resource,
            htag.clone(),
        )
        .summary("processes by CPU and memory with their hierarchy")
        .exec(vec![cmd(
            w,
            "ps",
            &["-eo", "pid,ppid,pcpu,rss,stat,comm", "--forest"],
            &cwd,
        )])
        .keywords(&["process", "processes", "tree", "ps", "pid"])
        .group("System")
        .freshness(fresh.clone())
        .launch(Launch::Snapshot {
            snapshot: "processes".into(),
        })
        .reason(Signal::Default, &format!("{} noted processes", s.top.len())),
    );
    out.push(
        Item::new(
            "system.port",
            "Find the process on a port…",
            Kind::System,
            ResultType::Action,
            htag.clone(),
        )
        .summary("which process listens on a port, with its PID and parent")
        .exec(vec![cmd(
            w,
            "lsof",
            &["-nP", "-iTCP:<port>", "-sTCP:LISTEN"],
            &cwd,
        )])
        .keywords(&[
            "port",
            "listen",
            "eaddrinuse",
            "address already in use",
            "lsof",
            "process on port",
        ])
        .group("System")
        .args(vec![ArgSpec::text("port", "TCP port", "5173").required()])
        .launch(Launch::Snapshot {
            snapshot: "port".into(),
        })
        .reason(Signal::Default, "available on this host"),
    );
    out.push(
        Item::new("system.kill", "Stop a process…", Kind::System, ResultType::Action, htag.clone())
            .summary("send a signal to one process after reviewing its identity, hierarchy and likely effect")
            .exec(vec![cmd(w, "kill", &["-<signal>", "<pid>"], &cwd)])
            .keywords(&["kill", "stop process", "signal", "terminate"])
            .group("System")
            .risk(Risk::Destructive)
            .confirm(Confirmation::One)
            .args(vec![ArgSpec::text("pid", "process id", "").required(), ArgSpec::choice("signal", &["TERM", "INT", "KILL"], 0)])
            .launch(Launch::Activity { script: None })
            .reason(Signal::Default, "reviewed before sending"),
    );
    if w.apt.is_some() {
        let n = w.apt.as_ref().map(|a| a.upgradable.len()).unwrap_or(0);
        out.push(
            Item::new("system.upgrade", "Upgrade everything on this system", Kind::Plan, ResultType::Flow, htag.clone())
                .summary(&format!("Debian packages and global mise tools on {} as a dependency graph: preflight, parallel discovery, review, apply, optional cleanup, verification", w.host.name))
                .exec(vec![cmd(w, "sudo", &["apt-get", "update"], "/"), cmd(w, "apt", &["list", "--upgradable"], "/"), cmd(w, "sudo", &["apt-get", "upgrade", "-y"], "/"), cmd(w, "mise", &["outdated", "--json"], "/"), cmd(w, "mise", &["upgrade"], "/"), cmd(w, "sudo", &["apt-get", "autoremove", "--purge", "-y"], "/")])
                .effects(&[&format!("{n} Debian packages · 4 mise tools · reboot required")])
                .keywords(&["upgrade", "update", "everything", "apt", "packages", "system", "debian", "mise"])
                .group("System")
                .risk(Risk::Privileged)
                .confirm(Confirmation::One)
                .freshness(freshness(w, "apt"))
                .launch(Launch::Plan { plan: "upgrade".into() })
                .reason(Signal::Urgency, &format!("{n} upgradable · 2 security")),
        );
    }
}

pub fn omz_dir(w: &World) -> Option<String> {
    if let Some(z) = &w.upgrade.zsh_env
        && w.fs.is_dir(z)
    {
        return Some(z.clone());
    }
    let home = format!("{}/.oh-my-zsh", w.location.home);
    w.fs.is_dir(&home).then_some(home)
}

fn upgrade_items(w: &World, out: &mut Vec<Item>) {
    let htag = host_tag(w);
    let cwd = w.location.cwd.clone();
    let brew = w.tools.contains("brew") && w.brew.is_some();
    let mise = w.tools.contains("mise") && w.mise.installed;
    let amp = w.tools.contains("amp") && w.upgrade.amp;
    let omz = omz_dir(w);
    let managers: Vec<&str> = [
        brew.then_some("brew"),
        mise.then_some("mise"),
        amp.then_some("amp"),
        omz.as_ref().map(|_| "oh-my-zsh"),
    ]
    .into_iter()
    .flatten()
    .collect();
    if managers.is_empty() {
        return;
    }
    let prov = format!("built-in · detected: {}", managers.join(", "));
    let brew_stages = |cask: bool| -> Vec<Command> {
        let up: Vec<&str> = if cask {
            vec!["upgrade", "--cask", "--greedy", "--yes"]
        } else {
            vec!["upgrade", "--greedy", "--yes"]
        };
        vec![
            cmd(w, "brew", &["update"], &cwd),
            cmd(w, "brew", &up, &cwd),
            cmd(w, "brew", &["cleanup"], &cwd),
            cmd(w, "brew", &["autoremove"], &cwd),
            cmd(w, "brew", &["doctor"], &cwd),
        ]
    };
    if brew {
        let n = w.upgrade.brew_outdated.len();
        out.push(
            Item::new("upgrade.brew-packages", "Upgrade Homebrew packages", Kind::Upgrade, ResultType::Action, htag.clone())
                .summary(&format!("brew update, upgrade --greedy --yes, cleanup, autoremove, doctor as separate stages · a later stage still runs after an earlier failure unless cancelled · {n} outdated"))
                .batch(BatchMode::Sequential, brew_stages(false).into_iter().map(|c| { let effect = (c.args.first().map(String::as_str) == Some("upgrade")).then_some(Effect::BrewUpgrade(false)); (format!("brew {}", c.args.join(" ")), vec![c], effect, htag.clone()) }).collect())
                .keywords(&["brew", "homebrew", "upgrade", "packages", "update", "formulae"])
                .group("System")
                .risk(Risk::Mutating)
                .confirm(Confirmation::One)
                .provenance(&prov)
                .reason(if n > 0 { Signal::Context } else { Signal::Default }, &format!("{n} outdated formulae")),
        );
        if w.host.os == Os::MacOs {
            let n = w.upgrade.casks_outdated.len();
            out.push(
                Item::new("upgrade.brew-casks", "Upgrade Homebrew casks", Kind::Upgrade, ResultType::Action, htag.clone())
                    .summary(&format!("brew update, upgrade --cask --greedy --yes, cleanup, autoremove, doctor · macOS only · {n} outdated casks"))
                    .batch(BatchMode::Sequential, brew_stages(true).into_iter().map(|c| { let effect = (c.args.first().map(String::as_str) == Some("upgrade")).then_some(Effect::BrewUpgrade(true)); (format!("brew {}", c.args.join(" ")), vec![c], effect, htag.clone()) }).collect())
                    .keywords(&["brew", "cask", "casks", "upgrade", "applications"])
                    .group("System")
                    .risk(Risk::Mutating)
                    .confirm(Confirmation::One)
                    .provenance(&prov)
                    .reason(Signal::Default, &format!("{n} outdated casks")),
            );
        }
    }
    if mise {
        let n = w.mise.outdated().len();
        out.push(
            Item::new(
                "upgrade.mise",
                "Upgrade mise-managed tools",
                Kind::Upgrade,
                ResultType::Action,
                htag.clone(),
            )
            .summary(&format!(
                "mise upgrade · {n} outdated tools reach their latest versions"
            ))
            .exec(vec![cmd(w, "mise", &["upgrade"], &cwd)])
            .effect(Effect::MiseUpgrade)
            .keywords(&["mise", "upgrade", "tools", "versions"])
            .group("System")
            .risk(Risk::Mutating)
            .confirm(Confirmation::One)
            .provenance(&prov)
            .launch(Launch::Activity { script: None })
            .reason(
                if n > 0 {
                    Signal::Context
                } else {
                    Signal::Default
                },
                &format!("{n} outdated tools"),
            ),
        );
    }
    if amp {
        out.push(
            Item::new(
                "upgrade.amp",
                "Upgrade the Amp CLI",
                Kind::Upgrade,
                ResultType::Action,
                htag.clone(),
            )
            .summary("amp update")
            .exec(vec![cmd(w, "amp", &["update"], &cwd)])
            .effect(Effect::AmpUpdate)
            .keywords(&["amp", "update", "upgrade"])
            .group("System")
            .risk(Risk::Mutating)
            .provenance(&prov)
            .launch(Launch::Activity { script: None })
            .reason(Signal::Default, "amp on PATH"),
        );
    }
    if let Some(dir) = &omz {
        let script = format!("{dir}/tools/upgrade.sh");
        out.push(
            Item::new(
                "upgrade.oh-my-zsh",
                "Upgrade Oh My Zsh",
                Kind::Upgrade,
                ResultType::Action,
                htag.clone(),
            )
            .summary(&format!(
                "sh {script} · resolved from {}",
                if w.upgrade.zsh_env.as_deref() == Some(dir.as_str()) {
                    "$ZSH"
                } else {
                    "~/.oh-my-zsh"
                }
            ))
            .exec(vec![Command::from_vec(
                vec!["sh".into(), script.clone()],
                &cwd,
                &w.host.name,
            )])
            .effect(Effect::OmzUpgrade)
            .keywords(&["oh-my-zsh", "omz", "zsh", "upgrade", "shell"])
            .group("System")
            .risk(Risk::Mutating)
            .provenance(&prov)
            .launch(Launch::Activity { script: None })
            .reason(Signal::Default, "Oh My Zsh directory found"),
        );
    }
    out.push(
        Item::new("upgrade.all", "Upgrade everything", Kind::Plan, ResultType::Flow, htag)
            .summary(&format!("every detected manager as one plan with independent branches: {} · Homebrew stages depend on each other, the others run beside them · availability is re-probed before execution", managers.join(", ")))
            .exec(vec![])
            .keywords(&["upgrade", "everything", "all", "update", "managers"])
            .group("System")
            .risk(Risk::Mutating)
            .confirm(Confirmation::One)
            .provenance(&prov)
            .launch(Launch::Plan { plan: "upgrade-all".into() })
            .reason(Signal::Context, &format!("{} managers detected", managers.len())),
    );
}

pub fn parse_brew_services(text: &str) -> Result<Vec<String>, String> {
    use manifest::{Json, parse_json};
    let json = parse_json(text).map_err(|e| format!("brew services list --json: {e}"))?;
    let arr: &[Json] = match &json {
        Json::Array(a) => a.as_slice(),
        Json::Object(_) => match json.get("services") {
            Some(Json::Array(a)) => a.as_slice(),
            _ => return Err("brew services list --json: no services array".into()),
        },
        _ => return Err("brew services list --json: unexpected shape".into()),
    };
    let mut names: Vec<String> = arr
        .iter()
        .filter_map(|s| s.get("name").and_then(Json::as_str))
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_owned)
        .collect();
    names.sort();
    names.dedup();
    Ok(names)
}

pub fn brew_service_names(w: &World) -> (Result<Vec<String>, String>, String) {
    let Some(b) = &w.brew else {
        return (Err("brew is not installed".into()), "no brew".into());
    };
    let now = w.now_secs();
    if let Some(c) = &b.cache
        && c.version == 1
        && now.saturating_sub(c.fetched_at) <= 300
    {
        return (
            Ok(c.services.clone()),
            format!("cached · {} s old", now.saturating_sub(c.fetched_at)),
        );
    }
    let state = match &b.cache {
        Some(c) if c.version != 1 => format!("cache version {} ignored · refreshed", c.version),
        Some(_) => "cache stale · refreshed".into(),
        None if b.cache_text.is_some() => "cache corrupt · refreshed".into(),
        None => "no cache · probed".into(),
    };
    match &b.list_json {
        Ok(t) => (parse_brew_services(t), state),
        Err(e) => (Err(format!("brew services list failed: {e}")), state),
    }
}

fn brew_items(w: &World, out: &mut Vec<Item>) {
    if !w.tools.contains("brew") || w.brew.is_none() || !source_ready(w, "brew") {
        return;
    }
    let htag = host_tag(w);
    let cwd = w.location.cwd.clone();
    let (names, cache_state) = brew_service_names(w);
    let linux = w.brew.as_ref().is_some_and(|b| b.linux);
    let prov = format!(
        "built-in · brew services list --json · {cache_state}{}",
        if linux { " · Linuxbrew" } else { "" }
    );
    let names = match names {
        Ok(n) => n,
        Err(e) => {
            out.push(
                Item::new(
                    "brew.services",
                    "Homebrew services",
                    Kind::Brew,
                    ResultType::Resource,
                    htag,
                )
                .summary(&format!("{e} · no service actions were invented"))
                .keywords(&["brew", "services"])
                .group("Services")
                .freshness(Freshness::Unavailable(e.clone()))
                .provenance(&prov)
                .launch(Launch::Snapshot {
                    snapshot: "brew-services".into(),
                })
                .reason(Signal::Default, "discovery failed"),
            );
            return;
        }
    };
    if names.is_empty() {
        return;
    }
    let total_actions = names.len() * 3;
    let shown: Vec<&String> = names.iter().take(10).collect();
    let cap = if total_actions > 30 {
        Some(format!("Homebrew services (30 of {total_actions})"))
    } else {
        None
    };
    for name in shown {
        let status = w
            .brew
            .as_ref()
            .and_then(|b| b.services.iter().find(|s| &s.name == name))
            .map(|s| s.status.clone())
            .unwrap_or("unknown".into());
        for verb in ["start", "stop", "restart"] {
            let mut item = Item::new(&format!("brew.service.{name}.{verb}"), &format!("{} {name}", capitalize(verb)), Kind::Brew, ResultType::Action, htag.clone())
                .summary(&format!("brew services {verb} {name} · currently {status} · every verb is offered whatever the status says"))
                .exec(vec![cmd(w, "brew", &["services", verb, name], &cwd)])
                .effect(Effect::BrewService(name.clone(), verb.into()))
                .keywords(&["brew", "service", "services", name, verb, "homebrew"])
                .group("Services")
                .risk(Risk::Mutating)
                .provenance(&prov)
                .launch(Launch::Activity { script: None })
                .reason(Signal::Context, &status);
            if let Some(c) = &cap {
                item.cap_note = Some(c.clone());
            }
            out.push(item);
        }
    }
    out.push(
        Item::new(
            "brew.services",
            &cap.clone().unwrap_or("Homebrew services".into()),
            Kind::Brew,
            ResultType::Resource,
            htag,
        )
        .summary(&format!(
            "{} services · {cache_state} · status is re-observed after every verb",
            names.len()
        ))
        .exec(vec![cmd(w, "brew", &["services", "list", "--json"], &cwd)])
        .keywords(&["brew", "services", "homebrew", "list"])
        .group("Services")
        .provenance(&prov)
        .launch(Launch::Snapshot {
            snapshot: "brew-services".into(),
        })
        .reason(Signal::Default, &format!("{} services", names.len())),
    );
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn gradle_items(w: &World, out: &mut Vec<Item>) {
    let cwd = w.location.cwd.clone();
    let here = ScopeTag::here(&cwd);
    let installed = w.tools.contains("gradle");
    let build_file = ["build.gradle", "build.gradle.kts"]
        .into_iter()
        .find(|f| w.fs.exists(&format!("{cwd}/{f}")));
    if installed && let Some(f) = build_file {
        let prov = format!("built-in · gradle on PATH · {f} here");
        let g = &w.gradle;
        for (verb, label, summary, risk) in [
            (
                "clean",
                "gradle clean",
                "tool-native clean of build outputs · external deletion by Gradle, not Trash",
                Risk::Mutating,
            ),
            (
                "build",
                "gradle build",
                "compile, test and assemble",
                Risk::Mutating,
            ),
            ("test", "gradle test", "run the test suites", Risk::Mutating),
        ] {
            let mut item = Item::new(
                &format!("gradle.{verb}"),
                label,
                Kind::Gradle,
                ResultType::Action,
                here.clone(),
            )
            .summary(summary)
            .exec(vec![cmd(w, "gradle", &[verb], &cwd)])
            .keywords(&["gradle", verb, "build", "java", "kotlin"])
            .group("Tasks")
            .risk(risk)
            .provenance(&prov)
            .launch(Launch::Activity { script: None })
            .reason(Signal::Context, &format!("{f} here"));
            if verb == "clean" {
                item = item.effect(Effect::GradleClean(cwd.clone()));
            }
            if g.wrapper {
                item = item.alt(
                    "Use the wrapper (./gradlew) instead",
                    &format!("gradlew.{verb}"),
                );
            }
            out.push(item);
            if g.wrapper {
                out.push(
                    Item::new(
                        &format!("gradlew.{verb}"),
                        &format!("./gradlew {verb}"),
                        Kind::Gradle,
                        ResultType::Action,
                        here.clone(),
                    )
                    .summary(&format!("{summary} · through the project's wrapper"))
                    .exec(vec![cmd(w, "./gradlew", &[verb], &cwd)])
                    .keywords(&["gradlew", "wrapper", verb])
                    .group("Tasks")
                    .risk(risk)
                    .provenance("expansion · gradlew here")
                    .launch(Launch::Activity { script: None })
                    .reason(Signal::Default, "wrapper present"),
                );
            }
        }
    } else if !installed && build_file.is_some() && w.gradle.wrapper {
        out.push(
            Item::new(
                "gradlew.build",
                "./gradlew build",
                Kind::Gradle,
                ResultType::Action,
                here.clone(),
            )
            .summary("gradle is not on PATH · the project wrapper still builds")
            .exec(vec![cmd(w, "./gradlew", &["build"], &cwd)])
            .keywords(&["gradlew", "wrapper", "build"])
            .group("Tasks")
            .risk(Risk::Mutating)
            .provenance("expansion · gradlew here · legacy needs gradle on PATH")
            .launch(Launch::Activity { script: None })
            .reason(Signal::Default, "wrapper only"),
        );
    }
    if installed {
        let cands = cleanup::walk_candidates(&w.fs, &cwd, &|p, is_dir| {
            is_dir && (p.ends_with("/.gradle") || p.ends_with("/build"))
        });
        let bytes: u64 = cands.iter().map(|p| w.fs.size_of(p)).sum();
        let daemon = if w.gradle.daemon_running {
            "a daemon is running · gradle --stop is the first step"
        } else {
            "no daemon running"
        };
        out.push(
            Item::new("gradle.clean-all", &format!("Clean Gradle outputs under {} · {}", w.location.cwd_short(), termrock::ui::text::bytes(bytes)), Kind::Cleanup, ResultType::Flow, here)
                .summary(&format!("gradle --stop, then move every .gradle and build directory below this folder (depth 5, no symlinks, no node_modules) to the Trash · {} candidates · {daemon}", cands.len()))
                .exec(vec![cmd(w, "gradle", &["--stop"], &cwd)])
                .keywords(&["gradle", "clean", "cleanup", "build", ".gradle", "recursive", "trash"])
                .group("Disk")
                .risk(Risk::Destructive)
                .confirm(Confirmation::One)
                .provenance("built-in · gradle on PATH · shared candidate walker")
                .launch(Launch::Cleanup { category: Some("gradle.clean-all".into()) })
                .reason(if cands.is_empty() { Signal::Default } else { Signal::Context }, &if cands.is_empty() { "no candidates · a successful no-op".to_owned() } else { format!("{} candidates", cands.len()) }),
        );
    }
}

fn idea_items(w: &World, out: &mut Vec<Item>) {
    let cwd = w.location.cwd.clone();
    let here = ScopeTag::here(&cwd);
    let applicable = w.fs.is_dir(&format!("{cwd}/.idea")) || w.tools.contains("idea");
    if !applicable {
        return;
    }
    let cands = cleanup::walk_candidates(&w.fs, &cwd, &|p, is_dir| {
        (is_dir && p.ends_with("/.idea")) || (!is_dir && p.ends_with(".iml"))
    });
    let bytes: u64 = cands.iter().map(|p| w.fs.size_of(p)).sum();
    out.push(
        Item::new("idea.clean", &format!("Clean IntelliJ metadata under {} · {}", w.location.cwd_short(), termrock::ui::text::bytes(bytes)), Kind::Idea, ResultType::Flow, here)
            .summary(&format!("move every .idea directory and lowercase .iml file below this folder (depth 5, no symlinks, no node_modules, nothing nested) to the Trash · {} candidates · no IDE shutdown step exists in the baseline", cands.len()))
            .exec(vec![])
            .keywords(&["idea", "intellij", "jetbrains", ".idea", ".iml", "clean", "cleanup", "metadata"])
            .group("Disk")
            .risk(Risk::Destructive)
            .confirm(Confirmation::One)
            .provenance(&format!("built-in · {}", if w.fs.is_dir(&format!("{cwd}/.idea")) { ".idea here" } else { "idea on PATH" }))
            .launch(Launch::Cleanup { category: Some("idea.clean".into()) })
            .reason(if cands.is_empty() { Signal::Default } else { Signal::Context }, &if cands.is_empty() { "no candidates · a successful no-op".to_owned() } else { format!("{} candidates", cands.len()) }),
    );
}

fn disk_items(w: &World, out: &mut Vec<Item>) {
    let d = &w.disk;
    let here = ScopeTag::here(&w.location.cwd);
    let htag = host_tag(w);
    let fresh = freshness(w, "filesystem");
    let mut usage = Item::new("disk.usage", "Analyze disk usage", Kind::Disk, ResultType::Flow, here.clone())
        .summary("largest-first tree of this folder that deepens as the scan streams; select any entry or rebuildable artifacts for cleanup")
        .exec(vec![Command::internal("holla disk analyze", &w.location.cwd, &w.host.name)])
        .keywords(&["disk", "usage", "why disk full", "space", "largest", "du", "analyze", "full", "scan here"])
        .aliases(&["du"])
        .page_aliases(&["u"])
        .group("Disk")
        .freshness(fresh.clone())
        .launch(Launch::Disk { path: w.location.cwd.clone() })
        .alt("Disk overview (home folders)", "disk.overview")
        .alt("Analyze a custom path…", "disk.scan-custom")
        .alt("Top files on this Mac", "disk.top-files");
    if let Some(fs) = d.root_fs()
        && fs.pct() >= 90
    {
        usage = usage.reason(
            Signal::Urgency,
            &format!("{} is {}% full", fs.mount, fs.pct()),
        );
    } else if let Some(fs) = d.root_fs() {
        usage = usage.reason(
            Signal::Default,
            &format!("{} is {}% full", fs.mount, fs.pct()),
        );
    }
    out.push(usage);
    out.push(
        Item::new("disk.overview", "Disk overview", Kind::Disk, ResultType::Flow, htag.clone())
            .summary("immediate home folders alphabetically, then detected insight roots · cached sizes with their age, unscanned otherwise · choosing a row starts a live analysis")
            .exec(vec![Command::internal("holla disk overview", &w.location.home, &w.host.name)])
            .keywords(&["disk", "home", "overview", "usage"])
            .page_aliases(&["h"])
            .group("Disk")
            .launch(Launch::Disk { path: String::new() })
            .reason(Signal::Default, "available"),
    );
    out.push(
        Item::new(
            "disk.scan-custom",
            "Analyze a custom path…",
            Kind::Disk,
            ResultType::Action,
            htag.clone(),
        )
        .summary("an absolute existing file or directory · validated inline before the scan starts")
        .exec(vec![Command::internal(
            "holla disk analyze <path>",
            &w.location.cwd,
            &w.host.name,
        )])
        .keywords(&["disk", "path", "custom", "analyze", "folder"])
        .group("Disk")
        .args(vec![
            ArgSpec::text("path", "absolute path", &w.location.cwd)
                .required()
                .help("must exist · relative paths are rejected"),
        ])
        .launch(Launch::Disk {
            path: "<path>".into(),
        })
        .reason(Signal::Default, "available"),
    );
    let mut top = Item::new("disk.top-files", "Top files on this Mac", Kind::Disk, ResultType::Flow, htag.clone())
        .summary("Spotlight query for regular files of 100 MiB or more · top 50 by allocated size · a scope of its own, no tree scan needed")
        .exec(vec![cmd(w, "mdfind", &["-0", "kMDItemFSSize >= 104857600"], &w.location.home)])
        .keywords(&["top", "files", "largest", "spotlight", "big files", "mdfind"])
        .group("Disk")
        .page_aliases(&["t"])
        .launch(Launch::TopFiles)
        .reason(Signal::Default, "macOS Spotlight");
    if w.host.os != Os::MacOs {
        top = top.freshness(Freshness::Unavailable(
            "Spotlight is unavailable on Linux · use the tree scan".into(),
        ));
    }
    out.push(top);
    let local: Vec<&crate::domain::stack::Candidate> = d
        .candidates
        .iter()
        .filter(|c| c.path.starts_with(&w.location.cwd))
        .collect();
    if !local.is_empty() {
        let total: f32 = local.iter().map(|c| c.gb).sum();
        let stale = local.iter().filter(|c| c.default_selected()).count();
        out.push(
            Item::new(
                "disk.artifacts",
                &format!("Inspect {:.0} GB of generated artifacts", total),
                Kind::Disk,
                ResultType::Recommendation,
                here.clone(),
            )
            .summary(&format!(
                "{} below this folder · {} stale enough to start selected",
                termrock::ui::text::plural(
                    local.len(),
                    "rebuildable candidate",
                    "rebuildable candidates"
                ),
                stale
            ))
            .exec(vec![Command::internal(
                "holla disk analyze --artifacts",
                &w.location.cwd,
                &w.host.name,
            )])
            .keywords(&[
                "artifacts",
                "target",
                "node_modules",
                "build",
                "generated",
                "rebuildable",
                "clean",
            ])
            .group("Disk")
            .freshness(fresh.clone())
            .launch(Launch::Disk {
                path: w.location.cwd.clone(),
            })
            .reason(
                Signal::Urgency,
                &format!("{:.0} GB generated artifacts", total),
            ),
        );
        if local.len() > 1 {
            out.push(
                Item::new("disk.clean_children", &format!("Clean developer artifacts under {}", w.location.cwd_short()), Kind::Plan, ResultType::Flow, ScopeTag::child(&w.location.cwd))
                    .summary("Cargo targets, Gradle outputs and Node dependency trees beneath their owning projects as a plan · shared targets serialised, independent projects in parallel")
                    .exec(vec![cmd(w, "cargo", &["clean"], &w.location.cwd), cmd(w, "./gradlew", &["clean"], &w.location.cwd), cmd(w, "trash", &["<node_modules>"], &w.location.cwd), cmd(w, "pnpm", &["store", "prune"], &w.location.cwd)])
                    .effects(&[&format!("~{:.1} GB reclaimable · recent and unverifiable data stays unselected", d.reclaimable_gb())])
                    .keywords(&["clean", "artifacts", "children", "rust", "gradle", "node", "cleanup", "build"])
                    .group("Disk")
                    .risk(Risk::Destructive)
                    .confirm(Confirmation::TwoGate { phrase: format!("REMOVE ALL BUILD ARTIFACTS UNDER {}", w.location.cwd), broad: false })
                    .launch(Launch::Plan { plan: "cleanup-work".into() })
                    .reason(Signal::Context, &format!("{} candidates in {} projects", local.len(), w.location.children.len())),
            );
        }
    }
    if !d.history.is_empty() || !w.ops_log.lines.is_empty() || !w.reports.is_empty() {
        out.push(
            Item::new("disk.history", "Show cleanup history", Kind::Disk, ResultType::Resource, htag.clone())
                .summary("every requested item with its outcome: removed, trashed, would-remove, failed, skipped · the operation log at ~/.cache/holla/ops.log")
                .exec(vec![Command::internal("holla disk history", &w.location.cwd, &w.host.name)])
                .keywords(&["history", "cleanup", "audit", "reclaimed", "log", "ops.log"])
                .page_aliases(&["y"])
                .group("Disk")
                .launch(Launch::Snapshot { snapshot: "disk-history".into() })
                .reason(Signal::Default, &format!("{} log records", w.ops_log.lines.len() + d.history.len())),
        );
    }

    let items = d
        .candidates
        .iter()
        .find(|c| c.path == w.location.cwd)
        .map(|c| (c.items, c.gb));
    out.push(
        Item::new("disk.delete_all", "Delete everything inside this folder", Kind::Cleanup, ResultType::Action, here)
            .summary(&format!("remove every file and folder below {} including hidden and nested content · the folder itself stays", w.location.cwd_short()))
            .exec(vec![cmd(w, "find", &[&w.location.cwd, "-mindepth", "1", "-delete"], &w.location.cwd)])
            .effect(Effect::DeleteAll(w.location.cwd.clone()))
            .effects(&[&match items {
                Some((n, gb)) => format!("{n} items · {gb:.1} GB · permanent"),
                None => format!("{} entries below the folder · permanent", w.fs.subtree(&w.location.cwd).len()),
            }])
            .keywords(&["delete", "everything", "empty", "folder", "rm -rf", "wipe", "clear"])
            .group("Disk")
            .risk(Risk::Destructive)
            .confirm(Confirmation::TwoGate { phrase: format!("DELETE EVERYTHING IN {}", w.location.cwd), broad: false })
            .launch(Launch::Activity { script: None })
            .reason(Signal::Default, "always available · two gates"),
    );
}

pub fn observe_process(w: &World, name: &str) -> ProcessObservation {
    if let Err(e) = &w.platform.process_probe {
        return ProcessObservation::Unknown(e.clone());
    }
    if w.system
        .top
        .iter()
        .any(|p| p.name == name || p.name.starts_with(&format!("{name} ")))
    {
        ProcessObservation::Running
    } else {
        ProcessObservation::NotRunning
    }
}

pub fn insight_candidates(w: &World) -> Vec<InsightCategory> {
    let home = w.location.home.clone();
    let mut out = vec![];
    for cat in &cleanup::CATEGORIES {
        if !cleanup::detected(cat, w.host.os, &home, &w.fs, &w.tools) {
            continue;
        }
        let mut roots: Vec<String> = cat.roots.iter().map(|r| format!("{home}/{r}")).collect();
        if cat.id == "pnpm.store" {
            let resolved = w
                .outputs
                .pnpm_store_path
                .clone()
                .unwrap_or(Err("pnpm store path was not run".into()));
            let (root, _) =
                cleanup::pnpm_store_root(&home, resolved.as_deref().map_err(String::as_str));
            roots = vec![root];
        }
        if cat.id == "uv.cache" {
            roots = vec![format!("{}/uv", w.platform.xdg_cache_home)];
        }
        let mut paths: Vec<String> = vec![];
        let mut unreadable: Vec<String> = vec![];
        if cat.id == "project.artifacts" {
            let mut scan_roots = vec![format!("{home}/Projects")];
            if let Some(parent) = crate::sim::fs::Fs::parent(&w.location.cwd) {
                scan_roots.push(parent);
            }
            let existing: Vec<String> = scan_roots.into_iter().filter(|r| w.fs.is_dir(r)).collect();
            (paths, unreadable) = cleanup::find_artifacts_report(&w.fs, &existing);
        } else {
            for r in &roots {
                if cat.children_of_root {
                    if let Ok(children) = w.fs.list(r) {
                        paths.extend(
                            children
                                .iter()
                                .filter(|c| c.is_dir())
                                .map(|c| c.path.clone()),
                        );
                    }
                } else if w.fs.exists(r) {
                    paths.push(r.clone());
                }
            }
        }
        let process = match cat.guard_process {
            Some(name) => observe_process(w, name),
            None => ProcessObservation::NotRunning,
        };
        let mut cands = vec![];
        for p in paths {
            let node = w.fs.get(&p);
            let age = node.and_then(|n| cleanup::age_days(n.mtime, w.now_secs()));
            let scan = w.fs.scan(
                &p,
                &crate::sim::fs::ScanOptions {
                    include_hidden: true,
                    ..Default::default()
                },
            );
            cands.push(InsightCandidate {
                path: p.clone(),
                bytes: scan.allocated,
                partial: scan.errors() > 0,
                age_days: age,
                eligibility: cleanup::eligibility(cat, age, &process),
            });
        }
        cands.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.path.cmp(&b.path)));
        out.push(InsightCategory {
            category: cat,
            candidates: cands,
            process,
            unreadable,
        });
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InsightCandidate {
    pub path: String,
    pub bytes: u64,

    pub partial: bool,
    pub age_days: Option<i64>,
    pub eligibility: Eligibility,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InsightCategory {
    pub category: &'static cleanup::Category,
    pub candidates: Vec<InsightCandidate>,
    pub process: ProcessObservation,

    pub unreadable: Vec<String>,
}

impl InsightCategory {
    pub fn bytes(&self) -> u64 {
        self.candidates.iter().map(|c| c.bytes).sum()
    }
}

fn cleanup_items(w: &World, out: &mut Vec<Item>) {
    let htag = host_tag(w);
    let cats = insight_candidates(w);
    let total: u64 = cats.iter().map(InsightCategory::bytes).sum();
    out.push(
        Item::new("cleanup.review-all", "Review cleanup candidates", Kind::Cleanup, ResultType::Flow, htag.clone())
            .summary(&format!("{} detected categories · {} · rebuilt-on-demand and safe-if-old candidates start selected, review-first never does, too-recent ones stay visible but ineligible", cats.len(), termrock::ui::text::bytes(total)))
            .exec(vec![Command::internal("holla cleanup review", &w.location.home, &w.host.name)])
            .keywords(&["cleanup", "clean", "caches", "insights", "review", "space", "free"])
            .group("Disk")
            .page_aliases(&["c"])
            .launch(Launch::Cleanup { category: None })
            .reason(if total > 5 * 1024 * 1024 * 1024 { Signal::Urgency } else { Signal::Default }, &format!("{} across {} categories", termrock::ui::text::bytes(total), cats.len())),
    );
    for c in &cats {
        let cat = c.category;
        let tool_kw = cat.id.split('.').next().unwrap_or("");
        out.push(
            Item::new(
                &format!("cleanup.{}", cat.id),
                &format!(
                    "Clean {} · {}",
                    cat.label,
                    termrock::ui::text::bytes(c.bytes())
                ),
                Kind::Cleanup,
                ResultType::Action,
                htag.clone(),
            )
            .summary(&format!(
                "{} · {} · {} · {}",
                cat.note,
                cat.safety.label(),
                if cat.min_age_days > 0 {
                    format!("older than {} d", cat.min_age_days)
                } else {
                    "any age".into()
                },
                termrock::ui::text::plural(c.candidates.len(), "candidate", "candidates")
            ))
            .exec(vec![Command::internal(
                &format!("holla cleanup review --category {}", cat.id),
                &w.location.home,
                &w.host.name,
            )])
            .keywords(&["cleanup", "clean", cat.label, tool_kw, cat.id])
            .group("Disk")
            .launch(Launch::Cleanup {
                category: Some(cat.id.into()),
            })
            .reason(
                Signal::Default,
                &format!(
                    "{} · {}",
                    cat.safety.label(),
                    if cat.macos_only {
                        "macOS"
                    } else {
                        "all platforms"
                    }
                ),
            ),
        );
    }
}

fn pg_items(w: &World, out: &mut Vec<Item>) {
    let Some(p) = &w.pg else {
        return;
    };
    if !source_ready(w, "postgres") {
        return;
    }
    let tag = w
        .location
        .project
        .as_ref()
        .map(|pr| tag_for_dir(w, &pr.root))
        .unwrap_or_else(|| host_tag(w));
    let fresh = freshness(w, "postgres");
    let blocked = p.blocked();
    let cwd = w.location.cwd.clone();
    out.push(
        Item::new(
            "pg.activity",
            "Show PostgreSQL activity",
            Kind::Postgres,
            ResultType::Resource,
            tag.clone(),
        )
        .summary(&format!(
            "{} · connections {}/{} · sessions by state, wait event, query and transaction age",
            p.label, p.connections, p.max_connections
        ))
        .exec(vec![cmd(
            w,
            "psql",
            &[
                "-c",
                "SELECT pid, state, wait_event, now() - xact_start FROM pg_stat_activity",
            ],
            &cwd,
        )])
        .keywords(&[
            "postgres",
            "postgresql",
            "database",
            "db",
            "activity",
            "connections",
            "sessions",
            "queries",
        ])
        .group("Services")
        .freshness(fresh.clone())
        .launch(Launch::Snapshot {
            snapshot: "pg".into(),
        })
        .alt("Open pg_activity", "pg.handoff")
        .reason(
            Signal::Context,
            &format!("{}/{} connections", p.connections, p.max_connections),
        ),
    );
    if !blocked.is_empty() {
        let age = blocked.iter().map(|s| s.query_secs).max().unwrap_or(0) / 60;
        out.push(
            Item::new("pg.blocking", "Who is blocking the database?", Kind::Postgres, ResultType::Recommendation, tag.clone())
                .summary("blocker dependency tree with query and transaction ages, wait state, database, user, client and affected sessions")
                .exec(vec![cmd(w, "psql", &["-c", "SELECT pid, pg_blocking_pids(pid), state, wait_event_type, query FROM pg_stat_activity WHERE cardinality(pg_blocking_pids(pid)) > 0"], &cwd)])
                .keywords(&["blocking", "blocked", "lock", "who is blocking", "postgres", "waiting", "stuck"])
                .group("Services")
                .freshness(fresh.clone())
                .launch(Launch::Snapshot { snapshot: "pg-blocking".into() })
                .alt("Cancel the blocking query", "pg.cancel")
                .alt("Open pg_activity", "pg.handoff")
                .reason(Signal::Urgency, &format!("{} sessions blocked for {age} min", blocked.len())),
        );
        if let Some(b) = p.blockers().first() {
            let cancel_sql = format!("SELECT pg_cancel_backend({})", b.pid);
            let term_sql = format!("SELECT pg_terminate_backend({})", b.pid);
            out.push(
                Item::new("pg.cancel", &format!("Cancel the blocking query (PID {})", b.pid), Kind::Postgres, ResultType::Action, tag.clone())
                    .summary(&format!("pg_cancel_backend({}) · {} · {} · idle in transaction for {} min · revalidated immediately before running", b.pid, b.user, b.app, b.txn_secs / 60))
                    .exec(vec![cmd(w, "psql", &["-c", &cancel_sql], &cwd)])
                    .effect(Effect::PgCancel(b.pid))
                    .effects(&["the current query is cancelled · the session stays connected"])
                    .keywords(&["cancel", "query", "postgres", "blocking", "pid"])
                    .group("Services")
                    .risk(Risk::Destructive)
                    .confirm(Confirmation::One)
                    .launch(Launch::Activity { script: None })
                    .alt("Terminate the backend instead", "pg.terminate")
                    .reason(Signal::Context, "offered before termination"),
            );
            out.push(
                Item::new(
                    "pg.terminate",
                    &format!("Terminate backend {}", b.pid),
                    Kind::Postgres,
                    ResultType::Action,
                    tag.clone(),
                )
                .summary(&format!(
                    "pg_terminate_backend({}) · disconnects {}@{} · use after cancel fails",
                    b.pid, b.user, b.app
                ))
                .exec(vec![cmd(w, "psql", &["-c", &term_sql], &cwd)])
                .effect(Effect::PgTerminate(b.pid))
                .effects(&["the session is disconnected · its transaction rolls back"])
                .keywords(&["terminate", "kill", "backend", "postgres", "session"])
                .group("Services")
                .risk(Risk::Destructive)
                .confirm(Confirmation::TwoGate {
                    phrase: format!("TERMINATE BACKEND {} ON {}", b.pid, w.host.name),
                    broad: false,
                })
                .launch(Launch::Activity { script: None })
                .reason(Signal::Default, "cancel comes first"),
            );
        }
    }
    if p.pg_activity_installed && w.tools.contains("pg_activity") {
        out.push(
            Item::new("pg.handoff", "Open pg_activity", Kind::Postgres, ResultType::Handoff, tag)
                .summary(&format!("live PostgreSQL activity in the specialist tool · {} · password never on the command line", p.label))
                .exec(vec![cmd(w, "pg_activity", &["-h", &p.host, "-p", &p.port.to_string(), "-U", &p.user, "-d", &p.db], &cwd)])
                .keywords(&["pg_activity", "postgres", "monitor", "live", "top"])
                .group("Services")
                .freshness(fresh)
                .launch(Launch::Handoff { tool: "pg_activity".into() })
                .reason(if blocked.is_empty() { Signal::Default } else { Signal::Context }, if blocked.is_empty() { "installed on this host" } else { "live view of the blocking tree" }),
        );
    }
}

fn ssh_items(w: &World, out: &mut Vec<Item>) {
    if !w.tools.contains("ssh") {
        return;
    }
    let cwd = w.location.cwd.clone();
    for a in &w.ssh.aliases {
        if a.alias == w.host.name {
            continue;
        }
        let dest = format!("{}@{}:{}", a.user, a.host, a.port);
        let mut item = Item::new(
            &format!("ssh.{}", a.alias),
            &format!("Connect to {}", a.alias),
            Kind::Ssh,
            ResultType::Handoff,
            ScopeTag::host(&w.host.name),
        )
        .summary(&format!(
            "ssh {} · resolves to {dest}{} · {}",
            a.alias,
            a.jump
                .as_ref()
                .map(|j| format!(" via {j}"))
                .unwrap_or_default(),
            a.role
        ))
        .exec(vec![cmd(w, "ssh", &[&a.alias], &cwd)])
        .effects(&[&format!("interactive session on {}", a.host)])
        .keywords(&["ssh", "connect", "remote", &a.alias, &a.host, &a.role])
        .group("System")
        .freshness(freshness(w, "ssh"))
        .provenance(&format!("ssh config · {}", w.location.short(&a.from_file)))
        .launch(Launch::Handoff {
            tool: format!("ssh {}", a.alias),
        })
        .alt(
            "Show the resolved configuration",
            &format!("ssh.{}.resolve", a.alias),
        )
        .reason(
            Signal::Default,
            &format!("from {}", w.location.short(&a.from_file)),
        );
        if a.role == "production" {
            item = item.risk(Risk::Privileged).confirm(Confirmation::One);
        }
        out.push(item);
    }
}

fn service_items(w: &World, out: &mut Vec<Item>) {
    if w.host.role != crate::domain::context::HostRole::Production || !w.tools.contains("systemctl")
    {
        return;
    }
    let tag = ScopeTag::remote(&w.host.name);
    let fresh = freshness(w, "systemd");
    let cwd = w.location.cwd.clone();
    let services = [
        ("payments", "active · 2 h", false),
        (
            "payments-worker",
            "activating (auto-restart) · 3 restarts in 10 min",
            true,
        ),
        ("nginx", "active · 41 d", false),
        ("postgresql", "active · 41 d", false),
    ];
    for (name, state, degraded) in services {
        let healed = w.healed_units.iter().any(|u| u == name);
        let degraded = degraded && !healed;
        let state = if healed {
            "active · restarted just now"
        } else {
            state
        };
        let mut st = Item::new(
            &format!("service.status.{name}"),
            &format!("Show {name} status"),
            Kind::System,
            ResultType::Resource,
            tag.clone(),
        )
        .summary(&format!("systemd unit {name}.service · {state}"))
        .exec(vec![cmd(w, "systemctl", &["status", name], &cwd)])
        .keywords(&["service", "status", "systemd", name, "health"])
        .group("Services")
        .freshness(fresh.clone())
        .launch(Launch::Snapshot {
            snapshot: format!("service-{name}"),
        })
        .alt("Follow the journal", &format!("service.journal.{name}"))
        .alt("Restart", &format!("service.restart.{name}"));
        st = if degraded {
            st.reason(Signal::Urgency, "3 restarts in 10 min")
        } else {
            st.reason(Signal::Context, "active")
        };
        out.push(st);
        out.push(
            Item::new(
                &format!("service.journal.{name}"),
                &format!("Follow {name} journal"),
                Kind::System,
                ResultType::Action,
                tag.clone(),
            )
            .summary(&format!("journalctl -u {name} -f"))
            .exec(vec![cmd(
                w,
                "journalctl",
                &["-u", name, "-f", "-n", "200"],
                &cwd,
            )])
            .keywords(&["logs", "journal", "service logs", name, "follow"])
            .group("Services")
            .freshness(fresh.clone())
            .launch(Launch::Activity {
                script: if name == "payments-worker" {
                    Some("journal:payments-worker".into())
                } else {
                    None
                },
            })
            .reason(
                if degraded {
                    Signal::Urgency
                } else {
                    Signal::Default
                },
                if degraded {
                    "unit is restarting"
                } else {
                    "available"
                },
            ),
        );
        out.push(
            Item::new(
                &format!("service.restart.{name}"),
                &format!("Restart {name}"),
                Kind::System,
                ResultType::Action,
                tag.clone(),
            )
            .summary(&format!(
                "systemctl restart {name} on {} · production · in-flight requests are dropped{}",
                w.host.name,
                if w.sudo_cached {
                    ""
                } else {
                    " · sudo asks for a password"
                }
            ))
            .exec(vec![
                cmd(w, "sudo", &["systemctl", "restart", name], &cwd),
                cmd(w, "systemctl", &["status", name], &cwd),
            ])
            .effect(Effect::ServiceRestart(name.into()))
            .effects(&[&format!(
                "{name} restarts · brief outage on {}",
                w.host.name
            )])
            .keywords(&["restart", "service", name, "bounce"])
            .group("Services")
            .risk(Risk::Privileged)
            .confirm(Confirmation::TwoGate {
                phrase: format!("RESTART {} ON {}", name.to_uppercase(), w.host.name),
                broad: false,
            })
            .launch(Launch::Activity { script: None })
            .reason(Signal::Default, "production host · two gates"),
        );
    }
}

fn custom_items(w: &World, out: &mut Vec<Item>) {
    let cwd = w.location.cwd.clone();
    for cfg in [&w.custom_global, &w.custom_project].into_iter().flatten() {
        let origin = cfg.origin;
        let dir = crate::sim::fs::Fs::parent(&cfg.path).unwrap_or(cwd.clone());
        for a in &cfg.actions {
            let run_dir = match origin {
                crate::domain::custom::Origin::Global => cwd.clone(),
                crate::domain::custom::Origin::Project => dir.clone(),
            };
            let argv: Vec<&str> = a.argv.iter().skip(1).map(String::as_str).collect();
            let mut command = cmd(w, &a.argv[0], &argv, &run_dir);
            if a.argv[0] == "sh" && a.argv.get(1).map(String::as_str) == Some("-c") {
                command.kind = crate::domain::exec::ExecKind::Shell;
            }
            let status = w.trust.status(&cfg.digest, &cfg.path, &run_dir);
            let trusted =
                origin == crate::domain::custom::Origin::Global || status == TrustStatus::Trusted;
            let risk = match a.danger {
                Danger::Safe => Risk::ReadOnly,
                Danger::Mutating => Risk::Mutating,
                Danger::Destructive => Risk::Destructive,
            };
            let kws: Vec<&str> = a
                .keywords
                .iter()
                .map(String::as_str)
                .chain(["custom", a.group.as_str()])
                .collect();
            let mut item = Item::new(
                &a.id,
                &a.label,
                Kind::Personal,
                ResultType::Action,
                tag_for_dir(w, &run_dir),
            )
            .summary(&if a.description.is_empty() {
                format!(
                    "{} action from {}",
                    origin.label(),
                    w.location.short(&cfg.path)
                )
            } else {
                a.description.clone()
            })
            .exec(vec![command])
            .keywords(&kws)
            .group("Tasks")
            .risk(risk)
            .provenance(&format!(
                "{} config · {} action[{}] · sha256 {}…",
                origin.label(),
                w.location.short(&cfg.path),
                a.index,
                termrock::ui::text::clip(&cfg.digest, 12)
            ))
            .launch(Launch::Activity {
                script: Some(a.argv.join(" ")),
            })
            .alt("Set an alias…", &format!("{}.alias", a.id))
            .alt("Show configuration diagnostics", "custom.config")
            .reason(
                Signal::Context,
                &format!(
                    "{} · {}",
                    a.group,
                    if trusted {
                        "trusted"
                    } else {
                        match status {
                            TrustStatus::Moved => {
                                "⚠ unreviewed · same bytes were trusted at another path"
                            }
                            TrustStatus::LegacyDigestOnly => {
                                "⚠ unreviewed · legacy digest-only record"
                            }
                            _ => "⚠ unreviewed",
                        }
                    }
                ),
            );
            if a.danger == Danger::Destructive || a.confirm {
                item = item.confirm(Confirmation::One);
            }
            if !trusted {
                item = item.confirm(Confirmation::Trust {
                    config: w.location.short(&cfg.path),
                });
                item.trust_key = Some((cfg.path.clone(), cfg.digest.clone()));
            }
            out.push(item);
        }
    }
    let any = w.custom_global.is_some() || w.custom_project.is_some();
    if any {
        let diags: usize = [&w.custom_global, &w.custom_project]
            .into_iter()
            .flatten()
            .map(|c| c.diagnostics.len())
            .sum();
        let actions: usize = [&w.custom_global, &w.custom_project]
            .into_iter()
            .flatten()
            .map(|c| c.actions.len())
            .sum();
        out.push(
            Item::new(
                "custom.config",
                "Custom action configuration",
                Kind::Personal,
                ResultType::Resource,
                ScopeTag::here(&cwd),
            )
            .summary(&format!(
                "{} valid actions · {} diagnostics · {}{}",
                actions,
                diags,
                w.custom_global
                    .as_ref()
                    .map(|c| w.location.short(&c.path))
                    .unwrap_or("no global config".into()),
                w.custom_project
                    .as_ref()
                    .map(|c| format!(" · {}", w.location.short(&c.path)))
                    .unwrap_or_default()
            ))
            .exec(vec![])
            .keywords(&[
                "custom",
                "config",
                "actions.toml",
                ".holla.toml",
                "diagnostics",
                "trust",
            ])
            .group("Tasks")
            .freshness(if diags > 0 {
                Freshness::Partial {
                    loaded: actions,
                    total: actions + diags,
                }
            } else {
                Freshness::Live { age_ms: 0 }
            })
            .launch(Launch::Config {
                path: w
                    .custom_project
                    .as_ref()
                    .or(w.custom_global.as_ref())
                    .map(|c| c.path.clone())
                    .unwrap_or_default(),
            })
            .reason(
                Signal::Default,
                if diags > 0 {
                    "some entries were skipped"
                } else {
                    "configuration ok"
                },
            ),
        );
    }
}

fn file_items(w: &World, out: &mut Vec<Item>) {
    let here = ScopeTag::here(&w.location.cwd);
    let cwd = w.location.cwd.clone();

    out.push(
        Item::new("find.files", "Find files under home…", Kind::Files, ResultType::Flow, ScopeTag::host(&w.host.name))
            .summary(&format!("search {} · exact name and stem first, then name substring, then fuzzy · at most 100 results · hidden, ignored and iCloud entries excluded", w.location.short(&w.location.home)))
            .exec(vec![Command::internal("holla find", &w.location.home, &w.host.name)])
            .keywords(&["find", "files", "search", "locate", "file", "home"])
            .group("Files")
            .page_aliases(&["/"])
            .launch(Launch::Find)
            .reason(Signal::Default, "always available"),
    );
    out.push(
        Item::new("browse.files", &format!("Browse {}", w.location.cwd_short()), Kind::Files, ResultType::Flow, here.clone())
            .summary("real directory listing with kind, size, modified time and hidden entries · Enter previews a file safely · exact-path jump with g")
            .exec(vec![Command::internal("holla browse", &cwd, &w.host.name)])
            .keywords(&["browse", "folder", "directory", "files", "ls", "explore"])
            .group("Files")
            .page_aliases(&["b"])
            .launch(Launch::Files { path: cwd.clone() })
            .reason(Signal::Default, "always available"),
    );
    if let Some(cfg) = w.mise.configs.first() {
        out.push(
            Item::new(
                "file.mise",
                &format!(
                    "Open {}",
                    cfg.path.rsplit('/').next().unwrap_or("mise.toml")
                ),
                Kind::Files,
                ResultType::Resource,
                tag_for_dir(w, cfg.path.rsplit_once('/').map(|(d, _)| d).unwrap_or(&cwd)),
            )
            .summary(&format!(
                "preview {} · {} tasks · {}",
                w.location.short(&cfg.path),
                cfg.tasks.len(),
                if cfg.trusted {
                    "trusted"
                } else {
                    "not trusted"
                }
            ))
            .exec(vec![Command::internal("holla preview", &cwd, &w.host.name)])
            .keywords(&["open", "config", "mise.toml", "edit", "file"])
            .group("Files")
            .launch(Launch::Files {
                path: cfg.path.clone(),
            })
            .reason(Signal::Default, "config here"),
        );
    }
    if let Some(p) = &w.location.project {
        let manifest = match p.kind {
            crate::domain::context::ProjectKind::Rust { .. } => Some("Cargo.toml"),
            crate::domain::context::ProjectKind::Node { .. } => Some("package.json"),
            crate::domain::context::ProjectKind::Gradle => Some("settings.gradle.kts"),
            crate::domain::context::ProjectKind::Python => Some("pyproject.toml"),
            _ => None,
        };
        if let Some(m) = manifest {
            out.push(
                Item::new(
                    "file.manifest",
                    &format!("Open {m}"),
                    Kind::Files,
                    ResultType::Resource,
                    tag_for_dir(w, &p.root),
                )
                .summary(&format!("preview {}/{m}", w.location.short(&p.root)))
                .exec(vec![Command::internal(
                    "holla preview",
                    &p.root,
                    &w.host.name,
                )])
                .keywords(&["open", "config", m, "manifest", "edit"])
                .group("Files")
                .launch(Launch::Files {
                    path: format!("{}/{m}", p.root),
                })
                .reason(Signal::Default, "manifest here"),
            );
        }
    }
    if w.docker.compose.is_some() || w.host.role == crate::domain::context::HostRole::Production {
        let (path, size) = if w.host.role == crate::domain::context::HostRole::Production {
            ("/var/log/payments/payments.log".to_owned(), "212 MB")
        } else {
            (format!("{cwd}/logs/api.log"), "2.1 MB")
        };
        out.push(
            Item::new(
                "file.log",
                &format!("Open {}", w.location.short(&path)),
                Kind::Files,
                ResultType::Resource,
                here.clone(),
            )
            .summary(&format!("nearby log file · {size} · opens in the pager"))
            .exec(vec![cmd(w, "less", &["+F", &path], &cwd)])
            .keywords(&["log", "logs", "file", "api.log", "tail"])
            .group("Files")
            .launch(Launch::Activity { script: None })
            .reason(Signal::Default, &format!("{size} nearby")),
        );
    }
    out.push(
        Item::new(
            "file.reveal",
            "Copy this folder's path",
            Kind::Files,
            ResultType::Action,
            here,
        )
        .summary(&format!(
            "copies {} to the clipboard through the terminal (OSC 52)",
            cwd
        ))
        .exec(vec![Command::internal("osc52 copy", &cwd, &w.host.name)])
        .keywords(&["copy", "path", "pwd", "folder", "clipboard"])
        .group("Files")
        .launch(Launch::Copy { value: cwd.clone() })
        .reason(Signal::Default, "available"),
    );
}

fn activity_items(w: &World, out: &mut Vec<Item>) {
    for a in &w.activities {
        let state = a.state.label();
        out.push(
            Item::new(
                &format!("activity.{}", a.id),
                &a.name,
                Kind::Activity,
                ResultType::Resource,
                a.scope.clone(),
            )
            .summary(&format!(
                "{} · started by {} · {} lines retained{}",
                state,
                a.origin,
                a.output.len(),
                if a.dropped > 0 {
                    format!(" · {} dropped", a.dropped)
                } else {
                    String::new()
                }
            ))
            .exec(a.argv.clone())
            .keywords(&["activity", "running", "tab", &a.name, state])
            .group("Activities")
            .launch(Launch::OpenActivity { id: a.id.clone() })
            .alt("Stop", &format!("activity.{}.stop", a.id))
            .alt("Restart", &format!("activity.{}.restart", a.id))
            .reason(
                if a.state.live() {
                    Signal::Context
                } else {
                    Signal::Default
                },
                &format!(
                    "{} {state} · {}",
                    match a.state {
                        crate::domain::activity::ActivityState::Running
                        | crate::domain::activity::ActivityState::Detached
                        | crate::domain::activity::ActivityState::Cancelling
                        | crate::domain::activity::ActivityState::Queued => "⠋",
                        crate::domain::activity::ActivityState::Succeeded => "✓",
                        crate::domain::activity::ActivityState::Failed => "!",
                        crate::domain::activity::ActivityState::Stopped => "○",
                    },
                    crate::screens::ticks_label(a.duration_ticks(w.tick))
                ),
            ),
        );
    }
}
