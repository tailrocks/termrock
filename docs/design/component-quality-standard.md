# TermRock component quality standard

**Status:** binding design SoT  
**Rule:** A component is **not** complete because it compiles and paints.  
**Related:** [`component-anatomy-spec.md`](./component-anatomy-spec.md), Studio stories, `docs/api/component-contracts.json`, lookbook SVG gate.

---

## 0. Completeness law

A public interactive component is **complete** only when:

1. Every **applicable** quality axis below is either **proven** by automated evidence or explicitly **`not_applicable`** with a one-line reason.  
2. Evidence is **machine-linkable** (story id, test name, snapshot path, bench name).  
3. Claiming `covered` without evidence **fails CI**.  
4. Design lints for that component are clean (or waived with ticket + expiry).

Static/decorative components may mark interaction axes `not_applicable`.

---

## 1. Mandatory quality contracts

Each axis has: **intent**, **pass criteria**, **evidence kinds**, **N/A when**.

### 1.1 Visual states

| Requirement | Pass criteria |
|-------------|---------------|
| Default, hover (if pointer), focus-visible, active/pressed (if applicable), selected, disabled, loading, invalid/error, empty | Distinct **non-color** cues where state is interactive; focus never color-only |
| Emphasis uses semantic `Role` tokens | No ad-hoc RGB for state meaning |

**Evidence:** buffer/SVG snapshots per state; Studio story matrix.  
**N/A:** pure paint decorations without state.

### 1.2 Keyboard access

| Requirement | Pass |
|-------------|------|
| All primary actions reachable without mouse | Documented chords → intents/keymap |
| No dead focus traps inside component | Tab/Esc behavior specified |
| No hardcoded product chords that bypass `Keymap`/`UiIntent` | Lint `hardcoded_key_handling` |

**Evidence:** interaction tests; keymap story; Studio keyboard-only scenario.  
**N/A:** non-interactive.

### 1.3 Mouse access

| Requirement | Pass |
|-------------|------|
| Click/activate hit regions match painted controls | `HitRegion` / scene registration |
| Every mouse action has keyboard equivalent **or** explicit N/A | Lint `mouse_without_keyboard` |
| Wheel ownership documented (capture vs bubble) | Overlay/scroll contracts |

**Evidence:** mouse tests; hit-region snapshot.  
**N/A:** non-interactive.

### 1.4 Focus entry and exit

| Requirement | Pass |
|-------------|------|
| Focus entry paints `Role::BorderFocused` / focus cue | Snapshot focus vs blur |
| Focus exit restores prior chrome | Opener restoration where overlays |
| Focus order stable and documented | Scene focus_order inspect |

**Evidence:** focus tests; semantic-scene snapshot.  
**N/A:** never focusable.

### 1.5 Disabled-state behavior

| Requirement | Pass |
|-------------|------|
| Disabled ignores activate keys/clicks | Tests return Ignored |
| Disabled non-color cue (`TextDisabled` / glyph) | Snapshot |
| Disabled items skip focus cycle | Focus order test |

### 1.6 Loading behavior

| Requirement | Pass |
|-------------|------|
| Loading does not look “empty success” | Distinct skeleton/spinner/pending |
| Motion respects `Motion::Off` | Static glyph when motion off |
| Partial load distinguishable (tables/grids) | `LoadState::Partial` story |

**N/A:** no async/projection.

### 1.7 Error behavior

| Requirement | Pass |
|-------------|------|
| Error state visible + non-color | Danger role + text/glyph |
| Retry affordance only if retryable | Outcome `Retry` optional |
| No panic on bad data | Fuzz / invalid input tests |

### 1.8 Empty behavior

| Requirement | Pass |
|-------------|------|
| Empty ≠ error ≠ loading | Dedicated empty copy/chrome |
| Primary action still reachable if applicable | Keyboard test |

### 1.9 Overlay behavior

| Requirement | Pass |
|-------------|------|
| Uses `OverlayStack` / scene layers when floating | No ad-hoc z invent |
| Placement flip/clamp/narrow documented | Placement tests |
| Backdrop policy matches kind | Policy table |

**N/A:** never an overlay.

### 1.10 Escape behavior

| Requirement | Pass |
|-------------|------|
| Esc closes **exactly one** conceptual layer | Overlay/scene tests |
| Trap vs dismissible explicit | `LayerDismissPolicy` |
| Esc never silently “approves” | Permission-class tests |

### 1.11 Responsive behavior

