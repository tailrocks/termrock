# Responsive layout system

**Status:** design SoT + foundation implemented (`crates/termrock/src/layout/responsive.rs`, migration `0044`)  
**Rule:** Responsive TUI design is **not** “truncate every label.”  
**Law:** **Primary labels and primary actions survive longer than decorative or secondary information.**

---

## 1. Goals

| Goal | Meaning in a terminal |
|------|------------------------|
| Preserve task | User can still select, confirm, cancel, navigate |
| Progressive | Stages 1→8, not a single cliff at 80 cols |
| Declarative | Surfaces declare priority + budgets, not ad-hoc `if width < 40` |
| Shared ladder | All surfaces share `ContentPriority` + `ContractionStage` |
| Testable | Fixed width matrix: 160…20 |

Non-goals: pixel-perfect web reflow; multi-font sizing; automatic translation of long copy.

---

## 2. Content priority (survival)

| Tier | Role | Examples | Drop order |
|------|------|----------|------------|
| **Essential** | Primary label, primary action | row title, Submit, selected tab name | Last (always while surface shown) |
| **Important** | Key secondary | path, key column, secondary pane body | Before essential only in line-mode |
| **Optional** | Metadata | badge, shortcut, timestamp, tertiary column | Early under pressure |
| **Decorative** | Flourish | extra glyphs, double chrome | First |

Higher priority **outlives** lower priority. Never hide Essential while Optional remains.

---

## 3. Contraction progression (narrower ⇒ higher stage)

| # | Stage | What changes |
|---|-------|----------------|
| 1 | **Full** | Full anatomy, comfortable density |
| 2 | **Compact spacing** | Density → Compact; structure intact |
| 3 | **Shorten secondary** | Abbreviate non-primary text; keep optional when possible |
| 4 | **Hide optional meta** | Drop badges, shortcuts, tertiary columns |
| 5 | **Collapse secondary actions** | Keep primary actions only (overflow menu optional) |
| 6 | **Single-pane** | Multi-pane/multi-column → one region |
| 7 | **Drawer / overlay** | Docked secondary → drawer or overlay |
| 8 | **Line-mode** | Essential-only tiny terminal |

Global default width bands (`ContractionStage::from_width`):

| Width | Stage |
|-------|--------|
| ≥ 160 | Full |
| 121–159 | CompactSpacing |
| 101–120 | ShortenSecondary |
| 81–100 | HideOptionalMeta |
| 61–80 | CollapseSecondaryActions |
| 41–60 | SinglePane |
| 25–40 | DrawerOrOverlay |
| ≤ 24 | LineMode |

Surfaces **override** bands via `SurfaceResponsivePolicy`. Height ≤ 5 also forces LineMode.

---

## 4. What each surface declares

| Declaration | Type | Meaning |
|-------------|------|---------|
| Essential content | parts / columns | Must remain |
| Important content | parts / panes | Keep until line-mode |
| Optional metadata | badges, shortcuts | Drop mid-ladder |
| Secondary actions | buttons, toolbars | Collapse mid-ladder |
| Preferred size | `SizeBudget.preferred` | Comfort target |
| Minimum usable | `SizeBudget.min_usable` | Below → strategy change |
| Contraction strategies | stage thresholds | Per-surface policy |
| Compact anatomy | `AdaptiveAnatomy` | Flags after stage |
| Tiny fallback | LineMode / Hide | Essential-only or hide |
| Overflow | `OverflowBehavior` | Ellipsis / Clip / Scroll / Wrap / Hide |

---

## 5. Rust APIs

```rust
use termrock::{
    AdaptiveAnatomy, AnatomyPart, ContentPriority, ContractionStage,
    OverflowBehavior, ResponsiveSurface, SizeBudget, ViewportClass,
    WIDTH_LADDER, contract_parts, essential_survives,
};

// Width → stage (global)
let stage = ContractionStage::from_width(80);
assert_eq!(stage.suggested_density(), Density::Dashboard); // stage-dependent

// Surface-aware classification
let class = ResponsiveSurface::AppShell.classify(80, 24);
assert!(class.anatomy.essential);
assert!(!class.anatomy.multi_pane); // single-pane at 80 for app shell policy

// Form columns
let cols = ResponsiveSurface::Form.form_columns(width); // 2 → 1

// Part contraction (cell budget + anatomy flags)
let parts = [
    AnatomyPart::essential("title", 24, 4), // preferred, min
    AnatomyPart::important("path", 20),
    AnatomyPart::optional("badge", 6),
    AnatomyPart::decorative("flourish", 2),
];
let shown = contract_parts(&parts, available_width, class.anatomy);
// Essential retained; optional/decorative dropped by stage + budget

// Invariant helper
assert!(essential_survives(ResponsiveSurface::Table, 20));
```

