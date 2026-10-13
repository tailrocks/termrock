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

Always optimize for the best current API, domain model, module boundary, and
architecture. Compatibility never blocks a better design: AI-assisted consumers
can migrate quickly, while compatibility constraints permanently weaken the
shared foundation. Freely rename, remove, restructure, or replace public APIs
and concepts. Prefer one coherent breaking redesign over deprecated aliases,
parallel old/new implementations, compatibility facades, or local exceptions.
Evaluate changes against the architecture TermRock should have next, not the
shape it happened to have before.

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