| Requirement | Pass |
|-------------|------|
| Contraction drops **secondary before primary** | Priority / ComposedRow / ColumnModel tests |
| Uses responsive kits where applicable | `ResponsiveSurface` / anatomy |
| No grapheme-unsafe truncate | Unicode tests |

### 1.12 Tiny-terminal behavior

| Requirement | Pass |
|-------------|------|
| Usable at ≤20×5 or documented hide | Story `tiny` / narrow |
| Essential labels survive | Contract primary survival |

### 1.13 Unicode / CJK / combining / emoji

| Requirement | Pass |
|-------------|------|
| Width via `display_cols` / grapheme boundaries | CJK + combining stories |
| No mid-grapheme split | Fuzz |
| Emoji / wide cells don’t corrupt layout | Snapshot |

### 1.14 ASCII fallback

| Requirement | Pass |
|-------------|------|
| `GlyphSet::Ascii` (or equivalent) substitutes | Story `ascii` |
| No mojibake boxes as sole status | Lint `missing_ascii_fallback` |

### 1.15 No-color / truecolor / reduced color

| Requirement | Pass |
|-------------|------|
| `ColorCapability::Mono` still conveys state | nonColor covered + story |
| Quantized themes acceptable | 256/16 stories optional matrix |
| State not color-alone | Lint `color_only_state` |

### 1.16 Streaming behavior

| Requirement | Pass |
|-------------|------|
| Append/partial update O(visible) or documented | Hot-path / stream story |
| Incomplete markdown/fences safe | Stream tests |
| Selection/follow semantics stable | Follow-break tests |

**N/A:** static content only.

### 1.17 Large-data performance

| Requirement | Pass |
|-------------|------|
| Paint O(viewport) not O(dataset) | Bench / hot-path |
| Virtual windows for large logical sets | data_view kits |
| Allocation budget documented if claimed | Bench alloc |

**N/A:** bounded tiny content.

### 1.18 Terminal resizing

| Requirement | Pass |
|-------------|------|
| Reflow without panic | Resize story / property test |
| Overlays reflow (`OverlayStack::reflow`) | Overlay tests |
| Selection remains valid or clamps | Unit test |

### 1.19 Panic and restoration safety

| Requirement | Pass |
|-------------|------|
| No panic on empty/zero rect | Zero-area tests |
| No panic on invalid cursor/paste | Fuzz |
| Session restore path documented for adapters | Runtime docs |
| Failed paint doesn’t corrupt consumer state machine | State still queryable after error outcomes |

---

## 2. Testing layers

| Layer | What | Tooling |
|-------|------|---------|
| **L0 Unit** | State machines, outcomes, clamp | `cargo test -p termrock` |
| **L1 Buffer snapshots** | Cell grid + role tags | Studio `.snap` / TestBackend |
| **L2 ANSI snapshots** | Serialized styled lines | Optional export |
| **L3 SVG previews** | Deterministic lookbook render | `termrock-lookbook check` |
| **L4 Semantic-scene snapshots** | Focus, layers, hits, actions | `InspectionFrame` digest |
| **L5 Interaction traces** | Key/mouse → outcome log | Unit + Studio |
| **L6 Replay recordings** | `.rec.json` scripts | Studio replay (design) |
| **L7 PTY integration** | Real terminal adapters | `#[cfg]` / CI job optional |
| **L8 Property layout** | Random sizes still contain rects | proptest-style |
| **L9 Unicode fuzz** | Random grapheme inserts | cargo-fuzz / quickcheck |
| **L10 Performance benches** | Viewport paint vs N | criterion / custom |
| **L11 Allocation budgets** | Max allocs per frame class | dhat / stats_alloc studio |
| **L12 Design linting** | Static/heuristic lints | `termrock-lint` / CI script |

Evidence links in the contract file must name **at least one** layer artifact per `covered` axis.

---

## 3. Design lints

Lints are **machine-checkable rules**. Severity: `error` (blocks complete), `warn` (debt).

| Id | Detects | Severity |
|----|---------|----------|
| `color_only_state` | Selected/focus/error differ only by fg/bg role without glyph/text/border role change | error |
| `invisible_keyboard_focus` | Focusable control with no focus-visible chrome in focus story | error |
| `primary_clipped_before_secondary` | Contraction drops primary label while secondary/meta still shown | error |
| `interactive_without_semantic_role` | Hit/focusable area registered without `SemanticRole::Control` (or Overlay) | error |
| `mouse_without_keyboard` | Mouse outcome path without keyboard path (unless N/A) | error |
| `unpredictable_overlay_dismiss` | Overlay open without Esc policy / outside policy declared | error |
| `hardcoded_key_handling` | Raw `KeyCode` match for product chords outside keymap adapter modules | warn→error |
| `missing_ascii_fallback` | Unicode-only status glyphs without ascii story or glyph set | error |
| `focus_selection_indistinguishable` | Focus and selection use identical chrome in snapshots | error |
| `idle_animation_redraw` | Spinner/animation advances when `Motion::Off` or no dirty state | warn |
| `zero_area_panic` | Missing empty-rect early return | error |
| `missing_contract_evidence` | Axis `covered` without evidence link | error |
| `stale_contract_component` | Contract key not in public API inventory | error |

