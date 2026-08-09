# `@termrock/agent` — source-owned agent UI collection

**Status:** design SoT  
**Distribution:** source-owned registry package (see [`source-owned-registry.md`](./source-owned-registry.md)); kernel stays in the `termrock` crate  
**Rule:** Components are **domain-neutral**. Provider policy (which tools run, sandbox rules, model routing, AGENTS.md loading) is **consumer-owned**. TermRock owns chrome, interaction, streaming presentation, contraction, and typed outcomes only.

**Related:** anatomy §11–12, plan 046 AgentWorkbench, OverlayStack, responsive layout, Studio stories.

---

## 1. Research synthesis (patterns only)

Studied interaction *models* from Grok Build–class studios, Amp, OpenCode, Claude Code, Codex CLI, and peer agent TUIs. **No branding, copy, or source reuse.**

### 1.1 Recurring interaction patterns

| Pattern | What it means for TermRock |
|---------|----------------------------|
| **Composer-first loop** | Always-visible multi-line prompt; Enter send / Chord newline; queue while busy |
| **Mode as safety dial** | Plan / ask / edit / auto / full-auto change *permission chrome*, not model weights |
| **Slash + @ surfaces** | `/` opens command menu; `@` / file pickers attach context without leaving composer |
| **Attachment chips** | Pastes, files, images become dismissible chips above or inside composer |
| **Streaming transcript** | Variable-height blocks; sticky user anchors; tool cards inline; follow-tail until scroll |
| **Tool cards as first-class rows** | Collapsible invocation + args + status + result; expand → log/diff |
| **Permission gates** | Default-deny risk-aware cards; never auto-focus destructive primary |
| **Question interruptions** | Multi-step questions pause the agent; answers resume queue |
| **Plan → approve → execute** | Read-only plan review before write tools; Esc closes one layer |
| **Autonomy ladder** | Suggest → auto-edit → full-auto (Codex-class); visible mode badge always |
| **Subagent / background work** | Side rail of tasks; expandable cards; cancel per task |
| **Context meter** | Token / window usage near composer or status; not a fake progress bar for “thinking” |
| **Diff + checkpoint review** | Hunk nav, stage/reject outcomes; timeline of restore points |
| **Session resume** | Picker of threads/sessions; remote/local is consumer policy |
| **Client/server optional** | UI must work against async event stream; no embedded provider SDK |
| **Command palette** | Global jump for modes, models, sessions, commands |

### 1.2 Anti-patterns to avoid

- Embedding API keys or provider SDKs in components.  
- Approval defaulting to Allow (historical TermRock bug class).  
- Treating “streaming” as full re-render of transcript.  
- Mixing shell process ownership into widgets.  
- Brand glyphs as the only status channel (colorless must work).

### 1.3 Ownership split

| TermRock (`@termrock/agent` + kernel) | Consumer agent |
|--------------------------------------|----------------|
| Layout, focus, overlays, Esc law | Model/provider choice |
| Prompt editor chrome, chips | What attachments mean |
| Permission / question / plan chrome | Whether a tool is allowed |
| Tool card presentation | Tool schemas & executors |
| Transcript virtualization | Message persistence |
| Task rail geometry | Task orchestration |
| Typed outcomes | Effects (run tool, write file, network) |

---

## 2. Package shape

```
@termrock/agent  (registry namespace: termrock/agent/*)
├── types/           # shared domain-neutral types (copied or crate module)
├── components/      # each component as installable source
├── blocks/
│   └── agent-workbench/
└── stories/         # Studio scenarios
```

**Kernel deps (crate, not copied):** `InteractionScene`, `OverlayStack`, `Workspace`, responsive anatomy, `Panel`, `List`/`ComposedRow`, `TextArea`, `MarkdownView`, `DiffView`, `Dialog`, `Picker`, tokens, intents.

**Copied:** opinionated agent chrome compositions listed below.

---

## 3. Shared domain-neutral types

All IDs are consumer-owned stable strings (`Id: Clone + Eq + Hash`). Text is borrowed at paint time where possible.

