use crate::domain::activity::{Branch, LineTone, Script, ScriptLine, line, prompt, toned};
use crate::domain::exec::{Command, ExecKind};
use crate::sim::world::World;

fn fail(lines: Vec<ScriptLine>, at: u64, exit: i32) -> Script {
    Script::ending(lines, at, exit)
}

fn ok(lines: Vec<ScriptLine>, at: u64) -> Script {
    Script::ending(lines, at, 0)
}

fn echo(c: &Command) -> ScriptLine {
    line(0, &format!("$ {}", c.display()))
}

fn arg(c: &Command, i: usize) -> &str {
    c.args.get(i).map(String::as_str).unwrap_or("")
}

fn git_parts(c: &Command) -> (String, String, Vec<String>) {
    let mut args = c.args.iter();
    let mut path = c.cwd.clone();
    let mut verb = String::new();
    let mut rest = vec![];
    while let Some(a) = args.next() {
        if a == "-C" {
            path = args.next().cloned().unwrap_or_default();
        } else if verb.is_empty() {
            verb = a.clone();
        } else {
            rest.push(a.clone());
        }
    }
    (path, verb, rest)
}

pub fn for_commands(w: &World, argv: &[Command]) -> Option<Script> {
    let mut merged: Option<Script> = None;
    for c in argv {
        let s = for_command(w, c)?;
        merged = Some(match merged {
            None => s,
            Some(prev) => chain(prev, s),
        });
    }
    merged
}

fn chain(prev: Script, next: Script) -> Script {
    if prev.exit != 0 || prev.ends_at.is_none() {
        return prev;
    }
    let base = prev.ends_at.unwrap_or(0);
    let mut lines = prev.lines;
    let shift = base + 1;
    lines.extend(next.lines.into_iter().map(|l| ScriptLine {
        at: l.at + shift,
        ..l
    }));
    let mut branches = prev.branches;
    branches.extend(next.branches);
    Script {
        lines,
        ends_at: next.ends_at.map(|e| e + shift),
        exit: next.exit,
        branches,
        resistant: prev.resistant || next.resistant,
        spawn_failure: None,
    }
}

pub fn for_command(w: &World, c: &Command) -> Option<Script> {
    if c.kind == ExecKind::Internal {
        return None;
    }
    if !w.tools.contains(&c.program) && c.kind != ExecKind::Shell && !c.program.contains('/') {
        return Some(Script::spawn_failed(&format!(
            "{}: command not found on {}",
            c.program, c.host
        )));
    }
    if !c.cwd.is_empty() && !w.fs.is_dir(&c.cwd) {
        return Some(Script::spawn_failed(&format!(
            "working directory {} does not exist",
            c.cwd
        )));
    }
    match c.program.as_str() {
        "git" => Some(git(w, c)),
        "docker" => Some(docker(w, c)),
        "cargo" => Some(cargo(w, c)),
        "mise" => Some(mise(w, c)),
        "psql" => Some(psql(w, c)),
        "sudo" => Some(sudo(w, c)),
        "systemctl" | "journalctl" => Some(systemd(w, c)),
        "kill" => Some(kill(w, c)),
        "brew" => Some(brew(w, c)),
        "amp" => Some(amp(w, c)),
        "sh" => Some(shell(w, c)),
        "gradle" | "./gradlew" => Some(gradle(w, c)),
        "pnpm" | "yarn" | "bun" | "npm" | "just" | "make" | "task" => Some(task_runner(w, c)),
        "gh" => Some(gh(w, c)),
        "less" => Some(ok(
            vec![echo(c), line(1, "Waiting for data... (interrupt to abort)")],
            3,
        )),
        "find" => Some(find(w, c)),
        "lsof" => Some(ok(vec![echo(c)], 1)),
        "open" | "xdg-open" => Some(opener(w, c)),
        "apt-get" | "apt" => Some(apt(w, c)),
        "trash" => Some(ok(vec![echo(c), line(1, "moved to Trash")], 2)),
        _ => None,
    }
}