### Lint implementation strategy

1. **Static (CI script / rust-analyzer-like):** scan for `KeyCode::` in `widgets/` excluding `keymap` adapters.  
2. **Snapshot-diff heuristics:** compare focus vs default buffers for glyph/border delta.  
3. **Contract validator:** schema + evidence existence.  
4. **Studio inspector:** optional live warnings.

---

## 4. Machine-readable contract schema

### 4.1 Schema location

- JSON Schema: `docs/api/component-contract.schema.json`  
- Instance catalog: evolve `docs/api/component-contracts.json` → **v2** documents  
- Legacy v1 (six axes) remains until all components migrate; CI accepts v1 **or** v2 during transition

### 4.2 Status vocabulary

| Status | Meaning |
|--------|---------|
| `covered` | Proven by linked evidence |
| `partial` | Some subcases covered; remainder tracked |
| `caller_owned` | Consumer must implement; component documents hook |
| `not_applicable` | Axis does not apply; reason required |
| `missing` | Known gap — **not shippable as complete** |

### 4.3 Evidence object

```json
{
  "stories": ["list/selection", "list/narrow"],
  "tests": ["list::tests::keyboard_moves"],
  "snapshots": ["docs/public/component-previews/list-selection.svg"],
  "benches": ["list_paint_10k"],
  "recordings": ["stories/list/selection.rec.json"],
  "notes": "optional"
}
```

### 4.4 Axis ids (canonical)

```
visual_states
keyboard
mouse
focus
disabled
loading
error
empty
overlay
escape
responsive
tiny_terminal
unicode
cjk
combining
emoji
ascii_fallback
no_color
color_ladder
streaming
large_data
resize
panic_safety
```

### 4.5 Example v2 contract (List excerpt)

```json
{
  "schema": 2,
  "component": "List",
  "module": "termrock::widgets::List",
  "kind": "interactive",
  "complete": false,
  "axes": {
    "keyboard": {
      "status": "covered",
      "evidence": { "tests": ["list::tests::keyboard_moves"], "stories": ["list/selection"] }
    },
    "mouse": {
      "status": "covered",
      "evidence": { "tests": ["list::tests::mouse_click"], "stories": ["list/selection"] }
    },
    "focus": {
      "status": "covered",
      "evidence": { "stories": ["list/selection"] }
    },
    "no_color": {
      "status": "covered",
      "evidence": { "stories": ["list/selection"], "notes": "gutter selection marker" }
    },
    "unicode": {
      "status": "covered",
      "evidence": { "stories": ["list/unicode"] }
    },
    "responsive": {
      "status": "covered",
      "evidence": { "stories": ["list/narrow"], "tests": ["composed_row::parts_drop"] }
    },
    "tiny_terminal": {
      "status": "partial",
      "evidence": { "stories": ["list/narrow"] },
      "reason": "explicit tiny 20x5 story pending"
    },
    "streaming": { "status": "not_applicable", "reason": "static projection per frame" },
    "large_data": {
      "status": "partial",
      "evidence": { "notes": "viewport paint; full virtualization via VirtualGrid/DataTable" }
    },
    "overlay": { "status": "not_applicable", "reason": "not an overlay host" },
    "escape": { "status": "caller_owned", "reason": "scene layer policy" },
    "ascii_fallback": { "status": "missing", "reason": "no ascii story yet" },
    "cjk": { "status": "partial", "evidence": { "stories": ["list/unicode"] } },
    "combining": { "status": "missing", "reason": "no combining story" },
    "emoji": { "status": "missing", "reason": "no emoji story" },
    "loading": { "status": "not_applicable", "reason": "no intrinsic async" },
    "error": { "status": "not_applicable", "reason": "no intrinsic error panel" },
    "empty": { "status": "covered", "evidence": { "stories": ["picker/empty"] } },
    "disabled": { "status": "covered", "evidence": { "tests": ["list::disabled_skips"] } },
    "visual_states": { "status": "covered", "evidence": { "stories": ["list/selection"] } },
    "color_ladder": { "status": "partial", "evidence": { "notes": "truecolor default; matrix pending" } },
    "resize": { "status": "partial", "evidence": { "notes": "relayout on area change" } },
    "panic_safety": { "status": "covered", "evidence": { "tests": ["widgets::tests::tiny_areas"] } }
  },
  "lints": {
    "hardcoded_key_handling": "pass",
    "color_only_state": "pass",
    "missing_ascii_fallback": "fail"
  }
}
```

