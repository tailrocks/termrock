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

## Forward-only design

Always optimize for the best current API, component model, and architecture.
Do not preserve an inferior design merely to reduce downstream migration work:
AI-assisted consumers can adapt quickly, while compatibility constraints make
the shared foundation progressively harder to improve. Freely rename, remove,
restructure, or replace public APIs and concepts when the resulting model is
clearer, more composable, more reusable, or more correct. Prefer a coherent
breaking redesign over deprecation layers, parallel old/new paths, or local
exceptions. Evaluate proposals against the architecture TermRock should have
next, not the shape it happened to have before.

## Breaking-change documentation

Backward compatibility must not constrain a better component model, but every
breaking or dramatic behavioral change must be understandable without reading
the implementation or commit history. Ship the change and its migration
documentation together on `main`:

- append one sequentially numbered file under `migrations/` that names the
  removed API or old concept, its replacement, and the downstream code or
  ownership change, then link it from the ordered `MIGRATING.md` index;
- use an old-to-new table when more than one symbol, namespace, state owner, or
  behavior changes;
- include short before/after Rust examples when a type signature or composition
  pattern changes and prose alone is ambiguous;
- state explicitly when consumers must delete a local implementation, move
  state into TermRock, or retain product-owned state/effects;
- update the component catalog, generated API inventory, contract docs, and
  relevant guide pages so they describe only the new design.

Never rewrite an older migration to represent a newer transition. Do not add
deprecated aliases, compatibility facades, or migration shims. The ordered
migration documents explain the transition; the library exposes the new model
only. A breaking change is incomplete until its new migration file and index
entry are committed.

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