fn git(w: &World, c: &Command) -> Script {
    let (path, verb, rest) = git_parts(c);
    let Some(g) = w.git.iter().find(|g| g.path == path) else {
        return fail(
            vec![
                echo(c),
                toned(
                    1,
                    &format!("fatal: not a git repository: {path}"),
                    LineTone::Error,
                ),
            ],
            2,
            128,
        );
    };
    if let Some(f) = &g.command_failure {
        return fail(
            vec![echo(c), toned(1, &format!("fatal: {f}"), LineTone::Error)],
            2,
            128,
        );
    }
    let branch = g.branch.clone().unwrap_or("HEAD".into());
    match verb.as_str() {
        "pull" => {
            let ff_only = rest.iter().any(|a| a == "--ff-only");
            let rebase = rest.iter().any(|a| a == "--rebase");
            if g.in_progress.is_some() || g.conflicts > 0 {
                return fail(
                    vec![
                        echo(c),
                        toned(
                            1,
                            &format!(
                                "error: cannot pull with {} in progress · resolve first",
                                g.in_progress.clone().unwrap_or("conflicts".into())
                            ),
                            LineTone::Error,
                        ),
                    ],
                    2,
                    128,
                );
            }
            if g.branch.is_none() {
                return fail(
                    vec![
                        echo(c),
                        toned(
                            1,
                            "fatal: You are not currently on a branch.",
                            LineTone::Error,
                        ),
                    ],
                    2,
                    128,
                );
            }
            if g.upstream.is_none() {
                return fail(
                    vec![
                        echo(c),
                        toned(
                            1,
                            &format!(
                                "fatal: There is no tracking information for the current branch {branch}."
                            ),
                            LineTone::Error,
                        ),
                    ],
                    2,
                    128,
                );
            }
            if g.behind == 0 {
                return ok(vec![echo(c), line(3, "Already up to date.")], 4);
            }
            if g.diverged && ff_only {
                return fail(
                    vec![
                        echo(c),
                        line(4, &format!("From github.com:acme/{}", g.name())),
                        toned(
                            6,
                            "fatal: Not possible to fast-forward, aborting.",
                            LineTone::Error,
                        ),
                    ],
                    7,
                    128,
                );
            }
            if g.diverged && g.dirty() {
                return fail(
                    vec![
                        echo(c),
                        toned(
                            4,
                            "error: Your local changes to the following files would be overwritten by merge:",
                            LineTone::Error,
                        ),
                        toned(
                            5,
                            &format!(
                                "        {}",
                                g.modified.first().cloned().unwrap_or("src/lib.rs".into())
                            ),
                            LineTone::Error,
                        ),
                        toned(
                            6,
                            "Please commit your changes or stash them before you merge.",
                            LineTone::Error,
                        ),
                    ],
                    7,
                    1,
                );
            }
            let mut lines = vec![
                echo(c),
                line(4, &format!("From github.com:acme/{}", g.name())),
                line(
                    4,
                    &format!("   {}..a9c0f1e  {branch} -> origin/{branch}", g.head_short),
                ),
            ];
            if g.diverged {
                if rebase {
                    lines.push(line(
                        7,
                        &format!("Successfully rebased and updated refs/heads/{branch}."),
                    ));
                } else {
                    lines.push(line(7, "Merge made by the 'ort' strategy."));
                    lines.push(line(7, &format!(" {} files changed", g.behind + 1)));
                }
            } else {
                lines.push(line(7, &format!("Updating {}..a9c0f1e", g.head_short)));
                lines.push(line(8, "Fast-forward"));
                lines.push(toned(
                    9,
                    &format!(
                        " {} files changed, {} insertions(+)",
                        g.behind * 2,
                        g.behind * 13
                    ),
                    LineTone::Success,
                ));
            }
            ok(lines, 10)
        }
        "push" => {
            let remote = rest.iter().find(|a| !a.starts_with("--")).cloned();
            let remote_name = remote.clone().unwrap_or("origin".into());
            if rest.iter().any(|a| a == "--dry-run") {
                return ok(
                    vec![
                        echo(c),
                        line(2, &format!("To github.com:acme/{}.git", g.name())),
                        line(
                            2,
                            &format!("   {}..{}  {branch} -> {branch}", g.head_short, "b2c3d4e"),
                        ),
                    ],
                    3,
                );
            }
            if !g.has_remote(&remote_name) {
                return fail(
                    vec![
                        echo(c),
                        toned(
                            1,
                            &format!(
                                "fatal: '{remote_name}' does not appear to be a git repository"
                            ),
                            LineTone::Error,
                        ),
                        toned(
                            1,
                            "fatal: Could not read from remote repository.",
                            LineTone::Error,
                        ),
                    ],
                    2,
                    128,
                );
            }
            if g.branch.is_none() {
                return fail(
                    vec![
                        echo(c),
                        toned(
                            1,
                            "fatal: You are not currently on a branch.",
                            LineTone::Error,
                        ),
                    ],
                    2,
                    128,
                );
            }
            if remote.is_none() && g.upstream.is_none() {
                return fail(
                    vec![
                        echo(c),
                        toned(
                            1,
                            &format!("fatal: The current branch {branch} has no upstream branch."),
                            LineTone::Error,
                        ),
                        line(1, &format!("    git push --set-upstream origin {branch}")),
                    ],
                    2,
                    128,
                );
            }
            if let Some(why) = &g.push_rejected {
                return fail(
                    vec![
                        echo(c),
                        line(3, &format!("To github.com:acme/{}.git", g.name())),
                        toned(
                            3,
                            &format!(" ! [rejected]        {branch} -> {branch} ({why})"),
                            LineTone::Error,
                        ),
                        toned(
                            4,
                            &format!("error: failed to push some refs to '{remote_name}'"),
                            LineTone::Error,
                        ),
                    ],
                    5,
                    1,
                );
            }
            if g.ahead == 0 {
                return ok(vec![echo(c), line(3, "Everything up-to-date")], 4);
            }
            ok(
                vec![
                    echo(c),
                    line(3, &format!("Enumerating objects: {}, done.", g.ahead * 5)),
                    line(4, &format!("To github.com:acme/{}.git", g.name())),
                    toned(
                        5,
                        &format!("   {}..b2c3d4e  {branch} -> {branch}", g.head_short),
                        LineTone::Success,
                    ),
                ],
                6,
            )
        }
        "status" => {
            let short = rest.iter().any(|a| a == "--short");
            let mut lines = vec![echo(c)];
            if !short {
                lines.push(line(1, &format!("On branch {branch}")));
                if let Some(u) = &g.upstream {
                    lines.push(line(1, &match (g.ahead, g.behind) {
                        (0, 0) => format!("Your branch is up to date with '{u}'."),
                        (a, 0) => format!("Your branch is ahead of '{u}' by {a} commit{}.", if a == 1 { "" } else { "s" }),
                        (0, b) => format!("Your branch is behind '{u}' by {b} commit{}, and can be fast-forwarded.", if b == 1 { "" } else { "s" }),
                        (a, b) => format!("Your branch and '{u}' have diverged,\nand have {a} and {b} different commits each, respectively."),
                    }));
                }
            }
            for f in &g.modified {
                lines.push(line(
                    2,
                    &if short {
                        format!(" M {f}")
                    } else {
                        format!("\tmodified:   {f}")
                    },
                ));
            }
            if g.untracked > 0 {
                lines.push(line(2, &format!("?? … {} untracked", g.untracked)));
            }
            if g.modified.is_empty() && g.untracked == 0 && !short {
                lines.push(line(2, "nothing to commit, working tree clean"));
            }
            ok(lines, 3)
        }
        "fetch" => ok(
            vec![
                echo(c),
                line(3, &format!("From github.com:acme/{}", g.name())),
                line(3, " - [deleted]         (none)     -> origin/feature/old"),
                line(
                    4,
                    &format!("   {}..a9c0f1e  {branch} -> origin/{branch}", g.head_short),
                ),
            ],
            5,
        ),
        "gc" => ok(
            vec![
                echo(c),
                line(2, "Enumerating objects: 4812, done."),
                line(6, "Compressing objects: 100% (1204/1204), done."),
                line(9, "Total 4812 (delta 2210), reused 4812 (delta 2210)"),
            ],
            10,
        ),
        "switch" => {
            let target = rest.first().cloned().unwrap_or_default();
            if let Some(r) = g.block_reason() {
                return fail(
                    vec![
                        echo(c),
                        toned(1, &format!("error: cannot switch: {r}"), LineTone::Error),
                    ],
                    2,
                    1,
                );
            }
            ok(
                vec![
                    echo(c),
                    line(2, &format!("Switched to branch '{target}'")),
                    line(
                        2,
                        &format!("Your branch is up to date with 'origin/{target}'."),
                    ),
                ],
                3,
            )
        }
        "branch" => {
            let names: Vec<&String> = rest.iter().skip_while(|a| *a != "--").skip(1).collect();
            let mut lines = vec![echo(c)];
            let mut exit = 0;
            let mut t = 1;
            for n in names {
                if g.occupied.iter().any(|o| o == n) {
                    lines.push(toned(
                        t,
                        &format!(
                            "error: cannot delete branch '{n}' used by worktree at '{}-wt'",
                            g.path
                        ),
                        LineTone::Error,
                    ));
                    exit = 1;
                } else if !g
                    .merged
                    .iter()
                    .any(|m| m.trim_start_matches(['*', '+', ' ']) == n)
                {
                    lines.push(toned(
                        t,
                        &format!("error: the branch '{n}' is not fully merged"),
                        LineTone::Error,
                    ));
                    lines.push(toned(
                        t,
                        &format!(
                            "hint: If you are sure you want to delete it, run 'git branch -D {n}'"
                        ),
                        LineTone::Muted,
                    ));
                    exit = 1;
                } else {
                    lines.push(line(t, &format!("Deleted branch {n} (was 3f1a9c2).")));
                }
                t += 1;
            }
            Script::ending(lines, t + 1, exit)
        }
        "stash" => ok(
            vec![echo(c), line(1, "stash@{0}: WIP on main: 9f2c1aa wip")],
            2,
        ),
        "diff" | "log" => ok(
            vec![
                echo(c),
                line(1, " 2 files changed, 38 insertions(+), 15 deletions(-)"),
            ],
            2,
        ),
        _ => fail(
            vec![
                echo(c),
                toned(1, &format!("git: '{verb}' is not modeled"), LineTone::Error),
            ],
            2,
            1,
        ),
    }
}

