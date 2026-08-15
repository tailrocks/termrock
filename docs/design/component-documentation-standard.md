# Canonical component documentation standard

| Field | Rule |
|---|---|
| Status | Binding for every canonical component route |
| Route | `/docs/components/<slug>` |
| Source of truth | Checked-in MDX, never generated prose |
| Preview | One persistent Rust demo mounted by stable ID |
| Duplicate docs | Forbidden; there is no separate Component Handbook |

## Completeness law

A public widget is incomplete until the generated API inventory maps it to one
canonical page and one shared Lookbook demo. A page is complete only when:

1. Frontmatter declares `component`, `demo`, `interaction`, `actions`,
   `expectedOutcomes`, and an existing Rust `source` path.
2. It embeds exactly one matching `<TerminalPreview>`.
3. Its behavior recipe names only events accepted by that mounted demo.
4. Interactive demos expose current hints and a visible outcome; passive demos
   expose neither fake actions nor a fake cursor.
5. Usage uses public TermRock APIs and the snippet checks accept it.
6. State ownership separates widget state, host policy, and external effects.
7. Story IDs name deterministic visual coverage without presenting variants as
   runtime steps.

The current inventory contains 135 public `Widget`/`StatefulWidget`
implementations and 165 canonical component routes. Some routes document public
component types that are not themselves Ratatui widget implementations. Both
numbers are checked; neither may be substituted for the other.

## Required frontmatter

```yaml
---
title: ComponentName
description: 'Purpose-first sentence.'
component: ComponentName
demo: component-name/basic
interaction: selection-navigation
actions:
  - 'ArrowDown select'
expectedOutcomes:
  - 'Selection changes and the status reports the typed outcome.'
source: crates/termrock/src/widgets/component_name.rs
---
```

Valid interaction families are `passive-paint`, `activation`,
`selection-navigation`, `editor-form`, `disclosure-overlay`,
`scrolling-virtualization`, `drag-continuous-value`, and `timed-state`.
Use `passive-paint` when no input belongs to the component. Configuration and
variant changes are not runtime interaction.

## Required page order

1. Live terminal and one-sentence behavior recipe.
2. Usage with public APIs.
3. Interaction contract: keyboard, mouse, focus, non-color, Unicode, narrow,
   motion, and caret where applicable.
4. Stories, with one primary demo and clearly labeled visual variants.
5. Try it: exact current actions and observable result.
6. State and typed outcomes: widget-owned versus host-owned.
7. Common mistakes.
8. Deterministic test recipe.
9. Source and related material.
10. **Seen in applications** — where the component is actually composed.
11. Migrated deep guidance when the old Handbook held useful material.

### Seen in applications

The section is one line: `<SeenInApplications component="Name" />`. It reads
`buildingBlocks` in `docs/api/pattern-catalog.json`, which is the **single**
source for the reverse index — never a hand-written list, which would drift the
day a pattern changed what it composes. A component no pattern composes renders
"not yet composed in a shipped example", and that is a coverage signal about
the examples rather than about the component (plans/018 Step 1).

## Preview law

The website and native Lookbook mount the same `DemoSession` factory. Browser
input is normalized into backend-neutral TermRock events and sent to persistent
Rust state. The web host does not implement component behavior.

- Hover, click, typing, paste, key press/repeat/release, wheel, drag, focus,
  resize, and host-controlled time are forwarded only when supported.
- Dialogs, menus, drawers, and toasts begin from a trigger and appear or
  disappear because the Rust demo accepted an event.
- Editors paint their real widget caret. Passive previews never receive a
  synthetic caret or cursor.
- Timed demos use injected elapsed time and pause offscreen or under reduced
  motion.
- Static poster JSON is fallback paint only. It never defines behavior.

## Writing rules

- Explain why and when before listing methods.
- Name concrete actions and concrete outcomes. Avoid “click for states.”
- Outcomes are not effects: the demo may show `Activated(id)`; the consumer
  decides whether to navigate, execute, authenticate, or mutate external data.
- Prefer borrowed/projection examples over owned product models.
- State contraction order and non-color cues rather than claiming
  “responsive” or “accessible” without evidence.
- Product-noun assemblies belong under `/docs/patterns`, not Components.

## Tooling and enforcement

| Tool | Contract |
|---|---|
| `docs/api/component-routes.json` | Stable component → route → primary demo mapping |
| `docs/api/handbook-route-migration.json` | Exactly one destination for each removed Handbook file |
| `docs/scripts/scaffold-component-page.ts` | Creates a missing page; never overwrites authored MDX |
| `docs/scripts/check-component-pages.ts` | Inventory, frontmatter, demo, source, and duplicate-route checks |
| `docs/scripts/check-component-snippets.ts` | Public usage snippets remain accepted |
| `docs/scripts/check-catalog.ts` | Cross-surface story, demo, pattern, and poster coverage |

Run:

```sh
rtk bun --cwd docs run check:components
rtk bun --cwd docs run check:snippets
rtk bun --cwd docs run build
```

New widgets and breaking interaction changes update the page, shared demo,
deterministic trace, public inventory, and migration documentation together.