```rust
/// Who produced a message or activity.
#[non_exhaustive]
pub enum ActorKind {
    User,
    Agent,
    System,
    Tool,
    Subagent,
}

pub struct Actor {
    pub id: String,
    pub kind: ActorKind,
    pub label: String,           // display; may shorten
    pub provenance: Option<String>, // e.g. "session:…", "tool:bash"
}

/// Lifecycle of a unit of work (task, tool, subagent).
#[non_exhaustive]
pub enum TaskStatus {
    Queued,
    Running,
    WaitingPermission,
    WaitingInput,        // question flow
    Streaming,
    Succeeded,
    Failed,
    Cancelled,
    TimedOut,
}

#[non_exhaustive]
pub enum ToolCallStatus {
    Pending,
    Running,
    StreamingOutput,
    AwaitingPermission,
    Succeeded,
    Failed,
    Cancelled,
}

/// What the permission UI is asking about (consumer interprets).
#[non_exhaustive]
pub enum PermissionScope {
    FileRead { path: String },
    FileWrite { path: String },
    Shell { command_preview: String },
    Network { host_preview: String },
    McpTool { name: String },
    Custom { label: String },
}

#[non_exhaustive]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// Sidebar / shelf activity row.
pub struct ActivityItem<Id> {
    pub id: Id,
    pub actor: Actor,
    pub title: String,
    pub status: TaskStatus,
    pub risk: Option<RiskLevel>,
    pub progress: Option<ActivityProgress>,
    pub parent: Option<Id>,      // subagent parent
}

#[non_exhaustive]
pub enum ActivityProgress {
    Indeterminate,
    Ratio { done: u32, total: u32 },
    Bytes { done: u64, total: Option<u64> },
}

pub struct Checkpoint<Id> {
    pub id: Id,
    pub label: String,
    pub at_label: String,        // consumer formats time
    pub kind: CheckpointKind,
}

#[non_exhaustive]
pub enum CheckpointKind {
    UserMessage,
    ToolBoundary,
    PlanApproved,
    ExplicitSave,
    Error,
}

#[non_exhaustive]
pub enum ReviewDecision {
    Accept,
    AcceptOnce,
    Reject,
    RequestChanges,
    Skip,
}

/// Streaming payload independent of provider wire format.
#[non_exhaustive]
pub enum StreamChunk {
    TextDelta(String),
    ReasoningDelta(String),
    ToolCallStart { id: String, name: String },
    ToolCallArgsDelta { id: String, json_delta: String },
    ToolCallEnd { id: String, status: ToolCallStatus },
    Status(TaskStatus),
    Error { message: String, retryable: bool },
    Done,
}

pub struct StreamingContentState {
    pub phase: StreamPhase,
    pub text: String,            // accumulated visible
    pub reasoning: Option<String>,
    pub error: Option<String>,
}

#[non_exhaustive]
pub enum StreamPhase {
    Idle,
    WaitingFirstToken,
    Streaming,
    Finalizing,
    Complete,
    Failed,
}

/// Composer queue entry while agent is busy.
pub struct QueuedPrompt {
    pub id: String,
    pub text: String,
    pub attachment_ids: Vec<String>,
}

#[non_exhaustive]
pub enum AgentMode {
    Ask,           // Q&A, prefer no writes
    Plan,          // read-only planning
    Edit,          // propose edits, gate shell
    AutoEdit,      // edits auto, shell gated
    FullAuto,      // consumer still owns sandbox policy
}

#[non_exhaustive]
pub enum PermissionDecision {
    AllowOnce,
    AllowSession,
    AlwaysAllowScope,  // consumer maps to policy store
    Deny,
    DenyAndStop,
}
```

**Serialization:** optional `serde` behind feature; Studio fixtures use JSON projections of these shapes.

---

## 4. Cross-cutting contracts (all components)