fn docker(w: &World, c: &Command) -> Script {
    let d = &w.docker;
    let echo_l = echo(c);
    if let Err(r) = &d.daemon {
        return fail(vec![echo_l, toned(1, r, LineTone::Error)], 2, 1);
    }
    let sub = arg(c, 0).to_owned();
    let sub2 = arg(c, 1).to_owned();
    let stage_fails = |stage: &str| d.fail_stage.as_deref() == Some(stage);
    match (sub.as_str(), sub2.as_str()) {
        ("restart", name) => {
            if d.containers.iter().any(|x| x.name == name) {
                ok(vec![echo_l, line(3, name)], 4)
            } else {
                fail(
                    vec![
                        echo_l,
                        toned(
                            1,
                            &format!("Error response from daemon: No such container: {name}"),
                            LineTone::Error,
                        ),
                    ],
                    2,
                    1,
                )
            }
        }
        ("stop", first) => {
            let names: Vec<&str> = c.args.iter().skip(1).map(String::as_str).collect();
            if names.is_empty() {
                return fail(
                    vec![
                        echo_l,
                        toned(
                            1,
                            "\"docker stop\" requires at least 1 argument.",
                            LineTone::Error,
                        ),
                    ],
                    2,
                    1,
                );
            }
            if stage_fails("stop") {
                return fail(
                    vec![
                        echo_l,
                        line(2, first),
                        toned(
                            3,
                            &format!(
                                "Error response from daemon: cannot stop container: {}: tried to kill container, but did not receive an exit event",
                                names.last().unwrap_or(&first)
                            ),
                            LineTone::Error,
                        ),
                    ],
                    4,
                    1,
                );
            }
            let mut lines = vec![echo_l];
            for (i, n) in names.iter().enumerate() {
                lines.push(line(2 + i as u64 * 2, n));
            }
            let end = 2 + names.len() as u64 * 2;
            ok(lines, end)
        }
        ("rm", _) => {
            let names: Vec<&str> = c.args.iter().skip(1).map(String::as_str).collect();
            if stage_fails("rm") {
                return fail(
                    vec![
                        echo_l,
                        toned(
                            1,
                            &format!(
                                "Error response from daemon: cannot remove container {}: container is running: stop the container before removing",
                                names.first().unwrap_or(&"?")
                            ),
                            LineTone::Error,
                        ),
                    ],
                    2,
                    1,
                );
            }
            let mut lines = vec![echo_l];
            for (i, n) in names.iter().enumerate() {
                lines.push(line(1 + i as u64, n));
            }
            ok(lines, names.len() as u64 + 2)
        }
        ("image", "prune") => {
            if stage_fails("images") {
                return fail(
                    vec![
                        echo_l,
                        toned(
                            2,
                            "Error response from daemon: conflict: unable to delete acme/api:dev (must be forced) - image is being used by running container acme-api-1",
                            LineTone::Error,
                        ),
                    ],
                    3,
                    1,
                );
            }
            ok(
                vec![
                    echo_l,
                    line(2, "Deleted Images:"),
                    line(3, &format!("untagged: … {} images", d.images)),
                    toned(
                        6,
                        &format!("Total reclaimed space: {:.1}GB", d.images_gb),
                        LineTone::Success,
                    ),
                ],
                7,
            )
        }
        ("network", "prune") => ok(
            vec![
                echo_l,
                line(1, "Deleted Networks:"),
                line(1, "acme_default"),
            ],
            3,
        ),
        ("volume", "prune") => {
            if stage_fails("volumes") {
                return fail(
                    vec![
                        echo_l,
                        toned(
                            2,
                            "Error response from daemon: remove acme_pgdata: volume is in use",
                            LineTone::Error,
                        ),
                    ],
                    3,
                    1,
                );
            }
            ok(
                vec![
                    echo_l,
                    line(2, "Deleted Volumes:"),
                    line(3, &format!("… {} volumes", d.volumes.len())),
                    toned(
                        5,
                        &format!("Total reclaimed space: {:.1}GB", d.volumes_gb()),
                        LineTone::Success,
                    ),
                ],
                6,
            )
        }
        ("builder", "prune") | ("buildx", "prune") => ok(
            vec![
                echo_l,
                line(2, "ID                       RECLAIMABLE  SIZE"),
                line(3, "z9q…                     true         1.8GB"),
                toned(
                    5,
                    &format!("Total: {:.1}GB", d.builder_cache_gb),
                    LineTone::Success,
                ),
            ],
            6,
        ),
        ("system", "df") => ok(
            vec![
                echo_l,
                line(
                    1,
                    "TYPE            TOTAL     ACTIVE    SIZE      RECLAIMABLE",
                ),
                line(
                    1,
                    &format!(
                        "Images          {:<9} {:<9} {:.1}GB     {:.1}GB",
                        d.images,
                        d.running(),
                        d.images_gb,
                        d.images_gb * 0.8
                    ),
                ),
                line(
                    1,
                    &format!(
                        "Containers      {:<9} {:<9} 0.4GB     0.2GB",
                        d.containers.len(),
                        d.running()
                    ),
                ),
                line(
                    1,
                    &format!(
                        "Local Volumes   {:<9} {:<9} {:.1}GB     {:.1}GB",
                        d.volumes.len(),
                        d.volumes.len() - d.named_volumes(),
                        d.volumes_gb(),
                        d.volumes_gb() * 0.5
                    ),
                ),
                line(
                    1,
                    &format!(
                        "Build Cache     124       0         {:.1}GB     {:.1}GB",
                        d.builder_cache_gb, d.builder_cache_gb
                    ),
                ),
            ],
            2,
        ),
        ("ps", _) => ok(
            vec![
                echo_l,
                line(1, &format!("{} containers", d.containers.len())),
            ],
            2,
        ),
        ("logs", _) => {
            let name = c.args.last().cloned().unwrap_or_default();
            if !d.containers.iter().any(|x| x.name == name) {
                return fail(
                    vec![
                        echo_l,
                        toned(
                            1,
                            &format!("Error response from daemon: No such container: {name}"),
                            LineTone::Error,
                        ),
                    ],
                    2,
                    1,
                );
            }
            Script::running(vec![
                echo_l,
                line(2, "INFO  listening on 0.0.0.0:8080"),
                line(5, "INFO  GET /health 200 · 1 ms"),
                toned(
                    9,
                    "ERROR health check failed · db pool exhausted (20/20)",
                    LineTone::Error,
                ),
            ])
        }
        ("compose", verb) => {
            if !d.compose_plugin {
                return fail(
                    vec![
                        echo_l,
                        toned(
                            1,
                            "docker: 'compose' is not a docker command.",
                            LineTone::Error,
                        ),
                    ],
                    2,
                    1,
                );
            }
            let Some(comp) = &d.compose else {
                return fail(
                    vec![
                        echo_l,
                        toned(
                            1,
                            "no configuration file provided: not found",
                            LineTone::Error,
                        ),
                    ],
                    2,
                    1,
                );
            };
            match verb {
                "up" => {
                    let mut lines = vec![echo_l];
                    for (i, s) in comp.services.iter().enumerate() {
                        lines.push(line(
                            2 + i as u64,
                            &format!(" Container {}-{s}-1  Started", comp.name),
                        ));
                    }
                    ok(lines, comp.services.len() as u64 + 3)
                }
                "down" | "stop" => {
                    let mut lines = vec![echo_l];
                    for (i, s) in comp.services.iter().enumerate() {
                        lines.push(line(
                            2 + i as u64,
                            &format!(
                                " Container {}-{s}-1  {}",
                                comp.name,
                                if verb == "down" { "Removed" } else { "Stopped" }
                            ),
                        ));
                    }
                    if verb == "down" {
                        lines.push(line(
                            comp.services.len() as u64 + 2,
                            &format!(" Network {}_default  Removed", comp.name),
                        ));
                        lines.push(toned(
                            comp.services.len() as u64 + 2,
                            "volumes kept (no --volumes)",
                            LineTone::Muted,
                        ));
                    }
                    ok(lines, comp.services.len() as u64 + 3)
                }
                "logs" => {
                    let follow = c.args.iter().any(|a| a == "-f" || a == "--follow");
                    let lines = vec![
                        echo_l,
                        line(1, "api-1        | INFO  listening on 0.0.0.0:8080"),
                        line(
                            2,
                            "worker-1     | INFO  worker ready · queue=orders concurrency=4",
                        ),
                        line(3, "scheduler-1  | INFO  scheduler tick · 3 jobs due"),
                        line(4, "api-1        | WARN  upstream timeout · retry 1 of 3"),
                    ];
                    if follow {
                        Script::running(lines)
                    } else {
                        ok(lines, 5)
                    }
                }
                "ps" => ok(
                    vec![
                        echo_l,
                        line(1, &format!("{} services", comp.services.len())),
                    ],
                    2,
                ),
                _ => fail(
                    vec![
                        echo_l,
                        toned(1, &format!("compose {verb}: not modeled"), LineTone::Error),
                    ],
                    2,
                    1,
                ),
            }
        }
        ("info", _) => fail(
            vec![
                echo_l,
                toned(1, "Cannot connect to the Docker daemon", LineTone::Error),
            ],
            2,
            1,
        ),
        _ => fail(
            vec![
                echo_l,
                toned(
                    1,
                    &format!("docker: '{sub}' is not modeled"),
                    LineTone::Error,
                ),
            ],
            2,
            1,
        ),
    }
}

