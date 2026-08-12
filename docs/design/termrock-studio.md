# TermRock Studio

**Status:** design SoT (lookbook is the seed; plan 048 shipped shell fragments)  
**Comparable to:** Storybook · shadcn previews · browser DevTools  
**Hard rule:** Studio uses **only public TermRock APIs**. A required private hook is evidence the public API is incomplete — fix the crate, do not special-case Studio.

**Related:**  
- Lookbook crate: `crates/termrock-lookbook`  
- Shell: `patterns::layout_studio_shell`, `DesignInspector`  
- Capability: `CapabilityPreviewHost`  
- Registry: [`source-owned-registry.md`](./source-owned-registry.md)  
- Responsive: [`responsive-layout.md`](./responsive-layout.md)  
- Overlay: [`overlay-stack.md`](./overlay-stack.md)

---

## 0. North star

TermRock Studio is the **authoritative environment** for discovering, inspecting, proving, and shipping terminal UI:

| Audience | Value |
|----------|--------|
| Component authors | Knobs, scenarios, contract evidence, snapshots |
| App teams | Browse blocks, copy install commands, theme/capability matrices |
| Agents / CI | Deterministic replay, snapshot gates, contract status |
| Design system | Token/recipe/density live editing with degradation proof |

The current lookbook proves gallery + SVG gate + partial knobs/inspectors. Studio elevates that into a full **harness + inspector + recorder**.

---

## 1. Studio information architecture

### 1.1 Application chrome (IA)

```
┌─ Studio ──────────────────────────────────────────────────────────────┐
│ [Search…]  Components │ Blocks │ Themes │ Registry │ Contracts │ Help │
├──────────┬────────────────────────────────────────────┬───────────────┤
│ Library  │  Stage (preview viewport)                   │ Properties    │
│ tree /   │  ┌──────────────────────────────────────┐  │ knobs         │
│ search   │  │  Story render surface (simulated     │  │ state         │
│ results  │  │  terminal size + capabilities)       │  │ theme/density │
│          │  └──────────────────────────────────────┘  │ capability    │
│ favorites│  [size] [cap] [glyph] [motion] [record]    │ resize sim    │
├──────────┴────────────────────────────────────────────┴───────────────┤
│ Inspector tabs: Focus │ Hits │ Scene │ Overlays │ Tokens │ Keymap      │
│                 Events │ Messages │ Perf │ Buffer │ Contracts │ Diff    │
├───────────────────────────────────────────────────────────────────────┤
│ Status: story id · frame · ms · alloc · contract PASS/FAIL · dirty?    │
└───────────────────────────────────────────────────────────────────────┘
```

### 1.2 Navigation model

| Region | Purpose | Focus trap |
|--------|---------|------------|
| **Library** | Browse/search components, blocks, registry items, stories | Esc → search clear / root |
| **Stage** | Live story; receives story input when focused | Esc → back to library (unless story traps) |
| **Properties** | Knobs, state, theme, density, capability, size | Tab cycles controls |
| **Inspector** | Read-only diagnostics (and token edit mode) | Does not steal story Esc when Stage focused |
| **Command palette** | Studio commands (`/` or `Ctrl+K`) | OverlayStack |

Studio shell layout uses `layout_studio_shell` + responsive `AppShell` policy (collapse Properties → drawer under pressure; LineMode shows Stage only).

### 1.3 Library taxonomy

```
Components/
  Panel/
    focused
    contracted-narrow
  List/
    selection
    multiselect
Blocks/
  agent-workbench/
    full-session
    approval-default-deny
Themes/
  phosphor-default
  phosphor-obsidian
Registry/                    # if termrock.toml present
  installed/
  available/
Contracts/
  failing
  unproven-axes
```

Search indexes: story id, component name, tags, registry id, contract axis names.

### 1.4 Studio modes

| Mode | Description |
|------|-------------|
| **Browse** | Default interactive gallery |
| **Matrix** | Tile same story across capability/density/glyph axes |
| **Record** | Capture input + ticks + outcomes |
| **Replay** | Drive story from recording / scenario file |
| **Compare** | Split stage: A/B theme, unicode/ascii, color ladders, upstream/local source |
| **Docs** | Render generated contract + API inventory for selection |

---

## 2. Story file format

Stories become **declarative scenario documents** plus optional Rust harness code. Two layers:

1. **`*.story.toml` / `*.story.json`** — portable fixture (CI, agents, docs).  
2. **Rust `StudioStory` impl** — typed render + state (compile-time safe).

### 2.1 Portable story document

```toml
# stories/list/selection.story.toml
schema = 1
id = "list/selection"
title = "List selection"
component = "List"
tags = ["selection", "narrow", "keyboard"]
description = "Single selection with metadata contraction."

[viewport]
width = 48
height = 12

[environment]
theme = "phosphor-default"          # named theme package or built-in
density = "comfortable"             # comfortable | compact | dashboard
color_capability = "truecolor"      # truecolor | ansi256 | ansi16 | mono
glyph_set = "unicode"               # unicode | ascii
motion = "full"                     # full | reduced | off
selection_chrome = "gutter"

[initial_state]
# Opaque JSON projected by the story harness (schema per component)
payload = '''
{
  "rows": ["alpha", "beta", "gamma"],
  "selected": "beta",
  "show_metadata": true
}
'''

# Optional typed knobs surface in Properties panel
[[knobs]]
id = "show_metadata"
label = "Metadata"
type = "bool"
default = true

[[knobs]]
id = "density"
label = "Density"
type = "choice"
choices = ["comfortable", "compact", "dashboard"]
default = "comfortable"

# Scripted interaction (deterministic)
[[script]]
at_frame = 0
op = "tick"
dt_ms = 16

[[script]]
at_frame = 1
op = "key"
code = "Down"

[[script]]
at_frame = 2
op = "key"
code = "Enter"

[[script]]
at_frame = 3
op = "mouse"
kind = "down"
button = "left"
x = 4
y = 3

# Expectations (contract evidence)
[expect]
messages = ["Activated(beta)"]          # component message log patterns
snapshot = "snapshots/list-selection.snap"
semantic_scene = "scenes/list-selection.scene.json"
focus = ["list:beta"]
overlay_layers = []
contract_axes = ["keyboard", "narrow", "selection"]

[registry]
# Optional: install command shown in Studio
install = "termrock add termrock/list-selection-demo"
item = "termrock/list-selection-demo"
```

### 2.2 Semantic scene expectation (`*.scene.json`)

```json
{
  "schema": 1,
  "focused": "list:beta",
  "layers": [
    { "id": "root", "kind": "Root", "owns_input": true, "esc": "Ignore" }
  ],
  "elements": [
    {
      "id": "list:beta",
      "layer": "root",
      "role": "Control",
      "focusable": true,
      "area": { "x": 1, "y": 2, "width": 40, "height": 1 },
      "actions": ["Activate"]
    }
  ]
}
```

### 2.3 Snapshot files

- **Cell snapshot** (primary): deterministic buffer dump — symbol + style indices.  
- **SVG** (docs/previews): current lookbook path retained as derived artifact.  
- **Text** (optional): plain grapheme grid for PR review.

Format: see §6.

### 2.4 Story completeness rule

A story is **Studio-complete** only if it can define:

| Field | Required |
|-------|----------|
| Initial state | yes |
| Terminal dimensions | yes |
| Terminal capabilities | yes (or matrix expands) |
| Theme | yes |
| Input events | if interactive |
| Frame ticks | if time-dependent |
| Expected messages | if outcomes claimed |
| Expected semantic scene | if focus/hit/scene claimed |
| Expected rendered snapshot | yes for CI gate |

Missing evidence ⇒ contract axis cannot claim PASS.

---

## 3. Rust APIs (public kernel + studio harness)

### 3.1 Public kernel additions (if missing → implement in crate)

Studio must not poke private fields. The following are **public contracts**:

```rust
// --- Inspection protocol (public) ---------------------------------

/// Frame-scoped inspection snapshot exported by hosts after render.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectionFrame {
    pub tick: FrameTick,
    pub viewport: Rect,
    pub focus: Option<String>,
    pub focus_order: Vec<String>,
    pub layers: Vec<InspectLayer>,
    pub elements: Vec<InspectElement>,
    pub hits: Vec<InspectHit>,
    pub overlays: Vec<InspectOverlay>,
    pub keymap_hints: Vec<InspectHint>,
    pub messages: Vec<ComponentMessage>,
    pub events: Vec<InspectEvent>,
    pub perf: PerfSample,
    pub tokens: TokenSnapshot,
    pub buffer_digest: BufferDigest,
}

pub struct InspectLayer {
    pub id: String,
    pub kind: LayerKind,
    pub owns_input: bool,
    pub esc: LayerDismissPolicy,
    pub outside: LayerDismissPolicy,
}

pub struct InspectElement {
    pub id: String,
    pub layer: String,
    pub area: Rect,
    pub role: SemanticRole,
    pub focusable: bool,
    pub enabled: bool,
    pub actions: Vec<String>,
}

pub struct InspectHit {
    pub id: String,
    pub area: Rect,
}

pub struct InspectOverlay {
    pub id: String,
    pub kind: OverlayKind,
    pub rect: Rect,
    pub z: u32,
    pub backdrop: BackdropPolicy,
}

pub struct ComponentMessage {
    pub at: FrameTick,
    pub component: String,
    pub kind: String,       // "outcome", "warning", "log"
    pub body: String,       // Display form of typed outcome
}

pub struct PerfSample {
    pub frame_time_us: u64,
    pub paint_time_us: u64,
    pub layout_time_us: u64,
    /// Optional; zero when not measured
    pub allocations: u64,
    pub allocation_bytes: u64,
}

pub struct BufferDigest {
    pub width: u16,
    pub height: u16,
    pub content_sha256: String,
    pub style_sha256: String,
}

pub struct TokenSnapshot {
    pub density: Density,
    pub capability: ColorCapability,
    pub glyph_set: GlyphSet,
    pub motion: Motion,
    pub roles: Vec<(Role, String)>, // role → debug color name
}
```

**Producer API** — stories / apps opt in:

```rust
/// Implemented by studio hosts and advanced app shells.
pub trait Inspectable {
    fn inspection_frame(&self) -> InspectionFrame;
}

/// Helper: build InspectionFrame from InteractionScene + OverlayStack + Perf.
pub fn inspect_scene<Id, LayerId, Action>(
    scene: &InteractionScene<Id, LayerId, Action>,
    overlays: Option<&OverlayStack>,
    tick: FrameTick,
    viewport: Rect,
    perf: PerfSample,
    tokens: TokenSnapshot,
    buffer: &Buffer,
) -> InspectionFrame
where
    Id: Clone + Display,
    LayerId: Clone + Display,
    Action: Display;
```

If a widget cannot contribute hits/focus without private access, that is a **crate bug**: expose projection methods (as List already exposes hit regions pattern).

### 3.2 Studio harness APIs (`termrock` public module `studio` or `termrock-studio` lib)

Prefer **`termrock::studio`** for formats + replay engine (no TUI), and keep the interactive binary in `termrock-studio` (rename from lookbook).

```rust
/// Declared story metadata + environment (mirrors story.toml).
pub struct StorySpec {
    pub id: String,
    pub title: String,
    pub component: String,
    pub tags: Vec<String>,
    pub viewport: ViewportSpec,
    pub environment: EnvironmentSpec,
    pub knobs: Vec<KnobSpec>,
    pub script: Vec<ScriptOp>,
    pub expect: ExpectSpec,
    pub registry: Option<RegistryLink>,
}

pub struct ViewportSpec { pub width: u16, pub height: u16 }
pub struct EnvironmentSpec {
    pub theme_id: String,
    pub density: Density,
    pub color_capability: ColorCapability,
    pub glyph_set: GlyphSet,
    pub motion: Motion,
    pub selection_chrome: SelectionChrome,
}

pub enum ScriptOp {
    Tick { dt_ms: u64 },
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize { width: u16, height: u16 },
    SetKnob { id: String, value: KnobValue },
    SetTheme { theme_id: String },
    SetCapability { color: ColorCapability },
    SetGlyphs { glyphs: GlyphSet },
    WaitMessages { count: usize },
}

/// Host implemented by each story (public widget APIs only).
pub trait StoryHost {
    fn id(&self) -> &str;
    fn spec(&self) -> &StorySpec;
    fn reset(&mut self, state: &serde_json::Value, env: &EnvironmentSpec);
    fn apply_knob(&mut self, id: &str, value: &KnobValue) -> bool;
    fn handle_event(&mut self, event: Event) -> Vec<ComponentMessage>;
    fn tick(&mut self, tick: FrameTick) -> Vec<ComponentMessage>;
    fn render(&mut self, area: Rect, buf: &mut Buffer);
    /// After render: contribute inspection via public scene/state.
    fn inspect(&self, tick: FrameTick, area: Rect, buf: &Buffer) -> InspectionFrame;
}
```

