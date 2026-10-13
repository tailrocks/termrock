# TermRock component quality standard

| Field | Value |
|-------|-------|
| **Status** | Binding design SoT |
| **Rule** | A component is **not** complete because it compiles and paints |
| **Schema** | `docs/api/component-contract.schema.json` |
| **Contracts v1** | `docs/api/component-contracts.json` (six axes; catalog CI) |
| **Contracts v2** | `docs/api/component-contracts.v2.json` (optional until Q2) + example |
| **Related** | [`component-anatomy-spec.md`](./component-anatomy-spec.md), Studio stories, lookbook SVG gate, migration `0048` |

---

## 0. Completeness law

A public interactive component is **complete** only when:

1. Every **applicable** quality axis (§1) is either **`covered`** with machine-linkable evidence or explicitly **`not_applicable` / `caller_owned`** with a one-line reason.  
2. Evidence is **machine-linkable** (story id, test filter, snapshot path, bench name, recording path).  
3. Claiming `covered` without evidence **fails CI**.  
4. Design lints (§3) for that component are clean (or **waived** with ticket + expiry).  
5. Public inventory alignment holds: `public-api.txt` ↔ `COMPONENTS.md` ↔ stories ↔ contracts.

**Compile + render alone never suffice.**

Static/decorative components may mark interaction axes `not_applicable`.

---

## 1. Mandatory quality contracts

Each axis: **intent · pass criteria · evidence kinds · N/A when**.

Canonical axis ids (JSON / CI):

```
visual_states | keyboard | mouse | focus | disabled | loading | error | empty
overlay | escape | responsive | tiny_terminal
unicode | cjk | combining | emoji | ascii_fallback
no_color | color_ladder | streaming | large_data | resize | panic_safety
```

### 1.1 Visual states

| Pass |
|------|
| Default, hover (if pointer), focus-visible, active/pressed (if applicable), selected, disabled, loading, invalid/error, empty are distinct |
| Interactive states use **non-color** cues (glyph, border role, text prefix) — never color alone |
| Emphasis uses semantic `Role` tokens (no ad-hoc RGB for meaning) |

**Evidence:** buffer/SVG snapshots per state; Studio story matrix.  
**N/A:** pure decorations without state.

### 1.2 Keyboard access

| Pass |
|------|
| All primary actions reachable without mouse |
| Chords map through intents / keymap adapters — not product-hardcoded only paths |
| No dead focus traps; Tab/Esc specified |

**Evidence:** interaction tests; keyboard-only Studio scenario.  
**Lint:** `hardcoded_key_handling`, `mouse_without_keyboard` (inverse).  
**N/A:** non-interactive.

### 1.3 Mouse access

| Pass |
|------|
| Hit regions match painted controls |
| Every mouse action has keyboard equivalent **or** explicit N/A reason |
| Wheel ownership documented (capture vs bubble) for scroll surfaces |

**Evidence:** mouse tests; hit-region / scene snapshot.  
**Lint:** `mouse_without_keyboard`.

### 1.4 Focus entry and exit

| Pass |
|------|
| Entry paints focus-visible chrome (`Role::BorderFocused` / gutter / underline) |
| Exit restores prior chrome (opener restoration for overlays) |
| Focus order stable and inspectable |

**Evidence:** focus tests; semantic-scene snapshot.  
**Lint:** `invisible_keyboard_focus`, `focus_selection_indistinguishable`.  
**N/A:** never focusable.

### 1.5 Disabled-state behavior

| Pass |
|------|
| Disabled ignores activate keys and clicks → `Ignored` |
| Non-color disabled cue (`TextDisabled` / glyph) |
| Disabled items skip focus cycle |

**Evidence:** unit tests + snapshot.

### 1.6 Loading behavior

| Pass |
|------|
| Loading does not look like empty success |
| Motion respects `Motion::Off` (static glyph) |
| Partial load distinguishable when applicable (`LoadState::Partial`) |

**N/A:** no async/projection.

### 1.7 Error behavior

| Pass |
|------|
| Error visible with non-color cue (text + Danger role) |
| Retry only if retryable (typed outcome optional) |
| No panic on bad / partial data |

**Evidence:** error story; invalid-input tests.

### 1.8 Empty behavior

| Pass |
|------|
| Empty ≠ error ≠ loading |
| Guidance copy present |
| Primary action still reachable when applicable |

### 1.9 Overlay behavior

| Pass |
|------|
| Floating UI uses `OverlayStack` / scene layers (no ad-hoc z) |
| Placement flip/clamp/narrow documented |
| Backdrop + dismiss policy match kind (menu vs alert) |

