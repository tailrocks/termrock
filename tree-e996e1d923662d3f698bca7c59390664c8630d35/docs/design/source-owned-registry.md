# Source-owned component ecosystem

**Status:** design SoT (full target architecture)  
**Spike (live):** `termrock-cli` offline plan/add/diff/check + `registry/fixtures/*` (plan 047)  
**Philosophy:** [shadcn/ui](https://ui.shadcn.com/docs) open-code ownership, adapted to Rust/Ratatui  
**Related:** plan `047-source-registry-cli-spike.md`, `shadcn-quality-roadmap.md` R7, `architecture-foundation.md`  
**Non-goal:** replace Cargo or force all widgets into copy-paste. The **kernel crate stays**.

---

## 1. Architecture

### 1.1 Hybrid distribution model

```
┌─────────────────────────────────────────────────────────────────┐
│  Application (user owns)                                        │
│  ├── termrock.toml          # project manifest                  │
│  ├── termrock.lock          # pinned digests + install state    │
│  ├── src/ui/…               # installed source (owned/modified) │
│  └── Cargo.toml             # kernel + transitive crates        │
└───────────────────────────────▲─────────────────────────────────┘
                                │ termrock add/diff/update
┌───────────────────────────────┴─────────────────────────────────┐
│  Registry layer (source packages, not crates)                   │
│  ├── Official: registry.termrock.dev / git tree                 │
│  ├── Private:  https://… / path:/… / git+ssh://…                │
│  └── Item types: component | block | theme | keymap | template  │
└───────────────────────────────▲─────────────────────────────────┘
                                │ depends on public APIs only
┌───────────────────────────────┴─────────────────────────────────┐
│  Stable crates (versioned, not copied)                          │
│  termrock (kernel): interaction, layout, tokens, scroll, input  │
│  optional: termrock-crossterm adapters, lookbook helpers        │
└─────────────────────────────────────────────────────────────────┘
```

| Layer | Ownership | Update path |
|-------|-----------|-------------|
| **Kernel crate** (`termrock`) | Shared binary dependency | Cargo pin / rev bump + migrations |
| **Registry items** | **Copied into app** | `termrock update` with 3-way safety |
| **App domain** | App-only | Never upstream |

### 1.2 What stays in stable crates vs what is copied

#### Stable crates (do **not** copy)

These are capability, correctness, and contract surfaces. Copying them would fork security, focus, Unicode, and Esc law.

| Module family | Why crate-stable |
|---------------|------------------|
| `interaction` (Scene, OverlayStack, intents, focus) | Esc law, focus traps, hit order |
| `layout` (Workspace, responsive, dialog geometry) | Shared contraction grammar |
| `style` (Theme, DesignTokens, Density, roles) | Semantic paint contract |
| `input` / `keymap` core | Neutral events, chord vocabulary |
| `scroll`, `text` (grapheme-safe measure/clip) | Hot-path + Unicode safety |
| `runtime` Session / FrameTick | Lifecycle + testable time |
| Leaf widgets that are **domain-neutral primitives** with heavy contracts (Panel, List virtualization, Tree, Table solver, TextInput/Area, Dialog shell, Progress, …) | Shared bugfixes must flow via crate |

Primitives **may** still appear in the registry as *thin re-export recipes* or *styled variants*, but the engine stays in the crate.

#### Source-owned (install / copy / adapt)

| Item type | Examples | Why copy |
|-----------|----------|----------|
| **Styled components** | ToolCard chrome, ApprovalCard product layout, PromptBox skin | Brand + wording + extra slots |
| **Application blocks** | AgentWorkbench shell, OpsDashboard, ResourceBrowser, StudioShell | Composition + product layout |
| **Theme packages** | Phosphor Obsidian preset, brand theme maps | Product identity |
| **Keymap packages** | Agent default chords, vim-like collection maps | Product muscle memory |
| **Templates** | Minimal TUI app, agent CLI skeleton | Scaffold, not runtime |
| **Story + fixture packs** | Lookbook-ready stories for an installed block | Local validation |

**Rule of thumb:** if changing the code is how you brand or productize it → registry. If a bugfix must reach every consumer without merge conflict → crate.

### 1.3 Trust boundary

```
[Untrusted] registry metadata + file contents + remote indexes
     │
     ▼  fetch → verify schema → verify digests → plan
[Trusted]   user's workspace + termrock.toml policy
     │
     ▼  atomic write only after plan + confirm
[Owned]     installed sources under allowed roots
```

Registry content is **untrusted input**. The CLI never executes registry scripts, never runs templates as code, never writes outside validated roots, never silently overwrites dirty files.

---

## 2. Registry schema

### 2.1 Index document (`registry.json` or `registry.toml`)

```json
{
  "schema": 1,
  "name": "termrock-official",
  "namespace": "termrock",
  "homepage": "https://termrock.dev/registry",
  "license_default": "Apache-2.0",
  "generated_at": "2026-08-09T00:00:00Z",
  "items": [
    { "name": "tool-card", "type": "component", "latest": "0.12.0", "path": "items/tool-card/0.12.0/item.json" }
  ]
}
```

TOML equivalent is allowed; **canonical digest uses JSON with sorted keys** (see §2.6).

### 2.2 Item document (`item.json`)

```json
{
  "schema": 1,
  "namespace": "termrock",
  "name": "agent-workbench",
  "type": "block",
  "version": "0.12.0",
  "title": "Agent Workbench",
  "description": "Flagship multi-pane agent shell: transcript, rail, prompt, approval.",
  "license": "Apache-2.0",
  "provenance": {
    "origin": "https://github.com/tailrocks/termrock",
    "path": "registry/termrock/blocks/agent-workbench",
    "revision": "FULL_GIT_SHA",
    "spdx": "Apache-2.0",
    "authors": ["Tailrocks contributors"],
    "notice": "NOTICE excerpt or path"
  },
  "kernel": {
    "crate": "termrock",
    "min_version": "0.12.0",
    "features": ["crossterm"],
    "git": {
      "url": "https://github.com/tailrocks/termrock.git",
      "rev_range_hint": ">= a230ab8"
    }
  },
  "cargo_dependencies": [
    { "name": "ratatui-core", "version": "0.1.2", "default_features": false },
    { "name": "unicode-width", "version": "0.2" }
  ],
  "features": {
    "provides": [],
    "requires_kernel": ["crossterm"]
  },
  "registry_dependencies": [
    { "namespace": "termrock", "name": "tool-card", "version": "^0.12.0" },
    { "namespace": "termrock", "name": "prompt-composer", "version": "^0.12.0" }
  ],
  "capabilities": {
    "color": ["truecolor", "ansi256", "mono"],
    "glyphs": ["unicode", "ascii"],
    "osc": ["8", "52"],
    "min_width": 40,
    "min_height": 12,
    "responsive_surface": "AppShell"
  },
  "files": [
    {
      "source": "src/workbench.rs",
      "target": "src/ui/blocks/agent_workbench.rs",
      "role": "primary",
      "hash": "sha256-…",
      "mode": "0644"
    },
    {
      "source": "src/layout.rs",
      "target": "src/ui/blocks/agent_workbench_layout.rs",
      "role": "support",
      "hash": "sha256-…"
    },
    {
      "source": "stories/agent_workbench.rs",
      "target": "src/ui/stories/agent_workbench.rs",
      "role": "story",
      "hash": "sha256-…",
      "optional": true
    },
    {
      "source": "tests/fixtures/session.json",
      "target": "tests/ui_fixtures/agent_workbench_session.json",
      "role": "fixture",
      "hash": "sha256-…",
      "optional": true
    }
  ],
  "module": {
    "rust_mod_path": "crate::ui::blocks::agent_workbench",
    "exports": ["AgentWorkbench", "AgentWorkbenchState", "AgentWorkbenchOutcome"]
  },
  "migrations": [
    { "from": "0.11.0", "to": "0.12.0", "doc": "migrations/0.12.0.md" }
  ],
  "docs": {
    "readme": "README.md",
    "contract": "CONTRACT.md",
    "lookbook_story_ids": ["agent-workbench/full", "agent-workbench/narrow"]
  },
  "item_digest": "sha256-…"
}
```

### 2.3 Registry item types

| `type` | Meaning | Install root default |
|--------|---------|----------------------|
| `component` | Single styled widget / composition | `{ui}/components/` |
| `block` | Application block (multi-file shell) | `{ui}/blocks/` |
| `theme` | Theme + token map + optional glyphs | `{ui}/themes/` |
| `keymap` | Keymap tables / intent bindings | `{ui}/keymaps/` |
| `template` | Project scaffold (files + Cargo snippets) | project root (explicit paths only) |
| `story` | Story/interactor pack only | `{ui}/stories/` |
| `fixture` | Test fixtures only | `tests/ui_fixtures/` |

### 2.4 Namespaces

- Format: `{namespace}/{name}` (e.g. `termrock/tool-card`, `acme/ops-sidebar`).
- Official namespace: `termrock`.
- Private registries declare their own namespace; CLI rejects collisions unless registry priority is ordered in `termrock.toml`.
- Names: lowercase `[a-z0-9-]+`, no path separators, no leading dots.

### 2.5 Component / Cargo / feature dependencies

| Dependency kind | Resolver behavior |
|-----------------|-------------------|
| `registry_dependencies` | Install graph (DFS, cycle fail). Version ranges: caret default. |
| `cargo_dependencies` | Plan Cargo.toml edits; never auto-bump beyond declared range without confirm. |
| `kernel.features` | Ensure consumer enables features; warn if missing. |
| `features.provides` | Optional Cargo features on the **app** crate when installing optional packs. |

### 2.6 Source hashes and digests

- Per-file: `sha256` of **raw bytes** as stored (LF-normalized at publish time; install preserves LF).
- `item_digest`: sha256 of canonical JSON of the item **with** file hashes, **without** `item_digest` field.
- Lockfile records both for offline verify.

### 2.7 Capability requirements

`capabilities` is advisory at install time and hard-checked by `termrock doctor` / lookbook:

- Color ladder, glyph set, OSC needs, min terminal size, responsive surface name for contraction tests.

---

## 3. Project manifest: `termrock.toml`

```toml
# termrock.toml — consumer project configuration
schema = 1

[project]
name = "my-agent"
# Allowed install roots (relative to workspace root). Writes outside → refuse.
ui_root = "src/ui"
stories_root = "src/ui/stories"
fixtures_root = "tests/ui_fixtures"

[kernel]
# Mirror Cargo pin; CLI validates lock vs Cargo.toml
source = "git"
git = "https://github.com/tailrocks/termrock.git"
rev = "a230ab8deadbeef…"          # exact pin preferred
# version = "0.12.0"             # when crates.io exists
features = ["crossterm"]

[[registries]]
name = "official"
namespace = "termrock"
# Exactly one source per registry entry
url = "https://registry.termrock.dev/v1/index.json"
# path = "./vendor/termrock-registry"   # offline / vendored
# git = "https://github.com/tailrocks/termrock-registry.git"
# rev = "…"
priority = 100
trust = "default"                 # default | pinned | offline-only

[[registries]]
name = "acme-private"
namespace = "acme"
url = "https://registry.internal.acme/termrock/index.json"
priority = 50
trust = "pinned"
# optional: pin index digest
# index_digest = "sha256-…"

[defaults]
# Non-interactive safety
require_confirm = true
install_stories = true
install_fixtures = true
# Never overwrite dirty files unless user passes --force-dirty with backup
force_dirty = false

[aliases]
workbench = "termrock/agent-workbench"
tool = "termrock/tool-card"

# Installed items (authoritative install set; digests in lockfile)
[components."termrock/tool-card"]
version = "0.12.0"
path = "src/ui/components/tool_card.rs"   # primary target override optional

[components."termrock/agent-workbench"]
version = "0.12.0"

[themes."termrock/phosphor-obsidian"]
version = "0.12.0"

[keymaps."termrock/agent-default"]
version = "0.12.0"
```

### 3.1 Lockfile: `termrock.lock`

Machine-written; do not hand-edit.

```toml
schema = 1
lock_version = 1

[[item]]
id = "termrock/agent-workbench"
version = "0.12.0"
item_digest = "sha256-…"
registry = "official"
installed_at = "2026-08-09T12:00:00Z"
kernel_rev = "a230ab8…"

[[item.file]]
target = "src/ui/blocks/agent_workbench.rs"
# Hash of content as originally installed (upstream at install time)
upstream_hash = "sha256-…"
# Hash after last successful clean install/update that user accepted
recorded_hash = "sha256-…"
```

**Three-way compare uses:**

1. `upstream_hash` / installed version’s file hash (version originally installed)  
2. Current upstream (resolved latest or requested version) file hash  
3. On-disk file hash (local modifications)

---

## 4. CLI UX

Binary: `termrock` (crate `termrock-cli`).

| Command | Behavior |
|---------|----------|
| `termrock init` | Create `termrock.toml`, optional `src/ui` tree, suggest kernel Cargo pin, write empty lock |
| `termrock search [query]` | Search configured registries (name, tags, type) |
| `termrock view <id>` | Show item metadata, files, deps, capabilities, license |
| `termrock add <id[@ver]>` | Resolve graph → dry-run plan → confirm → atomic install |
| `termrock diff [id]` | Three-way status + unified diffs; **no writes** |
| `termrock update [id]` | Plan upgrades; never blind overwrite (see §6) |
| `termrock doctor` | Kernel pin, digests, dirty files, missing files, capabilities, licenses |
| `termrock migrate [id]` | Apply registry item migration docs / scripted file renames **as plans** |
| `termrock story [id]` | List/run installed stories or open lookbook ids |
| `termrock remove <id>` | Remove from manifest; delete files only if clean & confirm |
| `termrock registry list` | Show registries + priority |
| `termrock registry add` | Add private registry |
| `termrock lock` | Re-hash installed files into lock (after intentional sync) |

### 4.1 Global flags

```
--dry-run              plan only (default for update until --apply)
--yes                  skip interactive confirm (still refuses dirty without --force-dirty)
--force-dirty          allow overwrite of modified files after writing .bak
--json                 machine output
--offline              only path/git-vendored registries + lock
--registry <name>      limit resolution
```

### 4.2 Example sessions

```text
$ termrock init
  wrote termrock.toml
  suggest: termrock = { git = "…", rev = "…" }

$ termrock add termrock/agent-workbench
  plan:
    + src/ui/blocks/agent_workbench.rs          (new)
    + src/ui/blocks/agent_workbench_layout.rs   (new)
    + src/ui/components/tool_card.rs            (dep)
    ~ Cargo.toml  features += crossterm
  Proceed? [y/N]

$ termrock diff termrock/tool-card
  tool-card@0.12.0
    src/ui/components/tool_card.rs  LOCAL_MODIFIED
    --- upstream 0.12.0
    +++ local
    @@ paint accent …

$ termrock update
  tool-card  0.12.0 → 0.12.1  (clean)     → apply
  workbench  0.12.0 → 0.12.1  (dirty)     → skip (use --force-dirty or merge manually)
```

### 4.3 Exit codes

| Code | Meaning |
|------|---------|
| 0 | Clean / success |
| 1 | Generic failure |
| 2 | Invalid input / schema |
| 3 | Conflict (dirty, cycle, path) |
| 4 | Network / registry unavailable |
| 5 | Differences found (`diff`/`doctor` when not clean) |

---

## 5. Resolution algorithm

```
resolve(request, config, lock, registries):
  1. Expand aliases; parse id@version (default: manifest pin or "latest compatible with kernel").
  2. Select registry by namespace + priority (highest priority wins; fail on ambiguous multi-match).
  3. Load item metadata; verify schema version; verify item_digest.
  4. Check kernel compatibility (min_version / rev hint vs config.kernel).
  5. Build dependency graph of registry_dependencies (BFS).
     - Fail on cycles.
     - Fail on two versions of same id required unless both satisfy intersection (prefer lock pin).
  6. For each item, compute file plan:
     - target path under allowed roots only (normalize, reject .., abs, symlink escape).
     - classify each target: Missing | Clean | Dirty | IdenticalUpstream
       Clean   = on-disk hash == recorded_hash == upstream_hash (installed)
       Dirty   = on-disk hash != recorded_hash
       IdenticalUpstream = on-disk hash == new_upstream_hash (already updated manually)
  7. Cargo plan: merge dependency requirements; detect major conflicts → fail.
  8. Return InstallPlan { creates, updates, skips, conflicts, cargo_edits, warnings }.
  9. Apply only after confirm; writes are atomic (temp + rename); update lock last.
```

**Kernel compatibility:** registry items declare public-API floor. If consumer pin is older, fail with “bump kernel rev” rather than copying code that won’t compile.

---

## 6. Conflict behavior (never blind overwrite)

### 6.1 Three-way file state

| Local disk | vs recorded | vs new upstream | Action on `update` |
|------------|-------------|-----------------|--------------------|
| == recorded | == upstream | == new | No-op |
| == recorded | == upstream | ≠ new | **Clean upgrade** (safe replace) |
| ≠ recorded | (any) | (any) | **Dirty** — skip; show `diff`; require `--force-dirty` + `.bak` |
| == new upstream | any | == new | Adopt as clean; refresh lock hashes |
| missing | — | — | Re-install from upstream |
| == recorded | ≠ upstream (tamper?) | | Doctor error: lock/registry mismatch |

### 6.2 `add` when file exists

| Condition | Behavior |
|-----------|----------|
| Target missing | Create |
| On-disk hash == item hash | Adopt; refresh lock |
| On-disk hash ≠ item hash | **Conflict** — refuse; print diff; no write |
| `--force-dirty` | Write `.bak.<timestamp>` then replace |

### 6.3 Interactive updates

`termrock update` default is **plan-only** listing:

```
CLEAN     termrock/tool-card  0.12.0 → 0.12.1
DIRTY     termrock/agent-workbench  (local edits; skipped)
BLOCKED   termrock/foo  needs kernel >= 0.13
```

`--apply` applies only **CLEAN** rows unless `--force-dirty` is set per-id.

### 6.4 Migrations

- Item-level `migrations[]` docs copy into `docs/termrock-migrations/` or print URL.
- `termrock migrate` can run **declarative** renames only (from → to paths in migration JSON), still with dirty checks.
- Kernel API breaks stay in TermRock `migrations/00xx-*.md` (crate); registry items reference kernel migration ids when needed.

---

## 7. Example registry items

### 7.1 Component: `termrock/tool-card`

```json
{
  "schema": 1,
  "namespace": "termrock",
  "name": "tool-card",
  "type": "component",
  "version": "0.12.0",
  "title": "Tool Card",
  "license": "Apache-2.0",
  "kernel": { "crate": "termrock", "min_version": "0.12.0", "features": [] },
  "registry_dependencies": [],
  "files": [
    {
      "source": "tool_card.rs",
      "target": "src/ui/components/tool_card.rs",
      "role": "primary",
      "hash": "sha256-EXAMPLE"
    }
  ],
  "module": {
    "exports": ["ToolCard", "ToolCardState", "ToolStatus"]
  },
  "capabilities": {
    "color": ["truecolor", "ansi256", "mono"],
    "glyphs": ["unicode", "ascii"],
    "min_width": 24,
    "min_height": 3
  },
  "docs": {
    "lookbook_story_ids": ["tool-card/status"]
  }
}
```

### 7.2 Theme: `termrock/phosphor-obsidian`

```json
{
  "schema": 1,
  "namespace": "termrock",
  "name": "phosphor-obsidian",
  "type": "theme",
  "version": "0.12.0",
  "kernel": { "crate": "termrock", "min_version": "0.12.0" },
  "files": [
    {
      "source": "theme.rs",
      "target": "src/ui/themes/phosphor_obsidian.rs",
      "role": "primary",
      "hash": "sha256-EXAMPLE"
    }
  ],
  "module": { "exports": ["phosphor_obsidian_theme", "PHOSPHOR_OBSIDIAN_TOKENS"] }
}
```

### 7.3 Keymap: `termrock/agent-default`

```json
{
  "schema": 1,
  "namespace": "termrock",
  "name": "agent-default",
  "type": "keymap",
  "version": "0.12.0",
  "kernel": { "crate": "termrock", "min_version": "0.12.0" },
  "files": [
    {
      "source": "keymap.rs",
      "target": "src/ui/keymaps/agent_default.rs",
      "role": "primary",
      "hash": "sha256-EXAMPLE"
    }
  ],
  "module": { "exports": ["agent_default_keymap"] }
}
```

### 7.4 Block: `termrock/agent-workbench`

Depends on `tool-card`, `prompt-composer`; multi-file; stories + fixtures optional.

### 7.5 Template: `termrock/app-minimal`

```json
{
  "type": "template",
  "name": "app-minimal",
  "files": [
    { "source": "main.rs", "target": "src/main.rs", "role": "primary" },
    { "source": "Cargo.toml.snippet", "target": "Cargo.toml", "role": "merge-cargo" }
  ]
}
```

`merge-cargo` only merges declared tables via structured edit — never full-file overwrite of existing Cargo.toml without plan.

---

## 8. Example installed project structure

```text
my-agent/
├── Cargo.toml
├── Cargo.lock
├── termrock.toml
├── termrock.lock
├── src/
│   ├── main.rs
│   └── ui/
│       ├── mod.rs                 # user-maintained exports
│       ├── components/
│       │   ├── mod.rs
│       │   ├── tool_card.rs       # installed (owned)
│       │   └── prompt_composer.rs
│       ├── blocks/
│       │   ├── mod.rs
│       │   ├── agent_workbench.rs
│       │   └── agent_workbench_layout.rs
│       ├── themes/
│       │   └── phosphor_obsidian.rs
│       ├── keymaps/
│       │   └── agent_default.rs
│       └── stories/
│           └── agent_workbench.rs
└── tests/
    └── ui_fixtures/
        └── agent_workbench_session.json
```

Installed files carry a short header comment (non-binding):

```rust
// @termrock-item termrock/tool-card@0.12.0
// @termrock-hash sha256-…
// This file is yours. Edit freely. Use `termrock diff` / `update` for upstream.
```

Headers are **not** the security boundary; lockfile hashes are.

---

## 9. Security model

| Threat | Mitigation |
|--------|------------|
| Path traversal (`../`, abs paths) | Normalize to relative UTF-8; must stay under `ui_root` / allowlist |
| Symlink escape | Refuse symlink targets on install; doctor detects new symlinks |
| Silent overwrite | Dirty detection via hashes; confirm; `--force-dirty` + backup |
| Malicious registry code execution | **No** install scripts, no `build.rs` from registry, no `include!` of remote |
| Supply-chain index swap | Optional `index_digest` pin; lock item_digest; `--offline` |
| Oversized payload | Max file count/size per item (defaults: 64 files, 1 MiB each) |
| Unicode / homograph names | Restrict charset; NFC normalize display names only |
| Partial install | Temp dir + rename; lock update last; crash ⇒ originals intact |
| Secret exfiltration in JSON logs | Default JSON omits file bodies; paths relative |
| License laundering | Require SPDX on every item; doctor fails unknown licenses vs policy |

**Network:** only HTTPS (or path/git with explicit config). No redirects to unlisted hosts when `trust = pinned`.

**Signing (phase 2):** optional minisign/sigstore over `item_digest`; not required for offline path registries.

---

## 10. Licensing and provenance model

1. Every item declares SPDX `license` (default Apache-2.0 for official).  
2. `provenance.origin` + `revision` + source path required for official items.  
3. Install copies `NOTICE` fragments into `src/ui/THIRD_PARTY_TERMROCK.md` append-only section per item.  
4. Consumer apps remain responsible for **their** final license compliance.  
5. Private registries may use proprietary licenses; `termrock doctor` warns if license not in allowlist:

```toml
[policy]
allowed_licenses = ["Apache-2.0", "MIT"]
```

6. Kernel crate provenance stays in repo `provenance.toml` / crate NOTICE — separate from registry item provenance.

---

## 11. Private registries & namespaces

```toml
[[registries]]
name = "acme"
namespace = "acme"
url = "https://registry.internal.acme/termrock/index.json"
priority = 50
trust = "pinned"
index_digest = "sha256-…"
# headers via env, not file:
# TERMROCK_REGISTRY_ACME_TOKEN
```

Resolution: match `namespace` of requested id → registry list filtered by namespace → highest `priority`.  
Publishing private items is out of band (company CI uploads index + blobs).

---

## 12. Theme / keymap packages / templates / stories / fixtures

| Kind | Contract |
|------|----------|
| Theme | Exports `Theme` or `DesignTokens` builder; no process I/O |
| Keymap | Exports `Keymap` / intent tables; pure data + constructors |
| Template | Scaffold only via `init`/`add template`; merge rules explicit |
| Story | Compiles against installed components; optional lookbook id map |
| Fixture | JSON/text only; size-bounded; used by `termrock story` / app tests |

---

## 13. Local modification detection

```
status(file):
  disk = sha256(read(file))
  if missing → Missing
  if disk == lock.recorded_hash:
       if disk == lock.upstream_hash → Clean
       else → LockDrift (doctor error)
  if disk == resolved_upstream_hash → AheadOrManualSync
  if disk != lock.recorded_hash → Dirty
```

`termrock doctor` prints a table; CI can `termrock doctor --strict` (exit 5 on Dirty if policy requires tracking).

---

## 14. Upstream comparison

```
termrock diff termrock/tool-card
```

Shows:

1. Installed version + digest  
2. Selected upstream version + digest  
3. Per-file: clean / dirty / changed-upstream  
4. Optional unified diff: local vs installed-upstream; installed-upstream vs new-upstream  

Never writes.

---

## 15. Initial implementation plan

Aligned with plan **047** (spike) then expand.

### Phase 0 — Approval & fixtures (required)

- Maintainer approval for `termrock-cli` workspace member.  
- Threat-model tests listed in plan 047 Step 1.  
- Design freeze: this document + schema version `1`.

### Phase 1 — Offline spike (plan 047)

1. Crate `termrock-cli` + library `termrock-registry` (pure resolve/plan).  
2. Schema parse/canonicalize/digest.  
3. Local filesystem registry under `registry/fixtures/`.  
4. Commands: `init`, `add`, `diff`, `doctor` (subset).  
5. Two items: tiny component + one block.  
6. Atomic install; dirty refuse; golden CLI tests.  
7. Migration doc when public (next free number at ship time).

### Phase 2 — Update & lock

- `termrock.lock` full three-way.  
- `update --dry-run` / `--apply`.  
- `remove`, `search`, `view`.

### Phase 3 — Official registry tree

- `registry/termrock/**` in monorepo or separate `termrock-registry` repo.  
- Publish pipeline: hash files, generate index, CI gate compile of items against pinned kernel.  
- HTTPS index hosting.

### Phase 4 — Private registries & themes/keymaps

- Multi-registry priority.  
- Theme/keymap item types.  
- `story` command integration with lookbook.

### Phase 5 — Hardening

- Index digest pins, optional signatures.  
- Declarative migrate.  
- Template merge-cargo.  
- Capability doctor vs live terminal.

**Explicit non-goals until Phase 3+:** marketplace UI, auto-merge of dirty files, replacing crates.io kernel distribution.

---

## 16. Tests

### 16.1 Unit (registry library)

| Suite | Cases |
|-------|-------|
| Schema | accept v1; reject unknown schema; reject bad names |
| Digest | canonical JSON stable; file hash LF policy |
| Resolve | diamond deps; cycle fail; version intersection |
| Paths | `..`, absolute, null byte, symlink, outside ui_root |
| Plan | create/adopt/conflict/dirty matrix |
| Kernel | min_version fail/pass |

### 16.2 Filesystem integration

| Suite | Cases |
|-------|-------|
| Atomicity | kill mid-write leaves original |
| Backup | `--force-dirty` writes `.bak` then new content |
| Lock | lock only updates after all files committed |
| Offline | path registry works without network |

### 16.3 CLI golden

| Command | Snapshot |
|---------|----------|
| `add --dry-run` | plan text + JSON |
| `diff` dirty | exit 5, hunk headers stable |
| `doctor` clean | exit 0 |
| `update` mixed | CLEAN/DIRTY table |

### 16.4 Compile fixtures

Temp Cargo project + installed block must `cargo test` against pinned kernel rev.

### 16.5 Security regression

Every malicious path fixture from plan 047 Step 1 must remain red (refuse).

---

## 17. Mapping to shadcn concepts

| shadcn | TermRock |
|--------|----------|
| `components.json` | `termrock.toml` |
| Registry JSON | `registry.json` + `item.json` |
| `npx shadcn add` | `termrock add` |
| Diff / update (community) | first-class `diff` / `update` with 3-way hashes |
| `ui/` owned source | `src/ui/**` owned source |
| radix / tailwind deps | `termrock` kernel crate + Cargo deps |
| blocks | `type = "block"` |
| themes | `type = "theme"` |

---

## 18. Decision summary

1. **Kernel crate remains the distribution unit for interaction truth.**  
2. **Opinionated UI is source-owned** via registry install.  
3. **Three-way hash comparison** is mandatory for updates.  
4. **Never silently overwrite** user-owned code.  
5. **No code execution** from registry content.  
6. **Namespaces + private registries** are first-class.  
7. **Plan 047** is the first executable slice; this doc is the full target architecture.

---

## 19. Open questions (resolve at Phase 0)

1. Official registry co-located in monorepo vs separate `termrock-registry` repo?  
2. crates.io publish of `termrock-cli` timing vs git-only?  
3. Whether leaf widgets eventually dual-ship (crate re-export **and** registry skin)? Default: **no dual paint bodies** — skins only in registry.  
4. Merge driver for dirty updates (manual only vs optional 3-way merge tool)? Default: **manual only**.
