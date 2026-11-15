# Current state

This repository is the design reference implementation: a Cargo workspace
of two crates (edition 2024, Rust 1.99.1). `junie-tui` (v0.1.0) holds the
terminal component library and the showcase, tablepro and holla demo
applications. `jackin-console` (`apps/jackin`) holds the jackin demo: a
library of the jackin console screens plus the `jackin-preview` binary. It is
the exact authority for product appearance and interaction (see
[product](product.md)). The modular multi-crate workspace described in
[architecture](architecture.md) is the future refactoring state; it
does not exist in this tree yet.

## Structure

| Path | Owns |
| --- | --- |
| `src/lib.rs` | Library root (`junie_tui`); public module surface. |
| `src/core/` | Foundational types: IDs, events, intents, validation, diagnostics. |
| `src/shell/` | Application shell behaviour shared by every app: focus routing, hit testing, layers, modals, hints, help, quit guards. |
| `src/ui/` | Rendering context, overlay layer (`RenderCtx::overlay`, `flush_overlays`, `begin_modal`, `modal_backdrop`), paint primitives. |
| `src/widgets/` | The reusable component families (lists, trees, tables, fields, dialogs, menus, pickers, editors, scroll regions, splits). |
| `src/bin/showcase/` | Showcase: the executable component catalog (54 sidebar pages, plus the hidden pages `terminalpane`, `fixedcontrols` and `terminalviewport`). Thin consumer: pages add wiring, demo data and option tables only. |
| `src/bin/tablepro/` | TablePro: simulated database workbench (connections, explorer, query tabs, results, history). |
| `src/bin/holla/` | Holla: simulated launcher/finder, files, disk cleanup, services. Backends are simulated; tests assert the next in-app consequence. Quit asks `World::quit_work` (live activities, running plans, a cleanup); a confirmed quit runs `World::stop_work_for_quit` (plans stop and finalize through `World::stop_plan`, activities keep their cancel schedule, a cleanup is never stopped), delivers the completions, leaves once nothing is left, and refuses new launches, restarts, plan starts and retries meanwhile. |
| `src/terminal.rs`, `src/terminal/viewport.rs`, `src/terminal/forward.rs` | Terminal pane block (feature `terminal`): `TerminalState`, `TermView`, `TermEvent`, `TerminalView`, `TerminalViewport`, `ForwardPolicy`, `encode_key`. The state renders a `termpane` grid fed from a byte stream, returns the events the stream raised (replies, title, clipboard, bell, notification, cwd, scrollback cleared) and does no I/O. `TerminalViewport` owns a state and is the terminal counterpart of `TextViewport` behind `widgets::viewport_ops::ViewportOps`: the scroll position is a row count from the live tail, kept on the same text while output arrives, rows are numbered from the first row ever pushed into scrollback so a selection survives output, the alternate screen has no history and a new screen or a cleared scrollback drops the selection, a copy joins soft-wrapped rows and trims blanks only at the end of a logical line, and the last column of the area is always the scroll track (`TerminalViewport::grid_size` gives the program every row and every other column, so its width never changes when history first appears; there is no bottom bar). A pane scrolled back fades its first and last rows like every scroll view; the live tail does not. `TerminalState::focus_events` and `mouse_reporting` read the modes the program turned on, `clear_scrollback` (on the state and on the viewport) drops the history and keeps the screen, and `ViewportOps::draws_track` tells a host that the view draws its own scroll track. `ForwardPolicy` is the pure byte filter for a pane: no key sends `Ctrl+D` or `Ctrl+Q`, a paste loses its control bytes (an `ESC` in it cannot close the bracket early), a paste with newlines into a program without bracketed paste is held (`Paste::NeedsConfirm`) until the host confirms (`ForwardPolicy::confirmed`), and a paste over `paste_max_bytes` (64 KiB) is refused (`Paste::TooLarge`). |
| `apps/jackin/src/` | The `jackin-console` library (`jackin_console`): the jackin screens (manager, editor, accounts, usage, settings, cockpit, capsule, prelude, roles, doctor and the modals), the `domain` types, `rain` and the other jackin-only modules, over the plain-data `world::World` and the `backend::Backend` and `backend::OnePassword` interfaces. The Capsule reaches each pane only through `panes::PaneBody` and `junie_tui::widgets::viewport_ops::ViewportOps`; `Backend::spawn_pane` and `Backend::initial_panes` build the bodies. `panes::TerminalPane` is the library's terminal body (no `demo` needed): a `PaneLink` (`send`, `set_size`, `drain(now_ms, budget)`) opened lazily by a `LinkSource` at the first size that fits (20 columns by 4 rows, the track column not counted), with the seven link states `LinkStatus::{Pending, Attaching, Attached(Lease), Detached(DetachCause), Exited, Failed}`, the replay window after an attach (`PaneTiming`: closes at 800 ms, or after 600 ms once the output was quiet for 150 ms; the pane shows a placeholder and sends no reply meanwhile), focus reports (`ESC [ I` and `ESC [ O` when the program asked for them; the replies, the OSC 52 copies, the wheel-to-arrows and the focus reports go out only while the pane owns the lease, and keys and a paste always go out because typing is how a viewer takes the lease) and attention (a bell or notification while the pane is not the focused pane of the active tab marks it `Blocked`; exit code 0 is `Done`, another code `Blocked`). `PanePolicy` (in `AppOptions.panes`; the preview sets `ClipboardWrite::Deny`) decides what the program may do: the pure rule `decide` (`panes/clipboard.rs`) takes an OSC 52 write only from the focused agent pane within `gesture_window_ms` (5,000) of the operator's last forwarded key or click in it, up to `max_decoded_bytes` (100,000), and refuses reads and every shell-pane write; each refusal shows its status at most every `refusal_status_every_ms` (10,000) per pane. The Capsule gives the policy to every daemon (`Daemon::set_policy`), tells each pane its size after every layout change and its focus after every event (`CapsuleScreen::sync_focus`, also when a menu, palette or dialog opens or closes), draws the pane's `face` (the grid, the dimmed grid, or an empty state) and its one-row `note`, drains the pane's `PaneEffect`s (a status, or a copy) on its tick, asks before closing a pane only when `close_needs_confirm()` (a pane with a live program), confirms a multi-line paste into a pane without paste protection, and shows the program's title (sanitised through `domain::shown::shown`, at most 64 characters and 64 columns; a server notice is cleaned the same way and cut at 240 characters) as a `Title` row of the container info. `Go::Attach` builds the daemon from `Backend::initial_panes` when an instance has none. |
| `apps/jackin/src/fixtures/`, `apps/jackin/src/sim/` | `fixtures::Scenario`, `Surfaces::demo` and `Vocabulary::demo` and the 17 fixture worlds (`fixtures::world_for`; `capsule-terminal` is the Capsule with seven tabs of terminal panes, one in each link state, over scripted `sim::link::SimLink`s; `openshell-fixed` is the one with `Surfaces.fixed` and the OpenShell words; `launch-checks` is the one whose backend refuses a role the workspace does not allow and asks the env values a role prompts for), the start table `fixtures::entry_for` and the one demo constructor `fixtures::app_for`, which calls `app::assemble`; the simulated `Backend` (it owns the launch plan and `sim::launch::SimLaunch`), `OnePassword`, launch content and the scripted session player (`sim::pane`: `ScriptPane`, `AgentProcess`, `SessionScript`, `Step`). All behind the `demo` feature so the jackin repository can render the same worlds. The library `App` takes a `World`, an `app::Entry` (first page, outro when framed) and `app::AppOptions` (motion, frame, theme, `ClipboardMode`); it has no `Scenario`. |
| `apps/jackin/src/bin/jackin-preview/` | The jackin demo binary: the command line over `fixtures` and `sim`. Binary name is `jackin-preview` (`cargo run --bin jackin-preview --features demo`). |
| `apps/jackin/tests/*.rs` | Library tests of the `jackin-console` data API (`World`, `Snapshot`, the time source, external messages), run by `cargo nextest run --workspace --all-targets --all-features` (most files are gated on the `demo` feature and are empty targets without `--all-features`). They are not part of the CI `functional` suite. |
| `apps/jackin/tests/functional/` | The jackin journeys and their approved baselines (`baselines/functional/jackin_*.tui.pb`), on the shared harness `tests/functional/support.rs`. |
| `tests/functional/` | The root crate's test estate: live PTY functional/integration tests plus the frozen visual gate. See [functional testing](functional-testing.md). |
| `.velnor/config.toml` | Source of truth for generated CI (`.github/` is generated, never hand-edited). See [CI](ci.md). |

