use crate::domain::activity::{Branch, LineTone, Script, line, prompt, toned};
use crate::domain::context::{Host, HostRole, Location, Os, Project, ProjectKind};
use crate::domain::custom::{Origin, TrustStore, parse_config};
use crate::domain::stack::*;
use crate::scenario::EPOCH_SECS;
use crate::scenario::{Motion, Scenario};
use crate::sim::fs::BLOCK;
use crate::sim::world::{Source, World};
use std::collections::BTreeSet;

const HOME: &str = "/Users/alex";
const LHOME: &str = "/home/alex";

fn mac() -> Host {
    Host {
        name: "mbp".into(),
        role: HostRole::Local,
        os: Os::MacOs,
        user: "alex".into(),
        remote: false,
    }
}

fn linux() -> Host {
    Host {
        name: "devbox".into(),
        role: HostRole::Development,
        os: Os::Debian,
        user: "alex".into(),
        remote: true,
    }
}

fn loc(cwd: &str, home: &str, project: Option<Project>) -> Location {
    Location {
        cwd: cwd.into(),
        home: home.into(),
        project,
        workspace: None,
        children: vec![],
    }
}

fn rust_project(root: &str) -> Project {
    Project {
        name: root.rsplit('/').next().unwrap_or("project").into(),
        root: root.into(),
        kind: ProjectKind::Rust {
            workspace: false,
            member: None,
        },
    }
}

fn base(scenario: Scenario, motion: Motion, host: Host, location: Location) -> World {
    let mut w = crate::domain::fixtures::base_world(scenario, motion, host, location);
    w.sources = vec![
        Source {
            name: "filesystem".into(),
            done_at: if motion == Motion::Reduced { 0 } else { 2 },
            failure: None,
        },
        Source {
            name: "git".into(),
            done_at: if motion == Motion::Reduced { 0 } else { 4 },
            failure: None,
        },
        Source {
            name: "system".into(),
            done_at: if motion == Motion::Reduced { 0 } else { 3 },
            failure: None,
        },
    ];
    w
}

fn src(name: &str, at: u64, motion: Motion) -> Source {
    Source {
        name: name.into(),
        done_at: if motion == Motion::Reduced { 0 } else { at },
        failure: None,
    }
}

fn failed(name: &str, at: u64, why: &str, motion: Motion) -> Source {
    Source {
        name: name.into(),
        done_at: if motion == Motion::Reduced { 0 } else { at },
        failure: Some(why.into()),
    }
}

pub fn world_for(scenario: Scenario, motion: Motion) -> World {
    match scenario {
        Scenario::ParityDiscovery => discovery(motion),
        Scenario::ParityHistory => history(motion),
        Scenario::ParityFiles => files(motion),
        Scenario::ParityBrowser => browser(motion),
        Scenario::ParityGitCurrent => git_current(motion),
        Scenario::ParityGitBatch => git_batch(motion),
        Scenario::ParityTaskSources => task_sources(motion),
        Scenario::ParityCargo => cargo(motion),
        Scenario::ParityDocker => docker(motion),
        Scenario::ParityBrewServices => brew_services(motion),
        Scenario::ParityGradle => gradle(motion),
        Scenario::ParityIdea => idea(motion),
        Scenario::ParityUpgradeManagers => upgrade_managers(motion),
        Scenario::ParityExecutor => executor(motion),
        Scenario::ParityTaskInput => task_input(motion),
        Scenario::ParityCustomActions => custom_actions(motion),
        Scenario::ParityDiskScan => disk_scan(motion),
        Scenario::ParityDiskNavigation => disk_navigation(motion),
        Scenario::ParityInsights => insights(motion),
        Scenario::ParityDeleteSafety => delete_safety(motion),
        Scenario::ParityCleanupResults => cleanup_results(motion),
        Scenario::ParityPlatforms => platforms_mac(motion),
        Scenario::ParityPlatformsLinux => platforms_linux(motion),
        other => panic!("{other:?} is not a parity scenario"),
    }
}

