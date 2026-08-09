# Flagship showcase: TermRock AI & Developer Workbench

**Status:** design SoT for the showcase application  
**Codename:** `termrock-showcase` (binary / example crate)  
**Law:** Built **only** from public TermRock APIs and source-installed registry blocks.  
**Meta-law:** Every weakness found while building is a **missing TermRock primitive or public API** — fix the library; never paper over with private app-only chrome.

**Stacks on:** AgentWorkbench pattern, OverlayStack, InteractionScene, PromptComposer, PermissionPrompt, responsive layout, perf coalesce/follow, capability profiles, handbook docs, quality contracts.

---

## 0. Product thesis

Prove TermRock can ship a **category-leading** terminal experience: an agent + developer workbench that feels as intentional as Claude Code / Amp / OpenCode / lazygit hybrids — while remaining **product-neutral** (mock agent runtime, no provider SDK in the showcase).

The showcase is:

1. **Demo** for humans and agents evaluating TermRock.  
2. **Dogfood** that forces API completeness.  
3. **Recording corpus** for Studio replay and quality gates.

---

## 1. Information architecture

### 1.1 Mental model

```
Sessions ──► Conversation (thread of blocks)
                ├── User / Agent / System messages
                ├── ToolCall / TerminalRun / Diff / Plan snippets
                └── Streaming markdown
Tasks / Subagents (rail) ── parallel work, drill into conversation anchors
Composer ── human input + queue + mode/model
Overlays ── permission, question, plan, diff, files, sessions, palette, help
Status ── connection, context, keymap hints, capability profile
```

### 1.2 Navigation objects (semantic)

| Object | Id prefix | Opened from |
|--------|-----------|-------------|
| Session | `session:` | Session picker, palette |
| Message block | `msg:` | Thread activate |
| Tool call | `tool:` | Thread / activity shelf |
| Task | `task:` | Task rail |
| Subagent | `sub:` | Task rail / SubagentCard |
| Checkpoint | `cp:` | Timeline / palette |
| File | `file:` | File browser / @ mention |
| Command | `cmd:` | Command palette |

### 1.3 App modes (not agent modes)

| App mode | Meaning |
|----------|---------|
| `Workbench` | Default multi-pane |
| `FocusThread` | Thread zoomed (rail drawer) |
| `FocusFiles` | Files pane primary |
| `Review` | Diff or plan fullscreen overlay chain |
| `Help` | Contextual help overlay |

Agent autonomy (`Ask/Plan/Edit/…`) is **AgentMode** on the composer badge, separate from app mode.

---

## 2. Major regions

### 2.1 Default geometry (wide ≥ 120×30)

```
┌─ TermRock Showcase ────────────────────────── profile: modern ──┐
│ Sessions ▾  ·  Workbench  ·  [palette ⌘K]  ·  help ?              │
├────────────┬────────────────────────────────────────────────────┤
│ TASK RAIL  │  CONVERSATION / THREAD                             │
│            │  ┌ user ─────────────────────────────────────────┐ │
│ ● main     │  │ …                                             │ │
│ ● sub-a ⟳  │  └───────────────────────────────────────────────┘ │
│ ○ sub-b    │  ┌ agent · streaming… ───────────────────────────┐ │
│ ○ review   │  │ markdown…                                     │ │
│            │  │ ┌ tool:bash · running ──────────────────────┐ │ │
│ Checkpoints│  │ │ $ cargo test                              │ │ │
│ · cp-3     │  │ └───────────────────────────────────────────┘ │ │
│ · cp-2     │  └───────────────────────────────────────────────┘ │
│            │  Activity: bash · read · search                    │
│ FILES      │  ───────────────────────────────────────────────── │
│ src/       │  PROMPT COMPOSER                                   │
│  lib.rs    │  [chip: main.rs]  mode:EDIT  model:demo  queue:0   │
│  main.rs   │  ▌_                                                │
├────────────┴────────────────────────────────────────────────────┤
│ status: connected · ctx 24k/128k · follow · hints               │
└─────────────────────────────────────────────────────────────────┘
```

### 2.2 Region inventory

| Region id | Component(s) | Collapse priority |
|-----------|--------------|-------------------|
| `chrome.top` | ModeRibbon / session chip / hints | last |
| `rail.tasks` | TaskRail + CheckpointTimeline compact | first (→ drawer) |
| `rail.files` | Tree or List file browser | early |
| `main.thread` | MessageThread / Transcript | never (primary) |
| `main.activity` | ActivityShelf | mid |
| `main.composer` | PromptComposer | high keep |
| `chrome.status` | StatusBar + ContextMeter | mid |
| `overlay.*` | OverlayStack layers | z-order |