### Core types

| Type | Role |
|------|------|
| `ContentPriority` | Essential / Important / Optional / Decorative |
| `ContractionStage` | Full … LineMode (ordered) |
| `AdaptiveAnatomy` | essential, important, optional_meta, secondary_actions, multi_pane, use_drawer, line_mode, density, overflow |
| `SizeBudget` | preferred / min_usable / max + `resolve(available)` |
| `OverflowBehavior` | Ellipsis \| Clip \| Scroll \| Wrap \| Hide |
| `AnatomyPart` | named part + priority + width budget |
| `ViewportClass` | width, height, stage, anatomy |
| `ResponsiveSurface` | 16 named surfaces with `policy()` |
| `SurfaceResponsivePolicy` | thresholds + budgets |
| `WIDTH_LADDER` | `[160, 120, 100, 80, 60, 40, 20]` |
| `contract_parts` | drop by priority then fit budget |
| `essential_survives` | matrix invariant |

### Integration with existing systems

| Existing | Role |
|----------|------|
| `Density` | Spacing from `anatomy.density` / stage |
| `ComposedRow::parts_for_width` | Row drop: shortcut → badge → secondary → leading → primary |
| `PanelSlots::for_width` | Title/subtitle/footer contraction |
| `Workspace` collapse_priority | Pane single-pane / drawer step |
| `OverlayStack` narrow | Drawer/overlay + fullscreen promotion |
| DataTable column `priority` | Column drop (higher survives) |

**Rule:** New width cutoffs go through `ResponsiveSurface` / `ViewportClass`, not private magic numbers.

---

## 6. Per-surface progressive behavior

Legend for each surface:

- **E** essential · **I** important · **O** optional · **A** secondary actions  
- Stages refer to the progression in §3

### 6.1 App shell

| | Content |
|--|---------|
| E | Main work region (transcript / table / editor) |
| I | One nav affordance (sidebar tab or rail glyph) |
| O | Extra panes, previews, tertiary chrome |
| A | Global toolbar extras |

| Width band (policy) | Behavior |
|---------------------|----------|
| Full (≥140) | Multi-pane workspace, comfortable density |
| Compact | Tighter pads |
| ≤100 | Hide chrome meta |
| ≤90 | Collapse secondary toolbar actions |
| ≤72 | **Single pane** — one main region |
| ≤48 | **Sidebar → drawer** / overlay host |
| ≤24 or h≤5 | **Line-mode** — main only, minimal status |

### 6.2 Sidebar

| | Content |
|--|---------|
| E | Selected / focused nav label |
| I | Tree/list of primary items |
| O | Counts, badges, section headers flourish |
| A | New / filter extras |

**Progression:** full tree → compact indent → hide badges → collapse filters → **drawer** under shell pressure → line-mode = current section title only.

### 6.3 Tabs

| | Content |
|--|---------|
| E | Active tab label (ellipsis ok) |
| I | Adjacent tabs |
| O | Tab badges / close icons |
| A | “+” / overflow menu |

**Progression:** all labels → compact → shorten → hide badges → collapse “+” → **scroll/overflow menu** for inactive tabs → line-mode = active name only. Overflow = **Scroll** (not truncate all tabs to one char blindly).

### 6.4 Table

| | Content |
|--|---------|
| E | Identity column + selection gutter |
| I | Key data columns |
| O | Secondary columns, trailing meta |
| A | Row action buttons |

**Progression:** all columns → compact cell pad → shorten secondary cells → drop optional columns → collapse row actions → line-mode = identity + selection cue. Align with composed-row: shortcut/badge before primary. Overflow = Ellipsis on cells.