**Lint:** `unpredictable_overlay_dismiss`.  
**N/A:** never an overlay host.

### 1.10 Escape behavior

| Pass |
|------|
| Esc closes **exactly one** conceptual layer |
| Trap vs dismissible explicit (`LayerDismissPolicy`) |
| Esc never silently grants / confirms destructive primary |

**Evidence:** overlay peel tests; permission-class tests.

### 1.11 Responsive behavior

| Pass |
|------|
| Contraction drops **secondary before primary** |
| Uses responsive kits / column priority / ComposedRow where applicable |
| Truncation is grapheme-safe |

**Lint:** `primary_clipped_before_secondary`.

### 1.12 Tiny-terminal behavior

| Pass |
|------|
| Usable at ≤20×5 **or** documented hide / LineMode |
| Essential labels survive |

**Evidence:** `tiny` / extreme narrow stories.

### 1.13 Unicode · CJK · combining · emoji

| Pass |
|------|
| Width via `display_cols` / grapheme boundaries |
| No mid-grapheme split |
| CJK wide cells lay out without corruption |
| Combining marks don’t break cursor/measure |
| Emoji / ZWJ sequences don’t destroy adjacent cells |

**Evidence:** dedicated stories or combined unicode matrix + fuzz.  
**Split axes** allow partial migration from a single “unicode covered” claim.

### 1.14 ASCII fallback

| Pass |
|------|
| Glyph sets provide ASCII substitutes |
| Status not only emoji / box-drawing that fails on limited terminals |

**Lint:** `missing_ascii_fallback`.  
**Evidence:** `ascii` story or glyph-set tests.

### 1.15 No-color · truecolor · reduced color

| Axis | Pass |
|------|------|
| **no_color** | Monochrome / `NO_COLOR` still conveys state via glyphs + text |
| **color_ladder** | Truecolor design target; 256/16 acceptable via quantization; no reliance on truecolor-only meaning |

**Lint:** `color_only_state`.

### 1.16 Streaming behavior

| Pass |
|------|
| Append/partial update O(visible) or budget-documented |
| Incomplete streams safe (no panic; stable prefixes where claimed) |
| Follow/selection semantics documented (e.g. wheel breaks follow) |

**N/A:** static content only.

### 1.17 Large-data performance

| Pass |
|------|
| Paint O(viewport), not O(dataset) |
| Virtual windows for large logical sets |
| Allocation budget documented when claimed |

**Evidence:** hot-path tests / benches (`data_view::bench`, `*_hot_path`).  
**N/A:** bounded tiny content.

### 1.18 Terminal resizing

| Pass |
|------|
| Reflow without panic on Resize events |
| Overlays reflow |
| Selection/focus clamp when out of range |

### 1.19 Panic and restoration safety

| Pass |
|------|
| Empty / zero rect early-return, no panic |
| Invalid cursor / paste / partial input safe |
| Adapter session restore path documented |
| Failed paint/outcomes leave state queryable |

**Lint:** `zero_area_panic`.  
**Evidence:** zero-area tests; fuzz; session lifecycle tests for adapters.

---

## 2. Testing layers

| Layer | What | Tooling |
|------:|------|---------|
| **L0** Unit | State machines, outcomes, clamp | `cargo test -p termrock` |
| **L1** Buffer snapshots | Cell grid + styles | TestBackend / Studio `.snap` |
| **L2** ANSI snapshots | Serialized styled lines | Optional export |
| **L3** SVG previews | Deterministic lookbook | `termrock-lookbook check` |
| **L4** Semantic-scene snapshots | Focus, layers, hits, actions | `InspectionFrame` digest |
| **L5** Interaction traces | Key/mouse → outcome log | Unit + Studio |
| **L6** Replay recordings | `.rec.json` scripts | Studio replay |
| **L7** PTY integration | Real terminal adapters | Feature-gated CI job |
| **L8** Property layout | Random sizes still valid geometry | proptest / quickcheck |
| **L9** Unicode fuzz | Random grapheme inserts | cargo-fuzz / quickcheck |
| **L10** Performance benches | Viewport paint vs N | criterion / custom hot-path |
| **L11** Allocation budgets | Max allocs per frame class | dhat / stats_alloc |
| **L12** Design linting | Static + heuristic rules | `docs/scripts/check-contracts.ts` + lints |

**Rule:** each `covered` axis links **≥1** evidence artifact from some layer.

---

## 3. Design lints

Machine-checkable rules. Severity: **error** (blocks complete) · **warn** (debt) · **waived** (ticket + expiry).

