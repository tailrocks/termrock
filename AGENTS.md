# TermRock contributor rules

## Product direction

TermRock is the ecosystem UI capability layer for building terminal interfaces
quickly. It is inspired by the open-code, composable-component, strong-default,
and distribution ideas demonstrated by [shadcn/ui](https://ui.shadcn.com/docs)
and its [open repository](https://github.com/shadcn-ui/ui). Those projects are
design references, not an API template or source of truth: terminal interaction,
Ratatui, accessibility, and Rust ownership constraints determine TermRock's APIs.

Assume a visual or interaction pattern belongs in TermRock unless it is provably
specific to a consumer's product domain. TermRock owns reusable rendering,
layout, styles and semantic theme roles, focus and navigation behavior, hit
geometry, narrow-terminal behavior, Unicode safety, non-color cues, and
domain-neutral widget state. Consumers own domain state and wording, effects,
process policy, secrets, executor choice, and projections from product models
into TermRock components.

Components must be composable, product-neutral, readable, and easy to adapt.
Give them strong defaults, stable identities where interaction needs them,
borrowed or projected data where practical, and focused override points instead
of consumer-specific modes. Do not add product-branded widgets, consumer
compatibility facades, or copied neutral rendering bodies. When a capability is
missing, extend or refactor TermRock rather than implementing a local visual
substitute.

## Modern-first, pre-stable API

TermRock's goal is to become the best possible components and widgets library
for the Rust TUI experience — the terminal equivalent of what shadcn/ui
demonstrated for React and frontend development. To get there, TermRock always
follows modern concepts, modern approaches, and cutting-edge technologies,
ideas, and API design. When choosing between preserving an existing shape and
adopting a better modern one, adopt the better one.

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