### 3.3 Property / state controls

```rust
pub enum KnobValue {
    Bool(bool),
    Choice { index: usize, labels: Vec<String> },
    Text(String),
    Number(i64),
    Color(String),      // token override id
}

pub struct KnobSpec {
    pub id: String,
    pub label: String,
    pub value: KnobValue,
}
```

Knobs **must** call `StoryHost::apply_knob` (real state), never paint-only toggles.

### 3.4 Theme + live token editing

```rust
/// Transactional theme edit (ThemePicker pattern, public).
pub struct ThemeEditSession {
    base: Theme,
    draft: Theme,
}

impl ThemeEditSession {
    pub fn set_role(&mut self, role: Role, color: Color);
    pub fn commit(self) -> Theme;
    pub fn revert(&mut self);
    pub fn quantized(&self, cap: ColorCapability) -> Theme;
}
```

Studio Properties “Tokens” panel binds to `ThemeEditSession` + `DesignTokens` density. Live preview re-renders Stage with draft theme. Commit/Revert are explicit.

### 3.5 Capability / density / resize simulation

```rust
pub struct StageEnvironment {
    pub size: (u16, u16),
    pub env: EnvironmentSpec,
}

impl StageEnvironment {
    pub fn with_width(self, w: u16) -> Self;
    pub fn matrix_axes() -> &'static [Axis]; // color, glyph, density, motion
}
```

Resize simulation does **not** require OS terminal resize: Stage clips a logical buffer of `viewport` inside a larger Studio window.

---

## 4. Recording format

### 4.1 Goals

- Deterministic replay across machines  
- Human-inspectable  
- Hash-stable for CI  
- No host paths / secrets  

### 4.2 `*.rec.json` (or msgpack for large)

```json
{
  "schema": 1,
  "story_id": "list/selection",
  "recorded_at": "2026-08-09T12:00:00Z",
  "studio_version": "0.12.0",
  "kernel_rev": "FULL_SHA",
  "environment": {
    pub_theme: "phosphor-default",
    "density": "comfortable",
    "color_capability": "truecolor",
    "glyph_set": "unicode",
    "motion": "full",
    "viewport": { "width": 48, "height": 12 }
  },
  "initial_state": { },
  "frames": [
    {
      "i": 0,
      "tick_ms": 0,
      "inputs": [],
      "messages": [],
      "buffer_digest": { "content_sha256": "…", "style_sha256": "…", "width": 48, "height": 12 },
      "inspect_digest": "sha256-…"
    },
    {
      "i": 1,
      "tick_ms": 16,
      "inputs": [
        { "type": "key", "code": "Down", "modifiers": [] }
      ],
      "messages": [
        { "kind": "outcome", "body": "SelectionChanged" }
      ],
      "buffer_digest": { "…": "…" },
      "inspect_digest": "sha256-…"
    }
  ],
  "recording_digest": "sha256-…"
}
```

**Rules:**

- Inputs are neutral `termrock::input` events (serde).  
- Digests only by default; full buffer dump is opt-in (`--record-buffers`).  
- `inspect_digest` hashes canonical `InspectionFrame` JSON without perf timings (perf is advisory).

---

## 5. Replay engine

```rust
pub struct ReplayEngine<H: StoryHost> {
    host: H,
    recording: Recording,
    cursor: usize,
    clock: VirtualClock,
}

pub enum ReplayStepResult {
    Advanced { frame: u32, messages: Vec<ComponentMessage>, inspect: InspectionFrame },
    Finished,
    Mismatch(ReplayMismatch),
}

pub struct ReplayMismatch {
    pub frame: u32,
    pub kind: MismatchKind, // Buffer | Message | Scene | Focus | Overlay
    pub expected: String,
    pub actual: String,
}

impl<H: StoryHost> ReplayEngine<H> {
    pub fn reset(&mut self);
    pub fn step(&mut self) -> ReplayStepResult;
    pub fn run_all(&mut self) -> Result<(), Vec<ReplayMismatch>>;
    pub fn seek(&mut self, frame: u32) -> Result<(), ReplayError>; // rebuild from 0
}
```

### 5.1 Algorithm