fn cargo(w: &World, c: &Command) -> Script {
    let cg = &w.cargo;
    let echo_l = echo(c);
    if !w.fs.exists(&format!("{}/Cargo.toml", c.cwd)) {
        return fail(
            vec![
                echo_l,
                toned(
                    1,
                    &format!(
                        "error: could not find `Cargo.toml` in `{}` or any parent directory",
                        c.cwd
                    ),
                    LineTone::Error,
                ),
            ],
            2,
            101,
        );
    }
    let name = c.cwd.rsplit('/').next().unwrap_or("crate");
    match arg(c, 0) {
        "build" | "check" => {
            if let Some(e) = &cg.build_error {
                return fail(
                    vec![
                        echo_l,
                        line(2, &format!("   Compiling {name} v0.3.1 ({})", c.cwd)),
                        toned(8, &format!("error[E0308]: {e}"), LineTone::Error),
                        toned(
                            9,
                            &format!("error: could not compile `{name}` due to 1 previous error"),
                            LineTone::Error,
                        ),
                    ],
                    10,
                    101,
                );
            }
            ok(
                vec![
                    echo_l,
                    line(2, &format!("   Compiling {name} v0.3.1 ({})", c.cwd)),
                    toned(
                        18,
                        "    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.21s",
                        LineTone::Success,
                    ),
                ],
                19,
            )
        }
        "test" => {
            if cg.test_failures > 0 {
                return fail(
                    vec![
                        echo_l,
                        line(2, &format!("   Compiling {name} v0.3.1 ({})", c.cwd)),
                        line(14, "     Running unittests src/lib.rs"),
                        line(16, "running 128 tests"),
                        toned(
                            20,
                            "test ranking::tests::aliases_beat_everything ... FAILED",
                            LineTone::Error,
                        ),
                        line(22, "failures:"),
                        line(22, "    ranking::tests::aliases_beat_everything"),
                        toned(
                            23,
                            &format!(
                                "test result: FAILED. {} passed; {} failed; 0 ignored",
                                128 - cg.test_failures,
                                cg.test_failures
                            ),
                            LineTone::Error,
                        ),
                    ],
                    24,
                    101,
                );
            }
            ok(
                vec![
                    echo_l,
                    line(2, &format!("   Compiling {name} v0.3.1 ({})", c.cwd)),
                    line(14, "     Running unittests src/lib.rs"),
                    line(16, "running 128 tests"),
                    toned(
                        24,
                        "test result: ok. 128 passed; 0 failed; 0 ignored; 0 measured",
                        LineTone::Success,
                    ),
                ],
                25,
            )
        }
        "clippy" => {
            let mut lines = vec![
                echo_l,
                line(2, &format!("    Checking {name} v0.3.1 ({})", c.cwd)),
            ];
            for i in 0..cg.clippy_warnings {
                lines.push(toned(
                    6 + i as u64,
                    &format!("warning: unused variable: `scope_{i}`"),
                    LineTone::Warning,
                ));
                lines.push(line(
                    6 + i as u64,
                    "  --> src/bin/holla/screens/here.rs:412:13",
                ));
            }
            let deny = c.args.iter().any(|a| a == "-D");
            if cg.clippy_warnings > 0 && deny {
                lines.push(toned(
                    10,
                    &format!(
                        "error: could not compile `{name}` due to {} previous error{}",
                        cg.clippy_warnings,
                        if cg.clippy_warnings == 1 { "" } else { "s" }
                    ),
                    LineTone::Error,
                ));
                return fail(lines, 11, 101);
            }
            lines.push(toned(
                10,
                &format!(
                    "    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.02s{}",
                    if cg.clippy_warnings > 0 {
                        format!(" · {} warnings", cg.clippy_warnings)
                    } else {
                        String::new()
                    }
                ),
                LineTone::Success,
            ));
            ok(lines, 11)
        }
        "fmt" => ok(vec![echo_l], 2),
        "run" => ok(
            vec![
                echo_l,
                line(2, &format!("   Compiling {name} v0.3.1")),
                line(12, &format!("     Running `target/debug/{name}`")),
                line(13, "listening on 127.0.0.1:8080"),
            ],
            14,
        ),
        "clean" => {
            let Some(target) = &cg.target else {
                return ok(vec![echo_l, line(1, "     Removed 0 files")], 2);
            };
            if c.args.iter().any(|a| a == "--dry-run") {
                return ok(
                    vec![
                        echo_l,
                        line(
                            1,
                            &format!(
                                "     Summary {} files, {} total (dry run)",
                                cg.target_files,
                                junie_tui::ui::text::bytes(cg.target_bytes)
                            ),
                        ),
                        toned(2, "nothing removed · --dry-run", LineTone::Muted),
                    ],
                    3,
                );
            }
            if !w.fs.exists(target) {
                return ok(
                    vec![
                        echo_l,
                        line(1, "     Removed 0 files · target directory absent"),
                    ],
                    2,
                );
            }
            ok(
                vec![
                    echo_l,
                    line(
                        2,
                        &format!(
                            "     Removed {} files, {} total",
                            cg.target_files,
                            junie_tui::ui::text::bytes(cg.target_bytes)
                        ),
                    ),
                ],
                3,
            )
        }
        other => fail(
            vec![
                echo_l,
                toned(
                    1,
                    &format!("error: no such command: `{other}`"),
                    LineTone::Error,
                ),
            ],
            2,
            101,
        ),
    }
}