| Concern | Rule |
|---------|------|
| **Esc** | One conceptual layer (`OverlayStack` / scene policy) |
| **Outcomes** | Pure enums — no I/O |
| **Streaming** | Append-only patches; O(visible) paint |
| **Queue** | Composer may enqueue; consumer drains |
| **Cancel** | `CancelRequested` outcome; consumer aborts work |
| **Errors** | Inline error tone + retry outcome; never panic on bad chunks |
| **Narrow** | Responsive stages; essential labels survive |
| **Colorless** | Glyph + wording carry status |
| **Fullscreen** | Diff/Plan/Terminal cards promote via overlay |
| **a11y** | Focus order, hit regions, non-color status prefixes |

---

## 5. Component catalog

For each: anatomy · state · outcomes · keyboard · mouse · streaming · queue · cancel · error · compact/expanded · fullscreen · responsive · a11y/colorless · stories/tests.

---

### 5.1 PromptComposer

**Purpose:** Primary human → agent input surface.

**Anatomy:** `root` · `chip_row` · `editor` · `mode_badge` · `model_chip` · `send` · `stop` · `footer_hints` · `queue_badge`

**State machine:**
```
Empty ──type──► Draft ──submit──► Submitted ──busy──► Locked
  ▲                │                   │                │
  └──── clear ─────┘                   │                │
                                       ▼                ▼
                                   QueuedMore ◄──── enqueue
                                       │
                                   busy ends ──► Draft/Empty
```

**Outcomes:** `TextChanged` · `Submitted { text }` · `QueueEnqueued` · `QueueRemoved { id }` · `CancelRequested` · `ModeMenuOpen` · `ModelMenuOpen` · `SlashOpen` · `MentionOpen` · `AttachRequested` · `FocusMoved`

**Keyboard:** Enter submit (configurable); Mod+Enter newline; Esc clears slash/mention first then blur; Up on empty → edit last user (outcome only); Ctrl+C / Ctrl+Backspace cancel when busy.

**Mouse:** Click chips dismiss; send/stop hit targets; drag-drop → `AttachRequested` (consumer).

**Streaming:** While agent streaming, editor stays editable for queue; send becomes **queue**; stop visible.

**Queue:** Shows count badge; open queue list (optional popover).

**Cancel:** Stop → `CancelRequested`.

**Error:** Validation under editor (empty submit ignored).

**Compact / expanded:** Compact = 2–3 rows; expanded = grow with content to max.

**Fullscreen:** Optional promote editor to overlay on tiny terminals.

**Responsive:** Hide mode/model text → icons; chips scroll horizontally; footer hints → Studio keymap only.

**A11y / colorless:** “SEND”/“STOP” labels; queue as `[n]`.

**Stories:** empty, draft, busy+queue, chips, narrow, slash open.  
**Tests:** submit empty ignored; busy submit enqueues; esc closes menu not app.

---

### 5.2 AttachmentChip

**Anatomy:** `icon` · `label` · `meta` · `remove`

**State:** idle · hover · focus · removing  
**Outcomes:** `Activated` · `Removed` · `FocusMoved`  
**Keyboard:** Left/Right among chips; Backspace/Delete remove focused; Enter activate.  
**Mouse:** click activate; × remove.  
**Streaming/queue:** N/A (composer-owned).  
**Error:** broken path → warning tone + still removable.  
**Compact:** icon + truncated name; expanded: path meta.  
**Responsive:** drop meta first.  
**Colorless:** type letter `F`/`I`/`P`.  
**Stories/tests:** remove restores focus to composer.

---

### 5.3 PasteChip

**Purpose:** Large paste summarized as chip (not wall of text in transcript until send).

**Anatomy:** `kind_badge` · `preview` · `bytes` · `remove`  
**State:** idle · expanded_preview  
**Outcomes:** `Expanded` · `Collapsed` · `Removed` · `InsertedIntoPrompt`  
**Keyboard:** Enter expand; Esc collapse; Delete remove.  
**Mouse:** click expand popover.  
**Error:** binary paste → “binary” label, no insert as text without confirm outcome.  
**Compact/expanded:** one line vs popover first N lines.  
**Stories:** large paste, binary paste.

---

### 5.4 FileMention

**Purpose:** `@` completion for paths/symbols (consumer supplies candidates).