```
reset host with initial_state + environment
for each frame in recording:
  apply inputs in order
  advance VirtualClock / FrameTick
  host.tick(tick)
  render to MemoryBackend buffer
  inspect = host.inspect(...)
  compare digests / expectations
  on mismatch: collect and continue or fail-fast (flag)
```

### 5.2 Virtual clock

Use `runtime::FrameTick` only — no `Instant::now()` in story logic. Studio supplies ticks from script/recording.

### 5.3 Headless CLI

```bash
termrock-studio replay stories/list/selection.rec.json
termrock-studio test --story list/selection
termrock-studio matrix --story list/selection --axes color,glyph
```

Exit codes: 0 pass, 5 mismatch, 2 invalid input (align with registry CLI).

---

## 6. Snapshot architecture

### 6.1 Pipeline

```
StoryHost.render → ratatui Buffer
       │
       ├─► BufferDigest (CI fast path)
       ├─► CellSnapshot file (stable, line-oriented)
       ├─► SVG (docs/public previews; existing lookbook path)
       └─► optional PNG via external renderer (out of core)
```

### 6.2 Cell snapshot format (`.snap`)

```
# termrock-snapshot 1
# id: list/selection
# size: 48x12
# content_sha256: …
# style_sha256: …
|row|cells...
|0|Panel title…|
|1| › beta     |
```

Or binary CBOR with schema version for fidelity. **Default for git:** text grid of graphemes + style class ids mapped through theme role names (not raw RGB) so phosphor tweaks don’t thrash snapshots unless roles change.

**Style encoding policy:**

1. Prefer **role tags** when cell style matches a known role.  
2. Fall back to quantized RGB hex under active `ColorCapability`.  
3. Snapshot header records capability + glyph set.

### 6.3 Update workflow

```bash
termrock-studio snapshot --update list/selection
termrock-studio check          # compare all (replaces lookbook check)
```

Never auto-update in CI.

### 6.4 Matrix snapshots

Story id × axis value → `snapshots/{id}/{axis}={value}.snap`.

---

## 7. Component inspection protocol

### 7.1 Lifecycle (each Studio frame)

```
1. begin_frame (scene.clear elements, overlay sync)
2. story render into Stage buffer (public widgets)
3. story registers InteractionScene elements (if participating)
4. inspect_scene(...) → InspectionFrame
5. Inspector panels bind to InspectionFrame (read-only)
6. overlays drawn: focus order numbers, hit rects, layer stack
```

### 7.2 Visualization layers (Stage overlays)

| Overlay | Paint |
|---------|--------|
| Focus order | Numbered badges in focus cycle order |
| Hit regions | Rect outlines (role-colored) |
| Semantic scene | Id labels at element origins |
| Overlay stack | Z-index chips + backdrop dim truth |
| Buffer inspect | Cell under cursor → char, role, width |

Toggles in Inspector; all drawn **above** story using public `Buffer` cell writes in Studio chrome (not inside the story widget).

### 7.3 Event log & message log

| Log | Source |
|-----|--------|
| **Event log** | Studio-routed `input::Event` after Stage focus (keys, mouse, resize, paste) |
| **Message log** | `ComponentMessage` from `StoryHost::handle_event` / outcomes |

Both ring-buffered (e.g. 500 entries), filterable, exportable to recording.

### 7.4 Perf metrics

```rust
// Measured by Studio harness around public calls — not inside widgets.
let t0 = VirtualInstant; // or std for host perf only
host.render(...);
perf.paint_time_us = elapsed;
```

Allocation metrics: optional `dhat` / `stats_alloc` feature on **studio binary only** — never required in kernel. Display “n/a” when disabled.

### 7.5 Keymap display

From public `Keymap` + `Visibility::Shown` bindings + scene `available_actions`. Studio shows chord → action → element affinity.

### 7.6 Incomplete API detection

Studio CI mode:

```
RUST_LOG=termrock_studio=info
```

If harness needs `#[cfg]` internal fields or `pub(crate)` types, build fails with:

```
API_GAP: InspectionFrame.hits requires ListState::hit_regions() public export
```

Track gaps in `docs/design/studio-api-gaps.md` until closed.

---

## 8. Registry integration

Studio is the **visual front door** for source-owned components ([source-owned-registry.md](./source-owned-registry.md)).

| Studio UI | CLI equivalent |
|-----------|----------------|
| Registry → Available → Install | `termrock add <id>` |
| Registry → Installed → Diff | `termrock diff <id>` |
| Registry → Update | `termrock update --dry-run` |
| Copy install command | clipboard / OSC 52 consumer-owned |