### 2.3 Mock agent runtime (showcase-only)

In-process **demo driver** (not a TermRock type):

- Scripted scenarios (see §17) emit `StreamChunk`-shaped events.  
- Tools are faked (sleep + canned stdout/diff/plan).  
- Permissions always go through `PermissionPrompt` before “running.”  
- No network, no real shell unless `TERMROCK_SHOWCASE_REAL_SHELL=1` (optional later).

---

## 3. Focus order

### 3.1 Screen Tab cycle (default)

1. `rail.tasks`  
2. `rail.files`  
3. `main.thread`  
4. `main.composer`  
5. (status is non-focusable chrome unless slot actions)

When a region is collapsed, skip it. Overlay top layer **traps** Tab inside itself.

### 3.2 Within regions

| Region | Internal focus |
|--------|----------------|
| Task rail | List selection |
| Files | Tree selection |
| Thread | Optional block selection; default scroll-only until `/` or click |
| Composer | Editor; chips via BackTab |

### 3.3 Opener restore

Every overlay records `opener_focus` (scene / OverlayStack). Dismiss → restore composer or previous pane.

---

## 4. Semantic navigation

| Intent | Default | Behavior |
|--------|---------|----------|
| `FocusNextPane` | Tab | Scene tab among panes |
| `FocusPrevPane` | Shift+Tab | |
| `OpenPalette` | Ctrl+K / Ctrl+P | Command palette overlay |
| `OpenSessions` | Ctrl+O | Session picker |
| `OpenFiles` | Ctrl+\\ | Focus files / open drawer |
| `ToggleFollow` | f (thread focused) | FollowMode |
| `JumpLatest` | g g / End | Thread end + clear new-content |
| `CancelRun` | Ctrl+C (busy) | Composer Interrupt / Cancel |
| `Submit` | Enter (composer) | Policy |
| `Help` | ? | Contextual help |
| `Esc` | Esc | **One layer only** |

All via `Keymap` + `UiIntent` / app intents — no hardcoded product chords inside widgets.

---

## 5. Command palette

**Commands (illustrative catalog):**

| Id | Label | Effect |
|----|-------|--------|
| `session.new` | New session | Demo runtime |
| `session.switch` | Switch session… | Opens SessionPicker |
| `mode.plan` | Agent mode: Plan | Composer mode badge |
| `mode.edit` | Agent mode: Edit | |
| `run.cancel` | Cancel active run | |
| `view.tasks` | Focus task rail | |
| `view.files` | Focus files | |
| `review.plan` | Open last plan | PlanReview overlay |
| `review.diff` | Open last diff | DiffReview overlay |
| `theme.cycle` | Cycle theme recipe | |
| `capability.doctor` | Show capability summary | Toast or overlay text |
| `help.keymap` | Keymap help | Help overlay |
| `demo.scenario` | Run demo scenario… | Nested picker |

Filter: consumer-side on query; rows as `ListRow`. Overlay: `open_command_palette_overlay`.

---

## 6. Keymap (showcase default)

```
Global
  Ctrl+K / Ctrl+P   OpenPalette
  Ctrl+O            OpenSessions
  Ctrl+\            FocusFiles / toggle files drawer
  Ctrl+B            Toggle task rail drawer
  ?                 Help
  Ctrl+C            CancelRun if busy else copy? (busy wins)
  Esc               OverlayStack / scene one layer

Composer (focused)
  Enter             Submit / Queue
  Alt+Enter         Newline
  Ctrl+Z/Y          Undo/Redo
  / @ #             Completion kinds
  Ctrl+E            ExternalEditor outcome

Thread (focused)
  j/k or arrows     Scroll / select block
  f                 ToggleFollow
  Enter             Expand tool / open diff
  y                 Copy block (outcome)

Task rail
  Enter             Focus thread at task anchor
  c                 Cancel task (outcome)

Overlays
  inherit PermissionPrompt / PlanReview / DiffReview maps
```

Remap table lives in showcase `keymap.rs` using TermRock `Keymap` only.

---

## 7. Mouse behavior

| Target | Action |
|--------|--------|
| Thread body | Wheel scroll (pauses follow); click expand tool/diff |
| Composer | Caret; chip remove/activate |
| Task row | Click select; double-click jump |
| File row | Click select; Enter open preview overlay |
| Status hints | Optional click → Help |
| Overlay backdrop | Outside policy (palette dismiss, dialog trap) |
| New-content chip | Click → JumpLatest |

All hits via public hit regions / scene registration.

---

## 8. Overlay behavior