fn mise(w: &World, c: &Command) -> Script {
    let echo_l = echo(c);
    match arg(c, 0) {
        "run" => {
            let task = arg(c, 1).to_owned();
            let known = w
                .mise
                .configs
                .iter()
                .flat_map(|cfg| cfg.tasks.iter())
                .find(|t| t.namespaced == task || t.name == task);
            match known {
                None => fail(
                    vec![
                        echo_l,
                        toned(
                            1,
                            &format!("mise ERROR no task {task} found"),
                            LineTone::Error,
                        ),
                    ],
                    2,
                    1,
                ),
                Some(t) => {
                    if c.args.iter().any(|a| a == "--dry-run") {
                        return ok(
                            vec![
                                echo_l,
                                line(1, &format!("[mise] would run: {}", t.run)),
                                line(1, &format!("[mise] in {}", t.dir)),
                                toned(2, "nothing executed · --dry-run", LineTone::Muted),
                            ],
                            3,
                        );
                    }
                    ok(
                        vec![
                            echo_l,
                            line(1, &format!("[mise] {} · {}", t.namespaced, t.run)),
                            line(4, &format!("[mise] {} finished", t.name)),
                        ],
                        5,
                    )
                }
            }
        }
        "install" => ok(
            vec![
                echo_l,
                line(2, "mise sqlx-cli@0.8.6 install"),
                line(9, "mise sqlx-cli@0.8.6 ✓ installed"),
            ],
            10,
        ),
        "upgrade" => {
            if w.upgrade.failing.iter().any(|f| f == "mise upgrade") {
                return fail(
                    vec![
                        echo_l,
                        line(
                            3,
                            "mise node@26.1.0 · downloading node-v26.1.0-linux-x64.tar.xz",
                        ),
                        toned(
                            8,
                            "mise node@26.1.0 · checksum mismatch · expected 9f3a… got 4c11…",
                            LineTone::Error,
                        ),
                        toned(
                            9,
                            "mise ERROR failed to install node@26.1.0",
                            LineTone::Error,
                        ),
                    ],
                    10,
                    1,
                );
            }
            let outdated: Vec<String> = w
                .mise
                .outdated()
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
            let mut lines = vec![echo_l];
            for (i, o) in outdated.iter().enumerate() {
                lines.push(line(2 + i as u64 * 3, &format!("mise {o} ✓")));
            }
            lines.push(toned(
                2 + outdated.len() as u64 * 3,
                &format!("{} tools upgraded", outdated.len()),
                LineTone::Success,
            ));
            ok(lines, 3 + outdated.len() as u64 * 3)
        }
        "outdated" => ok(
            vec![echo_l, line(1, "Tool     Requested  Current   Latest")],
            2,
        ),
        _ => fail(
            vec![echo_l, toned(1, "mise: not modeled", LineTone::Error)],
            2,
            1,
        ),
    }
}

