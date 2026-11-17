use std::collections::BTreeMap;

use crate::domain::activity::{LineTone, Script, line, service, toned};

pub fn scripts() -> BTreeMap<String, Script> {
    let mut m = BTreeMap::new();
    m.insert(
        "task:frontend-dev".into(),
        Script::running(vec![
            line(0, "$ mise run dev  (apps/frontend)"),
            line(
                2,
                "> frontend@2.4.0 dev /Users/alex/work/acme/apps/frontend",
            ),
            line(3, "> vite --host"),
            line(9, ""),
            line(10, "  VITE v6.3.1  ready in 412 ms"),
            line(11, ""),
            line(12, "  ➜  Local:   http://localhost:5173/"),
            line(12, "  ➜  Network: http://192.168.1.24:5173/"),
            line(
                30,
                "12:41:07 [vite] hmr update /src/components/OrderTable.tsx",
            ),
            line(52, "12:41:09 [vite] hmr update /src/routes/orders.tsx"),
            toned(
                80,
                "12:41:12 [vite] warning: circular import src/lib/api.ts",
                LineTone::Warning,
            ),
            line(120, "12:41:15 [vite] hmr update /src/lib/api.ts"),
            line(190, "12:41:21 [vite] page reload src/main.tsx"),
        ]),
    );
    m.insert(
        "task:frontend-dev-fail".into(),
        Script::ending(
            vec![
                line(0, "$ mise run dev  (apps/frontend)"),
                line(
                    2,
                    "> frontend@2.4.0 dev /Users/alex/work/acme/apps/frontend",
                ),
                line(3, "> vite --host"),
                line(9, ""),
                toned(14, "error when starting dev server:", LineTone::Error),
                toned(
                    14,
                    "Error: listen EADDRINUSE: address already in use :::5173",
                    LineTone::Error,
                ),
                line(
                    15,
                    "    at Server.setupListenHandle [as _listen2] (node:net:1908:16)",
                ),
                line(15, "    at listenInCluster (node:net:1965:12)"),
                line(16, "    at Server.listen (node:net:2067:7)"),
                toned(
                    17,
                    " ELIFECYCLE  Command failed with exit code 1.",
                    LineTone::Error,
                ),
                toned(
                    18,
                    "mise ERROR task failed: //apps/frontend:dev exited with 1",
                    LineTone::Error,
                ),
            ],
            19,
            1,
        ),
    );
    m.insert(
        "task:api-dev".into(),
        Script::running(vec![
            line(0, "$ mise run dev  (services/api)"),
            line(1, "[Running 'cargo run -p api']"),
            line(
                4,
                "   Compiling api v0.9.2 (/Users/alex/work/acme/services/api)",
            ),
            line(
                38,
                "    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.91s",
            ),
            line(39, "     Running `target/debug/api`"),
            line(41, "INFO  api: listening on 0.0.0.0:8080"),
            line(
                42,
                "INFO  api: connected to postgres://acme@localhost:5432/acme",
            ),
            line(70, "INFO  api: GET /orders 200 · 12 ms"),
            line(95, "INFO  api: GET /orders/1042 200 · 4 ms"),
            toned(
                140,
                "WARN  api: slow query 1.9 s · select * from orders where …",
                LineTone::Warning,
            ),
            line(200, "INFO  api: POST /orders 201 · 38 ms"),
        ]),
    );
    m.insert(
        "task:tests".into(),
        Script::ending(
            vec![
                line(0, "$ cargo nextest run"),
                line(3, "   Compiling holla v0.3.1 (/Users/alex/work/holla)"),
                line(
                    24,
                    "    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.80s",
                ),
                line(25, "    Starting 128 tests across 3 binaries"),
                line(
                    27,
                    "        PASS [   0.012s] holla ranking::tests::aliases_beat_everything",
                ),
                line(
                    28,
                    "        PASS [   0.010s] holla plan::tests::exclusion_cascades",
                ),
                line(
                    30,
                    "        PASS [   0.041s] holla app_tests::root_opens_with_suggestions",
                ),
                line(
                    31,
                    "        PASS [   0.009s] holla scope::tests::axis_moves_outward",
                ),
                line(
                    33,
                    "        PASS [   0.221s] holla app_tests::two_gate_flow_needs_phrase",
                ),
                line(
                    34,
                    "        PASS [   0.013s] holla activity::tests::scripts_emit_in_order",
                ),
                toned(
                    36,
                    "     Summary [   2.307s] 128 tests run: 128 passed, 0 skipped",
                    LineTone::Success,
                ),
            ],
            37,
            0,
        ),
    );
    m.insert(
        "task:tests-frontend".into(),
        Script::ending(
            vec![
                line(0, "$ mise run //apps/frontend:test"),
                line(
                    1,
                    "[mise] resolving //apps/frontend:test · depends on //apps/frontend:build",
                ),
                line(2, "[mise] //apps/frontend:build"),
                line(5, "> vite build"),
                line(18, "✓ 214 modules transformed."),
                line(19, "dist/index.html  0.46 kB │ gzip: 0.30 kB"),
                line(20, "[mise] //apps/frontend:test"),
                line(21, "> vitest run"),
                line(26, " ✓ src/lib/api.test.ts (12)"),
                line(28, " ✓ src/components/OrderTable.test.tsx (7)"),
                toned(31, " Test Files  2 passed (2)", LineTone::Success),
                toned(31, "      Tests  19 passed (19)", LineTone::Success),
            ],
            32,
            0,
        ),
    );
    m.insert(
        "task:clippy".into(),
        Script::ending(
            vec![
                line(0, "$ cargo clippy --all-targets -- -D warnings"),
                line(2, "    Checking holla v0.3.1 (/Users/alex/work/holla)"),
                toned(20, "warning: unused variable: `scope`", LineTone::Warning),
                line(20, "  --> src/bin/holla/screens/here.rs:412:13"),
                toned(
                    22,
                    "error: could not compile `holla` (bin \"holla\") due to 1 previous error",
                    LineTone::Error,
                ),
            ],
            23,
            101,
        ),
    );
    m.insert(
        "task:setup".into(),
        Script::ending(
            vec![
                line(0, "$ mise run setup  (acme)"),
                line(
                    1,
                    "[mise] installing tools: node@26.0.1 pnpm@10.4.0 rust@1.90.0 python@3.13.7",
                ),
                line(12, "[mise] node@26.0.1 already installed"),
                line(14, "[mise] pnpm@10.4.0 installed"),
                line(16, "> pnpm install --frozen-lockfile"),
                line(30, "Packages: +1842"),
                line(44, "Done in 3.4s"),
                toned(45, "[mise] setup finished", LineTone::Success),
            ],
            46,
            0,
        ),
    );
    m.insert(
        "task:up".into(),
        Script::ending(
            vec![
                line(0, "$ mise run up  (acme)"),
                line(1, "> docker compose up -d db redis"),
                line(6, " Container acme-db-1  Running"),
                line(7, " Container acme-redis-1  Running"),
                toned(
                    8,
                    "[mise] up finished · 2 containers running",
                    LineTone::Success,
                ),
            ],
            9,
            0,
        ),
    );
    m.insert(
        "task:eco".into(),
        Script::running(vec![
            line(0, "$ mise run dev  (acme)"),
            line(
                1,
                "[mise] dev depends on: up, //apps/frontend:dev, //services/api:dev",
            ),
            line(2, "[mise] up · docker compose up -d db redis"),
            line(7, "[mise] up finished"),
            line(
                8,
                "[mise] starting //apps/frontend:dev and //services/api:dev in parallel",
            ),
            line(18, "[frontend]   VITE v6.3.1  ready in 412 ms"),
            line(19, "[frontend]   ➜  Local:   http://localhost:5173/"),
            line(46, "[api] INFO  api: listening on 0.0.0.0:8080"),
        ]),
    );
    m.insert(
        "git:pull".into(),
        Script::ending(
            vec![
                line(0, "$ git -C /Users/alex/work/holla pull --ff-only"),
                line(6, "From github.com:alex-dev/holla"),
                line(6, "   9f2c1aa..c4e77d1  main       -> origin/main"),
                line(9, "Updating 9f2c1aa..c4e77d1"),
                line(10, "Fast-forward"),
                line(10, " src/bin/holla/screens/here.rs | 41 ++++++++-----"),
                line(10, " DESIGN.md                    | 12 +++-"),
                toned(
                    11,
                    " 2 files changed, 38 insertions(+), 15 deletions(-)",
                    LineTone::Success,
                ),
            ],
            12,
            0,
        ),
    );
    m.insert(
        "logs:acme".into(),
        Script::running(vec![
            line(
                0,
                "$ docker compose logs -f --tail 200 api worker scheduler",
            ),
            service(2, "api", "INFO  listening on 0.0.0.0:8080"),
            service(
                3,
                "worker",
                "INFO  worker ready · queue=orders concurrency=4",
            ),
            service(3, "scheduler", "INFO  scheduler tick · 3 jobs due"),
            service(8, "api", "INFO  GET /health 200 · 1 ms"),
            service(14, "worker", "INFO  processed order 1042 · 38 ms"),
            service(20, "api", "WARN  upstream timeout · retry 1 of 3"),
            service(26, "scheduler", "INFO  enqueued nightly-report"),
            service(
                31,
                "api",
                "ERROR health check failed · db pool exhausted (20/20)",
            ),
            service(33, "worker", "INFO  processed order 1043 · 41 ms"),
            service(
                40,
                "api",
                "ERROR health check failed · db pool exhausted (20/20)",
            ),
            service(55, "scheduler", "INFO  scheduler tick · 0 jobs due"),
            service(70, "worker", "WARN  order 1044 retried · lock timeout"),
            service(
                92,
                "api",
                "ERROR health check failed · db pool exhausted (20/20)",
            ),
            service(120, "worker", "INFO  processed order 1044 · 1.2 s"),
        ]),
    );
    m.insert(
        "docker:logs-api".into(),
        Script::running(vec![
            line(0, "$ docker logs -f --since 10m acme-api-1"),
            line(2, "INFO  listening on 0.0.0.0:8080"),
            line(4, "INFO  GET /health 200 · 1 ms"),
            toned(
                9,
                "ERROR health check failed · db pool exhausted (20/20)",
                LineTone::Error,
            ),
            toned(
                18,
                "ERROR health check failed · db pool exhausted (20/20)",
                LineTone::Error,
            ),
            line(25, "INFO  GET /orders 200 · 14 ms"),
            toned(
                40,
                "ERROR health check failed · db pool exhausted (20/20)",
                LineTone::Error,
            ),
        ]),
    );
    m.insert(
        "journal:payments-worker".into(),
        Script::running(vec![
            line(0, "$ journalctl -u payments-worker -f -n 200"),
            line(1, "Sep 10 08:31:02 prod-eu-1 payments-worker[41872]: INFO worker started · queue=settlements"),
            toned(3, "Sep 10 08:33:40 prod-eu-1 payments-worker[41872]: ERROR settlement batch 8812 failed: deadline exceeded", LineTone::Error),
            line(4, "Sep 10 08:33:40 prod-eu-1 systemd[1]: payments-worker.service: Main process exited, code=exited, status=1/FAILURE"),
            line(4, "Sep 10 08:33:45 prod-eu-1 systemd[1]: payments-worker.service: Scheduled restart job, restart counter is at 3."),
            line(6, "Sep 10 08:33:45 prod-eu-1 payments-worker[41903]: INFO worker started · queue=settlements"),
            toned(30, "Sep 10 08:36:12 prod-eu-1 payments-worker[41903]: WARN settlement batch 8813 slow · 41 s", LineTone::Warning),
        ]),
    );
    m.insert(
        "monitor:btm".into(),
        Script::running(vec![
            line(0, "btm 0.11.1 · attached · q quits the monitor, Ctrl+] returns to holla"),
            line(1, "CPU  ▏78%  core0 91  core1 84  core2 70  core3 66  core4 88  core5 79  core6 71  core7 75"),
            line(1, "MEM  ▏12.4 GiB / 16 GiB  swap 1.1 GiB"),
            line(1, "NET  ▏en0  rx 1.2 MiB/s  tx 240 KiB/s"),
            line(1, ""),
            line(1, "  PID   Name              CPU%   Mem     State"),
            line(1, "  4812  rust-analyzer     41%    2.9G    Running"),
            line(1, "  9120  cargo             32%    1.1G    Running"),
            line(1, "  2201  Docker            9%     3.8G    Sleeping"),
            line(1, "  7731  node (vite)       6%     420M    Sleeping"),
        ]),
    );
    m.insert(
        "monitor:pg_activity".into(),
        Script::running(vec![
            line(0, "pg_activity 3.5.1 · acme@localhost:5432/acme · attached · Ctrl+] returns to holla"),
            line(1, "PostgreSQL 17.2 · 24/100 connections · TPS 412 · cache hit 98.7%"),
            line(1, ""),
            line(1, "PID    DATABASE  USER   STATE     WAIT           TIME     QUERY"),
            line(1, "48211  acme      api    active    Lock:tuple     00:04:12 UPDATE orders SET status = …"),
            line(1, "48197  acme      alex   idle txn  ·              00:05:30 (idle in transaction)"),
            line(1, "48230  acme      worker active    Lock:tuple     00:03:58 UPDATE orders SET status = …"),
        ]),
    );
    m.insert(
        "ssh:devbox".into(),
        Script::running(vec![
            line(0, "$ ssh devbox"),
            line(
                4,
                "Linux devbox 6.12.0-1-amd64 #1 SMP Debian 6.12.9-1 x86_64",
            ),
            line(4, ""),
            line(5, "Last login: Wed Sep  9 22:14:03 2026 from 192.168.1.24"),
            line(6, "alex@devbox:~$ "),
        ]),
    );
    m.insert(
        "task:migrate".into(),
        Script::ending(
            vec![
                line(0, "$ mise run migrate  (services/api)"),
                line(2, "> sqlx migrate run"),
                line(
                    6,
                    "Applied 20260901120000/migrate add settlements (12.4 ms)",
                ),
                line(
                    7,
                    "Applied 20260908093000/migrate orders status index (8.1 ms)",
                ),
                toned(8, "[mise] migrate finished", LineTone::Success),
            ],
            9,
            0,
        ),
    );
    m
}

