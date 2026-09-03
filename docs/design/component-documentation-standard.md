# Canonical component documentation standard

| Field | Rule |
|---|---|
| Status | Binding for every generated catalog route |
| Public authority | `termrock-catalog scenarios --format json` |
| Editorial authority | One canonical MDX page per catalog entry |
| Generated projection | `docs/src/generated/catalog.ts` |
| Preview | One exact representative mounted Rust story |
| Duplicate routes | Forbidden; aliases never create pages |

## Identity and completeness law

The Rust inventory owns exact `PublicUiId`, render kind, family, docs kind,
slug, path, and representative story. Component docs are the inventory entries
whose docs kind is `component`. Public pattern structs use only their typed
pattern page. Composed patterns may have no linked `PublicUiId`.

Every component page must:

1. use the inventory slug and exact `catalogId`;
2. let the shared detail layout mount the inventory story;
3. name an existing exact Rust source definition;
4. keep purpose, tags, and search aliases editorial;
5. keep optional authored guidance additive, never a second identity or demo source;
6. rely on the shared layout for state/outcomes, compiled minimal implementation,
   mistakes, test recipe, and applications;
7. leave unproved quality axes `missing` or `partial` in the mandatory v2 contract.

Counts are always derived from the two typed Rust inventories. Prose and scripts
must not freeze a historical count.

## Required component frontmatter

```yaml
---
title: Button
description: Activation primitive with caller-owned effects.
catalogId: Button
source: crates/termrock/src/widgets/primitives.rs
tags:
  - action
  - widget
aliases: []
---
```

`component`, `demo`, `interaction`, `actions`, and `expectedOutcomes` are
forbidden. Those were generated duplicates of Rust inventory/story data.

Pattern pages use the same editorial fields plus `classification`, canonical
component `uses`, and non-catalog `supportingTypes`. Runtime hints and dimensions
come from the representative story; MDX does not copy them.

MDX may contain only frontmatter. The shared `ComponentDocLayout` and
`PatternDocLayout` own the required structure and bind `entry.story` from the
generated catalog. Optional MDX body content is editorial guidance below that
structure; embedded preview identity is forbidden.

## Page order

1. Exact live terminal preview.
2. Why and when, in a realistic task story.
3. State, behavior, and typed outcomes.
4. Minimal compiling story lookup and paint implementation.
5. Variants and composition.
6. API, tokens, accessibility, and tests.
7. FAQ, related APIs, and seen-in-application patterns.

Deep objections may use native `details`. Component pages never carry generated
coverage tables; the v2 contract is the machine evidence ledger.

## Preview law

The website and native catalog mount the same `CatalogSession` factory. Browser
input is normalized into backend-neutral events and sent to persistent Rust
state. Static poster JSON is lazy fallback paint and never defines behavior;
the poster gate rerenders every representative scenario and compares the full
dimensions and cell frame.

- Forward only supported hover, pointer, keyboard, paste, resize, focus, and time events.
- Overlays start from a trigger and restore focus on exit.
- Passive previews do not invent focus, cursors, or actions.
- Reduced motion suppresses decoration, never functional deadlines.

## Reverse usage and search

`SeenInApplications`, component gallery, pattern gallery, and site search read
only `docs/src/generated/catalog.ts`. Reverse usage comes from pattern `uses`.
Search ranks title, purpose, tags, and non-route aliases. Filters use URL `q`
and `family` parameters. No consumer imports an authored manifest directly.

## Enforcement

| Tool | Contract |
|---|---|
| `docs/scripts/generate-catalog.ts` | Strict Rust + stories + MDX + families + v2 join; deterministic output/meta |
| `docs/scripts/check-component-pages.ts` | Exact component page shape and no generated legacy fields |
| `docs/scripts/check-pattern-pages.ts` | Exact typed pattern page shape |
| `docs/scripts/check-contracts.ts` | Full v2 axes/evidence/lints and evidence targets |
| `docs/scripts/check-component-snippets.ts` | Generated Rust harness compiles every canonical demo-code story and rejects duplicate authored Rust fences |
| `docs/scripts/scaffold-component-page.ts` | Creates only an inventory-owned page and incomplete contract |

```sh
rtk bun --cwd docs run generate:catalog
rtk bun --cwd docs run check:catalog
rtk bun --cwd docs run check:contracts
rtk bun --cwd docs run check:components
rtk bun --cwd docs run check:patterns
rtk bun --cwd docs run check:snippets
```