fn psql(w: &World, c: &Command) -> Script {
    let echo_l = echo(c);
    let Some(pg) = &w.pg else {
        return fail(
            vec![
                echo_l,
                toned(1, "psql: error: connection refused", LineTone::Error),
            ],
            2,
            2,
        );
    };
    let sql = c
        .args
        .iter()
        .skip_while(|a| *a != "-c")
        .nth(1)
        .cloned()
        .unwrap_or_default();
    let pid: Option<u32> = sql.split(['(', ')']).nth(1).and_then(|p| p.parse().ok());
    let exists = pid.is_some_and(|p| pg.sessions.iter().any(|s| s.pid == p));
    if !exists {
        return ok(
            vec![
                echo_l,
                line(1, " pg_cancel_backend"),
                line(1, "-------------------"),
                line(1, " f"),
                toned(
                    2,
                    "(the backend was already gone: nothing to cancel)",
                    LineTone::Warning,
                ),
            ],
            3,
        );
    }
    let fname = if sql.contains("terminate") {
        "pg_terminate_backend"
    } else {
        "pg_cancel_backend"
    };
    ok(
        vec![
            echo_l,
            line(1, &format!(" {fname}")),
            line(1, "-------------------"),
            line(1, " t"),
            line(2, "(1 row)"),
        ],
        3,
    )
}

fn sudo(w: &World, c: &Command) -> Script {
    let inner = Command {
        program: arg(c, 0).to_owned(),
        args: c.args.iter().skip(1).cloned().collect(),
        cwd: c.cwd.clone(),
        host: c.host.clone(),
        kind: ExecKind::Argv,
    };
    let body = for_command(w, &inner).unwrap_or_else(|| Script::unmodeled(&inner.display()));
    if w.sudo_cached {
        return chain(ok(vec![echo(c)], 0), body);
    }

    let head = Script::ending(
        vec![
            echo(c),
            prompt(1, &format!("[sudo] password for {}: ", w.host.user), true),
        ],
        1,
        0,
    )
    .branches(vec![vec![
        Branch {
            matches: Some("\u{4}".into()),
            lines: vec![
                toned(0, "sudo: no password was provided", LineTone::Error),
                toned(0, "sudo: a password is required", LineTone::Error),
            ],
            end: Some((1, 1)),
        },
        Branch {
            matches: Some("wrong".into()),
            lines: vec![
                toned(0, "Sorry, try again.", LineTone::Error),
                toned(0, "sudo: 1 incorrect password attempt", LineTone::Error),
            ],
            end: Some((1, 1)),
        },
        Branch {
            matches: None,
            lines: vec![],
            end: None,
        },
    ]]);
    chain(head, body)
}

fn systemd(w: &World, c: &Command) -> Script {
    let echo_l = echo(c);
    if c.program == "journalctl" {
        return Script::running(vec![
            echo_l,
            line(
                1,
                "Sep 10 08:31:02 prod-eu-1 payments-worker[41872]: INFO worker started",
            ),
            toned(
                3,
                "Sep 10 08:33:40 prod-eu-1 payments-worker[41872]: ERROR settlement batch 8812 failed: deadline exceeded",
                LineTone::Error,
            ),
        ]);
    }
    let unit = arg(c, 1).to_owned();
    match arg(c, 0) {
        "restart" => {
            if w.service_failures.iter().any(|u| u == &unit) {
                return fail(
                    vec![
                        echo_l,
                        toned(
                            2,
                            &format!(
                                "Job for {unit}.service failed because the control process exited with error code."
                            ),
                            LineTone::Error,
                        ),
                        toned(
                            2,
                            &format!(
                                "See \"systemctl status {unit}.service\" and \"journalctl -xeu {unit}.service\" for details."
                            ),
                            LineTone::Muted,
                        ),
                    ],
                    3,
                    1,
                );
            }
            ok(
                vec![echo_l, line(3, &format!("{unit}.service restarted"))],
                4,
            )
        }
        "status" => ok(
            vec![
                echo_l,
                line(1, &format!("● {unit}.service - {unit}")),
                line(1, "     Active: active (running)"),
            ],
            2,
        ),
        _ => ok(vec![echo_l], 1),
    }
}

fn kill(w: &World, c: &Command) -> Script {
    let echo_l = echo(c);
    let pid: Option<u32> = c.args.iter().rev().find_map(|a| a.parse().ok());
    match pid {
        Some(p) if w.system.top.iter().any(|x| x.pid == p) => ok(
            vec![echo_l, line(1, &format!("sent {} to {p}", arg(c, 0)))],
            2,
        ),
        Some(p) => fail(
            vec![
                echo_l,
                toned(
                    1,
                    &format!("kill: ({p}) - No such process"),
                    LineTone::Error,
                ),
            ],
            2,
            1,
        ),
        None => fail(
            vec![
                echo_l,
                toned(1, "kill: no process id given", LineTone::Error),
            ],
            2,
            1,
        ),
    }
}