**Anatomy:** popup list (uses CompletionMenu / OverlayStack Completion policy)  
**State:** closed · open · loading · empty  
**Outcomes:** `Opened` · `Closed` · `QueryChanged` · `Selected { id }` · `Committed { id }`  
**Keyboard:** standard completion; Esc dismiss one layer.  
**Mouse:** click commit.  
**Streaming:** N/A.  
**Error:** load fail → empty state message.  
**Responsive:** place_overlay flip/clamp.  
**Stories:** open near edges, empty, commit inserts token.

---

### 5.5 SlashCommandMenu

**Purpose:** `/` commands (plan, model, clear, …) — **labels consumer-defined**.

**Anatomy:** query · list · description pane (wide)  
**State:** closed · open · filtering  
**Outcomes:** `Opened` · `Closed` · `Selected` · `Committed { id }` · `QueryChanged`  
**Keyboard:** type filters; Enter commit; Esc close.  
**Mouse:** click commit.  
**Compact:** list only; expanded: description column.  
**Responsive:** hide description < 60 cols.  
**Stories:** filter, commit replaces `/token`.

---

### 5.6 ModelSelector

**Purpose:** Pick model id from consumer list (no provider API).

**Anatomy:** trigger chip · popover list · optional capability tags  
**State:** closed · open  
**Outcomes:** `Opened` · `Closed` · `Selected { id }` · `Confirmed { id }`  
**Keyboard:** open from composer chord; j/k; Enter confirm.  
**Error:** empty list → empty state.  
**Compact:** id only; expanded: context window / tags.  
**Colorless:** selected `*`.  
**Stories:** switch model mid-session (outcome only).

---

### 5.7 AgentModeSelector

**Purpose:** Autonomy / safety mode (Ask/Plan/Edit/AutoEdit/FullAuto).

**Anatomy:** ribbon or select; active mode badge  
**State:** selected mode (controlled)  
**Outcomes:** `ModeChanged { mode }` · `MenuOpen`  
**Keyboard:** cycle chord (consumer keymap); Shift+Tab-class is **consumer binding**.  
**Visual:** FullAuto uses warning role never “success green only”.  
**Colorless:** text `PLAN`/`AUTO`/`FULL`.  
**Stories:** each mode badge; confirm FullAuto optional dialog (consumer).

---

### 5.8 MessageThread

**Purpose:** Scrollable conversation of heterogeneous blocks (user, agent, tools, system).

**Anatomy:** `viewport` · `block[]` · `sticky_anchor` · `follow_chip` · `jump_latest`  
**State:** offset · follow · selected_block · expanded_ids  
**Outcomes:** `Scrolled` · `FollowChanged` · `BlockActivated { id }` · `BlockExpanded` · `CopyRequested` · `RetryRequested`  
**Keyboard:** page/arrows; `f` follow; Enter expand tool card.  
**Mouse:** wheel breaks follow; click block.  
**Streaming:** tail append; if follow, stick to end; partial markdown reflow.  
**Queue:** N/A.  
**Cancel:** N/A (parent).  
**Error:** failed agent block shows error footer + Retry.  
**Compact:** collapse tool bodies; expanded: full cards.  
**Fullscreen:** single block promote.  
**Responsive:** drop timestamps; stack meta.  
**A11y:** actor prefixes `You`/`Agent`/`Tool`.  
**Stories:** stream append, unfollow on wheel, sticky user.  
**Tests:** virtualization O(visible); follow semantics.

---

### 5.9 StreamingMarkdown

**Purpose:** Markdown renderer tolerant of incomplete fences during stream.

**Anatomy:** projected `MarkdownView` blocks + caret/phase  
**State:** `StreamingContentState`  
**Outcomes:** none or `LinkActivated` (if OSC consumer).  
**Streaming:** incremental parse; unclosed fence → code block provisional.  
**Error:** show raw on parse fail.  
**Compact:** headings shrink.  
**Colorless:** no reliance on syntax colors alone.  
**Stories:** mid-fence stream, complete, error chunk.  
**Tests:** never panic on partial input; stable height growth.

---

### 5.10 ToolCallCard

**Purpose:** One tool invocation in the thread.