### 8.1 Story ↔ registry

```toml
[registry]
item = "termrock/tool-card"
install = "termrock add termrock/tool-card"
source_path = "src/ui/components/tool_card.rs"  # if project has termrock.toml
```

**Upstream/local source diff panel:**  

- Left: upstream blob from registry (read-only)  
- Right: local file if installed  
- Status: clean / dirty / missing (lock hashes)

Studio shells out to `termrock` CLI or links `termrock-registry` as a library — **no second resolver**.

### 8.2 Without registry CLI

Browse mode still works for in-tree stories. Registry tabs show “CLI not installed” CTA.

---

## 9. Documentation generation

### 9.1 From stories

| Artifact | Generator |
|----------|-----------|
| Live previews | shared Lookbook demo catalog mounted by native and web hosts |
| No-JS fallback | one deterministic JSON poster per embedded demo |
| MDX/docs pages | checked-in canonical pages under `docs/content/...` |
| Contract matrix | merge story `expect.contract_axes` × PASS/FAIL |
| API inventory | rustdoc / public-api (existing) + story coverage % |
| Interaction recipes | script ops → readable steps |

### 9.2 Doc page template per component

```markdown
# List
## Anatomy / contraction
## Live terminal
- mounted shared Rust demo with current hints, Reset, and typed outcomes
## Contracts
| Axis | Evidence story | Status |
## Install
`termrock add …` (if registry)
## Keyboard
(from keymap export)
```

### 9.3 Command

```bash
bun --cwd docs run build:preview-posters
bun --cwd docs run check:components
bun --cwd docs run build
```

---

## 10. Migration from current lookbook

### 10.1 Crate layout

| Today | Target |
|-------|--------|
| `termrock-lookbook` | Rename/alias `termrock-studio` (keep binary symlink `termrock-lookbook` one release) |
| `stories.rs` megafile | `stories/<component>/*.rs` + optional `*.story.toml` |
| `interactors.rs` | `StoryHost` impls per component |
| `svg.rs` | `studio::snapshot` + svg adapter |
| `app.rs` Lookbook | `StudioApp` multi-panel IA |
| `knobs.rs` | public `studio::KnobSpec` |
| DesignInspector | expand panels (Events, Perf, Buffer, Diff, Contracts) |
| `layout_studio_shell` | evolve slots: library | stage | properties | inspector |

### 10.2 Compatibility bridge

```rust
// Temporary adapter
impl StoryHost for LegacyInteractorAdapter {
    // wrap existing StoryInteraction trait
}
```

All current stories keep rendering on day one via adapter. Completeness upgrades are incremental.

### 10.3 CLI migration

| Old | New |
|-----|-----|
| `termrock-lookbook terminal` | `termrock-studio terminal` |
| `termrock-lookbook list` | `termrock-studio list` |
| `termrock-lookbook render` | `termrock-studio snapshot` / `docs` |
| `termrock-lookbook check` | `termrock-studio check` |
| — | `replay`, `test`, `matrix`, `record` |

Migration file (when shipped): next free `migrations/00xx-termrock-studio.md`.

### 10.4 Phased delivery

| Phase | Deliverable | Exit criteria |
|-------|-------------|---------------|
| **S0** | Design freeze (this doc) | Reviewed |
| **S1** | `termrock::studio` types: StorySpec, ScriptOp, InspectionFrame, digests | Unit tests; no private APIs |
| **S2** | Headless replay + snapshot check for 5 pilot stories | CI job green |
| **S3** | StudioApp IA: library search, stage env matrix, properties knobs real | Manual UX review |
| **S4** | Inspector: focus/hits/scene/overlays/events/messages/perf/buffer | Visual proof on InteractionScene stories |
| **S5** | Record/replay UI + `.rec.json` | Round-trip test |
| **S6** | Contract evidence gate (fail docs if axis unproven) | Replaces soft JSON claims |
| **S7** | Registry panels + source diff | Works with plan 047 CLI |
| **S8** | Full story migration off megafile; lookbook name deprecated | All stories StoryHost |

Plan 048 remains the historical “shell seed.” Studio phases S1+ are the continuation program.

---

## 11. Feature coverage map