## Cargo features

| Feature | Default | Adds |
| --- | --- | --- |
| `cli` | yes | `clap`, `serde_json` (Holla's persisted JSON codec, see below), the `cli` module and `Motion` as a `clap::ValueEnum`. The four binaries require it; `jackin-console` enables it on its `junie-tui` dependency. The library itself has no `serde_json` dependency with `--no-default-features`. |
| `terminal` | yes | `termpane` and the `terminal` module. |

`jackin-console` features (none is a default):

| Feature | Adds |
| --- | --- |
| `demo` | `clap`, `junie-tui/cli`, the `fixtures` and `sim` modules, the `jackin-preview` binary and the `functional` test target. A consumer that supplies its own `World` and `Backend` leaves it off. |
| `product` | Nothing. It lets the generated CI select the product shape (`--no-default-features --features product`); no code may test it. |

`tests/product_manifest.rs` fixes this shape, and
`tests/library_has_no_demo.rs` keeps demo names out of the library
outside `fixtures/`, `sim/` and `bin/`.

## jackin-console data hooks

A consumer changes content, never structure, through `World` fields and
`Surfaces` (`domain::surfaces`). `Surfaces` has no library `Default`:
`WorldParts::new` takes it, `fixtures::Surfaces::demo(scenario)` is the demo
value (so the approved baselines do not move) and
`Surfaces::openshell(LiveFacts { version, config_path, gateway })` is the live value (it builds the About rows from the `Vocabulary` labels). A new field
is a compile error in both. The host adds its own data with the builders
`.roles(..)`, `.docker(..)` and `.doctor(..)`, never text.

| Hook | Default | What it changes |
| --- | --- | --- |
| `World::new(WorldParts)` | | Builds a `World` without a struct literal; `WorldParts::new` takes the clock, home, cwd, `OnePassword`, `Backend` and the `Surfaces`; later fields get defaults. |
| `World.time` | `TimeSource::Sim` | `Wall(WallSource)` follows the host clock and UTC offset in `World::tick`. |
| `World::apply(Snapshot)` | | Replaces workspaces, roles, instances, global config, accounts, surfaces, GitHub repos, the discovery state and `daemon_health` (`Snapshot.daemon_health`); keeps panes (a `Snapshot` carries nothing about daemons, so a refresh closes no tab and no pane), jobs, clock, filesystem and the state of the doctor (`DoctorSurface.state`), which only a doctor run changes. |
| `Effect`, `Msg`, `Backend::perform`, `settle`, `due` | | A save effect carries the id (`None` for a new workspace), the `base` the screen loaded and the record to store (`SaveWorkspace`, `SaveGlobal`, `SaveAccounts`); `DeleteWorkspace`, `Restore` and `TrustRole` carry a key. After an effect no screen writes the `World`: a successful reply carries the `Snapshot` and `World::apply` installs it (`WorkspaceSaved`, `GlobalSaved`, `AccountsSaved`, `WorkspaceDeleted`, `Restored`, `RoleLoaded`, `RoleTrusted`, `Stopped`, `Purged`, `Refreshed`), a failed reply carries the error text and the screen keeps its pending copy and shows the text. `AccountRefreshed` carries a `Refresh` and `AccountValidated` a `Validation`, each with the credential source it was made for, and `World::absorb` writes them into the account only while that source is still the account's. A refresh also carries a `ticket`: `World::refresh_account` (the only starter; Accounts and Usage call it) allocates it with `World::next_ticket`, records it with the source as the account's pending refresh and sends `Effect::RefreshAccount { ticket, .. }`; it starts nothing while a refresh for the same source is pending, whichever page asks. `absorb` accepts an `AccountRefreshed` only when account, source and ticket all match that pending entry (then it clears the entry), otherwise it returns `Absorbed::Stale` before any account, screen, form or job sees it, so a duplicate or older reply cannot overwrite a newer result. `World::apply` reconciles the pending entries after every registry replacement (drops those whose account is gone or whose source changed, restores `Freshness::Refreshing` on the rest); a host that changes `accounts` or a source any other way must call `World::reconcile_pending_refreshes` before the next reply is absorbed. An external backend must echo the exact `ticket`, `account` and `source` of the effect in its `AccountRefreshed` reply, including for a transport error; there is no automatic timeout or retry. `Backend::perform` answers with `Completion::ready(after_ms, msg)` or `Completion::settle(after_ms)`; the second makes the world call `Backend::settle(effect, world)` when the reply falls due. A reply with no delay is delivered after the input that started it. `Backend::due(world)` lets a backend raise a message on its own (the simulated second client). The simulator (`sim::SimBackend`) owns the workspace id counter, the first-refresh failure, the broker outage, the heartbeat of `Instance.last_seen_secs` (answered with `Effect::Refresh`) and the takeover timer; `sim::snapshot(&World)` builds its replies. |
| `App::with_external`, `Remote` | no receiver | Lets another thread send `Remote::Msg` or `Remote::Reloaded` to the app. |
| `Backend::spawnable`, `Backend::capsule_left`, `Backend::pane_timing` | `Ok(())`, no call, `PaneTiming::default()` | `spawnable(&World, &Instance, &PaneKind)` answers whether a pane may start; an `Err(PaneRefusal)` (`MainProcessOpen { agent }`: one pane for each main process) disables the row of the new-tab picker, and `PaneWords::refusal` words it from `main_open`. `capsule_left(&World, &str)` runs once when the operator leaves the Capsule of an instance (Ctrl+B d, any page of the console, the end of the instance); tabs, zoom, pickers, dialogs and the clock are not leaving, and quitting the process is the backend's own drop. `App::go` is the one owner of a departure: when it takes the route off the open Capsule it calls `Daemon::blur` on that instance's panes before it returns and before `capsule_left` runs, so an offscreen program is no longer focused for attention (a bell marks it `Blocked`) or for the OSC 52 focus check, and a focus-out report goes out only where the pane's own rules allow it (live link, lease owner, focus reporting on). A refused or same-instance `Go::Attach`, and a `Go::NewSession` in the open instance, are not departures. `Go::NewSession` in another instance continues through the internal `navigate`, so the outer `go` blurs and notifies once. The clipboard policy and the gesture window are unchanged. `pane_timing()` is the replay window a backend gives the `TerminalPane`s it builds. |
| `Msg::PaneWake { instance, pane }` | not sent | A backend sends it (through `Remote::Msg`) when a pane's link has output or an event waiting. `World::wake_pane` drains that pane at once instead of at the next 80 ms tick, and answers `Changed` only when the pane had something new, so a burst of wakes before a frame asks for one redraw. A changed pump also republishes that instance's `Instance.daemon` summary once through `refresh_snapshot`, as a tick does, so the Manager row agrees with the pane (for example after `Exited`) before the next tick; an empty, missing or unmatched wake rebuilds nothing. |
| `LinkFault`, `FailNext`, `PaneWords::failure` | `FailNext::None` | `LinkSource::open` and `restart` return `Err(LinkFault)`, and `EndKind::{Refused, Failed}` and `LinkStatus::Failed` carry one: the host's `reason` and a typed `next` (`FailNext::RunDoctor`). The failed note and its status read `Press Enter to retry · could not attach to {agent}: {reason}`, then ` · run jackin doctor` for `RunDoctor` (`PaneWords::failed_next`, `next_doctor`); the key stays first. |
| `Surfaces.vocabulary` | `Vocabulary::demo()` (in `fixtures`) | Names the project, sandbox, terminal, gateway, GitHub, quit, stop and purge strings (`Vocabulary::openshell()` is the OpenShell set, with the capsule word `Capsule`; `DaemonWords` and `LifecycleWords` group the gateway and the stop and purge texts). The capsule words are fields too: the About, paste and close labels and bodies, the empty-clipboard, unavailable, starting and cleared statuses, the export result with its optional reveal and open notes, the optional host-log row, the optional config paths and save errors of the editor and Settings (with the shared `Next` line), the pane-menu reason for an empty clipboard, the optional `Sandbox ID` and `Invocation ID` rows of the Debug info (a `None` hides the row), and the status after an attach. `Vocabulary.pane` (`PaneWords`) holds what a terminal pane says: the note of each link state, the placeholders, the paste confirm, the clipboard statuses and the `Title` label. In the OpenShell set the Manager names the multiplexer `Capsule`, like the Capsule screens. A `None` path or note hides the row or the clause. No screen holds a demo literal; `tests/library_has_no_demo.rs` scans for them. |
| `Surfaces.version`, `Surfaces.cockpit_version`, `Surfaces.about` | `0.9.2`, `0.6.4 · preview`, the four demo rows | The version shown in the capsule Debug info; the cockpit Debug info shows `cockpit_version` and falls back to `version` when it is `None` (the live value); the rows of the About dialog. |
| `Surfaces.config_path` | `None` | The host's config file (`LiveFacts.config_path`). It fills `{config}` in the `Vocabulary` templates of the editor and Settings Scope rows (`workspace_config`, `global_config`) and of the two save-failure `Error` rows (`save_error_workspace`, `save_error_global`); `World::config_file` shows it with `~`. The demo templates use `{home}` and `{name}` and ignore it. |
| `Surfaces.account_form` | four providers, three sources | Provider and source lists, labels, the preselected source and the 1Password note. `AccountForm::default()` is the demo's four providers; `AccountForm::openshell()` lists every provider (`Provider::ALL`, D26), marks the local folder `(recommended)`, preselects it and sets the note. |
| `Account.launch_blocked`, `Account.edit_blocked` | `None` | `launch_blocked` is a `LaunchBlock` (`UsageOnlySource`); a blocked account is not offered for launch and the picker shows the text of `Fixed.launch_usage_only` (`LaunchBlock::text`, `Usability::label`). An account the form cannot represent keeps its other actions. `edit_blocked` is an `Option<EditBlocked>` (`text` and a required `short`): the status shows `text`, the menu border shows `short`. |
| `Surfaces.agent_blocked` | `None` | An optional host restriction disables every agent except its named launchable agent. OpenShell leaves this unset; an explicit `AuthMode::Ignore` still disables that agent's launch row. |
| `Backend::launch_content()` | the Docker stages | `domain::launch::LaunchContent`: the stage list and the launch words. `LaunchContent::openshell()` is the seven OpenShell stages and words, with an empty build log and failure detail for the backend to fill; the simulated backend adds its sample log and detail (`sim::launch::openshell_content`). |
| `Backend::launch(&World, &LaunchTarget)`, `LaunchFeed`, `LaunchEvent` | the simulated launch (`sim::launch::SimLaunch`) | The launch of an instance. The cockpit polls the `LaunchFeed` a backend returns on every step and draws its events (`StageChanged`, `StageTimed`, `Activity`, `BuildLine`, `ContainerReady`, `CredentialsResolved`, `CredentialError`, `Refused(Refusal)`, `NeedsInput(Vec<EnvQuestion>)`, `Failed(FailedLaunch)`, `Ready(ReadyInstance)`); `Failed` and `Ready` carry the instance record the backend made, which the cockpit adds with `World::admit`. The run id comes from `LaunchFeed::run_id`. `Refused` carries four texts the backend wrote (`title`, `found`, `state`, `next_step`): the cockpit draws them as an error dialog and creates nothing, and closing it returns to the manager. `NeedsInput` carries the questions of the launch (`name`, `prompt`, optional `default`): the cockpit opens one form with a field for each and sends the answers to `answer_env`, and Esc ends the launch through `cancel`. `x` and the Cancel button work while a stage is `Blocked`, and `cancel` is called then too. `retry_credentials(op)` and `use_plain_text(PlainKey)` answer the credential dialog (`PlainKey::new` trims the typed text and refuses blanks, control and direction characters, and the cockpit asks again when it does; `use_plain_text` returns the `TypedKey { origin, validation }` the backend wants shown, or `None`; the cockpit keeps the credentials line it derived itself, puts the origin of a resolved or typed key in front of the validation line, and shows every text of the feed through `domain::shown::shown`: no controls, direction marks or invisible characters, folded white space, at most 200 characters), `telemetry()` names the collector of the debug info (`None` shows no line), a stage shows a duration only when the feed sends `StageTimed` for it, and `cancel` returns the record of an ended launch (a `None` adds nothing). The cockpit writes no instance, no run id and no `OnePassword` session of its own, and creates no daemon: `Go::Attach` builds the daemon of an instance that arrives from the cockpit without one, with one tab for each `PaneRequest` of `Backend::initial_panes(&Instance)` and the body from `Backend::spawn_pane` (an agent request without an account gets the account the launch chose, because `Instance` does not carry it). The simulator keeps the launch plan (`Clean`, `FailNetwork`, `CredentialsLocked`, `BlockedSidecar`, chosen from the scenario and the `OnePassword` session) and, only for `SimBackend::guarded()`, the opening checks (`sim::launch::opening`: a workspace that does not allow the role refuses it, and the prompted `env` entries of the role manifest in `Surfaces.roles` are asked) and the `1Password locked` text; no library module names them. |
| Typed reasons (BG) | the simulator fills each | A backend names why something did not happen and the console words it from `Surfaces.vocabulary`: `OpError::NotAvailable`, `FailedLaunch.instance: Option<Instance>`, `SaveFailure { error, next }` (the `Next` row of a failed save is `next`, and absent when `None`), `Msg::DoctorFinished(Result<DoctorReport, DoctorFailure>)` and `Vocabulary::unbuilt(Unbuilt)`, `notices(&[Notice])`, `discovery(DiscoveryError)`, `engine_endpoint(&EngineEndpoint)`. `DoctorSurface.rerun_secs` makes `World::tick` start the checks again that many seconds after the last run; `health_view` is `Failing` for a failing check, a report with no gateway or a run that could not happen, and `Stale` for a stale report or a failed listing (`daemon_health`). `instance_id(suffix)` and `ID_PREFIX` own the `jk-` prefix. |
| Typed rejections and persistence data (BG2) | the simulator fills each | `Vocabulary::rejection(&Rejection)` words the short sentences a backend writes about a record it cannot store or find: `Rejection::{CannotSave(SaveField), NoProject, NotAFolder { folder }, NoRoleLeft, NotInRoster { instance }, UnknownRole { role }, RoleNotLoaded}` over the fields `cannot_save`, `no_project`, `not_a_folder`, `no_role_left`, `not_in_roster`, `unknown_role` and `role_not_loaded` (`SaveField::label()` names the 23 fields). `Unbuilt::AddGitRole` is the 21st unbuilt sentence. `LifecycleWords.purge_refused_next` (`None` in the demo) is appended to a refused purge: `{error} · {next}`; the console offers only the guarded purge, and the OpenShell set names `jackin purge --force`. `Workspace.github: Option<RepoRef>` is the repository of a saved workspace, and `World::workspace_repo` reads it (no instance is needed). `Account::is_unsaved()` and `UNSAVED_ID` (the empty id) mark the account a form sends to be stored; the backend gives it an id and names it in `Msg::AccountsSaved { added, result }`, and the screen selects that account. `RoleFinding.fix` is an `Option<String>` (the hint row is drawn only when it is `Some`; `key` was already optional). `ResolvedAccount.explicit` is false when the account is only the first one a workspace enables; `World::offer_for` preselects an account only when a rule chose it or exactly one is ready, so two ready accounts with no default preselect none and the launch picker asks (`manager.account`, `screens::modals::account_picker`, shared with the capsule). |
| Typed launch words and ends (BG3) | the simulator fills each | `LaunchEvent::Refused(RefusalReason)` (`RoleNotAllowed { workspace, role }`, `AgentNotLaunchable { agent, launchable }`) replaces the four-string `Refusal`; `LaunchWords::refusal` words its title, `found` line, `State` and `Next step` from `role_not_allowed`, `agent_not_launchable` and `refused_state`. `LaunchFailure.next_step` is a `NextStep` (`None`, `Retry { sandbox, launch }`, `RunDoctor`, `FixCredential`) worded by `LaunchWords::next_step`; `None` draws no `Next step` row. `InfoDialog` wraps an intro line that does not fit and honours `Prop::wrap` in its own list (a plain `PropsList` still clips). `World::offer_for` with no workspace offers the only ready account of the provider when no default and no discovered account exists (`AccountRegistry::only_ready`), after the agent-blocked and `Ignore` checks. `LaunchContent.when_empty` (`EmptyLanding::{Outro, Manager}`) decides where a failed, refused or cancelled launch ends when nothing runs: the demo keeps the outro, `LaunchContent::openshell()` returns to the manager. `Msg::LaunchCancelled { instance }` carries the record of a cancelled launch that the feed could not return from `cancel`; `World::absorb` replaces the row of the same id or adds it. `LaunchEvent::Warning(DockerCapability)` (`risks`) draws the Docker banner and the risk list from the event; without it they come from `Surfaces.docker`. |
| Typed reasons, outcomes and limits (BG4) | the simulator fills each | `RecoverableIssue.reason` (`IssueReason`), `CheckRow::{Ok(Found), Failed(IssueReason), Skipped(IssueReason)}` and `Refresh.outcome` (`RefreshOutcome`) replace the three free strings. `IssueReason` covers 1Password (`OnePassword(OpError)`: locked, not signed in, not available, access denied, account, vault, item or field not found, field empty or not concealed), `EnvInvalid`, `EnvMissing`, `EnvEmpty`, `BadReference`, `FileMissing`, `FileNotRegular`, `FolderMissing`, `FolderUnreadable`, `LoginMissing`, `LoginInvalid`, `WrongProvider`, the key cases, `Usage(UsageReason)` (the D25 reasons: `NoCredentials`, `ExpiredLogin`, `Unsupported`, `RateLimited`, `Network`, `Provider`), `Skipped` and the cases the simulator raises. `Vocabulary.reason` (`ReasonWords`, with a `Phrase { issue, check }` for each reason) words them: `Vocabulary::reason` for an issue line, `check` for a check row, `found` for a passed row, `refresh_outcome` for the status after a refresh and `op_error` for 1Password (`OpError::message()` is the same words for the picker, which holds no `Vocabulary`). The templates take the facts of the value; the demo and the OpenShell set share the words, and the demo's texts are unchanged. `ValidationState::label` and `Usability::label` take the `Vocabulary`. `OpVaultRow.access` and `OpItemRow.fields` are `Option`: a `None` draws no access tag and no field count. `OnePassword::accounts()` returns a `Vec`. `ChangeSet.omitted` (files the source did not list) and `ChangeSet.unread` (`UnreadFile`: listed, never read) are drawn by `InspectChanges` with `ChangeWords`, which the capsule fills from `Vocabulary.changes_omitted` (takes `{count}`) and `changes_unread`: `omitted` follows the summary line, an unread file is a row and a tree leaf with the word where the counts go, and a set with only an omitted count or an unread file is not empty. `Effect::RefreshAccount.stagger_ms` stays: the console spaces the requests of a refresh of several accounts by 160 ms (120 ms on the Usage page), and a backend that needs no spacing ignores it. |
| `Surfaces.launch_role` | `Demo` | `Configured` resolves the launch role from the default or the only allowed role, else the picker rows are disabled with a reason. |
| `Surfaces.wizard` | `None` | (`WizardDefaults::openshell()`) One fixed destination, no edit step, no subfolders, and the name from the source folder. The P9.3 design calls this `Fixed.wizard`; the code keeps `Surfaces.wizard`, and B4 does not move it. |
| `Surfaces.fixed` | `None` | `Fixed::openshell()` (`domain::fixed`) makes the workspace editor's `Keep awake`, `Git pull before launch`, `On dirty exit`, working directory `Choose…`, Project row and Add row, and the Accounts tab toggle `Control::Fixed`, each with its reason text, and refuses unchecking the last allowed role. In Settings it fixes the Co-author and DCO boxes (one shared reason line), the global Mounts Add row, the Trust toggle (`o` shows the source in a sticky status), every Agents row (the launchable agent and the others have their own reason and the two foot notes are data) and the global Role overrides (Add row, `a`, `s`, and a scope select that lists only `global`). In the env form it dims the 1Password and host variable sources under a plain-values note, refuses the `p` key, adds a note under a key name with `TOKEN`, `SECRET`, `PASSWORD` or `KEY`, and refuses a plain value that starts with `op://` or `openshell:resolve:` (any letter case, after leading space) with the typed key and value kept. Each screen refusal is posted by `App::on_route` through `screens::post_refusal`, and a refusal raised inside a modal by `App::post_layer_refusal`. The env form stays open on Save so an error shows, in every scenario. This is a deliberate change to the demo, limited to the error path: before B4b the demo form closed at once on any error raised by the save step (an empty value, a duplicate key), with no message and the typed input lost, and a valid Save is identical in both builds. The value input is required in every scenario (label `Value for the key *`, `Required` under it on an empty Save). A rename to a key already in the scope is rejected before anything is staged, so Esc after the error leaves no change. The OpenShell words for the three texts that would promise more than jackin does are `Vocabulary` data and the demo keeps its own: `env_op_label` (`1Password item / field`), `trust_hint` (`Show source`) and `agent_no_credentials` (`no credentials handed to the sandbox`). From B4c on it also fixes the Accounts screen (the global enable: the `t` key, the Disable button and the menu item, short `no disabled state`; the Plain-text API key option and the `Enter plain text instead` button, with the reason as a note under the radio; Edit… on an account with `edit_blocked` is a fixed button), the file browser of the wizard (the Read-only box, with its reason drawn beside it by `FileBrowser`, and the Git URL validator, which always answers `Fixed.git_url`), the wizard destination step (both options dim, the radio disabled, the reason as a line, `Next` the initial focus) and the capsule reveal and open rows (palette detail and File menu border `prints path only`; `Fixed.reveal_open.text` has no input route yet and is for the export status of J5). `domain::openshell::env_edit_block(var)` is the one definition of the Edit… block of a host-variable account, and `surfaces::fixed` sets it on those accounts. The scenario `openshell-fixed` renders it. From BG2 on `Fixed.save_hint` (`Saved {name} · set a default role in Roles`) is the status after an editor save when the saved workspace has no default role and more than one role is configured (the name is cut to 20 columns with `truncate`, so the longest status is 56 columns); the demo has no `Fixed` and keeps `Workspace {name} saved`. The new-workspace wizard starts a workspace with `domain::openshell::policies` (keep awake off, git pull off, dirty exit ask) under `Fixed`, so the editor's boxes match its record; the demo keeps git pull on. |

## Fixed fields of the OpenShell world

`domain::fixed_fields` names the class of every field of the records a
jackin world is made of. Fixed: the field holds one OpenShell value in every
jackin world; `workspace_fixed`, `global_fixed`, `account_fixed`,
`role_fixed`, `instance_fixed` and `surface_fixed` read it, and the tie
compares it. Data: the host supplies it; the extractors name it `field: _`.
Console: a `World` field the running console owns (`clock`, `backend`, `op`
and the like) that projection never produces; it is also `field: _`. Every
record is destructured without `..`, so a new field does not build until it
has a class. `world_fixed(&World)` and `snapshot_fixed(&Snapshot)` give the
same `FixedFields` view of a world and of a projection.

| Record (fields) | Fixed | Data |
| --- | --- | --- |
| `Account` (21) | `enabled` (`true`), `origin` (`Registered`) | the other 19 |
| `Workspace` (13) | `workdir` (`/sandbox/project`), `mounts` as one `MountShape`, `keep_awake` (`false`), `git_pull` (`false`), `dirty_policy` (`Ask`), `accounts` (below) | `id`, `name`, `roles`, `env`, `role_env`, `docker`, `github` |
| `AccountPolicy` (4) | `disabled_defaults` (empty), `role_preferred` (empty), `enabled` (through `enabled_is_every_account`: every account is effective) | `preferred` |
| `Mount` (8) | `destination`, `readonly`, `isolation` (`Clone`), `kind`, `scope`, the source being a host path, `drift`, `running_isolated` | the source path |
| `GlobalConfig` (7) | `coauthor_trailer`, `dco_signoff` (`false`), `mounts` (empty), `role_env` (empty), `agent_modes` (every agent but `Fixed.launchable` is `Ignore`), `trust` through `TrustRow.trusted` | `env`, `agent_modes[launchable]` |
| `TrustRow` (4) | `trusted` (`true`) | `source`, `kind`, `roles` |
| `RoleEntry` (8) | `trusted` (`true`: a source is trusted because the operator configured it, D28) | `namespace`, `name`, `source`, `in_registry`, `description`, `load_error`, `fetch_error` |
| `Instance` (18) | `workdir` (`/sandbox/project`) | the other 17 |
| `AccountRegistry` (2) | `accounts`, each as an `Account` | `revision` |
| `Surfaces` (13) | `vocabulary`, `fixed`, `account_form`, `agent_blocked`, `wizard`, `launch_role` | `roles`, `docker`, `doctor`, `version`, `cockpit_version`, `config_path`, `about` |
| `World` (24) | `workspaces`, `roles`, `instances`, `global`, `accounts`, `surfaces` (through the extractors above) | `github`; Console: `clock`, `time`, `arbiter`, `home`, `cwd`, `daemons`, `op`, `backend`, `fs`, `clipboard`, `jobs`, `daemon_health`, `last_refresh_secs`, `session_accounts`, `discovery_pending`, `tickets`, `pane_policy` |
| `Snapshot` (9) | `workspaces`, `roles`, `instances`, `global`, `accounts`, `surfaces` | `github`, `index_error`, `daemon_health` |

The normalizers of `domain::openshell` (`account`, `workspace`, `global`,
`role`, `instance`, `surfaces`) hold the one definition of each Fixed value;
the `openshell-fixed` world (`fixtures::surfaces::fixed`) and jackin's
projection both call them, and `Surfaces::openshell` takes its six Fixed
fields from `SurfaceFixed::openshell()`. `PROJECT_DIR` (`/sandbox/project`)
is the working directory of workspaces, instances, the mount destination and
the wizard. The `openshell-fixed` world holds no `Discovered` account:
jackin has no host discovery (D25, D26), so its five demo `Discovered`
accounts are `Registered` accounts (four named `host login`, one `ci-bot`,
provenance `ConfiguredSource`), and the world takes the live `account_form`,
`agent_blocked`, `launch_role` and `wizard`. No approved baseline moved.

Known residual hole: a field that is wrong in this table and wrong in the
expected literal of a test passes every check. The destructuring forces a
class for each field, and `tests/fixed_fields.rs` pins each Fixed value with
a literal, but nothing derives the class from a second source. Only a review
of this table closes the hole; `Fixed::openshell()` is the second source to
read it against, because each of its reasons names a Fixed control.

## Rules that hold today

- Behaviour lives in the shared library (`src/core`, `src/shell`,
  `src/ui`, `src/widgets`), never duplicated per screen or per app.
  Apps provide routes, pages and configuration only.
- Every widget follows the same conventions: builders, events,
  outcomes, one keyboard-focus gutter (`Theme::gutter_symbol`),
  selection distinct from focus, light-dismiss owned by the popup.
- Behaviour is overridable in layers: app/profile policy, then
  per-widget setting, then user key remapping.
- A control with one true value is `Control::Fixed(Reason)` (`core::refusal`): `Checkbox`, `Select`, `Button`, one `RadioGroup` option and one `ListItem` take `.fixed(reason)` or `.control(..)`. It stays in the focus ring, draws the disabled look with the `▎` gutter when focused, and refuses an edit: the value is untouched, the widget returns `Consumed` (`Select`, `Button`: `(Consumed, ..)`) and raises the reason into a thread-local slot (`RefCell<Option<Reason>>`). The call site reads the slot with `take_refusal()` (which clears it) and posts it with `Cx::refused(Reason)`, a neutral status of the reason text alone that lasts 5 s. `shell::handle` clears the slot at the start and at the end of each input, and redraws whenever an input changed the status line (text or tone), so a refusal by key paints at once. `Cx::sticky` posts a neutral status that does not expire and clears on the next key, paste or mouse press (`StatusLine::set_sticky_with`); it is for text a user must read or copy. A `Dialog` routes its mnemonics and its confirm key through `Button::activate`, and every finish (Enter in a prompt field, the typed acknowledgement) through `Button::refuse`, so a fixed action refuses on every route and `Dialog::hints` does not offer it. A fixed `ListItem` keeps its checked state through every selection path. `Reason` limits: text 60 columns, `short` 20 (`Reason::within_limits`); `ContextMenu::reason_room` is the room of the menu border. The showcase page `fixedcontrols` shows it. `Table` rows have no fixed state yet.
- The showcase Query workbench keeps one `CodeEditor` per document, selected by an authoritative key, not the tab strip. Record: [Query workbench document keys](audits/2026-11-15-query-workbench-document-keys.md).
- A Query workbench run is bound to its document key, a per-document generation and the SQL captured at run start (`OperationToken`, `RunRequest`). Completion is admitted once, by key and generation; a late edit, a close or a cancel cannot redirect or double-apply it. Record: [Query workbench run identity](audits/2026-11-15-query-workbench-run-identity.md).
- The showcase demonstrates every library capability; the coverage
  gate is described in
  [shared components plan](architecture/shared-components-plan.md).
- Holla keeps every `Activity` record for the lifetime of its simulated
  `World`. Closing a tab removes only the tab, so a batch keeps counting
  its members by ID after the tab closes, and reopening a tab shows the
  retained record without starting it or applying its effect again. No
  history eviction exists yet. Keeping records linear in session history is
  intentional, and idle scheduling must not assume a bounded record count.

## Holla persisted JSON

Holla decodes and encodes every persisted store (`sizes.json`,
`trusted.json`, `frecency.json`, the ops log) and every discovery input
(`package.json`, `mise tasks`, `brew services`) with `serde_json`, through
the single codec in `src/bin/holla/domain/manifest.rs` (`parse_json`,
`json_str`, `json_u64`, `json_i64`). No store has its own escaping or number
parser. The policy:

- Strings are exact UTF-8. Quotes, backslashes, newlines, control
  characters and non-BMP scalars round-trip unchanged, and an escaped
  surrogate pair decodes to one scalar.
- Integer fields (sizes, mtimes, timestamps, schema versions, stamps) are
  read as `u64` or `i64` with no `f64` step. `9007199254740993`, `u64::MAX`
  and the `i64` limits keep their value. A fraction, an exponent (`1.0`,
  `1e3`), a negative value for an unsigned field, or an out-of-range value is
  an invalid field, even when it is mathematically integral.
- Syntax is strict. Unknown escapes, raw control characters, lone or
  reversed surrogates, `NaN`, `Infinity` and trailing data are errors. A
  malformed file is reported as unreadable and the store starts empty.
- Duplicate object keys keep the last value. Before #63 the first match
  won.
- Unknown object fields are ignored. An invalid entry is skipped and its
  error is recorded as the store's first diagnostic (`sizes.json entry 1
  invalid: ...`). Valid siblings are kept, and the invalid entry is never
  filled with a default.
- Timestamp arithmetic on loaded values saturates, so an extreme stored
  value cannot panic.
- One-time loss of legacy caches. Before #63, `sizes.json` escaped only
  quotes. A path with a bare backslash (for example `\d`) or a raw control
  character is invalid JSON now, so the file loads empty with a
  `sizes.json unreadable` diagnostic and rebuilds on the next scan. A
  backslash that old files wrote unescaped is now read as an escape. `\n`
  becomes a newline and `\\` becomes one backslash, so that entry misses its
  real path and is a cache miss. `trusted.json` and `frecency.json` already
  escaped backslashes and quotes, so only a raw control character in a stored
  path or item makes them unreadable. Those files start empty, and the next
  save replaces them. Trust decisions are then asked again, and usage history
  restarts. No other data changes on upgrade.

## Testing today

- `cargo nextest run --locked -p junie-tui --test functional --profile functional`
  — 117 tests, 0 skipped (the demo journeys of `jackin-console` add 51 with
  `--all-features`): every app end-to-end (launch → operate →
  verify consequence → reopen → clean exit), component families in
  Showcase and in a real consumer, colour matrix
  (truecolor/256/16/mono/`NO_COLOR`), normal and small sizes.
- Every test but the key tests of a refusal page ends in
  `App::visual("<id>")` (see [functional testing](functional-testing.md)), a `FrozenStore` check
  against a committed baseline in `tests/functional/baselines/`.
  Baselines are approved only via the Tuiscotti CLI `accept` command.
- Full account of the deleted legacy estate and its replacement
  mapping: [functional testing](functional-testing.md).

## API changes for consumers

Changes to the public API of `junie-tui` since `origin/main`. A consumer pins a
rev of this repository, so each break lands at its next bump. No
`#[non_exhaustive]` is added to `Input` or `Copied`: no rule in
[AGENTS.md](../AGENTS.md) or [requirements](requirements.md) asks for it, the
other public event enums (`Outcome`, `MouseKind`) carry none, and adding it
now would be a second break for every exhaustive consumer match. A new
variant of these enums stays a breaking change and is recorded here.

| Item | Change |
| --- | --- |
| `shell::Clipboard` | Lost `Copy`, `PartialEq` and `Eq` (it holds `native: Option<Arc<dyn NativeClipboard>>`). `install(self)` became `install(&self)`. New `native(..)` builder and `NativeClipboard` trait. |
| `shell::Copied` | Gained `Requested { chars }` (the native sink refused and OSC 52 was queued instead). `TooLarge { encoded, limit }` became `TooLarge { bytes, limit }`: `Clipboard::limit` (default `DEFAULT_LIMIT`, 100,000) counts decoded bytes, as the jackin pane policy does. With a native sink that took the text, no OSC 52 is queued. |
| `core::event::Input` | Gained `External(External)`. `Application::external()` and `shell::App::external()` are new (default `None`). |
| `clock::ClockStep`, `Shell::clock_steps` | `ClockStep` (`PREFIX`, `request(ms)`, `parse -> Option<u64>`) became the enum `clock::TestRequest { Advance(ms), Fence }` (`ADVANCE_PREFIX`, `FENCE`, `FENCE_ACK`, `advance(ms)`, `parse -> Option<TestRequest>`). `Shell::clock_steps` became `Shell::test_requests`. Both stay inert unless `JUNIE_TUI_TEST_CLOCK=1`. |
| `terminal::TerminalState::feed` | Program output passes a guard first (`src/terminal/guard.rs`): a string sequence (OSC, DCS, APC, PM, SOS) over 144 KiB is dropped whole until its end, at most 16 characters join one cell (combining marks, variation selectors, joiners and what a joiner pulls in), and DEL, the bidi controls (U+061C, U+200E, U+200F, U+202A to U+202E, U+2066 to U+2069) and the tag characters (U+E0000 to U+E007F) never reach a cell. A byte that is not valid UTF-8, or a lead byte cut short by the next byte, becomes one U+FFFD and never passes on as it is. `terminal::is_spoofing_format` is that set, and the jackin console's title filter uses it. A 100,000 byte OSC 52 write still fits. |
| `terminal::encode_key` | Ctrl and Shift on arrows, Home, End, Page, Insert, Delete and F1 to F12 now send the xterm parameter forms (`CSI 1;m X`, `CSI n;m ~`), Ctrl+Backspace sends `0x08`, and Ctrl with `@ \ ] ^ _ /` sends the C0 byte. Alt alone keeps the `ESC` prefix; Shift+Enter stays `\r`. |
| `widgets::edit::EditField::validate`, `widgets::input::TextInput::validate` | Required precedes the custom validator. Empty required text (zero bytes; whitespace is not empty) fails with the required message and the validator does not run. Other text runs the validator once. Before, a validator replaced the required check, so an app that set `required(true)` with a validator accepting empty text saved an empty value. Apps that want the old result must call `required(false)`. Blank-text rules stay in the validator. See [docs/audits/2026-11-15-required-before-validator.md](audits/2026-11-15-required-before-validator.md). |
| `terminal::TerminalView`, `TerminalViewport` | Intentional edge-case correction (no API change). A view paints only inside the part of its area that overlaps the buffer, and a wide symbol that does not fit completely in the remaining columns (by Ratatui's `CellWidth`) paints one blank cell with its own style, so no emitted symbol is wider than the pane. A wide glyph's continuation cell that is the first visible column paints a blank. The focused cursor is drawn only when it is inside the buffer. `TerminalViewport::screen_pos` returns `None` for a column in the scroll track, past the pane or past the emulated grid; it used to clamp onto the last emulated column. `context_point` then falls back to the last drawn row of the pane. Showcase's Terminal pane page shows it with the `wide edge` fixture and the `Clip last column` option (off by default). |

## Known limits

- TablePro's SQL tokenizer (`src/bin/tablepro/sql.rs`) is total over any valid
  UTF-8 string: every token range starts and ends on a char boundary, and the
  ranges cover the input exactly. Identifiers accept alphabetic or `_` scalars
  first and alphanumeric or `_` scalars after; any other scalar (emoji, combining
  mark, symbol) is one punctuation token. This is a lexical policy, not SQL Unicode
  identifier support. A doubled quote inside a string or quoted identifier stays
  in one token.

- `TerminalState::scrolled_off` is exact while the history is below its
  limit (it follows the growth of `scrollback_len`). At the limit, and with
  a limit of 0, it sums the main-screen scroll operations that start at row
  0, so a delete-line at row 0 over-counts. The feed is split after every
  sequence that can switch screens (`CSI ? 47`, `1047` and `1049` with `h`
  or `l`, and `ESC c`), and each part is counted on the screen it started
  on, so a program that enters and leaves the alternate screen inside one
  feed adds nothing. `ESC[2J` discards the visible rows
  (`EraseDisplayPolicy::Discard`), so it changes neither value.
- `termpane` is a git dependency pinned to commit `46cc0977` (termpane
  `main` after PR #41, which follows xterm for DECSTR and `?1049`, and PR #40, the reuse of OSC 8 links, and PR #39: `EraseDisplayPolicy`, the cap on characters per
  cell, the clamp of the `CSI L`, `M`, `S` and `T` counts and the shared
  OSC 8 hyperlink). It becomes `=0.1.1` or later once published on
  crates.io; see
  [dependencies](dependencies.md).
- The crate is a monolith: library and four binaries in one package.
  Splitting it is the refactoring programme in
  [architecture](architecture.md); no part of that split has landed.
- Applications simulate their backends (PostgreSQL, Docker,
  filesystem). No real external effect is claimed or tested.
- TablePro's save and preview SQL (`model::preview_sql`) and the Showcase
  grid's SQL preview serialize a finite number as the shortest text that
  parses back to the stored `f64` (`1.2345` stays `1.2345`, `1.0` stays
  `1.0`), in assignments, inserted values and original-row predicates. A
  null key matches with `IS NULL`. A NaN or infinite number, a `DEFAULT` in a
  row predicate or a key column missing from the tab aborts the whole
  preview with the row and column named, opens no dialog, starts no save and
  keeps the draft. Digits discarded before the model stored a number cannot
  be recovered, and arbitrary-precision decimals are not supported. See
  [the audit note](audits/2026-11-15-exact-sql-literals.md).
- `DataGrid` keeps its drafts and undo history by row identity. Each row has
  an `ItemKey` (local keys are never reused, provider keys come from
  `try_set_rows_keyed`), `GridRevision` counts source and draft changes, and
  `DataGrid::pending()` is a read-only keyed view. Deleting an existing row
  keeps its staged cells, so undelete and Undo show them again. Removing an
  inserted row is undoable and restores the row, its edits, errors, selection,
  cursor and cell range. `undo()` and `GridAction::Undone` report the affected
  `GridUndo` key. See
  [the audit note](audits/2026-11-15-grid-keyed-drafts-undo.md).
- TablePro's simulator evaluates filters with SQL three-valued logic and
  Rust `regex` syntax, not PostgreSQL's regex dialect or collations. Its
  query editor does not yet read doubled apostrophes inside string
  literals; the lexer owns that fix (see
  [TablePro filter semantics](audits/2026-11-15-tablepro-filter-semantics.md)).
- The `visual-baseline` git tag is frozen historical evidence. It is
  never moved, and it is not a substitute for the current design.