**Anatomy:** `header` (name, status) · `args` · `result` · `timing` · `actions`  
**State:** collapsed · expanded · streaming_output  
**Outcomes:** `Expanded` · `Collapsed` · `CancelRequested` · `RetryRequested` · `OpenDiff` · `OpenLog` · `PermissionFocus`  
**Keyboard:** Enter toggle; `c` cancel when running (if allowed).  
**Mouse:** click header toggle.  
**Streaming:** args/result append; status → StreamingOutput.  
**Error:** Failed status + message; retry affordance.  
**Compact:** one line name+status; expanded: args+result.  
**Fullscreen:** result log overlay.  
**Responsive:** collapse args JSON pretty → single line.  
**Colorless:** status letter `R`/`✓`/`✗`/`…`.  
**Stories:** pending→run→ok; failed; long output.  
**Tests:** toggle preserves scroll anchor.

---

### 5.11 TerminalRunCard

**Purpose:** Shell/process presentation (PTY ownership is consumer).

**Anatomy:** `command` · `status` · `stdout_viewport` · `exit_code` · `actions`  
**State:** idle · running · exited · cancelled  
**Outcomes:** `CancelRequested` · `RerunRequested` · `CopyCommand` · `Fullscreen` · `Scrolled`  
**Keyboard:** as viewport; Ctrl+C → cancel outcome.  
**Streaming:** line append to log pane.  
**Error:** non-zero exit warning role.  
**Compact:** command+status; expanded: last N lines.  
**Fullscreen:** full log overlay.  
**Responsive:** wrap command.  
**Stories:** running scroll, exit codes, cancel.

---

### 5.12 ActivityShelf

**Purpose:** Horizontal/vertical strip of live activities (tools, searches).

**Anatomy:** `item_chip[]` · `overflow`  
**State:** items · selected  
**Outcomes:** `Selected` · `Activated` · `Dismissed`  
**Keyboard:** left/right.  
**Streaming:** status glyph updates.  
**Compact:** icons only.  
**Responsive:** overflow menu.  
**Stories:** many tools overflow.

---

### 5.13 TaskRail

**Purpose:** Vertical list of tasks / subagents / todos.

**Anatomy:** `header` · `list` (ComposedRow) · `footer_counts`  
**State:** ListState · filter  
**Outcomes:** `Selected` · `Activated` · `CancelTask` · `FocusTranscript`  
**Keyboard:** collection intents.  
**Streaming:** status badge live.  
**Error:** failed task danger role.  
**Compact:** status+title; expanded: progress.  
**Responsive:** drawer under AppShell narrow.  
**Colorless:** status prefixes.  
**Stories:** nested parent/child, cancel.  
**Tests:** collapse_priority with Workspace.

---

### 5.14 SubagentCard

**Purpose:** Nested agent run summary.

**Anatomy:** `title` · `mode` · `status` · `progress` · `preview` · `actions`  
**State:** collapsed · expanded  
**Outcomes:** `Open` · `Cancel` · `AttachToTranscript` · `PromoteFullscreen`  
**Streaming:** preview last line.  
**Compact/expanded/fullscreen:** as ToolCallCard.  
**Stories:** running subagent, failed.

---

### 5.15 BackgroundTaskPanel

**Purpose:** Overlay/drawer of long-running jobs.

**Anatomy:** panel · task list · clear completed  
**State:** open · closed  
**Outcomes:** `Closed` · `TaskActivated` · `Cancel` · `ClearCompleted`  
**Keyboard:** Esc closes panel.  
**Queue:** shows queued jobs.  
**Responsive:** drawer from end.  
**Stories:** open with mixed statuses.

---

### 5.16 ContextMeter

**Purpose:** Context window / token usage visualization.

**Anatomy:** `label` · `meter` · `detail`  
**State:** ratios (input/output/cached optional)  
**Outcomes:** `Activated` (open detail) · none  
**Error:** unknown totals → indeterminate.  
**Compact:** bar only; expanded: numbers.  
**Colorless:** hatch density + percent text always.  
**Stories:** low/mid/high pressure, mono.  
**Tests:** never claim 100% without totals.

