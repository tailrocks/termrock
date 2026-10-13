# TermRock contributor rules

## North star (non-negotiable)

This is the goal. This is the target. This is the view. This is the direction.
Every widget, component, API, test, story, migration, and design decision is
judged against it.

**TermRock is the de facto base layer for modern Rust TUIs.** Anyone building a
new high-quality terminal application in Rust should reach for TermRock on top
of [Ratatui](https://ratatui.rs/) and feel: *this is incredible — high-class,
high-quality, the standard.* We optimize for that wow moment of clarity,
beauty, and power — not for legacy shape, vendor lock-in comfort, or
compatibility theater.

### Stack law: Ratatui first; crossterm as session adapter

1. **Everything is built on Ratatui.** Paint, layout cells, `Buffer`/`Frame`,
   and widget composition are Ratatui-native. Do not invent a parallel retained
   DOM, a competing render graph, or a framework that *replaces* Ratatui. Extend
   Ratatui; do not bypass it.
2. **crossterm remains the preferred terminal I/O / session adapter** while it
   stays the best practical choice in the Ratatui ecosystem (`crossterm`
   feature: events, backend, scoped session lifecycle). Keep event vocabulary
   backend-neutral (`input::KeyEvent`, …) so the kernel is not hard-wired to one
   backend crate, but default production path and examples use crossterm unless
   a *measured* better adapter replaces it. Do not add a second first-class
   backend without evidence and a deliberate cutover.
3. **Interaction kernel on top of paint** — design system, intents, focus,
   overlays, semantic scene, capability ladder — is TermRock's job. Ratatui
   stays the paint engine; TermRock owns the product-grade contracts.

### Quality bar: simple *and* advanced (shadcn-class)

Inspired by [shadcn/ui](https://ui.shadcn.com/docs) (open, inspectable,
strong defaults, composable) — adapted to terminal physics, not copied as a
React API:

- **Simple** by default: clear constructors, one paint authority, one focus
  authority, semantic intents, readable recipes.
- **Advanced** when needed: virtualization, overlays, agent trust surfaces,
  capability/quantize ladders, Studio inspection, full theming — without
  forking the simple path into a second API.
- **Modern APIs only:** prefer the best current Rust/Ratatui/terminal approach.
  When an older TermRock shape blocks a better one, **rewrite**. Full
  ground-up redesigns of widgets, modules, or the public surface are always
  allowed and often required.

### Breaking changes are free; excellence is not

- Backward compatibility is **never** a design input. Cost of consumer
  migration, "we already shipped this shape," sunk effort, and hypothetical
  marketplace friction **do not** block a better design.
- Pin exact Git revisions; migrate forward via numbered `migrations/` +
  `MIGRATING.md`. Prefer one coherent break over aliases, dual paths, or
  compatibility facades.
- The only permanent constraint is quality: product-neutral, composable,
  accessible-enough terminal UX, and the north star above.

If a change makes TermRock more modern, clearer, more capable, or closer to
"best in class on Ratatui" and requires a rewrite — **do the rewrite**.

## Product direction

TermRock is the ecosystem UI capability layer for building terminal interfaces
quickly. shadcn/ui and its [open repository](https://github.com/shadcn-ui/ui)
are design references, not an API template: terminal interaction, Ratatui,
accessibility, and Rust ownership constraints determine TermRock's APIs.

Assume a visual or interaction capability belongs **in the TermRock repo**
unless it is provably specific to a single consumer product domain. That does
**not** mean every recipe is a default widget: see **Building block vs example
composite** below. TermRock owns reusable rendering, layout, styles and
semantic theme roles, focus and navigation behavior, hit geometry,
narrow-terminal behavior, Unicode safety, non-color cues, and domain-neutral
widget state. Consumers own domain state and wording, effects, process policy,
secrets, executor choice, and projections from product models into TermRock
building blocks (or copy example composites from `patterns` and adapt).

**Building blocks** must be composable, product-neutral, readable, and easy to
adapt. Give them strong defaults, stable identities where interaction needs
them, borrowed or projected data where practical, and focused override points
instead of consumer-specific modes. Do not add product-branded widgets,
consumer compatibility facades, or copied neutral rendering bodies under
`widgets`. When a **generic** capability is missing, extend `widgets` (or the
interaction kernel). When a **product-shaped recipe** is useful as a demo,
implement it under `patterns` by composing public building blocks—not as a
first-class default widget.

## Building block vs example composite (mandatory)

**Before adding, promoting, or substantially extending any UI surface**, every
agent and contributor **must** research and classify it. This is not optional
guidance—it is package-boundary law.

| Classification | Home | Meaning |
|----------------|------|---------|
| **Generic building block** | `termrock::widgets` (and kernel modules) | Product-neutral UI **part**: panel, input, button, list, table, dialog, form, chart, focus/chrome helper. Reusable without a product noun in the public model. |
| **Example composite** | `termrock::patterns` only | Multi-widget **recipe** or product-noun assembly that shows how to use building blocks (Connection Manager, AuthEntry/login, workbench, dashboard, session picker as inventory manager, …). |

### Decision checklist (run every time)

1. **Name & API:** Does the public type/API encode a product domain (connection
   inventory, login gate, git workbench, ops dashboard state, …)? → **example
   composite**.
2. **Composition:** Is the surface mainly assembling other public widgets
   (panel + inputs + list + dialog) with host-owned domain data? → **example
   composite**.
3. **Reuse:** Would an unrelated TUI (editor, game, cloud CLI) want this as a
   primitive without rewriting product models? If yes and the API is neutral →
   **building block**. If only “apps like ours” want it → **example**.
4. **Model-only types:** Thin identity/status structs shared by a primitive and
   a recipe (e.g. queue-item identity for a composer) may live under `widgets`
   so **widgets never depend on `patterns`**. Full management UIs still go to
   `patterns`.
5. **Placement:** Implement building blocks in `crates/termrock/src/widgets/`.
   Implement composites only in `crates/termrock/src/patterns/` (or a dedicated
   examples crate if introduced later). **Never** export a product composite as
   a first-class `termrock::widgets` type.
6. **Dependencies:** `patterns` may `use termrock::widgets`. **`widgets` must
   not** `use crate::patterns` (doc links OK). No dual-path facades or
   deprecated aliases to keep a composite on the widgets path.
7. **Catalog / lookbook:** Registry primary file + provenance for composites
   point at `patterns/…`. Lookbook imports blocks from `widgets`, composites
   from `patterns`.
8. **Breaking moves:** Document with sequential `migrations/` + `MIGRATING.md`.

### Positive / negative examples

| Building block (`widgets`) | Example composite (`patterns`) |
|----------------------------|--------------------------------|
| `Panel`, `TextInput`, `PasswordInput`, `Button`, `List`, `DataTable` | `ConnectionManager` (list + panel + password + outcomes) |
| `Checkbox`, `Form`, `Dialog`, `Chart` / `Gauge` | `AuthEntry` / login-style gate (panel + identity + secrets + actions) |
| `PermissionPrompt` (neutral trust chrome) | Agent/git/DB **workbench** and **dashboard** application shells |
| `ModeRibbon` / `WorkbenchMode` row (caller labels) | Full agent workbench recipe with product panes |
| `PromptQueueItem` (neutral FIFO identity) | `PromptQueue` management UI recipe |
| `MetricTile` (one measured number) | `MetricsDashboard` / `ObservabilityDashboard` |
| `StatusStrip` (budgeted segment row) | `AgentStatusHeader` |
| `ConfirmPrompt` (neutral destructive confirm) | `SessionPicker` delete flow |
| `ChromeRow` (query / mode / notice row) | A pane's own filter and rename modes |

Full standard: [`docs/design/building-block-vs-example-composite.md`](docs/design/building-block-vs-example-composite.md).

**Examples contain zero raw paint.** A file under `patterns/` must not call
`Buffer::set_stringn` or `Buffer::cell_mut`: single rows go through
`DesignSystem::paint_row`, and everything else is a widget. If a recipe cannot
be expressed that way, the missing widget is the finding — report it, do not
hand-roll chrome in the example. Every rendering example opens with a
`//! Teaches:` header naming its assembly. `design_gate.rs::patterns_only_compose`
and `patterns_have_charter_docs` enforce both.

**When unsure:** default the **primitive pieces** into `widgets` and the
**assembled product surface** into `patterns`. Do not ship “half-product”
managers under `widgets` for convenience.

## Modern-first, pre-stable API

TermRock's goal is to become the best possible components and widgets library
for the Rust TUI experience — the terminal equivalent of what shadcn/ui
demonstrated for React and frontend development, and the **default foundation**
for new modern Ratatui apps. To get there, TermRock always follows modern
concepts, modern approaches, and cutting-edge technologies, ideas, and API
design. When choosing between preserving an existing shape and adopting a
better modern one, adopt the better one.

The public API is always allowed to change. TermRock is deliberately not
stable yet and provides no backward-compatibility guarantees of any kind.
Every consumer that relies on this library must accept this reality: pin an
exact revision and migrate forward. Backward compatibility is never a design
input — we always look forward, never back.

The current phosphor design concept is loved and stays the default: it is the
default theme and the design language Tailrocks projects ship with. That
default must never prevent the library from being product-neutral, fully
re-themable, and adoptable by projects with entirely different brands.

## Focus-visible panel hierarchy

Every panel and dialog uses the same single-line border geometry. Border weight
never communicates focus: the semantic theme does. The one container that owns
keyboard or scroll interaction uses `Role::BorderFocused`; visible inactive and
background containers use `Role::Border`. In the default phosphor theme those
roles are bright `PHOSPHOR_GREEN` and neutral `BORDER_GRAY`, respectively.

Do not use double-line, heavy, or mixed border glyphs for focus, and do not let
scrollbar glyphs redefine a panel's border. Consumers pass semantic emphasis;
`Panel` owns the glyph set and role selection. Components that present active
chrome without using `Panel` must preserve the same semantic distinction.

## Forward-only design

Always optimize for the best current API, domain model, module boundary, and
architecture. Compatibility never blocks a better design: AI-assisted consumers
can migrate quickly, while compatibility constraints permanently weaken the
shared foundation. Freely rename, remove, restructure, or replace public APIs
and concepts. Prefer one coherent breaking redesign over deprecated aliases,
parallel old/new implementations, compatibility facades, or local exceptions.
Evaluate changes against the architecture TermRock should have next, not the
shape it happened to have before.

## Cross-surface consistency

Widgets, APIs, patterns, tokens, intents, recipes, stories, tests, and docs
must stay consistent across the whole TermRock surface—not only the file being
edited.

When you improve or change something in one widget or component (anatomy,
state model, public API shape, intent routing, design-system / recipe paint,
focus or selection chrome, density, contraction, glyphs / ASCII / colorless
cues, outcomes, empty/loading/error, stories, tests, migration notes), always:

1. **Ask whether the same improvement applies** to peer widgets, composite
   surfaces, application blocks, lookbook stories, crate public exports,
   design SoTs, contract matrices, and documentation.
2. **Prefer one shared abstraction** (tokens, recipes, composed row parts,
   `UiIntent`, hit regions, density, glyph catalog) over a local one-off that
   leaves siblings on an older path.
3. **Verify before finishing the change:** search call sites and analogous
   components; update or explicitly schedule the cascade. Do not leave the
   library half-migrated when the better pattern is already proven in one
   place.
4. **Document the boundary** in the same commit when the change is public or
   behavioral (migration file when breaking; design/contract/story updates
   when the contract claims coverage).

Inconsistency is a defect. A “local win” that invents a second way to do the
same terminal job is incomplete work.

## Breaking-change documentation

Every breaking or dramatic public change must add the next sequential file
under `migrations/` and link it from `MIGRATING.md` in the same commit. The file
records the removed surface, canonical replacement, exact consumer edits,
before/after examples, removed concepts, ownership changes, and validation
commands. Documentation must let another agent migrate a pinned consumer
without reconstructing intent from the implementation, diff, or commit history.

Existing migration files are historical boundaries. Add a new numbered file
instead of rewriting an older migration for a later API. Migration documentation
coordinates forward adoption; it never authorizes deprecated aliases, duplicate
implementations, compatibility facades, or retention of an inferior path. A
breaking change is incomplete until its migration file and ordered index entry
are committed.

Every public widget must be represented by the catalog's generated API
inventory, contract matrix, documentation, story, and deterministic preview.
The current distribution unit is the Rust crate. Preserve open, inspectable
source and design APIs that can later support registry or copy-and-adapt
distribution without making that future mechanism a constraint on today's
crate.

All TermRock work happens directly on `main`. Do not create feature branches or
pull requests for TermRock changes. Commit each independently verified change
to `main` and push `main` immediately.

## Repository rules

All commits after the imported-history boundary in `provenance.toml` use
Conventional Commits, carry DCO sign-off, build independently, and are pushed
only when the documented bootstrap gate is green.