### 6.5 Tree

| | Content |
|--|---------|
| E | Disclosure + primary label |
| I | Indent (reduced), status |
| O | Badge, shortcut, secondary path |
| A | Context actions |

**Progression:** full indent (density) → compact indent → hide badge/shortcut → reduce indent further → line-mode = disclosure + label. Never drop disclosure before primary.

### 6.6 DataTable

| | Content |
|--|---------|
| E | Pinned / identity columns (high `priority`) |
| I | Medium-priority data columns |
| O | Low-priority columns, toolbar meta |
| A | Bulk toolbar secondary |

**Progression:** all columns → compact → shorten headers → `contract_to_budget` drops lowest unpinned priority first → collapse bulk secondary → line-mode = pinned identity only. Toolbar: labels → icons → overflow.

### 6.7 Form

| | Content |
|--|---------|
| E | Focused field label + value + primary Submit |
| I | Other fields in viewport |
| O | Help text, optional sections |
| A | Secondary buttons (Reset) |

**Progression:** 2-column (`form_columns`) → compact → shorten help → hide help → collapse secondary buttons → **1-column** ≤48 → line-mode = current field only. Overflow = Wrap for long labels when height allows.

### 6.8 Dialog

| | Content |
|--|---------|
| E | Title + primary action |
| I | Body message / main control |
| O | Details table, footer hints |
| A | Secondary footer buttons |

**Progression:** full elevated dialog → compact inset → shorten body → hide details → collapse secondary footer → single column body → **drawer/fullscreen** via OverlayStack narrow → line-mode = title + primary. Outside-click trap unchanged.

### 6.9 Command palette

| | Content |
|--|---------|
| E | Query + selected result primary |
| I | Result list |
| O | Shortcuts, kind badges, footer hints |
| A | Category filters |

**Progression:** full center palette → compact → shorten meta → hide shortcuts/badges → collapse filters → narrow **fullscreen promote** → line-mode = query only. Uses OverlayStack CommandPalette policy.

### 6.10 Prompt composer

| | Content |
|--|---------|
| E | Editor + send (when enabled) |
| I | Mode indicator when relevant |
| O | Attachment chips, footer hints |
| A | Secondary mode toggles |

**Progression:** multi-line comfortable → compact height → shorten mode label → drop chips then hints → collapse secondary modes → min height 2–3 → line-mode = single-line input + send. Width shrinks labels before removing send.

### 6.11 Task rail

| | Content |
|--|---------|
| E | Selected task title + status glyph |
| I | Task list |
| O | Meta timestamps, counts |
| A | Extra task filters |

**Progression:** full rail → compact → shorten titles → hide meta → collapse filters → **drawer** when shell narrow → line-mode = current task one-liner.

### 6.12 Permission prompt

| | Content |
|--|---------|
| E | Risk title + **default decision** + confirm |
| I | Scope / target path |
| O | Provenance, audit meta |
| A | Secondary decision options (still reachable via arrows) |

**Progression:** full card → compact → shorten detail → hide provenance → collapse rarely used decisions into shorter set (Deny/Allow only) when tiny → **never** drop Deny for High risk. Line-mode = risk + Deny/Allow. Safety > aesthetics.

### 6.13 Plan review

| | Content |
|--|---------|
| E | Selected step title + Accept/Reject primary |
| I | Step list / detail body |
| O | Checkmarks meta, timestamps |
| A | Edit secondary |

**Progression:** list+detail multi-pane → compact → shorten → hide meta → collapse Edit → **single pane** (detail under list or list only) → drawer for list on narrow → line-mode = step title + Accept/Reject.

### 6.14 Diff review

| | Content |
|--|---------|
| E | Current hunk lines + markers `+/-` |
| I | Hunk headers, file name |
| O | Line numbers, stats |
| A | Stage/discard secondary |

**Progression:** side-by-side (wide) → compact → shorten headers → hide line numbers → collapse secondary actions → **unified single pane** ≤70 → line-mode = hunk header + few lines. Markers never dropped (colorless).

### 6.15 Log viewer

| | Content |
|--|---------|
| E | Visible log lines (scroll) |
| I | Follow indicator |
| O | Level glyphs density, timestamps if secondary |
| A | Filter chrome secondary |