---

### 5.17 PermissionPrompt

**Purpose:** Risk-aware permission gate (**default-deny**).

**Anatomy:** `title` · `scope` · `risk_badge` · `detail` · `actions[]` · `remember`  
**State:** focus index on **safe** action first (Deny or least privilege)  
**Outcomes:** `Decided { PermissionDecision }` · `DetailExpanded` · `Cancelled`  
**Keyboard:** Left/Right actions; Enter activate; Esc → Cancelled/Deny (policy table).  
**Mouse:** click action.  
**Streaming:** freezes related tool card in WaitingPermission.  
**Error:** missing scope text → still deny-capable.  
**Compact:** title+actions; expanded: path/command detail.  
**Fullscreen:** on narrow promote card.  
**Responsive:** stack actions vertically < 40 cols.  
**Colorless:** `RISK:HIGH` text.  
**Stories:** high risk default focus Deny; narrow clips no phantom Allow; multi-action.  
**Tests:** **default focus never AllowOnce for High/Critical**; Esc does not approve.

---

### 5.18 QuestionFlow

**Purpose:** Multi-step agent questions mid-run.

**Anatomy:** `progress` · `prompt` · `options` / `free_text` · `nav`  
**State:** step index · answers map  
**Outcomes:** `Answered { step, value }` · `Back` · `Skip` · `Completed { answers }` · `Cancelled`  
**Keyboard:** numbers select options; Enter confirm step; Esc cancel flow layer.  
**Queue:** agent paused until Completed/Cancelled.  
**Error:** invalid free text → inline validation.  
**Compact:** one question; expanded: list prior answers.  
**Stories:** 3-step, free text, cancel.

---

### 5.19 PlanReview

**Purpose:** Present plan steps for approval before execution.

**Anatomy:** `title` · `summary` · `step_list` · `risk` · `actions` (Approve/Edit/Reject)  
**State:** selected step · scroll  
**Outcomes:** `Approved` · `Rejected` · `EditRequested` · `StepActivated` · `Cancelled`  
**Keyboard:** j/k steps; Enter approve primary **only if** focus on Approve; default focus **Edit or Reject** for high-risk plans optional policy via props.  
**Streaming:** steps can stream in.  
**Fullscreen:** plan overlay.  
**Responsive:** steps only.  
**Stories:** stream steps, approve, reject.  
**Tests:** Approve not default-focused when `risk >= High`.

---

### 5.20 DiffReview

**Purpose:** Review file patches (builds on kernel DiffView).

**Anatomy:** file tabs · hunk list · line viewport · actions  
**State:** file · hunk · cursor · staged set (ids only)  
**Outcomes:** `HunkActivated` · `FileChanged` · `AcceptFile` · `RejectFile` · `AcceptAll` · `RejectAll` · `Scrolled` · `Fullscreen`  
**Keyboard:** n/p hunk; `[` `]` file; a/r accept/reject hunk (outcomes).  
**Streaming:** patch append mid-review.  
**Error:** binary file placeholder.  
**Compact:** unified only; expanded: split when wide.  
**Fullscreen:** yes.  
**Responsive:** `DiffReview` multi_pane → unified (`ResponsiveSurface`).  
**Colorless:** `+`/`-` prefixes mandatory.  
**Stories:** multi-hunk, narrow unified, streaming patch.  
**Tests:** hunk bounds; split disabled < 70 cols.

---

### 5.21 CheckpointTimeline

**Purpose:** Session restore points / boundaries.

**Anatomy:** vertical timeline · markers · labels  
**State:** selected checkpoint  
**Outcomes:** `Selected` · `RestoreRequested { id }` · `CompareRequested`  
**Keyboard:** up/down; Enter restore request.  
**Compact:** dots only; expanded: labels.  
**Stories:** mixed kinds, restore confirm (consumer dialog).

---

### 5.22 SessionPicker

**Purpose:** Switch/resume sessions (local or remote is consumer).

