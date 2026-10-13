# Building block vs example composite

**Status:** mandatory SoT for TermRock package boundaries  
**Audience:** agents and contributors adding or promoting UI surfaces  
**Agent rule:** repository `Agents.md` / `AGENTS.md` (section *Building block vs example composite*)  
**Homes:** `termrock::widgets` (blocks) · `termrock::patterns` (example composites)

## Design-system framing

Industry design systems separate:

- **Component / primitive libraries** — reusable UI *parts* (buttons, inputs,
  panels, tables). Consumers compose them into products.
- **Pattern libraries** — higher-level *solutions* assembled from components
  (settings screens, data workbenches, auth gates). Patterns document
  *how to* use the parts for a job.

TermRock maps that split to Rust module trees:

| Design-system term | TermRock home | Public import |
|--------------------|---------------|---------------|
| Component / primitive | Building block | `termrock::widgets::…` (+ interaction/style/text kernel) |
| Pattern / recipe | Example composite | `termrock::patterns::…` |

This is **not** “patterns are second-class quality.” Composites must still be
high-class, tested, and documented—but they are **examples of assembly**, not
the default building-block catalog.

## Classification test

Answer in order. Stop at the first decisive yes.

1. **Product noun in the public model?**  
   Types named or shaped as connection inventory, login/signup gate, git/DB
   workbench, ops dashboard *application state*, session *manager*, integration
   *status board*, project *launcher*, etc. → **example composite**.

2. **Multi-widget recipe with host-owned domain data?**  
   Surface primarily routes focus between panel, list, form fields, dialogs,
   status bars and emits outcomes for the host to execute → **example
   composite**.

3. **Single-purpose terminal chrome with neutral API?**  
   One clear job (edit text, paint a list row, draw a gauge, show a permission
   prompt) with product-neutral identifiers and projected labels → **building
   block**.

4. **Shared model for a block and a recipe?**  
   Small identity/status types required so a building block can hold state
   without importing the recipe → **building block** (model only). The full
   management UI remains an **example composite**.

5. **Still ambiguous?**  
   Put primitives in `widgets`, the assembled surface in `patterns`. Prefer one
   clean break over dual export paths.

## Decision checklist (copy into PR / agent notes)

- [ ] Classified as **building block** or **example composite** (one only).
- [ ] Public API has **no product noun** if classified as building block.
- [ ] Building blocks live under `crates/termrock/src/widgets/` and export via
      `widgets/mod.rs`.
- [ ] Composites live under `crates/termrock/src/patterns/` and export via
      `patterns/mod.rs` only.
- [ ] Composite **composes** public `widgets` APIs (and kernel modules)—no
      private parallel paint stack inside widgets for the product job.
- [ ] **No** `use crate::patterns` from `widgets` (doc links allowed).
- [ ] Composite contains **zero raw buffer paint** (`set_stringn`, `cell_mut`).
      Single rows go through `DesignSystem::paint_row`; anything else is a
      widget. A composite that cannot be expressed through widgets has found a
      missing widget, which is the finding to report.
- [ ] Composite opens with a `//! Teaches:` header naming the assembly and the
      widgets it composes.
- [ ] Lookbook: blocks from `widgets`, composites from `patterns`.
- [ ] Registry catalog: primary file + provenance paths match the real home.
- [ ] Breaking moves: sequential `migrations/` + `MIGRATING.md` index.

## Concrete examples

### Building blocks (`widgets`)

| Surface | Why block |
|---------|-----------|
| `Panel`, `Card`, `Surface` | Generic chrome containers |
| `TextInput`, `PasswordInput`, `Checkbox`, `Button` | Form primitives |
| `List`, `DataTable`, `Tree`, `Picker` | Selection / data primitives |
| `Dialog`, `AlertDialog`, `Form` | Neutral dialog/form machinery |
| `Chart`, `Gauge`, `Sparkline`, `BarSeries` | Visualization primitives |
| `PermissionPrompt` | Domain-neutral trust chrome (host supplies request model) |
| `ModeRibbon` + `WorkbenchMode` | Neutral mode strip; labels projected by host |
| `PromptQueueItem` / `PromptQueueRef` / `PromptQueueStatus` | Neutral FIFO identity for composer + recipes |
| `three_pane_panels` / `main_end_panels` | Layout presets without product names |
| `MetricTile` / `MetricTileView` | One measured number; the dashboard around it is the product |
| `StatusStrip` | Priority-dropped segment row with one status hue and one accent |
| `ConfirmPrompt` | Neutral destructive confirmation (same precedent as `PermissionPrompt`) |
| `ChromeRow` | The query / mode / notice row a pane grows while a mode is active |
| `PanelTitleSpec` | `Name(scope)[count] /filter` title composition |

### Example composites (`patterns`)

| Surface | Why composite |
|---------|----------------|
| `ConnectionManager` | Connection inventory: list + detail + form + secrets + confirm |
| `AuthEntry` | Login/signup/email-only gate: panel + identity + passwords + terms |
| `*Workbench*` / `*Dashboard*` app state | Multi-pane product shells (agent, git, DB, observability, ops) |
| `SessionPicker`, `ApprovalQueue`, `PromptQueue` (UI) | Product management recipes |
| `QueryEditor` + `SchemaBrowser` + `ResultGrid` | DB workbench pieces as recipes |
| `example_agent_workbench_nav` / `example_database_nav` | Product-shaped seed data for demos |

## Anti-patterns

- Shipping a product manager under `widgets` “just for lookbook convenience.”
- Dual export (`widgets` + `patterns`) or deprecated aliases that keep the
  composite on the building-block path.
- Widgets importing `patterns` to share code (extract a **neutral model** or
  shared helper into `widgets` / kernel instead).
- Registry or catalog still pointing at a removed `widgets/…` path after a move.
- Treating “used by agents” as automatic entry into `widgets` without a
  product-neutral API.

## Relation to product direction

“Belongs in TermRock” means the capability is **in-repo and reusable**—either as
a building block or as a documented example composite. It does **not** mean
every recipe is a default widget. Package boundary law wins over convenience.

## Validation helpers

```bash
# Structural: known product surfaces must not appear as widgets pub-use exports
rtk bun run docs/scripts/check-building-block-boundary.ts

# Catalog paths must exist on disk
rtk cargo test -p termrock --lib registry
```

## History

- **0257** — Major product composites moved widgets → patterns  
- **0258** — Queue model in widgets; ops/resource state in patterns  
- **0259** — Neutral panel preset names; product example nav seeds in patterns  
- **0260** — Agent rule + this standard (documentation boundary)