| Id | Detects | Severity |
|----|---------|----------|
| `color_only_state` | Selected/focus/error differ only by color, no glyph/text/border change | error |
| `invisible_keyboard_focus` | Focusable control with no focus-visible chrome | error |
| `primary_clipped_before_secondary` | Contraction drops primary while secondary remains | error |
| `interactive_without_semantic_role` | Hit/focusable area without control semantic role | error |
| `mouse_without_keyboard` | Mouse path without keyboard equivalent (unless N/A) | error |
| `unpredictable_overlay_dismiss` | Overlay without Esc/dismiss policy | error |
| `hardcoded_key_handling` | Product `KeyCode` chords outside keymap adapters | warn→error |
| `missing_ascii_fallback` | Unicode-only status without ASCII path | error |
| `focus_selection_indistinguishable` | Focus and selection identical chrome | error |
| `idle_animation_redraw` | Animation advances under `Motion::Off` / idle dirty spam | warn |
| `zero_area_panic` | Missing empty-rect guard | error |
| `missing_contract_evidence` | `covered` without evidence | error |
| `stale_contract_component` | Contract key not in public API | error |

### Implementation strategy

1. **Static (CI):** scan `crates/termrock/src/widgets/**/*.rs` for raw product-key patterns; allowlist `keymap`, `input`, adapters.  
2. **Contract validator:** schema + evidence path/story existence.  
3. **Snapshot heuristics (Q4):** buffer diff focus vs default for glyph/border delta.  
4. **Studio inspector:** live warnings during authoring.

---

## 4. Machine-readable contract schema

### 4.1 Locations

| Artifact | Path |
|----------|------|
| JSON Schema | `docs/api/component-contract.schema.json` |
| v1 catalog (live CI) | `docs/api/component-contracts.json` |
| v2 example | `docs/api/component-contracts.v2.example.json` |
| v2 catalog (target) | `docs/api/component-contracts.v2.json` |
| Validator | `docs/scripts/check-contracts.ts` |
| Catalog inventory | `docs/scripts/check-catalog.ts` |

### 4.2 Status vocabulary (v2)

| Status | Meaning |
|--------|---------|
| `covered` | Proven by linked evidence |
| `partial` | Some cases covered; remainder tracked (reason required) |
| `caller_owned` | Host/consumer must implement; hook documented (reason) |
| `not_applicable` | Axis does not apply (reason) |
| `missing` | Known gap — **not shippable as complete** |

### 4.3 Evidence object

```json
{
  "stories": ["list/selection", "list/narrow"],
  "tests": ["list::tests::keyboard_moves"],
  "snapshots": ["docs/public/component-previews/list-selection.svg"],
  "benches": ["list_paint_10k"],
  "recordings": ["stories/list/selection.rec.json"],
  "notes": "optional free text"
}
```

`covered` requires at least one of: stories · tests · snapshots · recordings · benches.

### 4.4 `complete` flag

`complete: true` only if:

- no axis is `missing`, and  
- no axis is `partial` **without** a waiver `{ ticket, expires }`, and  
- all design lints are `pass` or `waived`.

### 4.5 Example (excerpt)

See `docs/api/component-contracts.v2.example.json` (ApprovalCard, List). Full schema: `component-contract.schema.json`.

```json
{
  "schema": 2,
  "component": "List",
  "kind": "interactive",
  "complete": false,
  "axes": {
    "keyboard": {
      "status": "covered",
      "evidence": { "tests": ["list::tests::…"], "stories": ["list/selection"] }
    },
    "streaming": { "status": "not_applicable", "reason": "static projection per frame" },
    "ascii_fallback": { "status": "missing", "reason": "no ascii story yet" }
  },
  "lints": {
    "hardcoded_key_handling": "pass",
    "color_only_state": "pass"
  }
}
```

### 4.6 v1 → v2 mapping

| v1 | v2 |
|----|-----|
| keyboard | keyboard |
| mouse | mouse |
| focus | focus |
| nonColor | no_color (+ visual_states) |
| unicode | unicode (+ cjk / combining / emoji) |
| narrowTerminal | responsive + tiny_terminal |

During transition CI accepts **v1 catalog** (inventory) and optionally validates **v2** documents when present.

---

## 5. CI enforcement

### 5.1 Pipeline