`complete: true` only if no axis is `missing`/`partial` (unless project policy allows partial with waiver).

---

## 5. CI enforcement

### 5.1 Jobs (recommended)

```
check-contracts
  - validate JSON against component-contract.schema.json
  - every public Widget/StatefulWidget has a contract entry
  - no stale contract keys
  - every status=covered has ≥1 existing evidence path/test/story
  - every status=not_applicable has reason
  - complete flag consistency

check-catalog (existing)
  - public API ↔ COMPONENTS.md ↔ stories ↔ contracts

check-previews
  - termrock-lookbook check (SVG)

check-design-lints
  - hardcoded KeyCode in widgets (allowlist adapters)
  - optional snapshot heuristics

check / gate (existing)
  - unit + clippy + deny + …
```

### 5.2 Validator algorithm (pseudocode)

```
public = widgets from public-api.txt
contracts = load component-contracts.v2.json
schema_validate(contracts)

for c in public:
  assert c in contracts
  for axis in REQUIRED_AXES:
    cell = contracts[c].axes[axis]
    assert cell.status in VOCAB
    if cell.status == covered|partial:
      assert evidence non-empty
      assert all stories exist in lookbook list
      assert all snapshot files exist
      assert all tests match cargo test --list regex (optional)
    if cell.status == not_applicable|caller_owned|missing:
      assert reason non-empty
  if contracts[c].complete:
    assert no axis in {missing, partial} without waiver

for c in contracts:
  assert c in public  # no stale
```

### 5.3 Migration from v1 six-axis map

| v1 key | v2 axes |
|--------|---------|
| keyboard | keyboard |
| mouse | mouse |
| focus | focus |
| nonColor | no_color (+ visual_states partial) |
| unicode | unicode (+ cjk/combining/emoji split later) |
| narrowTerminal | responsive + tiny_terminal |

CI: if document has `"schema": 2` use v2 rules; else v1 rules (current `check-catalog.ts`).

### 5.4 mise / GitHub Actions sketch

```toml
# mise.toml
[tasks.contracts]
run = "bun run docs/scripts/check-contracts.ts"

[tasks.gate]
run = "mise run check && mise run contracts && mise run docs-quality && …"
```

```yaml
# .github/workflows/ci.yml (add step)
- run: mise run contracts
```

---

## 6. Relationship to Studio & registry

| System | Role in quality |
|--------|-----------------|
| **Studio stories** | Primary evidence for visual/interaction axes |
| **Replay recordings** | Escape, overlay, streaming proofs |
| **InspectionFrame** | Semantic-scene snapshots |
| **Registry items** | Must ship contract + stories with block |
| **Anatomy spec** | Human 24-point; quality standard is gate |

---

## 7. Definition of Done (component PR checklist)

- [ ] Public API + COMPONENTS inventory  
- [ ] Contract v2 entry; no `missing` on ship path  
- [ ] Stories for every `covered` interaction/visual axis  
- [ ] SVG/buffer evidence generated  
- [ ] Unit tests for outcomes + empty/disabled  
- [ ] Narrow/tiny or explicit N/A  
- [ ] Unicode or explicit N/A  
- [ ] Design lints pass  
- [ ] Migration if breaking  
- [ ] No color-only state  

---

## 8. Phased rollout

| Phase | Work |
|-------|------|
| **Q0** | This standard + JSON Schema (shipped) |
| **Q1** | `check-contracts.ts` dual v1/v2; evidence path existence |
| **Q2** | Expand all components to v2 axes (many partial/missing OK) |
| **Q3** | Design lint: hardcoded keys + missing evidence = error |
| **Q4** | Snapshot heuristics for color-only / focus visible |
| **Q5** | Require `complete: true` for new public widgets |

---

## 9. Decision summary

1. **Compile ≠ complete.**  
2. **Axes are mandatory**; N/A needs reason.  
3. **Evidence is mandatory** for covered.  
4. **Lints catch systemic UX failures.**  
5. **CI enforces schema + inventory + evidence.**  
6. **v1 contracts remain** until v2 migration finishes.