**Anatomy:** search · list · meta (time, title, dirty)  
**State:** PickerState  
**Outcomes:** `QueryChanged` · `Selected` · `Opened { id }` · `Deleted { id }` · `Cancelled`  
**Keyboard:** picker patterns; Esc close.  
**Error:** load failure empty+retry.  
**Compact:** title only.  
**Stories:** many sessions filter, open.

---

## 6. AgentWorkbench block

**ID:** `termrock/agent-workbench`  
**Composition only from public primitives + `@termrock/agent` components.**

### 6.1 Geometry (Workspace)

```
┌─ TaskRail (west) ──┬─ MessageThread (center) ──────────────┐
│ tasks/subagents    │  user / agent / tool cards            │
│                    │  StreamingMarkdown · ToolCallCard     │
│                    ├───────────────────────────────────────┤
│                    │ ActivityShelf (optional thin)         │
│                    ├───────────────────────────────────────┤
│                    │ PromptComposer                        │
│                    │ ContextMeter · Mode · Model           │
├────────────────────┴───────────────────────────────────────┤
│ StatusBar: session · mode · context · hints                │
└────────────────────────────────────────────────────────────┘
Overlays (OverlayStack): Permission · Question · Plan · Diff ·
  BackgroundTasks · SessionPicker · Slash · Mentions · CommandPalette
```

**Responsive (AppShell + Workbench):**

| Width | Behavior |
|-------|----------|
| ≥120 | Full multi-pane |
| 80–119 | Compact density; rail narrow |
| 60–79 | Single pane: Thread+Composer; rail → drawer |
| 40–59 | Drawer overlays; chips iconized |
| ≤24 | LineMode: last message + one-line composer |

### 6.2 State (`AgentWorkbenchState`)

Consumer-owned across frames:

- `WorkspaceState`, `InteractionScene`, `OverlayStack`  
- Child states: transcript, prompt, task list, mode, model  
- Layer flags: permission/question/plan/diff open  
- `follow` transcript, `busy`, `queue: Vec<QueuedPrompt>`

### 6.3 Input routing

```
key/mouse → OverlayStack (top) → Scene layer → focused pane host
Esc → overlay one layer → else pane-local → else UnhandledEscape (quit = consumer)
```

### 6.4 Workbench outcomes (union)

```rust
pub enum AgentWorkbenchOutcome<Id> {
    Prompt(PromptComposerOutcome),
    Thread(MessageThreadOutcome),
    Task(TaskRailOutcome),
    Permission(PermissionDecision),
    Question(QuestionFlowOutcome),
    Plan(PlanReviewOutcome),
    Diff(DiffReviewOutcome),
    Session(SessionPickerOutcome),
    Mode(AgentMode),
    Model(String),
    CancelAll,
    // …
}
```

No effects inside the block.

### 6.5 Streaming integration (consumer loop)

```
on StreamChunk:
  patch transcript / tool cards / meters
  if Permission needed → open PermissionPrompt overlay
  if Question → open QuestionFlow
  if Plan ready → open PlanReview
  reflow workbench; scene.begin_frame; register; paint
```

### 6.6 Cancellation

- Stop in composer → `CancelRequested` for current run.  
- Task rail cancel → per-id.  
- Overlay Esc does not cancel agent unless consumer binds it.

### 6.7 Error handling

- Stream error chunk → agent message block + toast optional (consumer).  
- Tool fail → card Failed + retry outcome.  
- Overlay errors stay on card.

### 6.8 Stories (Studio)

| Story id | Proves |
|----------|--------|
| `agent-workbench/full-session` | rail+thread+prompt |
| `agent-workbench/streaming-tools` | tool cards stream |
| `agent-workbench/permission-high-risk` | default-deny focus |
| `agent-workbench/plan-then-diff` | layered overlays Esc law |
| `agent-workbench/queue-while-busy` | queue badge |
| `agent-workbench/narrow-drawer` | rail drawer |
| `agent-workbench/tiny-line` | essential only |
| `agent-workbench/colorless` | mono status |
| `agent-workbench/question-flow` | pause/resume |
| `agent-workbench/session-picker` | overlay session |

### 6.9 Tests

