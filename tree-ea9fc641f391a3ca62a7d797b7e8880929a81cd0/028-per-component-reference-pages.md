# Plan 028: Per-component reference pages — preview, usage snippet, contract table for every widget

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat c51e11c..HEAD -- docs/ crates/termrock/src/widgets/`
> This plan is written against the POST-011/013/027 state: site shell exists,
> widget APIs converged. Verify those rows are DONE; write snippets against
> LIVE code, not this plan's assumptions.

> **Reconcile note (2026-07-16, round 3, HEAD `5c4758b`)**: widget set is now
> 20 (Progress, LogPane added); 011/013 landed so usage snippets can be written
> against the final API; plan 014's doctests exist as snippet sources — BUT
> plan 036 (rustdoc placeholder sweep) should land first, since generated pages
> quote doc content and ~330 public items currently carry placeholder stubs.

## Status

- **Priority**: P2
- **Effort**: L
- **Risk**: LOW (content + generator script; no library risk)
- **Depends on**: plans/027-docs-website-shell-and-deploy.md; strongly prefer after plans/011 + 013 (write usage snippets once, against the final API)
- **Category**: docs
- **Planned at**: commit `c51e11c`, 2026-07-16

## Why this matters

`components.mdx` is a flat bulleted inventory — one prose line + a story ID per widget, zero code fences in the whole file. The 19+ SVG previews are referenced only by the catalog checker's existence test; no page embeds them. The per-component page — rendered preview + copy-paste usage + API/contract table — IS the shadcn value proposition this project cites as its north star, and it's entirely absent. The machinery to generate most of it already exists: `component-contracts.json` (per-widget axis table), the preview SVGs, story metadata via `list --format json`, and (post-Plan-014) doctested usage snippets.

## Current state

- `docs/content/docs/components.mdx`: flat list, `grep -c '\`\`\`'` = 0. `docs/content/docs/meta.json`: `{"pages":["index","components","application-patterns","interaction","quality-migrations"]}` — five pages, none per-widget.
- `docs/api/component-contracts.json`: per-component rows like

```json
"ChoiceDialog": { "keyboard": "covered", "mouse": "covered", "focus": "covered", "nonColor": "covered", "unicode": "covered", "narrowTerminal": "covered" },
```

- Previews: `docs/public/component-previews/<story-id-with-dashes>.svg` (visible post-Plan-022; Plan 023 added narrow/unicode variants).
- Catalog gate (`docs/scripts/check-catalog.ts` ~40-45): every story id must appear in backticks in SOME `.mdx` under `docs/content/docs/`, and its preview must exist. Per-component pages must keep those mentions (moving them from components.mdx to per-widget pages is fine — the checker concatenates all MDX).
- Story metadata source: `cargo run -q -p termrock-lookbook -- list --format json` → `[{id,title,component}...]` (escaped post-Plan-022).
- Component inventory source of truth: `docs/api/public-api.txt` widget-impl lines (same regex the checker uses).
- Usage-snippet source: post-Plan-014 doctests on core types; each widget's construction idiom is post-013 (`X::new(required).builder(...)`).
- fumadocs supports nested page trees via `meta.json` — a `components/` subdirectory with its own `meta.json` is the natural layout.
- Repo conventions: docs ship with the change; the site build (`bun run build`) runs the catalog gate.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Story metadata | `cargo run -q -p termrock-lookbook -- list --format json` | JSON array |
| Site build + gate | `cd docs && bun run build` | exit 0 |
| Dev preview | `cd docs && bun run dev` | pages render |
| Snippet compile check | `cargo test --doc --workspace --locked` | all pass (snippets sourced from doctests) |

## Scope

**In scope**:
- `docs/content/docs/components/` (new directory: one `<component>.mdx` per public widget + `meta.json`)
- `docs/content/docs/components.mdx` (becomes the index/overview linking the pages)
- `docs/scripts/gen-component-pages.ts` (new generator) + `docs/scripts/check-catalog.ts` (extend: require a per-component page)
- `docs/package.json` (wire the generator into `build` or a `gen` script)

**Out of scope**:
- Rust source changes. Snippets must mirror existing doctests/examples — if a widget lacks a usable doctest, write the snippet by hand against the live API and flag it as a Plan-014 backfill candidate; do NOT add doctests here.
- Interactive/browser-rendered component demos (SVG stills only; a live-terminal embed is future work).
- Site shell/layout (Plan 027).

## Git workflow

- Directly on `main`; `git commit -s -m "docs(site): generate per-component reference pages"`.

## Steps

### Step 1: Design the page template + generator

Write `docs/scripts/gen-component-pages.ts` (bun, matching check-catalog.ts's style — plain script, node:fs): inputs = `public-api.txt` (component list via the checker's regex), `component-contracts.json`, story metadata (`list --format json`), and a per-component authored-content block (see below). For each component emit `docs/content/docs/components/<kebab-name>.mdx`:

```mdx
---
title: List
description: <one-liner from the authored block>
---