fn discovery(motion: Motion) -> World {
    let root = format!("{HOME}/work/probe");
    let mut w = base(
        Scenario::ParityDiscovery,
        motion,
        mac(),
        loc(&root, HOME, Some(rust_project(&root))),
    );

    w.tools = [
        "git", "cargo", "docker", "mise", "just", "make", "task", "brew", "gradle", "idea", "npm",
        "ssh", "kill", "ps", "lsof", "less", "find", "sh", "open", "mdfind", "psql", "lazygit",
        "trash", "gh",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect::<BTreeSet<_>>();
    let mut g = GitState::clean(&root, "main", "main");
    g.git_file = true;
    g.behind = 1;
    w.git = vec![g];
    w.mise.installed = true;
    w.mise.version = "2026.9.3".into();
    w.mise.configs = vec![MiseConfig {
        path: format!("{root}/mise.toml"),
        trusted: true,
        env: vec![],
        tools: vec![],
        tasks: vec![MiseTask {
            name: "test".into(),
            namespaced: "test".into(),
            dir: root.clone(),
            run: "cargo test".into(),
            depends: vec![],
            description: "tests".into(),
        }],
    }];
    w.docker = DockerState::unavailable(
        "Cannot connect to the Docker daemon at unix:///var/run/docker.sock",
    );
    w.brew = Some(BrewState {
        list_json: Ok(r#"[{"name":"redis","status":"started"}]"#.into()),
        services: vec![BrewService {
            name: "redis".into(),
            status: "started".into(),
        }],
        cache: None,
        cache_text: None,
        cache_write_fails: false,
        linux: false,
        failing: vec![],
    });
    w.gradle.wrapper = false;
    w.fs.text(
        &format!("{root}/package.json"),
        r#"{"scripts":{"dev":"vite","build":"vite build"}}"#,
        3,
    );
    w.fs.text(&format!("{root}/justfile"), "build:\n  cargo build\n", 3);
    w.outputs.just_summary = Some(Ok("build test".into()));
    w.fs.text(
        &format!("{root}/Makefile"),
        "all: build\nbuild:\n\tcargo build\n",
        3,
    );
    w.fs.text(&format!("{root}/Taskfile.yml"), "version: '3'\n", 3);
    w.outputs.task_list = Some(Ok(r#"{"tasks":[{"name":"lint"}]}"#.into()));
    w.fs.text(&format!("{root}/build.gradle"), "apply plugin: 'java'\n", 3);
    w.fs.dir(&format!("{root}/.idea"), 3);

    let toml = "[[action]]\nid = \"git.pull\"\nlabel = \"Collides\"\ncommand = [\"true\"]\ndanger = \"safe\"\n\n[[action]]\nid = \"deploy\"\nlabel = \"Deploy\"\ncommand = [\"tools/deploy.sh\"]\ndanger = \"mutating\"\n\n[[action]]\nid = \"broken\"\nlabel = \"\"\ncommand = \"rm -rf\"\ndanger = \"safe\"\n";
    let path = format!("{root}/.holla.toml");
    w.fs.text(&path, toml, 1);
    w.custom_project = Some(parse_config(
        &path,
        Origin::Project,
        toml,
        &["git.pull"],
        &[],
    ));
    w.sources = vec![
        src("filesystem", 2, motion),
        src("git", 40, motion),
        src("mise", 6, motion),
        src("cargo", 8, motion),
        failed(
            "docker",
            12,
            "Cannot connect to the Docker daemon at unix:///var/run/docker.sock",
            motion,
        ),
        src("system", 3, motion),
        src("brew", 30, motion),
        src("ssh", 5, motion),
        failed(
            "github",
            20,
            "gh: not logged in · run gh auth login",
            motion,
        ),
    ];
    w.github.logged_in = false;
    w
}

fn history(motion: Motion) -> World {
    let root = format!("{HOME}/work/holla");
    let mut w = base(
        Scenario::ParityHistory,
        motion,
        mac(),
        loc(&root, HOME, Some(rust_project(&root))),
    );
    w.git = vec![GitState::clean(&root, "main", "main")];
    w.mise = crate::domain::fixtures::mise_holla_pub(true);
    let now = EPOCH_SECS;
    let mut usage = crate::domain::usage::UsageStore::default();

    usage.seed("cargo.test", Some(&root), "mbp", 3, now - 600);
    usage.seed("cargo.build", Some(&root), "mbp", 3, now - 600);
    usage.seed("cargo.check", Some(&root), "mbp", 1, now - 80 * 86_400);

    usage.seed("git.pull", Some(&root), "mbp", 25, now - 10 * 86_400);

    usage.learn_query("pull", "git.fetch", "mbp", now - 3_600);
    usage.learn_query("Ünï  Code", "cargo.clippy", "mbp", now - 3_600);

    usage.seed("gone.action", Some(&root), "mbp", 1, now - 95 * 86_400);
    usage.learn_query("gone", "gone.action", "mbp", now - 95 * 86_400);
    w.memory.usage = usage;

    let mut other = crate::domain::usage::UsageStore::default();
    other.seed("system.resources", None, "mbp", 2, now - 100);
    other.seed("git.pull", Some(&root), "mbp", 1, now - 50);
    w.persisted.frecency = Some(other.serialize());
    w.sources = vec![
        src("filesystem", 2, motion),
        src("git", 3, motion),
        src("mise", 4, motion),
        src("cargo", 5, motion),
        src("system", 3, motion),
    ];
    w
}

fn files(motion: Motion) -> World {
    let root = format!("{HOME}/work/notes");
    let mut w = base(Scenario::ParityFiles, motion, mac(), loc(&root, HOME, None));
    let fs = &mut w.fs;
    fs.text(&format!("{HOME}/.ignore"), "scratch\nnode_modules\n", 30);
    fs.text(&format!("{HOME}/work/notes/todo.md"), "- ship it\n", 1);
    fs.text(&format!("{HOME}/work/notes/README.md"), "notes readme\n", 9);
    fs.text(&format!("{HOME}/work/app/README.md"), "app readme\n", 9);
    fs.text(&format!("{HOME}/work/app/src/main.rs"), "fn main() {}\n", 2);
    fs.text(
        &format!("{HOME}/work/app/node_modules/x/index.js"),
        "x\n",
        2,
    );
    fs.text(&format!("{HOME}/scratch/tmp.txt"), "tmp\n", 2);
    fs.text(&format!("{HOME}/Documents/café menu.txt"), "☕\n", 2);
    fs.text(&format!("{HOME}/Documents/東京.md"), "東京\n", 2);
    fs.dir(&format!("{HOME}/work/app/target"), 5);
    fs.symlink(
        &format!("{HOME}/work/link-to-app"),
        &format!("{HOME}/work/app"),
    );
    fs.text(
        &format!("{HOME}/Library/Mobile Documents/com~apple~CloudDocs/cloud-only.txt"),
        "c\n",
        2,
    );
    fs.text(&format!("{HOME}/.hidden-config"), "h\n", 2);
    w.platform.osc52_limit = 64;
    w.sources = vec![
        src("filesystem", 2, motion),
        src("index", 30, motion),
        src("system", 3, motion),
    ];
    w
}

fn browser(motion: Motion) -> World {
    let root = format!("{HOME}/work/site");
    let mut w = base(
        Scenario::ParityBrowser,
        motion,
        mac(),
        loc(&root, HOME, None),
    );
    let fs = &mut w.fs;
    fs.dir(&root, 1);
    fs.text(
        &format!("{root}/index.html"),
        "<html>\n<body>hello</body>\n</html>\n",
        1,
    );
    fs.text(&format!("{root}/.env"), "SECRET=1\n", 1);
    fs.text(&format!("{root}/empty.txt"), "", 1);
    fs.bytes(
        &format!("{root}/logo.png"),
        vec![0x89, 0x50, 0x4e, 0x47, 0, 1],
        3,
    );
    fs.bytes(&format!("{root}/bad.txt"), vec![0x68, 0xff, 0x69], 3);
    fs.text(
        &format!("{root}/control.txt"),
        "a\tb\u{1b}[31mred\u{7}\n",
        3,
    );
    let big: String = (0..2100).map(|i| format!("line {i}\n")).collect();
    fs.text(&format!("{root}/big.log"), &big, 2);
    let long = format!("{}\n", "x".repeat(5000));
    fs.text(&format!("{root}/long.txt"), &long, 2);
    fs.dir(&format!("{root}/assets"), 4);
    for i in 0..2100 {
        fs.file(&format!("{root}/assets/img{i:04}.png"), 2048, 4);
    }
    fs.symlink(&format!("{root}/current"), &format!("{root}/index.html"));
    fs.symlink(&format!("{root}/broken"), &format!("{root}/missing"));
    fs.fifo(&format!("{root}/pipe"));
    fs.dir(&format!("{root}/private"), 7);
    fs.unreadable(&format!("{root}/private"));
    fs.text(&format!("{root}/caf\u{e9}.txt"), "precomposed\n", 1);
    fs.text(&format!("{root}/cafe\u{301}.txt"), "decomposed\n", 1);
    fs.text(
        &format!("{root}/package.json"),
        r#"{"scripts":{"a":"1","b":"2","c":"3","d":"4","e":"5","f":"6","g":"7","h":"8","i":"9"}}"#,
        2,
    );
    fs.text(&format!("{root}/pnpm-lock.yaml"), "lockfileVersion: 9\n", 2);
    fs.dir(&format!("{root}/.git"), 2);
    w.fs_latency = vec![(format!("{root}/assets"), 8)];
    w.tools.insert("npm".into());
    w.tools.insert("pnpm".into());
    w.sources = vec![src("filesystem", 2, motion), src("system", 3, motion)];
    w
}

fn git_current(motion: Motion) -> World {
    let root = format!("{HOME}/work/svc");
    let sub = format!("{root}/src/api");
    let mut w = base(
        Scenario::ParityGitCurrent,
        motion,
        mac(),
        loc(&sub, HOME, Some(rust_project(&root))),
    );
    let mut g = GitState::clean(&root, "feature/x", "main");
    g.behind = 2;
    g.ahead = 1;
    g.diverged = true;
    g.unstaged = 1;
    g.modified = vec!["src/api/mod.rs".into()];
    g.pull_config = "merge";
    g.push_rejected = Some("fetch first".into());
    g.git_file = true;
    w.git = vec![g];
    w.fs.dir(&sub, 1);
    w.sources = vec![
        src("filesystem", 2, motion),
        src("git", 4, motion),
        src("cargo", 5, motion),
        src("system", 3, motion),
    ];
    w
}

fn git_batch(motion: Motion) -> World {
    let root = format!("{HOME}/work/repos");
    let mut w = base(
        Scenario::ParityGitBatch,
        motion,
        mac(),
        loc(
            &root,
            HOME,
            Some(Project {
                name: "repos".into(),
                root: root.clone(),
                kind: ProjectKind::Collection,
            }),
        ),
    );
    let mut a = GitState::clean(&format!("{root}/alpha"), "main", "main");
    a.behind = 3;
    a.ahead = 1;
    a.remote_urls
        .push(("gitlab".into(), "git@gitlab.com:acme/alpha.git".into()));
    a.merged = vec![
        "feature/done".into(),
        "hotfix/1".into(),
        "+ feature/wt".into(),
        "main".into(),
    ];
    a.occupied = vec!["feature/wt".into()];
    let mut b = GitState::clean(&format!("{root}/beta"), "develop", "develop");
    b.default_from = Some("main");
    b.command_failure = None;
    b.merged = (0..35).map(|i| format!("merged/{i:02}")).collect();
    let mut c = GitState::clean(&format!("{root}/gamma"), "main", "main");
    c.upstream = None;
    c.default_from = None;
    let mut d = GitState::clean(&format!("{root}/delta"), "main", "main");
    d.command_failure = Some("index file corrupt".into());

    let e = GitState::clean(&format!("{root}/nested/alpha"), "main", "main");
    w.git = vec![a, b, c, d, e];
    w.location.children = vec!["alpha", "beta", "gamma", "delta"]
        .into_iter()
        .map(|n| Project {
            name: n.into(),
            root: format!("{root}/{n}"),
            kind: ProjectKind::Service,
        })
        .collect();
    w.sources = vec![
        src("filesystem", 2, motion),
        src("git", 4, motion),
        src("children", 6, motion),
        src("system", 3, motion),
    ];
    w
}

fn task_sources(motion: Motion) -> World {
    let root = format!("{HOME}/work/poly");
    let mut w = base(
        Scenario::ParityTaskSources,
        motion,
        mac(),
        loc(
            &root,
            HOME,
            Some(Project {
                name: "poly".into(),
                root: root.clone(),
                kind: ProjectKind::Node { manager: "yarn" },
            }),
        ),
    );
    for t in ["just", "make", "task", "npm", "yarn", "pnpm", "bun"] {
        w.tools.insert(t.into());
    }
    w.tools.remove("mise");
    w.mise.installed = false;
    let fs = &mut w.fs;
    let many: Vec<String> = (0..31)
        .map(|i| format!("\"s{i:02}\": \"echo {i}\""))
        .collect();
    fs.text(&format!("{root}/package.json"), &format!("{{\"scripts\":{{\"dev\":\"vite\",\"weird key\":\"x\",\"ünï\":\"y\",\"it's; rm\":\"z\",{}}}}}", many.join(",")), 2);
    fs.text(&format!("{root}/yarn.lock"), "# yarn\n", 2);
    fs.text(&format!("{root}/bun.lockb"), "", 2);
    fs.text(&format!("{root}/.justfile"), "build:\n  echo\n", 2);
    fs.text(
        &format!("{root}/Makefile"),
        "all: build\nbuild:\n\techo\n%.o: %.c\n\tcc\nVAR := 1\ndeploy-prod:\n\techo\n",
        2,
    );
    fs.text(&format!("{root}/Taskfile.yaml"), "version: 3\n", 2);
    w.outputs.just_summary = Some(Ok("build test  lint build".into()));
    w.outputs.task_list = Some(Err("yaml: line 3: mapping values are not allowed".into()));
    w.task_failures = vec!["s01".into()];
    w.sources = vec![src("filesystem", 2, motion), src("system", 3, motion)];
    w
}

fn cargo(motion: Motion) -> World {
    let root = format!("{HOME}/work/engine");
    let mut w = base(
        Scenario::ParityCargo,
        motion,
        mac(),
        loc(&root, HOME, Some(rust_project(&root))),
    );
    w.git = vec![GitState::clean(&root, "main", "main")];
    w.cargo.clippy_warnings = 2;
    w.cargo.test_failures = 1;
    w.cargo.target_shared_with = Some(format!("{HOME}/work/engine-wt"));
    w.fs.dir(&format!("{root}/target/debug"), 3);
    w.fs.file(&format!("{root}/target/debug/engine"), 900 * 1024 * 1024, 3);
    w.sources = vec![
        src("filesystem", 2, motion),
        src("git", 4, motion),
        src("cargo", 5, motion),
        src("system", 3, motion),
    ];
    w
}

fn docker(motion: Motion) -> World {
    let root = format!("{HOME}/work/stack");
    let mut w = base(
        Scenario::ParityDocker,
        motion,
        mac(),
        loc(&root, HOME, None),
    );
    w.docker = crate::domain::fixtures::docker_acme_pub();
    w.docker.compose = Some(Compose {
        file: format!("{root}/compose.yaml"),
        dir: root.clone(),
        name: "acme".into(),
        services: vec!["db".into(), "api".into(), "worker".into()],
    });
    w.docker.fail_stage = Some("stop".into());
    w.plans = vec![crate::domain::fixtures::plan_docker_cleanup(&w)];
    w.sources = vec![
        src("filesystem", 2, motion),
        src("docker", 5, motion),
        src("system", 3, motion),
    ];
    w
}

fn brew_services(motion: Motion) -> World {
    let root = format!("{HOME}/work");
    let mut w = base(
        Scenario::ParityBrewServices,
        motion,
        mac(),
        loc(&root, HOME, None),
    );
    let names: Vec<String> = (0..11).map(|i| format!("svc{i:02}")).collect();
    let json = format!(
        "{{\"services\":[{}]}}",
        names
            .iter()
            .map(|n| format!("{{\"name\":\"{n}\",\"status\":\"started\"}}"))
            .chain([
                "{\"status\":\"none\"}".to_owned(),
                "{\"name\":\"\"}".to_owned()
            ])
            .collect::<Vec<_>>()
            .join(",")
    );
    w.brew = Some(BrewState {
        list_json: Ok(json),
        services: names
            .iter()
            .enumerate()
            .map(|(i, n)| BrewService {
                name: n.clone(),
                status: if i % 3 == 0 {
                    "started".into()
                } else if i % 3 == 1 {
                    "stopped".into()
                } else {
                    "error".into()
                },
            })
            .collect(),
        cache: Some(BrewCache {
            version: 1,
            fetched_at: EPOCH_SECS - 400,
            services: vec!["stale-one".into()],
        }),
        cache_text: Some("{\"version\":1}".into()),
        cache_write_fails: true,
        linux: false,
        failing: vec![(
            "svc02".into(),
            "stop".into(),
            "Bootstrap failed: 5: Input/output error".into(),
        )],
    });
    w.sources = vec![
        src("filesystem", 2, motion),
        src("brew", 4, motion),
        src("system", 3, motion),
    ];
    w
}

fn gradle(motion: Motion) -> World {
    let root = format!("{HOME}/work/android");
    let mut w = base(
        Scenario::ParityGradle,
        motion,
        mac(),
        loc(
            &root,
            HOME,
            Some(Project {
                name: "android".into(),
                root: root.clone(),
                kind: ProjectKind::Gradle,
            }),
        ),
    );
    w.tools.insert("gradle".into());
    w.gradle = GradleState {
        daemon_running: true,
        stop_fails: None,
        build_fails: true,
        test_fails: false,
        wrapper: true,
    };
    let fs = &mut w.fs;
    fs.text(&format!("{root}/build.gradle.kts"), "plugins {}\n", 3);
    fs.dir(&format!("{root}/build"), 3);
    fs.file(&format!("{root}/build/out.apk"), 40 * 1024 * 1024, 3);
    fs.dir(&format!("{root}/.gradle"), 3);
    fs.file(&format!("{root}/.gradle/cache.bin"), 9 * 1024 * 1024, 3);
    fs.file(&format!("{root}/app/build/classes/A.class"), 4 * 1024, 3);
    fs.file(&format!("{root}/a/b/c/d/e/build/deep.txt"), 1024, 3);
    fs.file(&format!("{root}/a/b/c/d/e/f/build/too-deep.txt"), 1024, 3);
    fs.file(&format!("{root}/node_modules/pkg/build/x"), 1024, 3);
    fs.symlink(&format!("{root}/linked"), &format!("{HOME}/work/other"));
    fs.file(&format!("{HOME}/work/other/build/y"), 1024, 3);
    w.sources = vec![
        src("filesystem", 2, motion),
        src("gradle", 4, motion),
        src("system", 3, motion),
    ];
    w
}

fn idea(motion: Motion) -> World {
    let root = format!("{HOME}/work/ide");
    let mut w = base(Scenario::ParityIdea, motion, mac(), loc(&root, HOME, None));
    let fs = &mut w.fs;
    fs.dir(&format!("{root}/.idea"), 3);
    fs.file(&format!("{root}/.idea/workspace.xml"), 20_000, 3);
    fs.file(&format!("{root}/app.iml"), 2_000, 3);
    fs.file(&format!("{root}/Upper.IML"), 2_000, 3);
    fs.dir(&format!("{root}/.idea/nested/.idea"), 3);
    fs.file(&format!("{root}/mod/mod.iml"), 1_000, 3);
    fs.file(&format!("{root}/node_modules/x/.idea/x.xml"), 1_000, 3);
    fs.file(&format!("{root}/a/b/c/d/e/deep.iml"), 1_000, 3);
    fs.file(&format!("{root}/a/b/c/d/e/f/g/too.iml"), 1_000, 3);
    fs.symlink(&format!("{root}/link"), &format!("{HOME}/work/elsewhere"));
    fs.file(&format!("{HOME}/work/elsewhere/x.iml"), 1_000, 3);
    fs.dir(&format!("{root}/locked"), 3);
    fs.file(&format!("{root}/locked/l.iml"), 1_000, 3);
    fs.unreadable(&format!("{root}/locked"));
    w.ops_log.write_failure = Some("EROFS".into());
    w.sources = vec![src("filesystem", 2, motion), src("system", 3, motion)];
    w
}

fn upgrade_managers(motion: Motion) -> World {
    let mut w = base(
        Scenario::ParityUpgradeManagers,
        motion,
        mac(),
        loc(HOME, HOME, None),
    );
    w.mise.installed = true;
    w.mise.version = "2026.9.3".into();
    w.mise.global_tools = vec![
        MiseTool {
            name: "node".into(),
            requested: "26".into(),
            active: Some("26.0.1".into()),
            latest: "26.1.0".into(),
            installed: true,
            global: true,
        },
        MiseTool {
            name: "python".into(),
            requested: "3.13".into(),
            active: Some("3.13.5".into()),
            latest: "3.13.7".into(),
            installed: true,
            global: true,
        },
    ];
    w.brew = Some(BrewState {
        list_json: Ok("[]".into()),
        services: vec![],
        cache: None,
        cache_text: None,
        cache_write_fails: false,
        linux: false,
        failing: vec![],
    });
    w.upgrade = UpgradeState {
        zsh_env: Some(format!("{HOME}/.config/omz")),
        amp: true,
        failing: vec!["brew doctor".into()],
        brew_outdated: vec![
            ("node".into(), "26.0.1".into(), "26.1.0".into()),
            ("git".into(), "2.47.0".into(), "2.47.1".into()),
        ],
        casks_outdated: vec![("docker".into(), "4.40".into(), "4.41".into())],
    };
    w.sources = vec![
        src("filesystem", 2, motion),
        src("mise", 4, motion),
        src("brew", 5, motion),
        src("system", 3, motion),
    ];
    w
}

fn executor(motion: Motion) -> World {
    let root = format!("{HOME}/work/batch");
    let mut w = base(
        Scenario::ParityExecutor,
        motion,
        mac(),
        loc(
            &root,
            HOME,
            Some(Project {
                name: "batch".into(),
                root: root.clone(),
                kind: ProjectKind::Collection,
            }),
        ),
    );
    let mut a = GitState::clean(&format!("{root}/one"), "main", "main");
    a.behind = 1;
    let mut b = GitState::clean(&format!("{root}/two"), "main", "main");
    b.command_failure = Some("index file corrupt".into());
    let c = GitState::clean(&format!("{root}/three"), "main", "main");
    w.git = vec![a, b, c];
    w.location.children = vec!["one", "two", "three"]
        .into_iter()
        .map(|n| Project {
            name: n.into(),
            root: format!("{root}/{n}"),
            kind: ProjectKind::Service,
        })
        .collect();

    let raw: Vec<u8> = b"first\r\n\x1b[32mgreen\x1b[0m\nbad \xff byte\nlast fragment".to_vec();
    let lines: Vec<_> = crate::domain::activity::split_stream(&raw)
        .into_iter()
        .enumerate()
        .map(|(i, t)| line(i as u64, &t))
        .collect();
    w.scripts
        .insert("./emit-stream".into(), Script::ending(lines, 4, 0));
    let burst: Vec<_> = (0..4500)
        .map(|i| line(i / 300, &format!("burst line {i}")))
        .collect();
    w.scripts
        .insert("./emit-burst".into(), Script::ending(burst, 16, 0));

    let toml = "[[action]]\nid = \"stream\"\nlabel = \"Emit a mixed stream\"\ncommand = [\"./emit-stream\"]\ndanger = \"safe\"\ndescription = \"CRLF, ANSI, invalid UTF-8 and a final fragment\"\n\n[[action]]\nid = \"burst\"\nlabel = \"Emit a burst\"\ncommand = [\"./emit-burst\"]\ndanger = \"safe\"\ndescription = \"4500 lines in sixteen ticks\"\n";
    let path = format!("{root}/.holla.toml");
    w.fs.text(&path, toml, 1);
    let cfg = parse_config(&path, Origin::Project, toml, &["git.pull"], &[]);
    let _ = w.trust.approve(&cfg.digest, &path, &root);
    w.custom_project = Some(cfg);
    w.sources = vec![
        src("filesystem", 2, motion),
        src("git", 4, motion),
        src("children", 5, motion),
        src("system", 3, motion),
    ];
    w
}

fn task_input(motion: Motion) -> World {
    let mut w = base(
        Scenario::ParityTaskInput,
        motion,
        linux(),
        loc(
            "/srv/app",
            LHOME,
            Some(Project {
                name: "app".into(),
                root: "/srv/app".into(),
                kind: ProjectKind::Service,
            }),
        ),
    );
    w.host.role = HostRole::Production;
    w.sudo_cached = false;
    w.tools.insert("systemctl".into());
    w.tools.insert("sudo".into());
    w.tools.insert("journalctl".into());

    w.scripts.insert(
        "./deploy.sh".into(),
        Script::ending(
            vec![
                line(0, "$ ./deploy.sh"),
                line(1, "connecting to release server…"),
                prompt(3, "Password: ", true),
                line(4, "authenticated"),
                prompt(6, "Proceed with rollout? [y/N] ", false),
                line(7, "rolling out"),
            ],
            9,
            0,
        )
        .branches(vec![
            vec![
                Branch {
                    matches: Some("\u{4}".into()),
                    lines: vec![toned(0, "authentication aborted (EOF)", LineTone::Error)],
                    end: Some((1, 1)),
                },
                Branch {
                    matches: None,
                    lines: vec![],
                    end: None,
                },
            ],
            vec![
                Branch {
                    matches: Some("y".into()),
                    lines: vec![],
                    end: None,
                },
                Branch {
                    matches: None,
                    lines: vec![toned(0, "rollout cancelled by operator", LineTone::Warning)],
                    end: Some((1, 2)),
                },
            ],
        ]),
    );
    w.scripts.insert(
        "./stubborn".into(),
        Script::running(vec![line(0, "$ ./stubborn"), line(1, "ignoring SIGTERM…")]).resistant(),
    );
    w.scripts
        .insert("true".into(), Script::ending(vec![line(0, "$ true")], 1, 0));
    let toml = "[[action]]\nid = \"deploy\"\nlabel = \"Deploy the release\"\ncommand = [\"./deploy.sh\"]\ndanger = \"mutating\"\ndescription = \"asks for a password, then for confirmation\"\n\n[[action]]\nid = \"stubborn\"\nlabel = \"Run the stubborn worker\"\ncommand = [\"./stubborn\"]\ndanger = \"safe\"\ndescription = \"ignores SIGTERM\"\n\n[[action]]\nid = \"quick\"\nlabel = \"Quick no-op\"\ncommand = [\"true\"]\ndanger = \"safe\"\n";
    let path = "/srv/app/.holla.toml".to_owned();
    w.fs.text(&path, toml, 1);
    let cfg = parse_config(&path, Origin::Project, toml, &["git.pull"], &[]);
    let _ = w.trust.approve(&cfg.digest, &path, "/srv/app");
    w.custom_project = Some(cfg);
    w.tools.insert("true".into());
    w.sources = vec![
        src("filesystem", 2, motion),
        src("systemd", 3, motion),
        src("system", 3, motion),
    ];
    w
}

fn custom_actions(motion: Motion) -> World {
    let root = format!("{HOME}/work/team");
    let mut w = base(
        Scenario::ParityCustomActions,
        motion,
        mac(),
        loc(&root, HOME, Some(rust_project(&root))),
    );
    w.git = vec![GitState::clean(&root, "main", "main")];
    let global_path = format!("{HOME}/.config/holla/actions.toml");
    let global = "[[action]]\nid = \"notes\"\nlabel = \"Open notes\"\ncommand = [\"open\", \"/Users/alex/Documents/notes.md\"]\ndanger = \"safe\"\nkeywords = [\"notes\", \"docs\"]\n\n[[action]]\nid = \"shared\"\nlabel = \"Global shared id\"\ncommand = [\"true\"]\ndanger = \"safe\"\n";
    w.fs.text(&global_path, global, 10);
    let project_path = format!("{root}/.holla.toml");
    let project = "# team actions\n[[action]]\nid = \"deploy.preview\"\nlabel = \"Deploy preview\"\ncommand = [\"tools/deploy-preview.sh\", \"--env\", \"preview stack\", \"$(whoami)\"]\ndanger = \"mutating\"\ndescription = \"push the current branch to the preview stack\"\nkeywords = [\"deploy\", \"preview\"]\n\n[[action]]\nid = \"wipe\"\nlabel = \"Wipe local caches\"\ncommand = [\"sh\", \"-c\", \"rm -rf .cache && echo done\"]\ndanger = \"destructive\"\nconfirm = true\ngroup = \"Maintenance\"\n\n[[action]]\nid = \"shared\"\nlabel = \"Duplicate of a global id\"\ncommand = [\"true\"]\ndanger = \"safe\"\n\n[[action]]\nid = \"Bad Id\"\nlabel = \"x\"\ncommand = [\"true\"]\ndanger = \"safe\"\n\n[[action]]\nid = \"multi\"\nlabel = \"Multi-line argument\"\ncommand = [\"printf\", \"%s\\n\", \"a\\nb c\"]\ndanger = \"safe\"\n";
    w.fs.text(&project_path, project, 1);
    let g = parse_config(&global_path, Origin::Global, global, &["git.pull"], &[]);
    let reserved: Vec<String> = g.actions.iter().map(|a| a.id.clone()).collect();
    let p = parse_config(
        &project_path,
        Origin::Project,
        project,
        &["git.pull"],
        &reserved,
    );

    let mut trust = TrustStore::load(Some(&format!("{{\"v\":1,\"hashes\":[\"{}\"]}}", p.digest)));
    trust.entries.push(crate::domain::custom::TrustEntry {
        digest: p.digest.clone(),
        path: Some(format!("{HOME}/work/old/.holla.toml")),
        cwd: Some(format!("{HOME}/work/old")),
    });
    w.trust = trust;
    w.custom_global = Some(g);
    w.custom_project = Some(p);
    w.tools.insert("tools/deploy-preview.sh".into());
    w.tools.insert("printf".into());
    w.tools.insert("true".into());
    w.sources = vec![
        src("filesystem", 2, motion),
        src("git", 4, motion),
        src("config", 3, motion),
        src("system", 3, motion),
    ];
    w
}

fn disk_scan(motion: Motion) -> World {
    let root = format!("{HOME}/work/data");
    let mut w = base(
        Scenario::ParityDiskScan,
        motion,
        mac(),
        loc(&root, HOME, None),
    );
    w.disk.scan_ticks = 48;
    let fs = &mut w.fs;
    fs.sparse(&format!("{root}/sparse.img"), 50 * BLOCK, 3 * BLOCK, 2);
    fs.file(&format!("{root}/media/a.mov"), 12 * BLOCK, 20);
    fs.hardlink(
        &format!("{root}/media/a-copy.mov"),
        &format!("{root}/media/a.mov"),
    );
    fs.symlink(&format!("{root}/media/link"), &format!("{HOME}/Documents"));
    fs.dir(&format!("{root}/.hidden"), 2);
    fs.file(&format!("{root}/.hidden/h.bin"), 4 * BLOCK, 2);
    fs.dir(&format!("{root}/locked"), 2);
    fs.file(&format!("{root}/locked/secret.bin"), 6 * BLOCK, 2);
    fs.unreadable(&format!("{root}/locked"));
    fs.dir(&format!("{root}/cloud"), 2);
    fs.file(&format!("{root}/cloud/evicted.bin"), 8 * BLOCK, 2);
    fs.dataless(&format!("{root}/cloud"));
    fs.dir(&format!("{root}/skip-me"), 2);
    fs.file(&format!("{root}/skip-me/s.bin"), 7 * BLOCK, 2);
    for i in 0..60 {
        fs.file(&format!("{root}/many/f{i:03}.bin"), BLOCK * (i % 5 + 1), 5);
    }

    let mut cache = crate::domain::cleanup::SizeCache::default();
    let media = fs
        .get(&format!("{root}/media"))
        .map(|n| n.mtime)
        .unwrap_or(0);
    cache.entries.push(crate::domain::cleanup::CacheEntry {
        path: format!("{root}/media"),
        allocated: 999 * BLOCK,
        mtime: media,
        recorded_secs: EPOCH_SECS - 3 * 86_400,
    });
    cache.entries.push(crate::domain::cleanup::CacheEntry {
        path: format!("{root}/many"),
        allocated: 5 * BLOCK,
        mtime: 1,
        recorded_secs: EPOCH_SECS - 3 * 86_400,
    });
    cache.entries.push(crate::domain::cleanup::CacheEntry {
        path: format!("{root}/gone"),
        allocated: 5 * BLOCK,
        mtime: 1,
        recorded_secs: EPOCH_SECS - 3 * 86_400,
    });
    cache.entries.push(crate::domain::cleanup::CacheEntry {
        path: format!("{root}/expired"),
        allocated: 5 * BLOCK,
        mtime: 1,
        recorded_secs: EPOCH_SECS - 9 * 86_400,
    });
    w.persisted.sizes = Some(cache.serialize());
    w.size_cache =
        crate::domain::cleanup::SizeCache::load(w.persisted.sizes.as_deref(), EPOCH_SECS);
    w.sources = vec![src("filesystem", 2, motion), src("system", 3, motion)];
    w
}

fn disk_navigation(motion: Motion) -> World {
    let root = format!("{HOME}/work/big");
    let mut w = base(
        Scenario::ParityDiskNavigation,
        motion,
        mac(),
        loc(&root, HOME, Some(rust_project(&root))),
    );
    w.disk.scan_ticks = 30;
    let fs = &mut w.fs;
    fs.text(&format!("{root}/Cargo.toml"), "[package]\n", 30);
    fs.file(
        &format!("{root}/target/debug/deps/lib.rlib"),
        300 * BLOCK,
        20,
    );
    fs.file(&format!("{root}/target/release/bin"), 200 * BLOCK, 20);
    for n in crate::domain::cleanup::NOISE_NAMES {
        fs.file(&format!("{root}/noise/{n}/x.bin"), 2 * BLOCK, 20);
    }
    fs.text(&format!("{root}/noise/package.json"), "{}", 20);
    fs.file(&format!("{root}/src/main.rs"), BLOCK, 1);
    fs.file(&format!("{root}/docs/guide.md"), BLOCK, 1);
    fs.sparse(&format!("{root}/vm.img"), 900 * BLOCK, 40 * BLOCK, 5);
    fs.file(&format!("{HOME}/Movies/big.mov"), 400 * 1024 * 1024, 5);
    fs.file(&format!("{HOME}/Downloads/iso.iso"), 700 * 1024 * 1024, 5);
    fs.file(&format!("{HOME}/Downloads/small.bin"), 50 * 1024 * 1024, 5);
    w.platform.spotlight = Spotlight::Available(vec![
        (format!("{HOME}/Downloads/iso.iso"), 700 * 1024 * 1024),
        (format!("{HOME}/Movies/big.mov"), 400 * 1024 * 1024),
        (format!("{HOME}/Downloads/iso.iso"), 700 * 1024 * 1024),
        (format!("{HOME}/Downloads/vanished.bin"), 300 * 1024 * 1024),
    ]);
    w.sources = vec![
        src("filesystem", 2, motion),
        src("cargo", 3, motion),
        src("system", 3, motion),
    ];
    w
}

fn insights(motion: Motion) -> World {
    let root = format!("{HOME}/Projects/web");
    let mut w = base(
        Scenario::ParityInsights,
        motion,
        mac(),
        loc(
            &root,
            HOME,
            Some(Project {
                name: "web".into(),
                root: root.clone(),
                kind: ProjectKind::Node { manager: "pnpm" },
            }),
        ),
    );
    for t in ["brew", "npm", "pnpm", "yarn", "bun", "cargo", "uv", "pip3"] {
        w.tools.insert(t.into());
    }
    w.brew = Some(BrewState {
        list_json: Ok("[]".into()),
        services: vec![],
        cache: None,
        cache_text: None,
        cache_write_fails: false,
        linux: false,
        failing: vec![],
    });
    let fs = &mut w.fs;
    let mb = 1024 * 1024;
    fs.file(
        &format!("{HOME}/Library/Developer/Xcode/DerivedData/App-abc/x.o"),
        800 * mb,
        3,
    );

    fs.dir(
        &format!("{HOME}/Library/Developer/Xcode/iOS DeviceSupport"),
        91,
    );
    fs.file(
        &format!("{HOME}/Library/Developer/Xcode/iOS DeviceSupport/17.0/x"),
        2000 * mb,
        91,
    );
    fs.dir(
        &format!("{HOME}/Library/Developer/Xcode/watchOS DeviceSupport"),
        89,
    );
    fs.file(
        &format!("{HOME}/Library/Developer/Xcode/watchOS DeviceSupport/10.0/x"),
        500 * mb,
        89,
    );
    fs.file(
        &format!("{HOME}/Library/Developer/Xcode/Archives/2026/app.xcarchive/x"),
        300 * mb,
        40,
    );
    fs.file(
        &format!("{HOME}/Library/Developer/CoreSimulator/Caches/dyld/x"),
        900 * mb,
        2,
    );
    fs.file(
        &format!("{HOME}/Library/Caches/Homebrew/node.tar.gz"),
        100 * mb,
        2,
    );
    fs.file(&format!("{HOME}/.npm/_cacache/index/x"), 200 * mb, 2);
    fs.file(&format!("{HOME}/.npm/_logs/x.log"), mb, 2);
    fs.file(
        &format!("{HOME}/Library/pnpm/store/v3/files/x"),
        2200 * mb,
        31,
    );
    fs.file(&format!("{HOME}/.yarn/cache/x.zip"), 50 * mb, 2);
    fs.file(&format!("{HOME}/.bun/install/cache/x"), 60 * mb, 2);
    fs.file(
        &format!("{HOME}/.cargo/registry/cache/x.crate"),
        4800 * mb,
        29,
    );
    fs.file(&format!("{HOME}/.cargo/git/checkouts/x"), 100 * mb, 40);
    fs.file(
        &format!("{HOME}/.gradle/caches/modules-2/x.jar"),
        1500 * mb,
        45,
    );
    fs.file(&format!("{HOME}/.gradle/daemon/8.5/x.log"), mb, 1);
    fs.file(&format!("{HOME}/.m2/repository/org/x.jar"), 700 * mb, 200);
    fs.file(&format!("{HOME}/Library/Caches/pip/http/x"), 150 * mb, 2);
    fs.file(&format!("{HOME}/.cache/uv/x"), 300 * mb, 2);
    fs.file(
        &format!("{HOME}/Library/Caches/com.old.app/db"),
        400 * mb,
        31,
    );
    fs.file(
        &format!("{HOME}/Library/Caches/com.new.app/db"),
        400 * mb,
        7,
    );
    fs.file(&format!("{HOME}/Library/Logs/old.log"), 20 * mb, 8);
    fs.file(&format!("{HOME}/Library/Logs/fresh.log"), 20 * mb, 6);
    fs.file(
        &format!("{HOME}/Library/Logs/JetBrains/idea.log"),
        30 * mb,
        9,
    );
    fs.text(&format!("{root}/package.json"), "{}", 40);
    fs.file(&format!("{root}/node_modules/x/index.js"), 300 * mb, 40);
    fs.file(&format!("{root}/dist/bundle.js"), 3 * mb, 1);
    fs.text(&format!("{HOME}/Projects/rs/Cargo.toml"), "[package]\n", 40);
    fs.file(&format!("{HOME}/Projects/rs/target/debug/x"), 2000 * mb, 20);
    fs.file(
        &format!("{HOME}/Projects/rs/target/node_modules/nested/x"),
        mb,
        20,
    );
    fs.file(&format!("{HOME}/Projects/plain/build/x"), mb, 20);
    fs.file(&format!("{HOME}/Projects/go/go.mod"), 10, 20);
    fs.file(&format!("{HOME}/Projects/go/build/x"), 10 * mb, 20);
    fs.file(&format!("{HOME}/Projects/py/pyproject.toml"), 10, 20);
    fs.file(&format!("{HOME}/Projects/py/.venv/lib/x"), 200 * mb, 20);
    fs.file(&format!("{HOME}/Projects/py/__pycache__/x.pyc"), mb, 20);
    fs.file(&format!("{HOME}/Projects/jvm/pom.xml"), 10, 20);
    fs.file(&format!("{HOME}/Projects/jvm/target/x.jar"), 10 * mb, 20);
    fs.file(&format!("{HOME}/Projects/kt/build.gradle.kts"), 10, 20);
    fs.file(&format!("{HOME}/Projects/kt/build/x"), 10 * mb, 20);
    fs.file(&format!("{HOME}/Projects/kt/.gradle/x"), 10 * mb, 20);
    fs.file(&format!("{HOME}/Projects/ios/.git/HEAD"), 10, 20);
    fs.file(&format!("{HOME}/Projects/ios/Pods/x"), 10 * mb, 20);
    fs.file(&format!("{HOME}/Projects/ios/DerivedData/x"), 10 * mb, 20);
    fs.file(&format!("{HOME}/Projects/next/package.json"), 10, 20);
    fs.file(&format!("{HOME}/Projects/next/.next/x"), 10 * mb, 20);
    fs.file(&format!("{HOME}/Projects/next/.turbo/x"), 10 * mb, 20);
    fs.file(&format!("{HOME}/Projects/next/venv/x"), 10 * mb, 20);
    fs.file(
        &format!("{HOME}/Projects/deep/a/b/c/d/e/f/package.json"),
        10,
        20,
    );
    fs.file(
        &format!("{HOME}/Projects/deep/a/b/c/d/e/f/node_modules/x"),
        10 * mb,
        20,
    );
    fs.symlink(&format!("{HOME}/Projects/linked"), &format!("{HOME}/work"));
    fs.file(&format!("{HOME}/work/package.json"), 10, 20);
    fs.file(&format!("{HOME}/work/node_modules/x"), 10 * mb, 20);
    fs.dir(&format!("{HOME}/Projects/locked"), 20);
    fs.unreadable(&format!("{HOME}/Projects/locked"));
    fs.file(
        &format!("{HOME}/Library/Developer/Xcode/DerivedData/Recent/x.o"),
        10 * mb,
        -1,
    );
    w.system.top.push(Proc {
        pid: 700,
        name: "Xcode".into(),
        cpu_pct: 2,
        mem_mb: 2000,
        state: "sleeping".into(),
        port: None,
    });
    w.outputs.pnpm_store_path = Some(Ok(format!("{HOME}/Library/pnpm/store/v3")));
    w.gradle.daemon_running = true;
    w.tools.insert("gradle".into());
    w.sources = vec![
        src("filesystem", 2, motion),
        src("insights", 6, motion),
        src("system", 3, motion),
    ];
    w
}

fn delete_safety(motion: Motion) -> World {
    let root = format!("{HOME}/Projects/safe");
    let mut w = base(
        Scenario::ParityDeleteSafety,
        motion,
        mac(),
        loc(&root, HOME, Some(rust_project(&root))),
    );
    let fs = &mut w.fs;
    let mb = 1024 * 1024;
    fs.text(&format!("{root}/Cargo.toml"), "[package]\n", 40);
    fs.file(&format!("{root}/target/debug/x"), 500 * mb, 20);
    fs.file(&format!("{root}/target/debug/y"), 500 * mb, 20);
    fs.file(&format!("{root}/ünï\ncode/build/x"), mb, 20);
    fs.text(&format!("{root}/ünï\ncode/package.json"), "{}", 20);
    fs.symlink("/tmp", "/private/tmp");
    fs.file("/private/tmp/holla-scratch/x", mb, 2);
    fs.symlink(&format!("{root}/linked"), &format!("{HOME}/work/other"));
    fs.file(&format!("{HOME}/work/other/node_modules/x"), mb, 40);
    fs.text(&format!("{HOME}/work/other/package.json"), "{}", 40);
    fs.file(&format!("{HOME}/Library/Caches/com.app/x"), 100 * mb, 40);
    fs.file(
        &format!("{HOME}/Library/Caches/Google/Chrome/x"),
        100 * mb,
        40,
    );
    fs.file(
        &format!("{HOME}/Library/Containers/com.apple.Safari/Data/x"),
        100 * mb,
        40,
    );
    fs.file(
        &format!("{HOME}/Library/Application Support/App/x"),
        100 * mb,
        40,
    );
    fs.file(&format!("{HOME}/.Trash/old"), 100 * mb, 40);
    fs.file(
        &format!("{HOME}/Library/Mobile Documents/com~apple~CloudDocs/x"),
        100 * mb,
        40,
    );
    fs.dir(&format!("{root}/gone-later"), 20);
    fs.file(&format!("{root}/gone-later/x"), mb, 20);
    fs.dir(&format!("{root}/locked"), 20);
    fs.file(&format!("{root}/locked/x"), mb, 20);
    fs.unreadable(&format!("{root}/locked"));
    fs.trash_collisions = 2;
    w.disk.scan_ticks = 20;
    w.sources = vec![
        src("filesystem", 2, motion),
        src("cargo", 3, motion),
        src("system", 3, motion),
    ];
    w
}

fn cleanup_results(motion: Motion) -> World {
    let mut w = delete_safety(motion);
    w.scenario = Scenario::ParityCleanupResults;
    w.ops_log.write_failure = None;
    w.fs.trash_collisions = 0;
    let mb = 1024 * 1024;
    let root = format!("{HOME}/Projects/safe");
    w.fs.file(&format!("{root}/big/blob"), 3000 * mb, 20);
    w.fs.file(&format!("{root}/big/blob-clone"), 3000 * mb, 20);
    if let Some(v) = w.fs.volumes.first_mut() {
        v.purgeable = 20_000 * mb;
    }
    w.ops_log.lines.push(
        crate::domain::cleanup::LogRecord {
            timestamp_ms: (EPOCH_SECS - 12 * 86_400) * 1000,
            mode: crate::domain::cleanup::Mode::Trash,
            dry_run: false,
            path: format!("{HOME}/work/legacy/node_modules"),
            size: 3_900 * mb,
            outcome: crate::domain::cleanup::Outcome::Trashed,
            error: None,
        }
        .json(),
    );
    w.persisted.ops_log = w.ops_log.lines.clone();
    w
}

fn platforms_mac(motion: Motion) -> World {
    let root = format!("{HOME}/work/mac");
    let mut w = base(
        Scenario::ParityPlatforms,
        motion,
        mac(),
        loc(&root, HOME, None),
    );
    w.tools.insert("brew".into());
    w.brew = Some(BrewState {
        list_json: Ok("[{\"name\":\"redis\"}]".into()),
        services: vec![BrewService {
            name: "redis".into(),
            status: "started".into(),
        }],
        cache: None,
        cache_text: None,
        cache_write_fails: false,
        linux: false,
        failing: vec![],
    });
    w.upgrade.casks_outdated = vec![("docker".into(), "4.40".into(), "4.41".into())];
    w.platform.dataless_failure = Some("setiopolicy_np failed: EPERM".into());
    w.platform.spotlight = Spotlight::Timeout;
    w.fs.symlink("/tmp", "/private/tmp");
    w.fs.symlink("/var", "/private/var");
    w.fs.file("/private/tmp/x", BLOCK, 2);
    w.fs.file(&format!("{HOME}/Library/Caches/Homebrew/x"), BLOCK, 2);
    w.sources = vec![
        src("filesystem", 2, motion),
        src("brew", 4, motion),
        src("system", 3, motion),
    ];
    w
}

fn platforms_linux(motion: Motion) -> World {
    let root = format!("{LHOME}/work/box");
    let mut w = base(
        Scenario::ParityPlatformsLinux,
        motion,
        linux(),
        loc(&root, LHOME, None),
    );
    w.host.remote = false;
    w.host.role = HostRole::Local;
    w.tools.insert("brew".into());
    w.tools.remove("xdg-open");
    w.brew = Some(BrewState {
        list_json: Ok("[{\"name\":\"postgresql@17\"}]".into()),
        services: vec![BrewService {
            name: "postgresql@17".into(),
            status: "stopped".into(),
        }],
        cache: None,
        cache_text: None,
        cache_write_fails: false,
        linux: true,
        failing: vec![],
    });
    w.platform = Platform {
        trash: TrashBackend::Unavailable("no ~/.local/share/Trash and no mount helper".into()),
        opener: None,
        opener_fails: None,
        spotlight: Spotlight::Unavailable("Spotlight is macOS only · use the tree scan".into()),
        osc52: false,
        osc52_limit: 0,
        xdg_config_home: format!("{LHOME}/.config"),
        xdg_cache_home: format!("{LHOME}/.cache"),
        subreaper: true,
        process_probe: Err("pgrep: command not found".into()),
        dataless_failure: None,
    };
    w.fs.file(
        &format!("{LHOME}/.cargo/registry/cache/x.crate"),
        100 * BLOCK,
        40,
    );

    w.fs.text(&format!("{LHOME}/Projects/app/package.json"), "{}", 40);
    w.fs.file(
        &format!("{LHOME}/Projects/app/node_modules/x"),
        30 * BLOCK,
        40,
    );
    w.fs.file(&format!("{LHOME}/Library/Caches/Homebrew/x"), BLOCK, 2);
    w.fs.trash_broken = Some("no Trash backend on this host".into());
    w.sources = vec![
        src("filesystem", 2, motion),
        src("brew", 4, motion),
        src("system", 3, motion),
    ];
    w
}