pub fn step_outputs() -> BTreeMap<String, Vec<String>> {
    let mut m: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut put = |k: &str, v: &[&str]| {
        m.insert(k.into(), v.iter().map(|s| (*s).to_owned()).collect());
    };

    put(
        "docker-cleanup/stop",
        &[
            "$ docker stop acme-api-1 acme-worker-1 acme-scheduler-1 acme-db-1 acme-redis-1",
            "acme-api-1",
            "acme-worker-1",
            "acme-scheduler-1",
            "acme-db-1",
            "acme-redis-1",
        ],
    );
    put(
        "docker-cleanup/rm",
        &[
            "$ docker rm acme-api-1 acme-worker-1 acme-scheduler-1 acme-db-1 acme-redis-1 \\",
            "    build-cache-7f2 registry-mirror tmp-shell tmp-shell-2 pgadmin grafana loki",
            "acme-api-1",
            "acme-worker-1",
            "acme-scheduler-1",
            "acme-db-1",
            "acme-redis-1",
            "build-cache-7f2",
            "registry-mirror",
            "tmp-shell",
            "tmp-shell-2",
            "pgadmin",
            "grafana",
            "loki",
        ],
    );
    put(
        "docker-cleanup/images",
        &[
            "$ docker image prune -a --force",
            "Deleted Images:",
            "untagged: acme/api:dev",
            "deleted: sha256:4c1f…9a2e",
            "untagged: postgres:17.2",
            "untagged: redis:7.4-alpine",
            "… 36 more",
            "Total reclaimed space: 9.8GB",
        ],
    );
    put(
        "docker-cleanup/networks",
        &[
            "$ docker network prune --force",
            "Deleted Networks:",
            "acme_default",
            "monitoring",
        ],
    );
    put(
        "docker-cleanup/volumes",
        &[
            "$ docker volume prune -a --force",
            "Deleted Volumes:",
            "acme_pgdata",
            "acme_redisdata",
            "grafana_data",
            "loki_data",
            "pgadmin_data",
            "3a9c…e01f",
            "b71d…44c0",
            "Total reclaimed space: 4.1GB",
        ],
    );
    put(
        "docker-cleanup/builder",
        &[
            "$ docker builder prune -a --force",
            "$ docker buildx prune -a --force",
            "ID                       RECLAIMABLE  SIZE",
            "z9q…                     true         1.8GB",
            "… 118 more",
            "Total: 6.1GB",
        ],
    );
    put(
        "docker-cleanup/verify",
        &[
            "$ docker system df",
            "TYPE            TOTAL     ACTIVE    SIZE      RECLAIMABLE",
            "Images          0         0         0B        0B",
            "Containers      0         0         0B        0B",
            "Local Volumes   0         0         0B        0B",
            "Build Cache     0         0         0B        0B",
            "reclaimed 18.4 GB on devbox",
        ],
    );

    put(
        "upgrade/preflight",
        &[
            "os: Debian GNU/Linux 13 (trixie) · kernel 6.12.0-1-amd64",
            "privileges: sudo cached (asked once)",
            "apt lock: free",
            "network: deb.debian.org reachable · 18 ms",
            "free space on /: 41.2 GB",
            "pending reboot: no",
        ],
    );
    put(
        "upgrade/apt-update",
        &[
            "$ sudo apt-get update",
            "Hit:1 http://deb.debian.org/debian trixie InRelease",
            "Get:2 http://security.debian.org/debian-security trixie-security InRelease [48.0 kB]",
            "Get:3 http://deb.debian.org/debian trixie-updates InRelease [52.1 kB]",
            "Fetched 100 kB in 1s (84.2 kB/s)",
            "Reading package lists… Done",
        ],
    );
    put(
        "upgrade/apt-review",
        &[
            "$ apt list --upgradable",
            "libssl3/trixie-security 3.0.13-1 [upgradable from: 3.0.12-1]",
            "openssl/trixie-security 3.0.13-1 [upgradable from: 3.0.12-1]",
            "curl/trixie-updates 8.5.0-2 [upgradable from: 8.5.0-1]",
            "libcurl4/trixie-updates 8.5.0-2 [upgradable from: 8.5.0-1]",
            "git/trixie-updates 2.47.1-1 [upgradable from: 2.47.0-1]",
            "… 9 more",
            "14 upgradable · 2 security",
        ],
    );
    put(
        "upgrade/apt-apply",
        &[
            "$ sudo apt-get upgrade -y",
            "Reading package lists… Done",
            "Building dependency tree… Done",
            "14 upgraded, 0 newly installed, 0 to remove and 0 not upgraded.",
            "Need to get 38.4 MB of archives.",
            "Get:1 libssl3 3.0.13-1 [1,986 kB]",
            "Preparing to unpack …/libssl3_3.0.13-1_amd64.deb …",
            "Unpacking libssl3:amd64 (3.0.13-1) over (3.0.12-1) …",
            "Setting up libssl3:amd64 (3.0.13-1) …",
            "Preparing to unpack …/openssl_3.0.13-1_amd64.deb …",
            "Unpacking openssl (3.0.13-1) over (3.0.12-1) …",
            "Setting up openssl (3.0.13-1) …",
            "Preparing to unpack …/curl_8.5.0-2_amd64.deb …",
            "Unpacking curl (8.5.0-2) over (8.5.0-1) …",
            "Setting up curl (8.5.0-2) …",
            "Processing triggers for man-db (2.13.0-1) …",
            "Processing triggers for libc-bin (2.41-4) …",
        ],
    );
    put(
        "upgrade/mise-inspect",
        &[
            "$ mise outdated --json",
            "Tool     Requested  Current   Latest",
            "node     26         26.0.1    26.1.0",
            "python   3.13       3.13.5    3.13.7",
            "go       1.24       1.24.6    1.25.1",
            "pnpm     10         10.4.0    10.6.2",
            "4 outdated · 1 excluded (go)",
        ],
    );
    put(
        "upgrade/mise-upgrade",
        &[
            "$ mise upgrade --exclude go",
            "mise node@26.1.0 · downloading node-v26.1.0-linux-x64.tar.xz",
            "mise node@26.1.0 · checksum mismatch · expected 9f3a… got 4c11…",
            "mise ERROR failed to install node@26.1.0",
        ],
    );
    put(
        "upgrade/cleanup",
        &[
            "$ sudo apt-get autoremove --purge -y",
            "0 upgraded, 0 newly installed, 2 to remove and 0 not upgraded.",
            "Removing linux-image-6.11.0-2-amd64 …",
            "Removing libperl5.38 …",
        ],
    );
    put(
        "upgrade/verify",
        &[
            "$ apt list --upgradable",
            "Listing… Done",
            "$ mise ls --json",
            "$ systemctl --failed",
            "0 loaded units listed.",
            "$ test -f /var/run/reboot-required",
            "reboot required: no",
        ],
    );

    put(
        "cleanup-work/frontend-node_modules",
        &[
            "moving /Users/alex/work/frontend/node_modules to Trash",
            "121,402 items · 8.4 GB",
            "moved · restore from Trash if needed",
        ],
    );
    put(
        "cleanup-work/backend-target",
        &[
            "$ cargo clean --manifest-path /Users/alex/work/backend/Cargo.toml",
            "     Removed 48,113 files, 12.7GiB total",
        ],
    );
    put(
        "cleanup-work/android-build",
        &[
            "$ ./gradlew clean  (in /Users/alex/work/android)",
            "> Task :app:clean",
            "> Task :core:clean",
            "BUILD SUCCESSFUL in 4s",
        ],
    );
    put(
        "cleanup-work/pnpm-store",
        &[
            "$ pnpm store prune",
            "Removed 1,204 packages",
            "Removed 3 files",
            "2.2 GB reclaimed",
        ],
    );
    put(
        "cleanup-work/verify",
        &[
            "rescanning /Users/alex/work …",
            "reclaimed 26.4 GB · skipped 2 · failed 0",
            "/ now 88% used (was 91%)",
        ],
    );

    put(
        "git-pull-all/frontend",
        &[
            "$ git -C /Users/alex/work/frontend pull --ff-only",
            "Updating 3d1e0f2..8ab7c91",
            "Fast-forward",
            " 6 files changed, 120 insertions(+), 14 deletions(-)",
        ],
    );
    put(
        "git-pull-all/android",
        &[
            "$ git -C /Users/alex/work/android pull --ff-only",
            "Already up to date.",
        ],
    );
    put(
        "git-pull-all/shared-libs",
        &[
            "$ git -C /Users/alex/work/backend/vendor/shared-libs pull --ff-only",
            "fatal: Not possible to fast-forward, aborting.",
        ],
    );
    put(
        "git-pull-all/verify",
        &[
            "frontend   main     up to date · pulled 6 files",
            "android    develop  up to date",
            "backend    master   skipped · 3 modified",
            "shared-libs main    failed · diverged",
        ],
    );
    m
}