| Layer id | Kind policy | Esc | Outside |
|----------|-------------|-----|---------|
| `palette` | CommandPalette | Dismiss | Dismiss |
| `sessions` | Dialog-like | Dismiss | Dismiss |
| `permission` | Alert-ish / Dialog | Cancel≠Allow | Trap |
| `question` | Dialog | Cancel flow | Trap |
| `plan` | Dialog | Dismiss/cancel plan | Trap |
| `diff` | Fullscreen-capable | Dismiss | Trap |
| `help` | Popover/Dialog | Dismiss | Dismiss |
| `completion` | Completion | Dismiss | Dismiss |
| `files.drawer` | Drawer | Dismiss | Dismiss |
| `subagent.fs` | Fullscreen | Dismiss | Trap |

**Law:** Esc closes exactly one conceptual layer. Nested: completion under composer under permission — peel top first.

Stack: single `OverlayStack` + `InteractionScene` layers synced each frame.

---

## 9. Responsive layouts

| Width | Layout |
|-------|--------|
| ≥120 | Full: tasks+files rails, thread, composer, status |
| 80–119 | Compact density; files in drawer only; rail narrower |
| 60–79 | Single primary: thread+composer; tasks drawer; files palette command |
| 40–59 | Composer compact; thread only; all secondary overlays/drawers |
| ≤24 / height≤5 | LineMode: last agent line + one-line composer; palette for rest |

Use `ResponsiveSurface::AppShell`, `Workspace` collapse_priority, `ComposerPresentation`.

---

## 10. Streaming behavior

```
demo runtime ──chunks──► channel
UI: StreamCoalescer.push_*
    batch = take_for_frame(tick)
    apply to thread model (last block append / new tool card)
    apply_follow_after_append(window, follow, indicator, …)
    dirty body → paint
```

- Token text → StreamingMarkdown last assistant block.  
- Tool start/end → ToolCallCard / TerminalRunCard.  
- High priority: permission required, errors, done.  
- Backpressure: Soft/Hard from coalescer to demo runtime (pause script ticks).

**Follow:** default Following during active run; user wheel → Paused + NewContentIndicator.

---

## 11. Task and subagent presentation

**Task rail rows (ComposedRow):**

- leading: status glyph  
- primary: task title  
- secondary: agent/sub label  
- badge: `⟳` / `✓` / `✗`  

**SubagentCard** in thread or expand-in-rail: progress, cancel, “show in thread”.

**Multiple running:** rail lists all `Running`; activity shelf shows top 3 tools; status shows `agents:2`.

Selecting a subagent filters thread highlight or jumps to anchor message id.

---

## 12. Permission flows

1. Demo tool requests gate → `PermissionRequest` with provenance (main → sub → mcp).  
2. `PermissionPromptState::enqueue`; open overlay; `composer.set_focused(false)`.  
3. User Deny/Allow/Edit… → `Decided { generation }` → demo runtime apply.  
4. Stale generations ignored if run cancelled.  
5. Esc → Cancelled, no grant; composer refocus.

**Never** default Allow on High/Critical.

---

## 13. Empty, loading, disconnected, failure

| State | UI |
|-------|-----|
| **Empty session** | Thread EmptyState “Start with a prompt”; composer focused |
| **Loading session** | Skeleton list in thread; composer disabled optional |
| **Streaming** | Assistant phase WaitingFirstToken / Streaming; tool Running |
| **Disconnected** | `ComposerConnection::Disconnected`; status “offline”; submit ValidationFailed |
| **Tool failure** | ToolCallCard Failed + Retry outcome → demo re-queue |
| **Agent error** | System/error block + toast |
| **Permission denied** | Tool cancelled card; optional agent follow-up message |

---

## 14. Theme and visual hierarchy

- Default **phosphor** recipe (quiet hierarchy: canvas → surface → elevated).  
- **One** focused border role (`BorderFocused`) for the pane owning keyboard.  
- Thread: user muted panel, agent default, tools nested cards.  
- Risk: PermissionPrompt danger chrome.  
- Status: muted; warnings only for offline/busy queue depth.  
- Density: Comfortable default; Compact under 100 cols.

Capability: `resolve_capabilities` → quantize theme + glyph set; doctor command in palette.

---

## 15. Narrow and tiny variants

### Narrow (~40×16)

```
┌ showcase · EDIT · offline? ─────┐
│ thread (full width)               │
│ … tool cards collapsed            │
│ [n new ↓]                         │
├───────────────────────────────────┤
│ composer compact                  │
│ › prompt…                         │
├───────────────────────────────────┤
│ q:0 · hints: ⌘K ?                 │
└───────────────────────────────────┘
# Tasks/Files: Ctrl+B / Ctrl+\ drawers
```