fn brew(w: &World, c: &Command) -> Script {
    let echo_l = echo(c);
    let Some(b) = &w.brew else {
        return Script::spawn_failed("brew: command not found");
    };
    match arg(c, 0) {
        "services" => {
            let verb = arg(c, 1).to_owned();
            let svc = arg(c, 2).to_owned();
            if !b.services.iter().any(|s| s.name == svc) {
                return fail(
                    vec![
                        echo_l,
                        toned(
                            1,
                            &format!("Error: Formula `{svc}` is not installed."),
                            LineTone::Error,
                        ),
                    ],
                    2,
                    1,
                );
            }
            if let Some((_, _, why)) = b.failing.iter().find(|(s, v, _)| *s == svc && *v == verb) {
                return fail(
                    vec![
                        echo_l,
                        toned(
                            2,
                            &format!(
                                "Error: Failure while executing; `{}/bin/launchctl bootstrap gui/501 …/homebrew.mxcl.{svc}.plist` exited with 5: {why}",
                                if b.linux { "/usr" } else { "/bin" }
                            ),
                            LineTone::Error,
                        ),
                    ],
                    3,
                    1,
                );
            }
            let past = match verb.as_str() {
                "start" => "started",
                "stop" => "stopped",
                _ => "restarted",
            };
            ok(
                vec![
                    echo_l,
                    line(
                        3,
                        &format!("==> Successfully {past} `{svc}` (label: homebrew.mxcl.{svc})"),
                    ),
                ],
                4,
            )
        }
        "update" => {
            if w.upgrade.failing.iter().any(|f| f == "brew update") {
                return fail(
                    vec![
                        echo_l,
                        toned(
                            3,
                            "fatal: unable to access 'https://github.com/Homebrew/homebrew-core/': Could not resolve host: github.com",
                            LineTone::Error,
                        ),
                        toned(
                            4,
                            "Error: Fetching /opt/homebrew/Library/Taps/homebrew/homebrew-core failed!",
                            LineTone::Error,
                        ),
                    ],
                    5,
                    1,
                );
            }
            ok(
                vec![
                    echo_l,
                    line(4, "==> Updating Homebrew..."),
                    line(
                        8,
                        &format!("==> Outdated Formulae · {}", w.upgrade.brew_outdated.len()),
                    ),
                ],
                9,
            )
        }
        "upgrade" => {
            let cask = c.args.iter().any(|a| a == "--cask");
            if cask && w.upgrade.failing.iter().any(|f| f == "brew upgrade --cask") {
                return fail(
                    vec![
                        echo_l,
                        toned(4, "Error: Cask 'docker' is not installed.", LineTone::Error),
                    ],
                    5,
                    1,
                );
            }
            let list = if cask {
                &w.upgrade.casks_outdated
            } else {
                &w.upgrade.brew_outdated
            };
            let mut lines = vec![
                echo_l,
                line(
                    1,
                    &format!(
                        "==> Upgrading {} outdated {}{}",
                        list.len(),
                        if cask { "cask" } else { "package" },
                        if list.len() == 1 { "" } else { "s" }
                    ),
                ),
            ];
            for (i, (n, from, to)) in list.iter().enumerate() {
                lines.push(line(
                    3 + i as u64 * 4,
                    &format!("==> Upgrading {n}\n  {from} -> {to}"),
                ));
            }
            ok(lines, 4 + list.len() as u64 * 4)
        }
        "cleanup" => ok(
            vec![
                echo_l,
                line(
                    2,
                    "Removing: /opt/homebrew/Cellar/node/26.0.1... (3,412 files, 84MB)",
                ),
                line(
                    3,
                    "==> This operation has freed approximately 412MB of disk space.",
                ),
            ],
            4,
        ),
        "autoremove" => ok(
            vec![
                echo_l,
                line(2, "==> Autoremoving 1 unneeded formula:"),
                line(2, "libpng"),
            ],
            3,
        ),
        "doctor" => {
            if w.upgrade.failing.iter().any(|f| f == "brew doctor") {
                return fail(
                    vec![
                        echo_l,
                        toned(
                            2,
                            "Warning: Some installed formulae are missing dependencies.",
                            LineTone::Warning,
                        ),
                        line(2, "  brew install libpng"),
                    ],
                    3,
                    1,
                );
            }
            ok(vec![echo_l, line(2, "Your system is ready to brew.")], 3)
        }
        _ => fail(
            vec![echo_l, toned(1, "brew: not modeled", LineTone::Error)],
            2,
            1,
        ),
    }
}

fn amp(w: &World, c: &Command) -> Script {
    let echo_l = echo(c);
    if w.upgrade.failing.iter().any(|f| f == "amp update") {
        return fail(
            vec![
                echo_l,
                toned(
                    2,
                    "error: update check failed: 503 Service Unavailable",
                    LineTone::Error,
                ),
            ],
            3,
            1,
        );
    }
    ok(
        vec![
            echo_l,
            line(2, "amp 0.9.14 → 0.9.16"),
            toned(6, "updated", LineTone::Success),
        ],
        7,
    )
}

fn shell(w: &World, c: &Command) -> Script {
    let echo_l = echo(c);
    let script = if arg(c, 0) == "-c" {
        arg(c, 1).to_owned()
    } else {
        arg(c, 0).to_owned()
    };
    if script.ends_with("tools/upgrade.sh") {
        if !w.fs.exists(&script) {
            return fail(
                vec![
                    echo_l,
                    toned(
                        1,
                        &format!("sh: {script}: No such file or directory"),
                        LineTone::Error,
                    ),
                ],
                2,
                127,
            );
        }
        if w.upgrade.failing.iter().any(|f| f == "omz") {
            return fail(
                vec![
                    echo_l,
                    toned(
                        3,
                        "Error: git fetch failed · There was an error updating. Try again later?",
                        LineTone::Error,
                    ),
                ],
                4,
                1,
            );
        }
        return ok(
            vec![
                echo_l,
                line(2, "Updating Oh My Zsh"),
                line(5, "Hooray! Oh My Zsh has been updated!"),
            ],
            6,
        );
    }

    ok(
        vec![
            echo_l,
            line(1, "(shell script output modelled as one unit)"),
        ],
        2,
    )
}

fn gradle(w: &World, c: &Command) -> Script {
    let echo_l = echo(c);
    let g = &w.gradle;
    let cwd = c.cwd.clone();
    if c.program == "./gradlew" && !w.fs.exists(&format!("{cwd}/gradlew")) {
        return Script::spawn_failed(&format!("{cwd}/gradlew: no wrapper in this project"));
    }
    match arg(c, 0) {
        "--stop" => {
            if let Some(e) = &g.stop_fails {
                return fail(
                    vec![echo_l, toned(2, &format!("FAILURE: {e}"), LineTone::Error)],
                    3,
                    1,
                );
            }
            if g.daemon_running {
                ok(
                    vec![
                        echo_l,
                        line(2, "Stopping Daemon(s)"),
                        line(4, "1 Daemon stopped"),
                    ],
                    5,
                )
            } else {
                ok(vec![echo_l, line(1, "No Gradle daemons are running.")], 2)
            }
        }
        "clean" => ok(
            vec![
                echo_l,
                line(3, "> Task :app:clean"),
                line(4, "> Task :core:clean"),
                toned(6, "BUILD SUCCESSFUL in 4s", LineTone::Success),
            ],
            7,
        ),
        "build" => {
            if g.build_fails {
                return fail(
                    vec![
                        echo_l,
                        line(3, "> Task :app:compileKotlin FAILED"),
                        toned(
                            9,
                            "e: file:///src/main/kotlin/Main.kt:12:5 Unresolved reference: ledger",
                            LineTone::Error,
                        ),
                        toned(10, "BUILD FAILED in 9s", LineTone::Error),
                    ],
                    11,
                    1,
                );
            }
            ok(
                vec![
                    echo_l,
                    line(3, "> Task :app:compileKotlin"),
                    line(12, "> Task :app:assemble"),
                    toned(20, "BUILD SUCCESSFUL in 41s", LineTone::Success),
                ],
                21,
            )
        }
        "test" => {
            if g.test_fails {
                return fail(
                    vec![
                        echo_l,
                        line(3, "> Task :app:test"),
                        toned(14, "LedgerTest > reconcile() FAILED", LineTone::Error),
                        toned(16, "3 tests completed, 1 failed", LineTone::Error),
                        toned(17, "BUILD FAILED in 22s", LineTone::Error),
                    ],
                    18,
                    1,
                );
            }
            ok(
                vec![
                    echo_l,
                    line(3, "> Task :app:test"),
                    toned(18, "BUILD SUCCESSFUL in 30s", LineTone::Success),
                ],
                19,
            )
        }
        other => fail(
            vec![
                echo_l,
                toned(
                    1,
                    &format!("Task '{other}' not found in root project."),
                    LineTone::Error,
                ),
            ],
            2,
            1,
        ),
    }
}