![List — selection](/component-previews/list-selection.svg)

## Usage

```rust
<authored usage snippet>
```

## Interaction contract

| Axis | Status |
|---|---|
| Keyboard | covered |
...

## Stories

- `list/selection` — Selection list
- `list/narrow` — ...
```

Authored content (description + usage snippet) lives in a checked-in map the generator consumes — simplest: `docs/scripts/component-docs.ts` exporting `{ List: { description, usage }, ... }` so regeneration never loses hand-written prose. Generator is idempotent + deterministic (stable ordering).

**Verify**: run the generator; 18+ pages appear; `bun run dev` renders one spot-checked page with image + table.

### Step 2: Author the usage snippets

For each widget: shortest realistic construction + one interaction line, copied from (in priority order) the widget's doctest (Plan 014), `tests/` fixtures, or lookbook stories — then adjusted to the post-013 idiom. Compile-verify each snippet by temporarily assembling them into a scratch doctest locally OR by eyeballing against a doctest that already compiles (state which method you used per snippet batch in the report; snippets that mirror compiled doctests verbatim need no re-verification).

**Verify**: snippets match live signatures (spot-grep 5 widgets' `new(` signatures against their snippets).

### Step 3: Rewrite the index + wire the gate

`components.mdx` becomes an overview: intro paragraph + a linked table (component → one-liner → page). Keep any story-id backtick mentions the checker needs IF a story isn't mentioned on its component page (post-Step-1 every story IS listed on its page — the index can drop raw ids). Extend `check-catalog.ts`: every public component must have `docs/content/docs/components/<kebab>.mdx`, and every story id must appear in the concatenated MDX (existing rule — now satisfied by the pages). Add `components/meta.json` listing pages alphabetically; add the `components` folder to the parent `meta.json`.

Wire generation: add `"gen:components": "bun run scripts/gen-component-pages.ts"` and a check mode (`--check` diffing generated vs committed) into `build` so stale pages fail CI — same freshness philosophy as the public-api gate (Plan 003).

**Verify**: `cd docs && bun run build` → exit 0; delete one page locally → build fails → restore. `bun run dev` → nav shows the Components tree.

## Test plan

- Generator determinism: run twice, `git diff` empty.
- Gate negative-test (Step 3).
- Visual spot-check of 3 pages (image renders, table correct vs contracts.json, snippet highlights as Rust).

## Done criteria

- [ ] One MDX page per public widget with preview embed, usage snippet, contract table, story list
- [ ] Generator + `--check` freshness gate wired into `bun run build`
- [ ] `components.mdx` is the linked overview; catalog gate green; site builds and renders
- [ ] Snippets verified against live APIs (method stated in report)
- [ ] `plans/README.md` status row updated

## STOP conditions

- Plans 011/013 not landed — snippets would be written twice; stop, dependency (or get explicit go-ahead recorded in the README row).
- fumadocs' MDX pipeline rejects the generated frontmatter/format — fix format, not framework; if a framework bug blocks images-in-MDX, report.
- The authored-content map grows a snippet the live API can't express (drifted widget) — that's an API-docs mismatch to report, not paper over.

## Maintenance notes

- New-widget checklist (final form): code + story + preview + contract row + doctest + authored `component-docs.ts` entry — the gate now fails on a missing page, closing the loop AGENTS.md promises ("inventory, contract matrix, documentation, story, deterministic preview").
- When Plan 020's knobs land, consider auto-embedding variant previews (narrow/unicode) on each page — the generator already lists variant stories.