### Tiny (~20×5)

```
│ agent: running tests…     │
│ › _                       │
│ * offline                 │
```

Palette + drawers for everything else; LineMode anatomy.

---

## 16. Test recordings (Studio / showcase)

| Id | Script |
|----|--------|
| `rec/conversation-basic` | user submit → stream markdown → done |
| `rec/tool-running` | tool card expand + follow pause |
| `rec/permission-high` | nested provenance; Enter stays Deny; n deny |
| `rec/plan-approve` | plan overlay; approve → tools |
| `rec/diff-hunks` | diff n/p hunks; Esc |
| `rec/multi-subagent` | two Running in rail; jump |
| `rec/narrow-drawer` | resize 40; open tasks drawer |
| `rec/no-color` | mono profile; selection still visible |
| `rec/queue-busy` | submit while busy → Queued |
| `rec/esc-layers` | palette over composer; Esc peels one |

Format: Studio `.rec.json` (see termrock-studio design). CI: `termrock-showcase record --check` later.

---

## 17. Demo scenarios

| Scenario | User sees |
|----------|-----------|
| **Hello stream** | Token stream + follow |
| **Read file + permission** | Low-risk read gate → tool card |
| **Destructive shell** | High-risk permission → deny/allow path |
| **Plan then build** | PlanReview → approve → multi tools |
| **Failing tests + diff** | TerminalRun fail → DiffReview overlay |
| **Question mid-run** | QuestionFlow pause/resume |
| **Parallel subagents** | Two cards + rail |
| **Reconnect** | Disconnect banner → reconnect → draft preserved |
| **Session switch** | Picker; empty vs resume |
| **Capability stress** | Toggle ascii/mono; doctor |

Script runner: timed events + optional wait-for-outcome (permission decided).

---

## 18. Terminal mockups (detail)

### 18.1 Normal conversation

```
┌ TermRock Showcase · session:demo ────────────── modern · phosphor ─┐
│ tasks 22% │ conversation                                           │
│ ● plan    │ You                                                    │
│ ○ build   │   Summarize the workbench layout.                      │
│           │ Agent                                                  │
│ files     │   The workbench splits **tasks**, **thread**, and      │
│  src/     │   **composer**. Focus moves with Tab.                  │
│           │                                                        │
│           │ [follow]                                               │
│           ├────────────────────────────────────────────────────────│
│           │ EDIT · demo-model · ctx 12k/128k                       │
│           │ › Ask a follow-up…                                     │
├───────────┴────────────────────────────────────────────────────────┤
│ connected · Tab panes · Ctrl+K palette · ? help                    │
└────────────────────────────────────────────────────────────────────┘
```

### 18.2 Active tool execution

```
│ Agent                                                              │
│   I'll run the test suite.                                         │
│   ┌ ● bash · running ───────────────────────────────────────────┐  │
│   │ $ cargo test -p termrock --lib                              │  │
│   │ running 417 tests                                           │  │
│   │ ..........                                                  │  │
│   └─────────────────────────────────────────────────────────────┘  │
│ Activity: bash                                                      │
│ EDIT · busy · queue:0                                               │
│ › (queued submits allowed) _                                        │
```

### 18.3 Permission request

```
┌─ !! high risk · bash ─────────────────────────────────────────────┐
│ from agent:main > subagent:worker > mcp:shell                     │
│ shell → workspace                                                 │
│ at local · cwd: ~/proj                                            │
│ DESTRUCTIVE: shell may be hard to undo                            │
│ expect: remove build artifacts                                    │
│ $ rm -rf target/debug                                             │
│ scope: Once · [] · q:1                                            │
│ [ Deny ] [ Details ] [ Edit&Allow ] [ Change ] [ Restrict ] [Allow]│
└───────────────────────────────────────────────────────────────────┘
  focus: Deny     composer blurred, draft preserved under overlay
```

### 18.4 Plan review

```
┌─ Plan review · medium risk ───────────────────────────────────────┐
│ 1. Audit public APIs                                              │
│ 2. Add showcase binary                                            │
│ 3. Record Studio scenarios                                        │
│ [ Reject ] [ Edit ] [ Approve ]                                   │
└───────────────────────────────────────────────────────────────────┘
```

### 18.5 Diff review

```
┌─ Diff · crates/termrock/src/lib.rs ──────────── hunk 2/5 ─────────┐
│ @@ pub mod capability;                                            │
│ +pub mod showcase;                                                │
│  pub mod perf;                                                    │
│ [n]ext hunk  [p]rev  [a]ccept file  Esc close                     │
└───────────────────────────────────────────────────────────────────┘
```