```
┌─────────────────┐
│ public-api.txt  │──┐
│ COMPONENTS.md   │──┼──► check-catalog.ts (inventory + v1 six-axis)
│ lookbook stories│──┤
│ contracts v1    │──┘
└─────────────────┘
┌─────────────────┐
│ schema.json     │──┐
│ contracts v2*   │──┼──► check-contracts.ts (schema rules + evidence + lints)
│ example v2      │──┘
└─────────────────┘
┌─────────────────┐
│ lookbook SVG    │──► termrock-lookbook check
└─────────────────┘
┌─────────────────┐
│ unit / nextest  │──► axis evidence (tests)
│ hot_path / bench│──► large_data / streaming
└─────────────────┘
```

\* `component-contracts.v2.json` optional until Q2; example always validated.

### 5.2 Validator algorithm

```
public = widgets from public-api.txt
stories = lookbook list --format json
validate_schema_struct(v2_docs)

for each v2 component:
  for each REQUIRED_AXIS:
    cell.status ∈ vocabulary
    if covered: evidence non-empty; stories ⊆ lookbook; snapshots exist if listed
    if not_applicable|caller_owned|missing|partial: reason non-empty
  if complete: no missing/partial without waiver; lints not fail

if component-contracts.v2.json present:
  every public interactive widget has entry (or policy allowlist)
  no stale component names

design_lint_hardcoded_keys(widgets/) → warn/error by phase
```

### 5.3 Commands

```bash
# inventory + v1 contracts + story axis names
bun run docs/scripts/check-catalog.ts

# v2 schema rules + example + optional v2 catalog + static key lint
bun run docs/scripts/check-contracts.ts

# SVG drift
cargo run -p termrock-lookbook -- check --dir docs/public/component-previews

# mise
mise run contracts
mise run check   # includes docs-quality; gate includes lookbook check
```

### 5.4 GitHub Actions

`rust-required` / docs lane should run:

1. `bun run docs/scripts/check-catalog.ts`  
2. `bun run docs/scripts/check-contracts.ts`  
3. lookbook `check` (existing gate)  
4. unit tests that prove axis evidence  

`complete: true` enforcement for **new** widgets starts phase **Q5**.

---

## 6. Studio, registry, anatomy

| System | Role |
|--------|------|
| **Studio / lookbook stories** | Primary visual + interaction evidence |
| **Replay recordings** | Escape, overlay, stream proofs (L6) |
| **InspectionFrame** | Semantic-scene snapshots (L4) |
| **Registry items** | Ship contract + stories with block |
| **Anatomy spec (1–24)** | Human checklist; this standard is the **gate** |

---

## 7. Definition of Done (component PR)

- [ ] Public API + COMPONENTS inventory updated  
- [ ] Contract entry (v1 minimum; v2 preferred)  
- [ ] No un-evidenced `covered`; no silent `missing` on ship path without waiver  
- [ ] Stories for interaction + visual claims  
- [ ] SVG/buffer evidence generated where claimed  
- [ ] Unit tests: outcomes, empty, disabled, zero-area  
- [ ] Narrow/tiny or explicit N/A  
- [ ] Unicode / colorless or explicit N/A  
- [ ] Design lints pass  
- [ ] Migration file if public break  
- [ ] No color-only state  

---

## 8. Phased rollout

| Phase | Work | Status |
|-------|------|--------|
| **Q0** | Standard + JSON Schema + example | **Done** |
| **Q1** | `check-contracts.ts` dual validation + evidence paths | **Done** (this pass) |
| **Q2** | Expand components to v2 axes (partial/missing OK) | Next |
| **Q3** | Design lint hardcoded keys → error | Next |
| **Q4** | Snapshot heuristics color-only / focus-visible | Later |
| **Q5** | Require `complete: true` for new public widgets | Later |

---

## 9. Decision summary

1. **Compile ≠ complete.**  
2. **22 quality axes** are mandatory to classify (cover, N/A, or missing).  
3. **Evidence is mandatory** for `covered`.  
4. **12 testing layers** scale from unit to PTY to fuzz to benches.  
5. **Design lints** catch systemic UX failures (color-only focus, undismissible overlay, …).  
6. **CI enforces** inventory (v1) + schema/evidence (v2) + previews.  
7. **v1 six-axis map remains** until v2 migration finishes — dual-accept.

---

## 10. References

- `docs/api/component-contract.schema.json`  
- `docs/api/component-contracts.json`  
- `docs/api/component-contracts.v2.example.json`  
- `docs/scripts/check-catalog.ts`  
- `docs/scripts/check-contracts.ts`  
- `migrations/0048-v0.12.0-component-quality-standard.md`  
- `docs/design/component-anatomy-spec.md`  
- `docs/design/overlay-stack.md`, `responsive-layout.md`, `data-presentation.md`