- Esc closes exactly one overlay.  
- Permission High never focuses Allow.  
- Busy submit enqueues not dual-send.  
- Follow broken by wheel.  
- Workspace collapse under width matrix.  
- Public-API-only compile of block package.

---

## 7. Keyboard map (suggested defaults — consumer remap)

| Chord | Intent |
|-------|--------|
| Enter | Submit prompt |
| Mod+Enter | Newline |
| Esc | Dismiss one layer |
| Ctrl+C | Cancel run (when busy) |
| Ctrl+X Ctrl+P / configurable | Command palette |
| `/` at line start | Slash menu |
| `@` | File mention |
| Shift+Tab cycle | Mode selector (optional) |
| Ctrl+O | Session picker (Amp-class pattern, remappable) |

All via `Keymap` + intents — no hard-coded product chords inside components.

---

## 8. Registry items (install unit)

| Item | Type |
|------|------|
| `termrock/agent-types` | shared types module |
| `termrock/prompt-composer` | component |
| `termrock/attachment-chip` | component |
| `termrock/paste-chip` | component |
| `termrock/file-mention` | component |
| `termrock/slash-command-menu` | component |
| `termrock/model-selector` | component |
| `termrock/agent-mode-selector` | component |
| `termrock/message-thread` | component |
| `termrock/streaming-markdown` | component |
| `termrock/tool-call-card` | component |
| `termrock/terminal-run-card` | component |
| `termrock/activity-shelf` | component |
| `termrock/task-rail` | component |
| `termrock/subagent-card` | component |
| `termrock/background-task-panel` | component |
| `termrock/context-meter` | component |
| `termrock/permission-prompt` | component |
| `termrock/question-flow` | component |
| `termrock/plan-review` | component |
| `termrock/diff-review` | component |
| `termrock/checkpoint-timeline` | component |
| `termrock/session-picker` | component |
| `termrock/agent-workbench` | **block** (depends on above) |

```bash
termrock add termrock/agent-workbench
# pulls dependency graph of components + types
```

---

## 9. Implementation plan

| Phase | Deliverable |
|-------|-------------|
| A0 | This design + types module in kernel or registry types item |
| A1 | PermissionPrompt + PlanReview safety contracts (default-deny) |
| A2 | PromptComposer + chips + slash/mention overlays |
| A3 | MessageThread + StreamingMarkdown + ToolCallCard |
| A4 | TaskRail + Subagent + Background + ActivityShelf |
| A5 | DiffReview + Checkpoint + Session + ContextMeter |
| A6 | AgentWorkbench block + Studio stories matrix |
| A7 | Registry publish `@termrock/agent` collection |

Depends on: OverlayStack, responsive, InteractionScene, Transcript engine (done/partial), Studio story format for evidence.

---

## 10. Decision summary

1. **Extract patterns**, never brand or providers.  
2. **Safety dial** (mode) and **permission cards** are first-class.  
3. **Streaming** is structural (chunks + virtualization), not a paint hack.  
4. **Queue** while busy prevents double-submit races.  
5. **Workbench** is a block of public parts + one scene + one overlay stack.  
6. **Consumer owns** execution, policy, persistence, and provider choice.

---

## 11. Mapping research → components

| Research pattern | Component |
|------------------|-----------|
| Composer + queue | PromptComposer |
| Paste/file context | PasteChip, AttachmentChip, FileMention |
| `/` commands | SlashCommandMenu |
| Model pick | ModelSelector |
| Plan/auto/suggest ladder | AgentModeSelector |
| Chat + tools inline | MessageThread, ToolCallCard, StreamingMarkdown |
| Shell runs | TerminalRunCard |
| Live activity | ActivityShelf |
| Todos/subagents | TaskRail, SubagentCard, BackgroundTaskPanel |
| Context window | ContextMeter |
| Approvals | PermissionPrompt |
| Clarifying Qs | QuestionFlow |
| Plan approve | PlanReview |
| Patch review | DiffReview |
| Restore points | CheckpointTimeline |
| Thread resume | SessionPicker |
| Full product | AgentWorkbench |