| Required capability | Mechanism |
|---------------------|-----------|
| Component/block browsing | Library tree + tags |
| Search | Fuzzy over id/title/component/tags |
| Registry install commands | Registry tab + copy `termrock add` |
| Interactive properties | Knobs → `apply_knob` |
| State controls | `initial_state` JSON + knobs + reset |
| Theme switching | Environment + ThemePicker session |
| Live token editing | `ThemeEditSession` |
| Density switching | Environment density → DesignTokens |
| Terminal resize simulation | ViewportSpec independent of host tty |
| Capability simulation | ColorCapability quantize path |
| Unicode vs ASCII | GlyphSet matrix / compare mode |
| Truecolor / 256 / ANSI / no-color | ColorCapability ladder |
| Keyboard map display | Keymap + available_actions inspect |
| Focus-order visualization | Overlay on Stage from InspectionFrame |
| Hit-region visualization | Hit outlines |
| Semantic-scene inspection | Inspector Scene tab |
| Overlay-layer inspection | Overlay stack tab + z |
| Event log | Studio event ring |
| Component message log | Outcome messages |
| Frame time | PerfSample host timing |
| Allocation metrics | Optional studio feature |
| Buffer inspection | Cursor cell inspector |
| Snapshot creation | `snapshot --update` |
| Interaction recording | Record mode → `.rec.json` |
| Replay | ReplayEngine |
| Upstream/local source diff | Registry + lock hashes |
| Contract-test status | expect × replay results |

---

## 12. Security & determinism

- Stories are code in-repo; **do not** download story code from registry without install flow.  
- Recordings contain no absolute paths.  
- Replay uses virtual time only.  
- Network: only registry CLI integration when user opens Registry.  
- Studio binary may use crossterm; **story crates** stay backend-agnostic.

---

## 13. Testing strategy

| Layer | Tests |
|-------|-------|
| Spec parse | TOML/JSON fixtures |
| Digest stability | golden buffers under env matrix |
| Replay | pilot recordings pass |
| Adapter | legacy interactors still render |
| API surface | studio module public-api approved |
| Gap detector | forbid `pub(crate)` imports from studio → termrock internals |
| Contract gate | unproven axis fails `studio check --strict` |

---

## 14. Example: pilot StoryHost (List)

```rust
pub struct ListSelectionStory {
    spec: StorySpec,
    state: ListState<&'static str>,
    rows: Vec<ListRow<'static, &'static str>>,
    tokens: DesignTokens,
    messages: Vec<ComponentMessage>,
    scene: InteractionScene<&'static str, &'static str, UiIntent>,
}

impl StoryHost for ListSelectionStory {
    fn reset(&mut self, payload: &Value, env: &EnvironmentSpec) {
        // parse payload; rebuild rows; tokens from env.density + theme
        self.messages.clear();
        self.scene = InteractionScene::new();
    }

    fn handle_event(&mut self, event: Event) -> Vec<ComponentMessage> {
        // route via public List APIs / intents
        // push ComponentMessage for outcomes
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        // StatefulWidget::render List
        // register hits into scene with public HitRegion data
    }

    fn inspect(&self, tick: FrameTick, area: Rect, buf: &Buffer) -> InspectionFrame {
        inspect_scene(&self.scene, None, tick, area, PerfSample::default(),
            token_snapshot(&self.tokens), buf)
    }
}
```

No `termrock` internals. If scene registration is painful, improve **public** List/scene helpers.

---

## 15. Decision summary

1. Lookbook is the seed; **Studio is the product**.  
2. Declarative scenarios + typed `StoryHost` dual format.  
3. **InspectionFrame** is the DevTools protocol — public, stable, complete.  
4. Record/replay/snapshot form the CI backbone.  
5. Registry is integrated, not reimplemented.  
6. Private API use is a **failed build**, not a shortcut.  
7. Migrate incrementally via legacy adapter; do not big-bang break gallery.

---

## 16. Open questions

1. Ship studio types in `termrock` vs separate `termrock-studio-core` crate?  
   **Recommendation:** specs/replay/digests in `termrock::studio` (no extra dep); TUI binary separate.  
2. Serde on `KeyEvent` — already neutral input; confirm feature flag `studio` or always-on.  
3. Allocation metrics: require nightly? **Optional feature only.**  
4. Whether SVG remains canonical for docs or CellSnapshot supersedes — **both**: digest CI, SVG human docs.