fn task_runner(w: &World, c: &Command) -> Script {
    let echo_l = echo(c);
    let name = match c.program.as_str() {
        "pnpm" | "yarn" | "bun" | "npm" => arg(c, 1).to_owned(),
        _ => arg(c, 0).to_owned(),
    };
    if w.task_failures.iter().any(|f| f == &name) {
        return fail(
            vec![
                echo_l,
                toned(4, &format!("{name}: exit status 1"), LineTone::Error),
            ],
            5,
            1,
        );
    }
    match c.program.as_str() {
        "pnpm" | "yarn" | "bun" | "npm" => ok(
            vec![echo_l, line(1, &format!("> {name}")), line(6, "done")],
            7,
        ),
        "just" => ok(
            vec![
                echo_l,
                line(1, &format!("just: running recipe {name}")),
                line(4, "done"),
            ],
            5,
        ),
        "make" => ok(
            vec![
                echo_l,
                line(1, &format!("make: entering target {name}")),
                line(5, "done"),
            ],
            6,
        ),
        _ => ok(
            vec![
                echo_l,
                line(1, &format!("task: [{name}] running")),
                line(4, "done"),
            ],
            5,
        ),
    }
}

fn gh(w: &World, c: &Command) -> Script {
    let echo_l = echo(c);
    if !w.github.logged_in {
        return fail(
            vec![
                echo_l,
                toned(
                    1,
                    "To get started with GitHub CLI, please run:  gh auth login",
                    LineTone::Error,
                ),
            ],
            2,
            4,
        );
    }
    match (arg(c, 0), arg(c, 1)) {
        ("repo", "clone") => {
            let spec = arg(c, 2).to_owned();
            let dest = arg(c, 3).to_owned();
            let (owner, name) = spec.split_once('/').unwrap_or(("", &spec));
            let known = w
                .github
                .repos
                .iter()
                .any(|r| r.owner == owner && r.name == name);
            if !known {
                return fail(
                    vec![
                        echo_l,
                        toned(
                            1,
                            &format!(
                                "GraphQL: Could not resolve to a Repository with the name '{spec}'. (repository)"
                            ),
                            LineTone::Error,
                        ),
                    ],
                    2,
                    1,
                );
            }
            if w.fs.exists(&dest) {
                return fail(
                    vec![
                        echo_l,
                        toned(
                            1,
                            &format!(
                                "fatal: destination path '{dest}' already exists and is not an empty directory."
                            ),
                            LineTone::Error,
                        ),
                    ],
                    2,
                    128,
                );
            }
            ok(
                vec![
                    echo_l,
                    line(2, &format!("Cloning into '{dest}'...")),
                    line(6, "remote: Enumerating objects: 2410, done."),
                    toned(
                        12,
                        "Resolving deltas: 100% (1188/1188), done.",
                        LineTone::Success,
                    ),
                ],
                13,
            )
        }
        _ => ok(vec![echo_l], 1),
    }
}

fn find(w: &World, c: &Command) -> Script {
    let echo_l = echo(c);
    let dir = arg(c, 0).to_owned();
    if !w.fs.is_dir(&dir) {
        return fail(
            vec![
                echo_l,
                toned(
                    1,
                    &format!("find: {dir}: No such file or directory"),
                    LineTone::Error,
                ),
            ],
            2,
            1,
        );
    }
    let n = w.fs.subtree(&dir).len();
    ok(
        vec![echo_l, line(3, &format!("{n} entries removed below {dir}"))],
        4,
    )
}

fn opener(w: &World, c: &Command) -> Script {
    let echo_l = echo(c);
    let p = &w.platform;
    if p.opener.as_deref() != Some(c.program.as_str()) {
        return Script::spawn_failed(&format!("{}: command not found", c.program));
    }
    if let Some(e) = &p.opener_fails {
        return fail(vec![echo_l, toned(1, e, LineTone::Error)], 2, 1);
    }
    let target = c.args.last().cloned().unwrap_or_default();
    if !w.fs.exists(&target) {
        return fail(
            vec![
                echo_l,
                toned(
                    1,
                    &format!("The file {target} does not exist."),
                    LineTone::Error,
                ),
            ],
            2,
            1,
        );
    }
    ok(vec![echo_l, line(1, &format!("opened {target}"))], 2)
}

fn apt(w: &World, c: &Command) -> Script {
    let echo_l = echo(c);
    let Some(a) = &w.apt else {
        return Script::spawn_failed("apt-get: command not found");
    };
    match arg(c, 0) {
        "update" => ok(
            vec![
                echo_l,
                line(3, "Hit:1 http://deb.debian.org/debian trixie InRelease"),
                line(6, "Reading package lists… Done"),
            ],
            7,
        ),
        "upgrade" => ok(
            vec![
                echo_l,
                line(
                    2,
                    &format!(
                        "{} upgraded, 0 newly installed, 0 to remove and 0 not upgraded.",
                        a.upgradable.len()
                    ),
                ),
                line(12, "Processing triggers for man-db …"),
            ],
            13,
        ),
        "autoremove" => ok(
            vec![
                echo_l,
                line(
                    2,
                    "0 upgraded, 0 newly installed, 2 to remove and 0 not upgraded.",
                ),
            ],
            3,
        ),
        _ => ok(vec![echo_l], 1),
    }
}
