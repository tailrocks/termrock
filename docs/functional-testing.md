# Functional testing through the real terminal

Use the ordinary `functional` integration target as the reference for new
application tests. It launches the shipped executables in a pseudo-terminal,
sends the same keyboard and mouse reports that a terminal sends, and asserts
the visible consequences. It never constructs application state, calls widget
handlers, injects hit-region IDs, or renders into a test backend.

The reference source is [tests/functional](../tests/functional/main.rs). The
initial application review was against PR #26, commit
`5b3dbd7ae799f325efbb4f4cc11b2f5518dec948`, on
`design/interaction-polish`. This is the monolithic `junie-tui` package with four binaries. (The former
`xtask` workspace member existed only to serve the deleted visual-baseline
estate; it was removed with that estate — see the migration mapping.) Architecture reports describing
`crates/*` on other branches are not evidence about these binaries.

## Run the acceptance examples

Install the repository's pinned tools, then run:

```sh
mise install
cargo nextest run --locked --workspace --profile functional --no-tests fail
```

This is the command CI runs in `suite-functional`. The profile selects
the `functional` target of the workspace crates and retains a JUnit
report. The `functional` target of `jackin-console` needs the feature
`demo`, which is off by default, so this command runs the root journeys
only. Add `--all-features` to include the jackin journeys; CI runs them in
`Rust / jackin-console` (see [CI](ci.md#the-demo-feature)). To run the
target without the profile:

```sh
cargo nextest run --locked --test functional --no-tests fail
```

The cases that drive more than one app binary (`g02_harness` and the `x01_*`,
`x02_*`, `x03_*` files, except `x01_input` and `x02_suspend`) live in the
`functional` target of `crates/functional-cross`. That crate has no app
binaries of its own, so it finds `showcase`, `tablepro` and `holla` by name
next to its test executable in `target/<profile>/`, or at `TCB_BIN_<NAME>`. A
nextest setup script (`app-bins` in `.config/nextest.toml`) runs
`cargo build --locked --workspace --bins` before any selected
`functional-cross` test, so `cargo nextest run -p functional-cross` works from
a clean target directory. The script builds the dev profile, so
`--cargo-profile release` would still launch the debug binaries.

Filter one journey during development, without hiding missing tests:

```sh
cargo nextest run --locked --test functional --no-tests fail \
  -E 'test(showcase_button_hover_hold_release_cancel_and_disabled_truecolor)'
```

These tests are ordinary, non-ignored integration tests. They also run in the
existing default Nextest gate. Do not add a missing-baseline early return or
an environment flag that silently turns a requested case into success. The one
allowed `#[ignore]` is a quarantine of a flaky test: its reason names the
tracking entry in `docs/P9.3-red-green.md`, and the entry names the cause and
the fix that removes the `#[ignore]`. No baseline approval is needed to run this target. Linux and macOS
are the supported test hosts; Windows requires separate application/runtime
qualification, not just a compatible test library.

Each test gets its own process, terminal, home, working directory, and evidence
directory. Nextest limits PTY concurrency; a functional test consumes two
thread-budget slots. Each observation has a ten-second deadline, and Nextest
terminates a test after three thirty-second slow periods. Repository agent
sessions should additionally bound individual commands with `timeout 230` and
split larger existing gates as necessary.

## Which harness to use

| Tool | Verified identity | Role |
| --- | --- | --- |
| Live driver | Registry `tuiscotti-tui-test = "=0.2.1"` (crate `tui_test`) | The only live driver: checked mouse/keyboard input, native PTY launch, owned terminal snapshots, bounded waits. |
| Visual gate | Latest Tuiscotti main `92ef2d9ee082b35f68e066c5129fedbf07675a80` (renamed dev-dependency `tuiscotti-latest`, 0.4.0) | Offline frozen check: every functional test ends with `App::visual("<id>")`, a `FrozenStore` check that asserts `is_pass()`. A missing or changed baseline fails. |
| Tuiscotti CLI (latest, built from the pinned rev) | `tuiscotti accept --candidate <sample> --candidate-sha256 <sha> --expect-baseline missing --baselines tests/functional/baselines` | The only way a baseline is approved. Tests never approve baselines. |

```toml
[dev-dependencies]
tui-test = { package = "tuiscotti-tui-test", version = "=0.2.1" }
tuiscotti-latest = { package = "tuiscotti", git = "https://github.com/tailrocks/tuiscotti", rev = "92ef2d9ee082b35f68e066c5129fedbf07675a80" }
```

The legacy Tuiscotti 0.2 live API (git rev `a1ea27f`), `libtest-mimic` and
the old `sha2` oracle pin are removed: nothing in the repository references
them. No functional test constructs widgets or application state
in-process, and none calls event handlers directly. The only in-process
tests are the data tests described under
[Library data tests](#library-data-tests). The immutable `visual-baseline` git
tag and its release are untouched; they remain a historical visual oracle,
not a test gate.

No raster feature is needed to assert terminal cells. A terminal-cell snapshot
records graphemes, foreground/background, attributes, underline, hyperlinks,
dimensions, and cursor state. Pixel images are useful review evidence, but a
plain text screenshot cannot establish hover or pressed styling.

## The old visual suite is retired

The former `tests/visual_baseline` harness (ignored libtest-mimic trials
over 702 frozen scenarios in `snapshots/`) is deleted, together with its
oracle, scenario tables and the `baselines/oracle` pin. Its contract —
"this screen looks like the approved frame" — is now carried per journey
by the `App::visual` frozen check at the end of every functional test,
with one approved protobuf `.tui.pb` baseline per test in
`tests/functional/baselines/`. The frozen `visual-baseline` tag is not
moved, deleted or retargeted.

## Library data tests

`apps/jackin/tests/*.rs` (beside the `functional/` journeys) prove the
data API that `jackin-console` offers another crate: `World::new`,
`Snapshot`, the time source, external messages, `Vocabulary`, the
account, launch-role and wizard data. They build the real `App` and
`World` from the `demo` feature, drive it with keys through the
`Application` trait, render into a test buffer and assert text and
state. The tests of the library's own types live in the library's root
`tests/` directory instead: `fixed.rs` and `reason.rs` (the fixed
control state and the reason widths, rendered into a test buffer and
driven with keys and clicks), `shell.rs` (the refusal slot and the
redraw rule, through a minimal `shell::App` in `tests/common/mod.rs`)
and `status.rs` (the sticky status line). No test of a `junie-tui` type
lives in `apps/jackin/tests`. These tests add no visual baseline and are
not journeys: a user-visible flow still needs a functional journey.

Put a new data or regression test in the package's shared `regressions`
target: `tests/regressions/<topic>.rs` with a `mod <topic>;` line in
`tests/regressions/main.rs`, and the same under `apps/jackin/tests/` for
`jackin-console`. Do not add a new top-level `tests/*.rs`: the generated CI
lists every test target, so each new one changes the generated workflows
(see [CI](ci.md#test-targets-and-generated-ci)).
`cargo nextest run --all-features --all-targets` runs them, and so do the
`Rust / junie-tui` and `Rust / jackin-console` CI jobs
(`nextest run --profile ci --package <crate> --all-features`); the `functional` suite
does not.

Three files prove the packaging rule that the demo is off by default.
`product_mode.rs` compiles only without `demo`: it builds a `World` from
a test `Backend`, renders the manager and applies a pushed `Snapshot`, so
the product shape is run, not only built. `product_manifest.rs` reads
`cargo metadata` and fails when `default` is not empty, `clap` is not
optional, `demo` or `product` change, or the `jackin-preview` binary or
the `functional` target loses `required-features = ["demo"]`.
`library_has_no_demo.rs` lexes `apps/jackin/src` outside `fixtures/`,
`sim/` and `bin/` and fails on a demo name or literal; its `PENDING` list
holds the demo items that later P9.3 steps still move, and a row that no
longer matches fails.

Each test has a named break (a one-edit change to the library that must
turn it red) recorded in [P9.3 red and green](P9.3-red-green.md).

## The test boundary

The path under test is:

1. Real executable startup and terminal mode negotiation.
2. Terminal keyboard, paste, pointer, wheel, and resize input.
3. Production input decoding, focus routing, overlays, and event loop.
4. Application operations and shared components.
5. Production rendering and emitted terminal control sequences.
6. The resulting visible cells and clean process shutdown.

Use `env!("CARGO_BIN_EXE_...")`, exposed as `SHOWCASE`, `TABLEPRO`, `HOLLA`,
and `JACKIN` by the helper. Avoid a shell command string. `App::new` passes
arguments as native process arguments and clears inherited environment. It
sets a fixed locale, timezone, terminal type, client platform, and color mode;
isolates HOME and XDG directories; and disables Holla's optional history.

These applications are deterministic demonstrations. TablePro executes its
local SQL/catalog model; Holla simulates task execution; Jackin simulates
workspace/account operations. A saved row or launched task is a real
application interaction and an in-memory simulated domain effect. It does
not prove an external PostgreSQL connection, Docker command, real Jackin
service, or credential provider works. When real adapters are added, give
them a separate functional tier with disposable services, observable external
effects, and teardown; preserve the same terminal interaction layer.

Use `--motion paused --frame <n>` for held-state and static view tests.
Holla and TablePro asynchronous operations need normal/reduced motion.
Jackin and Holla additionally support opt-in `JUNIE_TUI_TEST_CLOCK=1`, which
the helper always sets: wall time then never moves the simulated clock (so a
relative label such as "live · 3 s ago" cannot drift under load), and the
helper's `advance_clock(ms)` sends its documented bracketed-paste clock
request to step it. A Holla journey that waits for a task or activity to
progress calls `advance_clock` first. So does a Jackin journey that waits for a
simulated save or for a status line to expire: under the opt-in Jackin starts
its clock stopped, so only `advance_clock` moves it. TablePro steps only its status line
(the 5 s lifetime of a message) this way, and Showcase holds it (it does not
implement `advance_clock`); their pointer timers stay on wall time. A launch
with `launch.clock` set opts in. Do not assume every application
implements that clock. Never send clock input as a substitute for a domain
interaction.

`JUNIE_TUI_TEST_FENCE=1` is the opt-in that makes an app answer the shell fence (a BEL queued behind the frame of every earlier input) without freezing the status clock as `JUNIE_TUI_TEST_CLOCK` does; `App::resize` and `App::fence` wait on it for Showcase, TablePro, Holla and Jackin.

Every read of the screen by the helper (`screen`, the `wait_*` predicates,
`settle`, `stable_screen`) begins with a fence: the helper pastes
`junie-tui:fence`, and under the same opt-in the shell answers it with a BEL
after the frame of every input written before it. The helper returns when the
terminal's bell count rises, so a snapshot shows every action sent so far as
handled. The writes stay as asynchronous as before (a key and the next mouse
move may share one frame, as in a real terminal). Without the fence a wait that
one frame of a multi-input gesture already satisfies, or a `settle()` that
reads 150 ms of quiet before the application has read its input, passes or
fails by machine load. Write a wait for the state the action produces, not for
a state the frame before it also has.

## Write one journey around observable contracts

Start from a user task, such as "cancel a write, retry it, and execute it only
after entering the required table name." Write down the initial condition,
input, expected visible result, and forbidden side effect for each step.

An effective assertion answers a specific question:

| Input | Strong visible proof | Insufficient proof |
| --- | --- | --- |
| Hover a button | That button's cell styling changes; its action has not run. | Any cell on the screen changes. |
| Hold a button | Target styling differs from rest and hover while the action remains pending. | A screenshot after a complete click. |
| Release a button | Its expected status, selection, dialog, or domain result appears. | The terminal emitted more bytes. |
| Cancel a save | The preview closes and pending edits remain; reopening shows them. | The word "Cancel" disappears. |
| Save | Reopen the affected object and observe the exact changed field. | A generic saved toast alone. |
| Scroll a pane | Its visible item/line identities advance and the neighboring pane is unchanged. | The terminal viewport moved or the scrollbar changed color. |
| Delete an item | A search/list scoped to the item's identity has no matching row. | A removed toast followed by an unrelated screen. |
| Resize | The app's rendered size/layout changes and the open view remains usable. | The emulator reports new dimensions before the app repaints. |

### Locate targets from rendered output

`find(screen, label)` requires exactly one physical-row match and returns
zero-based terminal cells. It maps UTF-8 offsets through actual cells, rather
than treating byte offsets or Unicode code points as columns. For repeated
labels, use `find_all`, then scope the matches to a visible panel or dialog
action row and assert that the scoped result is unique.

Do not blindly select the first match. A dimmed editor's Save action can
remain visible behind a modal's Save action. A footer can advertise
`g Go to path` while the path dialog is closed. Prefer a dialog-specific
title or prompt as readiness evidence, and locate the action within that
dialog. The reference account and file-browser journeys demonstrate this.

Polling predicates must return `false` while a transient frame lacks their
required content. Do not put an unconditional `find(...).unwrap()` inside a
wait predicate. Resolve strict locators after readiness, or use total
`find_all`/`Option` logic inside the predicate.

### Test rest, hover, held press, release, and cancellation separately

This example uses the same real-process helper as the suite:

```rust
let app = App::new(
    "run-button-example",
    SHOWCASE,
    &["--page", "buttons", "--motion", "paused"],
    160,
    50,
    "truecolor",
);
app.wait_text("Disabled primary");
let initial = app.settle();
let run = find(&initial, "Run task");
let rest = crop(&initial, run, 8, 1);

app.move_to(run);
let hovered = app.wait_screen("Run task hover", |s| crop(s, run, 8, 1) != rest);
let hover = crop(&hovered, run, 8, 1);

app.down(run);
let held = app.wait_screen("Run task held", |s| {
    let cells = crop(s, run, 8, 1);
    cells != rest && cells != hover
});
assert!(!text(&held).contains("Run task ✓"));

app.up(run);
app.wait_text("Run task ✓");
app.finish();
```

Use a new session or a distinct prior action state when asserting that the
button has not fired: a status already present before the input is not new
evidence. For an exactly-once action, prefer a visible count or a stateful
toggle whose resulting on/off state exposes an extra activation. The
Showcase's internal activation counter is not painted at the current
Playground height; do not assert invisible text or read its private field.

A combined `click` is appropriate when only the completed action matters.
It cannot prove the intermediate held appearance. For drag cancellation,
call `down`, then `drag_to` outside the hit region, then `up` outside. Plain
`move_to` encodes movement with no button held and is not the correct
held-drag event.

Checked mouse operations also verify that the application enabled the
required tracking and SGR coordinate modes. The released upstream raw mouse
API can inject events even when a real terminal would not send them. This
can conceal a missing `EnableMouseCapture` regression, which is one reason
the fork's checked API is preferable here.

Do not confuse persistent selected/active-page state with transient held
press. A selected tab remains selected after release; a pressed button
should not retain its held appearance indefinitely. Some controls show a
brief release flash. Before comparing the next hover/press sequence, wait
for that specific target to return to its expected non-flash state.

### Send wheel input to the application component

The inspected fork and upstream convenience `MouseAction::Scroll` encode
wheel events at `(0, 0)`, irrespective of the last mouse move. The separate
`Operation::Scroll` scrolls the terminal emulator's history viewport and
does not exercise application scrolling. Neither is suitable for a pane
routing assertion.

`App::wheel(point, down, amount)` first uses checked pointer motion, validating
the live coordinate and negotiated mode. It then calls the fork's public
SGR wheel encoder at that exact point and writes the bytes through the same
PTY. Down uses code 65 and up code 64; the wire coordinate is one-based.
There is no separate PTY implementation or synthetic application event.
The helper is deliberately small and should be replaced by a checked,
coordinate-aware upstream wheel API when one is released.

After scrolling, assert the actual first visible row or line changed. Also
assert a neighboring pane's content stayed fixed, the original selection
did not unexpectedly move, and keyboard navigation follows the clicked
focus. Test reverse scrolling and boundary clamping for long content. For
scrollbar dragging, split press/drag/release and inspect the thumb while held.

### Synchronize on effects

`wait_text` and `wait_screen` have bounded deadlines and check that the process
is still alive. `settle` is a supplemental quiet wait. It is not an event
acknowledgment or application frame boundary; the fork itself documents this
limitation. A negative assertion evaluated immediately after an input can
pass before the application handles that input.

Pair absence/unchanged assertions with positive progress: observe a modal
close, wait for a distinct result, or perform a subsequent visible action
that makes an unwanted earlier effect observable. Check disabled styling
with the target held, then exercise a working control in the same session.
Do not increase arbitrary sleeps until a flaky test appears green.

Walk and look up through the shared helpers in `tests/functional/support.rs`,
never with a per-test loop that polls against a short deadline of its own. A
deadline of 800 ms, 3 s or 5 s passes on an idle machine and fails when the
harness shares CPU with other jobs, so these helpers end on the state they
wait for:

- `App::press_until(key, goal, max_presses, watch, reached)` presses a key
  until `reached` accepts the screen. After each press it waits for
  `watch(screen)` to change, so a slow application is waited for, not
  pressed again. A press that moves nothing fails with the screen.
- `App::wait_point` and `support::button` locate a target on the screen once
  it is painted, in place of a one-shot `screen()` lookup.
- `App::wait_exit` waits for the child to quit.

For an asynchronous operation, wait for a durable result rather than a
very short spinner frame. Inspect an intermediate progress state only when
the application provides a controlled clock or stable observable barrier.

## Coverage to retain and extend

The suite covers every application journey and every shared component
family. The inventory below is the coverage contract; a new capability
adds its journey here and in the showcase in the same change.

| Application | Reference journeys |
| --- | --- |
| Showcase | Button rest/hover/held/release/cancel/disabled/toggle in truecolor, 256, 16, monochrome and `NO_COLOR`, plus literal held colours and one focus gutter; wheel routing (adjacent panes, grid with unchanged sidebar); checks/radios (mixed, disabled group, exclusivity), tabs selection-vs-focus, select/menu light-dismiss and Escape dismissal, lists (selection, keyboard, multi-select toggle), trees (expand/collapse, file selection), inputs (edit/apply/Escape-revert), dialogs (open/Escape-cancel), table header sort, forms required validation, scrollbar thumb drag, splitter seam drag, keyboard activation, popover/help resize with focus return. Multi-search pickers preserve spaces typed in the filter; Ctrl+Space toggles the focused result and Enter reports the selected labels. |
| TablePro | Authentication failure, retry, and recovery by choosing another connection; write-confirmation outside-click policy, cancel/retry, wrong token, disabled Execute, correct token, held Execute, completed operation; dirty-tab close guard via Ctrl+W and the tab picker (cancel preserves SQL, confirm closes only the requested tab), filtered picker, clean-tab immediate close. The backend is simulated: assertions check the next in-app consequence, never a real PostgreSQL effect. |
| Holla | Child-directory task search, trust cancel/confirm, arguments and simulated task output; finder search → run → history; second-gate cancellation with an empty-activity acknowledgement; hidden-file toggling, preview wheel movement with unchanged file list, bad-path recovery and reopening. The backend is simulated. |
| Jackin | Workspace mount save with pointer states and reopened values; editor cancel/discard/reopen; account required-field, masked save, canceled/confirmed removal with filtered-list absence; launch → detach → reconnect; help ownership and focus restoration. The backend is simulated. |

Add shared component contracts through Showcase and at least one real
consumer when a change affects that consumer. Use this matrix to choose
follow-up journeys rather than multiplying screenshots without a behavioral
question:

| Component family | Required functional contracts |
| --- | --- |
| Buttons, icon actions, links | Rest/hover/press, release inside, drag-release outside, disabled no-op, keyboard activation, exactly-once observable effect. |
| Checkboxes, radios, toggles | Pointer and keyboard changes, group exclusivity, disabled values, persistent chosen state after focus moves, mixed-state transitions where supported. |
| Tabs, segmented controls, sidebars | Selection distinct from focus, click and arrow navigation, active page content, closable/dirty tabs, selection neutralized beneath an overlay, focus restoration on close. |
| Lists, trees, menus | Selection retained after blur, disabled-item skipping, type-ahead, expanded/collapsed children, menu hover switching, outside dismissal, long-content scrolling, no viewport snap after hover. |
| Tables and grids | Row/column selection, sort and filter effects on actual IDs, horizontal/vertical scrolling, inline edit cancel/commit, pending-change gate, history, context-menu action and column resize. |
| Inputs, editors, forms | Typing, bracketed paste, selection, Unicode and wide text, required/invalid input, secret masking, submit/cancel, focus traversal and short-form scrolling, completion acceptance and dismissal. |
| Selects, pickers, dialogs, drawers | Correct top layer receives pointer/keys, no click-through, outside-click policy, Escape cancel, no lower-layer focus gutter, reopen state and return focus, nested overlays. |
| Scrollbars, splitters, pane layouts | Thumb/seam rest/hover/held appearance, drag position/content effect, track clicks, independent neighboring pane, bounds, resize while open and small-terminal recovery. |
| Runtime/shell | TTY startup, input/paste/mouse negotiation, keyboard fallback, resize, supported color modes, process exit code, restored alternate-screen/paste/mouse state. |

Prioritize TablePro row edit → canceled save → confirmed save → history;
query error → correction → results; Jackin launch failure and recovery;
Holla activity stop/quit guards; then wider catalog states. Cover different
data shapes: empty, one item, long lists, disabled rows, repeated labels,
long labels, and combining/wide Unicode. Avoid fixtures that only work at one
magic coordinate or exact viewport.

For pull requests, keep a small representative matrix fast and mandatory.
Run broader adjacent/breakpoint sizes and all color modes in a scheduled
tier. Include 80×24 and a larger size for responsive behavior; add widths
immediately below and above actual layout breakpoints. A complete matrix of
sizes and colors does not replace distinct domain journeys.

## CI and failure evidence

The source configuration is [.velnor/config.toml](../.velnor/config.toml)
(`compile_driver = "mbx"`: CI and local builds both run cargo through
mr-boxington; local setup is the mise postinstall `mbx setup`). The
dedicated functional suite runs on PRs and branch pushes and can be
selected manually with `tier=functional`. Its generated command, run
through `mbx`, is:

```sh
cargo nextest run --workspace --no-tests fail --profile functional
```

The profile selects only the non-ignored functional target, so a missing
target fails (`--no-tests fail`) instead of passing empty. Failures retain
`target/functional` and `target/nextest/functional` for fourteen days. The
harness writes an ordered action trace, a text checkpoint for every wait, and
final screen evidence. Full-cell JSON is the costly part (about 1.1 MB and
tens of milliseconds of serialization per checkpoint), so a failing test writes
it for its latest 16 checkpoints, and a passing test writes none (`last.json`
still holds the final frame). Each
checkpoint's line in the action trace carries a hash of its cells, so a text
checkpoint without JSON can still be matched to a frame. A run that is killed
for a timeout never reaches the failure write and keeps only the text
checkpoints and the action trace.

`finish()` asserts a live child before Ctrl+Q, exit code zero without a
termination signal, and restored terminal modes. `Drop` still closes the
session and removes the child home after assertion failure. A cleanup kill
does not count as a successful application exit.

Velnor generates the workflows; change its configuration and run
`velnor-actions generate`, never hand-edit generated YAML. The suite sets
`required = true`, so the aggregate `Required` check needs
`suite-functional`. `Required` is the check to require in branch protection.

The push filters and the `head_branch` predicates of the follow-up workflows
(`state-seal`, `cache-prune`) come from `[workflow].default_branch`. That
value must equal the repository's actual default branch. `tests/ci_default_branch.rs`
checks it offline: `repository_config_default_branch_is_main` reads
`.velnor/config.toml`, and fixture tests cover match, mismatch, fork names,
and invalid shapes. Run it with `cargo nextest run -p junie-tui --test
ci_default_branch --no-tests fail`. To change the branch, edit the source value, run
`mise exec -- velnor-actions generate` from the repository root, and confirm a
second generation leaves no diff. After the change merges, record one real
push run on that branch in the Actions tab before closing the issue.

On a failure, read the action trace and the last frame first. Classify the
failure as an incorrect locator/precondition, input-mode/routing failure,
product regression, stale visual reference, or infrastructure timeout. Fix
the specific cause. Preserve first-failure evidence, do not silently retry
until green, and do not update expected snapshots from a failed candidate.

## Migration mapping (deleted estate → replacement or dropped reason)

Every pre-migration test is accounted for below. A deleted test was only
removed after its named functional replacement passed; contracts with no
functional replacement are listed as dropped with the reason. This is
100% of the former estate: `tests/` integration files, the
`visual_baseline` harness, all in-module `#[cfg(test)]` tests and all
per-app `app_tests*` suites.

| Former estate | Count | Disposition |
| --- | --- | --- |
| tests/ascii_glyphs.rs | 10 | Dropped: in-process Buffer glyph/width assertions. Colour/glyph behaviour is asserted on real screens by the button colour matrix and frozen baselines. |
| tests/choice_containment.rs | 7 | Superseded in behaviour by `showcase_checks_radio_exclusivity_mixed_and_disabled` (mixed/disabled/exclusivity on a real screen); cell-containment geometry dropped as an in-process contract. |
| tests/cli.rs | 5 | Dropped: `--help`/exit-code surface is not a terminal journey and cannot end in a frozen screen check. |
| tests/esc_vocabulary.rs, tests/naming_conventions.rs | 4 | Dropped: static source scans (lint), not executable behaviour. |
| tests/focus_gutter.rs | 4 | Superseded by `showcase_primary_button_exact_rest_hover_held_colours` (exactly one `▎`) and `showcase_tab_selection_survives_focus_move_and_single_gutter`. |
| tests/hit_parts.rs | 3 | Dropped: HitRegistry part resolution is in-process; routing is proven by real clicks/wheel in every journey. |
| tests/holla_pty.rs | 3 | Palette classes superseded by the colour matrix; input-flood and NO_COLOR backend rules dropped as emulator-level contracts (clean-exit/mode restoration is asserted by `finish()` in all 39 tests). |
| tests/keymap_leftovers.rs | 6 | Dropped: direct `on_key` routing. Keyboard contracts are proven through real keys in lists/trees/inputs/dialogs/help journeys. |
| tests/mouse_targets.rs | 4 | Superseded by coordinate wheel/button/menu/select journeys across Showcase and all three consumer apps. |
| tests/navigable.rs | 2 | Dropped: cross-widget trait parity in-process; navigation proven per widget on real screens. |
| tests/scroll_panes.rs | 13 | Superseded by `showcase_scrollbar_thumb_drag_moves_list`, `showcase_wheel_targets_the_pointed_pane_then_keys_follow_clicked_focus` and the datagrid wheel test; page/notch unit geometry dropped. |
| tests/strings.rs | 6 | Dropped: theme Strings rendering in-process; copy is asserted as visible text in journeys. |
| tests/table_perf.rs | 4 | Dropped: timing/O(rows) scaling gate; not reproducible as a PTY journey. |
| tests/terminal_suspend.rs | 1+helper | Dropped: SIGTSTP job control is outside the harness; exit/restoration asserted by `finish()`. |
| tests/visual_baseline/** (harness, oracle, scenarios, determinism/isolation/modes/startup_* trials; 702 scenarios; snapshots/, baselines/oracle) | ~12,600 files | Retired: frozen-census contract replaced per journey by `App::visual` FrozenStore baselines. `visual-baseline` tag untouched. Startup/resize/mode contracts partially superseded by lifecycle resize tests and `finish()` mode assertions. |
| In-module lib tests: src/core | 172 | Dropped: pure-logic units (keymap, scroll offsets, edit, selection, typeahead). Behavioural slices superseded by the journeys above. |
| In-module lib tests: src/ui | 157 | Dropped: in-process render/layout units; superseded in behaviour by Showcase family journeys. |
| In-module lib tests: src/widgets | 619 | Dropped: in-process widget variant/state units; superseded in behaviour by Showcase family journeys (deep) plus one consumer journey per app. |
| In-module tests: src/shell, src/*.rs, xtask | ~440 | Dropped: app-state/router units and xtask tool tests; shell behaviour proven by real-app journeys. The xtask crate itself was removed afterwards: every command operated on the deleted visual-baseline estate (`tests/visual_baseline/`, `baselines/oracle/`), superseded by the Tuiscotti CLI. |
| src/bin/showcase/app_tests* (incl. catalog/coverage gate) | 546 + macro suites | Superseded by Showcase functional journeys; the catalog/coverage source-scan gate is dropped with the estate (its files `catalog_scan.rs` deleted; production-compiled `coverage.rs`/`src/catalog.rs` registry retained as production source, not tests). The declared-API gate returned with #14 as `src/catalog_contract.rs` (source coverage) and `src/bin/showcase/app_tests_catalog.rs` (callable scenarios); see AGENTS.md. |
| src/bin/tablepro/app_tests*, motion_tests, perf_tests | 151 | Superseded by TablePro functional journeys; perf tests dropped as timing gates. |
| src/bin/holla/app_tests* | 162 | Superseded by Holla functional journeys. |
| src/bin/jackin/app_tests* (formerly jackin_preview) (incl. reach_tests! suites) | 147 + generated | Superseded by Jackin functional journeys. The `reach_tests!` macro in `src/shell/reach.rs` is retained as library support code with no remaining invocations. |
| tools/frozen* (frozen baseline CLI parity tooling) | 6 files + crate | Deleted: dead helpers for the retired frozen harness. |

Kept and why: `src/catalog.rs` and `src/bin/showcase/coverage.rs`
(production-compiled capability registry/declarations referenced by the
library API), `src/shell/reach.rs` (catalog-registered reach support),
the `visual-baseline` git tag/release (immutable historical oracle), and
all production source behaviour and contract comments (unchanged except
test-module removal).

## Primary references

- [Current tuiscotti architecture and installation][tuiscotti-readme].
- [Released fork package and features][fork-package].
- [Public Session launch, input, capture, and quiet-wait API][fork-session].
- [Checked input, mouse mode validation, and held-drag semantics][fork-checked].
- [SGR mouse encoder][fork-mouse] and [legacy wheel routing][fork-engine].
- [Owned terminal snapshot schema and equality][fork-snapshot].
- [Quiet wait sampling and frame-boundary limitations][fork-wait].
- [Microsoft tui-test 0.1.1 release][upstream-release].
- [Upstream checked-input contribution #282][upstream-checked].

[tuiscotti-readme]: https://github.com/tailrocks/tuiscotti/blob/92ef2d9ee082b35f68e066c5129fedbf07675a80/README.md
[fork-package]: https://github.com/tailrocks/tui-test/blob/7abeac8478aa9c05ec7bef93afb10209cc348bec/crates/tui-test/Cargo.toml
[fork-session]: https://github.com/tailrocks/tui-test/blob/7abeac8478aa9c05ec7bef93afb10209cc348bec/crates/tui-test/src/runtime.rs
[fork-checked]: https://github.com/tailrocks/tui-test/blob/7abeac8478aa9c05ec7bef93afb10209cc348bec/crates/tui-test/src/session.rs
[fork-mouse]: https://github.com/tailrocks/tui-test/blob/7abeac8478aa9c05ec7bef93afb10209cc348bec/crates/tui-test/src/input/mouse.rs
[fork-engine]: https://github.com/tailrocks/tui-test/blob/7abeac8478aa9c05ec7bef93afb10209cc348bec/crates/tui-test/src/engine.rs
[fork-snapshot]: https://github.com/tailrocks/tui-test/blob/7abeac8478aa9c05ec7bef93afb10209cc348bec/crates/tui-test/src/snapshot.rs
[fork-wait]: https://github.com/tailrocks/tui-test/blob/7abeac8478aa9c05ec7bef93afb10209cc348bec/crates/tui-test/src/wait.rs
[upstream-release]: https://github.com/microsoft/tui-test/releases/tag/0.1.1
[upstream-checked]: https://github.com/microsoft/tui-test/pull/282

## The standard for every functional test

Every test in this target follows one standardized pattern, with the one
exception below and no legacy-API tests:

1. Launch the real executable through the shared `support::App` harness
   (`tui_test` / `tuiscotti-tui-test =0.2.1`, isolated environment, bounded
   semantic waits, clean-exit and terminal-restoration assertions).
2. Drive it with terminal input and assert the observable outcome between
   steps (hover/held/release, cancellation, focus, neighbouring panes).
3. End with `app.visual("<test-id>")`: the live screen is frozen as a
   Tuiscotti state sample and compared with `FrozenStore` from latest
   Tuiscotti (`tuiscotti-latest`, main `92ef2d9`). The test asserts
   `outcome.is_pass()`; a missing or changed baseline fails. A baseline
   for every test that ends in `visual` lives in `tests/functional/baselines/`;
   each was approved once with the Tuiscotti CLI `accept` command — tests
   never approve.
   Checkpoints sit on deterministic screens (after `settle()`, not on
   cycling status words or mid-flash frames). `App::visual` additionally
   requires two identical consecutive cell fingerprints (cells + cursor)
   before freezing, and tests that end beside a status flash or a live
   relative-time readout wait for that element to fade or freeze the
   app clock (`--motion paused`) first.

The one exception: a key test of a page that asserts refusals, reasons and
status expiry may end with `app.finish()` and no baseline. P9.3 design v6
section 5.2 (last paragraph) requires one such test per refusal in each B4
step, "a PTY test that passes or fails without a baseline file". The
fixed-controls tests `showcase_fixed_controls_*` are of this kind; the page
itself has the baseline `showcase_page_fixedcontrols`, and the focused,
refused state has `showcase_fixedcontrols_focused_refused`. Other tests
already end without a baseline (for example
`tablepro_connect_development_opens_workbench` and
`jackin_roles_add_role`). The jackin editor key tests `jackin_fixed_*` (P9.3 B4a, one per
refusal on `Scenario::OpenshellFixed`) read each reason from
`Fixed::openshell()`, and four of them also freeze a baseline. The B4c key tests (`jackin_fixed_account_*`, `jackin_fixed_browser_*`, `jackin_fixed_wizard_*`, `jackin_fixed_palette_*`, `jackin_fixed_file_menu_*`) follow the same rule and freeze 13 more baselines.

The legacy Tuiscotti 0.2 dependency is removed entirely; nothing in the
repository uses it.

## Suite composition

Tests live one module per application — `showcase.rs`, `tablepro.rs`,
`holla.rs` in `tests/functional`, and `jackin.rs` in
`apps/jackin/tests/functional` (the `jackin-console` crate, because the test
needs `CARGO_BIN_EXE_jackin`) — plus cross-cutting `lifecycle.rs` (resize and
overlay ownership) and `visual.rs` (strict frozen check), all on the
single shared harness in `tests/functional/support.rs`. The jackin test
crate includes that file with `#[path]`; each crate keeps its approved
baselines under its own `tests/functional/baselines`, and evidence goes to
the workspace `target/functional` for both.

`visual.rs` uses latest Tuiscotti main `92ef2d9` (0.4.0, renamed dev-dependency
`tuiscotti-latest`): a live `tui_test` snapshot is checked with
`FrozenStore::check` and the test asserts `outcome.is_pass()` — an `Ok` result
alone is not a pass, and a missing baseline fails. The approved baseline lives
in `tests/functional/baselines/`; approval was done once with the Tuiscotti
CLI `accept` command, never by the test itself.

TablePro fix: the tab picker's Delete route now shares the Ctrl+W dirty-tab
guard (`request_close_tab` in `src/bin/tablepro/app.rs`). The regression test
`tablepro::tablepro_dirty_picker_close_matches_ctrl_w_and_cancel_preserves_sql`
fails on the original source and passes with the fix.

Verified on macOS (in addition to the Linux evidence in the audit):
`cargo nextest run --locked --test functional --profile functional` — 39 passed.