**Progression:** full → compact → shorten timestamps → hide level chrome if optional → collapse filters → always **Scroll** overflow. Line-mode = last N essential lines. No multi-pane.

### 6.16 Status bar

| | Content |
|--|---------|
| E | Highest-priority slot (e.g. mode / errors) |
| I | Branch / path |
| O | Counters, decorative separators |
| A | Clickable low-priority slots |

**Progression:** all slots → compact → shorten → hide optional slots by priority → collapse interactive extras → line-mode = one essential slot. Overflow = **Hide** (drop slots), never ellipsis entire bar into noise.

---

## 7. Width test matrix

Canonical widths: **`WIDTH_LADDER = [160, 120, 100, 80, 60, 40, 20]`**.

### Global invariants (all surfaces, all ladder widths)

1. `anatomy.essential == true`  
2. Stage severity **non-decreasing** as width decreases  
3. Optional never outlives important (`ContentPriority` order)  
4. Line-mode ⇒ no secondary_actions, no multi_pane  
5. `contract_parts` retains essential primary labels  
6. `essential_survives(surface, width)` holds  

### Per-width expectations (AppShell example)

| Cols | Stage (approx) | Layout |
|------|----------------|--------|
| 160 | Full | Multi-pane workbench |
| 120 | Compact / shorten | Multi-pane, tighter |
| 100 | Hide meta | Toolbars quieter |
| 80 | Collapse actions / single-pane edge | Fewer actions |
| 60 | Single-pane | Main only |
| 40 | Drawer | Nav as drawer |
| 20 | Line-mode | Essential main strip |

### Automated tests

`layout::responsive` module (15+ tests) including:

- `width_ladder_matrix_all_surfaces`  
- `essential_survives_every_ladder_width_on_every_surface`  
- `contract_parts` keeps essential at 40/20  
- `status_bar_matrix_keeps_primary`  
- Form column collapse  

Widget tests: list/tree/table narrow, form responsive, progress narrow, data_view column contract.

---

## 8. Component rules

1. Declare priorities; do not invent ad-hoc width cutoffs.  
2. **Essential survives** until the surface is hidden.  
3. Drop **Decorative → Optional → Important** before Essential.  
4. Prefer hide/collapse over mid-grapheme truncate; use grapheme-safe ellipsis.  
5. Primary actions outlive secondary actions.  
6. Multi-pane → single-pane → drawer/overlay before deleting main content.  
7. Pair OverlayStack narrow with stage 7.  
8. Permission/safety: never remove safest decision to save cells.  
9. Diff/log: keep semantic markers (`+/-`, scroll).  
10. Cross-surface consistency (AGENTS.md): new contraction patterns roll to peers.  
11. Stories: at least narrow + tiny for interactive surfaces.  
12. Matrix: use `WIDTH_LADDER` in tests.

---

## 9. Implementation status

| Piece | Status |
|-------|--------|
| Priority / stage / anatomy / budgets / contract_parts | **Implemented** |
| 16 `ResponsiveSurface` policies | **Implemented** |
| WIDTH_LADDER matrix tests | **Implemented** |
| Form columns via policy | **Implemented** |
| ComposedRow / Panel / Workspace / Overlay narrow | **Aligned** |
| Every widget paint path querying `ResponsiveSurface` | **Partial** — migrate remaining ad-hoc cuts |
| Lookbook stories per ladder width | **Partial** — expand systematically |

---

## 10. Success criteria

1. At 20 cols, primary labels still identifiable (ellipsis ok).  
2. At 40 cols, primary actions still reachable.  
3. Optional badges gone before primary labels shrink to unusable.  
4. App shell becomes single-pane before deleting the main region.  
5. No widget invents a private 37-column special case without a policy reason.  
6. `WIDTH_LADDER` × `ResponsiveSurface::ALL` matrix stays green in CI.

---

## Related

- Design tokens / density: [`terminal-design-system.md`](./terminal-design-system.md)  
- Overlay narrow: [`overlay-stack.md`](./overlay-stack.md)  
- Visual calm under contraction: historical phosphor-obsidian notes; current grammar in [`DESIGN.md`](../../DESIGN.md)