### 18.6 Multiple running subagents

```
│ TASKS           │ conversation (filter: all)                       │
│ ● research  45% │ ┌ subagent:research · streaming ───────────────┐ │
│ ● implement 12% │ │ probing OverlayStack usage…                  │ │
│ ○ review        │ └──────────────────────────────────────────────┘ │
│                 │ ┌ subagent:implement · running tool ───────────┐ │
│                 │ │ tool:search · src/**                         │ │
│                 │ └──────────────────────────────────────────────┘ │
│ status: agents:2 · tools:2                                         │
```

### 18.7 Narrow terminal (~40 cols)

```
┌ showcase · EDIT · agents:1 ──┐
│ Agent: running cargo test…   │
│ ┌ bash · run ● ────────────┐ │
│ │ $ cargo test             │ │
│ └──────────────────────────┘ │
│ [3 new ↓]                    │
├──────────────────────────────┤
│ › _                          │
│ q:1 · Ctrl+K · Ctrl+B tasks  │
└──────────────────────────────┘
```

### 18.8 No-color terminal

```
| TermRock Showcase [mono] [ascii]                    |
| * task research  R                                  |
|   You> Summarize layout                             |
|   Agent> The workbench splits tasks, thread, ...    |
|   tool:bash [R] cargo test                          |
| >_                                                  |
| = connected | ctx 12k/128k | Deny is default on perms|
```

Selection/focus via `*` / `>` gutters and labels; risk via `[!!]` text; no reliance on green/red alone.

---

## 19. Crate layout (implementation plan)

```
crates/termrock-showcase/          # or examples/showcase_workbench
  Cargo.toml                       # depends on termrock public only
  src/
    main.rs                        # capability resolve, session, loop
    app.rs                         # state, focus, overlays
    demo_runtime.rs                # scripted agent (showcase-only)
    scenarios/
    keymap.rs
    views/
      thread.rs                    # projects public Transcript/cards
      rail.rs
      files.rs
  recordings/
  README.md                        # how to run demos
```

**Registry (when ready):** install `termrock/agent-workbench` + skins; showcase only wires demo runtime + keymap.

**Forbidden:** `pub(crate)` TermRock internals; copy-paste of private widgets; local reimplementation of Esc/focus/placement.

---

## 20. Gap protocol (dogfood → core)

When the showcase needs something awkward:

| Symptom | Likely missing primitive | Home |
|---------|--------------------------|------|
| Can’t show nested tool under message cleanly | MessageThread block model | component |
| Draft cleared on permission | PromptComposer focus API misuse or gap | component |
| Esc closes two things | OverlayStack / scene misuse or bug | core |
| Subagent list custom paint | TaskRail / SubagentCard gap | component |
| Diff not keyboard-navigable | DiffReview intents | component |
| 1M log lines jank | LogStream virtualize + coalesce | component + perf |
| Mux truecolor wrong | capability resolve | core |
| No story for flow | Studio recording | Studio |

Track gaps in `docs/design/showcase-api-gaps.md` (append-only) with severity; each gap opens a TermRock change, not a showcase hack.

---

## 21. Success criteria

1. Runs with public TermRock only; `cargo run -p termrock-showcase`.  
2. All §18 mockups achievable.  
3. Recordings §16 pass headless replay.  
4. Capability Minimal + no-color still usable.  
5. Narrow 40×16 keeps submit + read stream.  
6. Permission default never grants.  
7. Zero private workarounds (gap list empty or ticketed to core).  
8. README demo path &lt; 2 minutes to first “wow”.

---

## 22. Phased build

| Phase | Deliverable |
|-------|-------------|
| S0 | This design + gap log file |
| S1 | Scaffold crate + capability session + empty IA layout |
| S2 | Thread + composer + demo hello stream |
| S3 | Tools + terminal cards + activity |
| S4 | Permission + question + plan + diff overlays |
| S5 | Task rail + multi-subagent scenario |
| S6 | Files + sessions + checkpoints |
| S7 | Palette + help + keymap |
| S8 | Responsive + mono/ascii + recordings |
| S9 | README polish; fix remaining gaps in TermRock |

---

## 23. Decision summary

1. Showcase is the **proof** and the **pressure test**.  
2. Architecture = Workbench regions + one scene + one overlay stack + demo runtime.  
3. Agent UX uses TermRock agent surfaces; domain policy stays in demo driver.  
4. Mockups define the quality bar for conversation, tools, trust, review, multi-agent, narrow, mono.  
5. **No private shortcuts** — gaps become TermRock primitives.
