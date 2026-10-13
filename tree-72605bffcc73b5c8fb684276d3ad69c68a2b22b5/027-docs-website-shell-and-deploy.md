# Plan 027: Stand up the docs website for real — app shell, local dev, CI build, deploy

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat c51e11c..HEAD -- docs/ .github/workflows/docs.yml`
> Plans 003/022/023 touch docs.yml/previews — expected. Structural surprises
> in docs/ config = STOP.

## Status

- **Priority**: P2
- **Effort**: L
- **Risk**: LOW-MED (new frontend code; no library-crate risk)
- **Depends on**: none hard; Plan 028 (component pages) depends on THIS
- **Category**: dx
- **Planned at**: commit `c51e11c`, 2026-07-16

## Why this matters

The "docs site" cannot be built, served, or previewed: `docs/vite.config.ts` wires fumadocs-mdx + TanStack Start (prerender enabled) + React, and `package.json` locks fumadocs-ui/core, React 19, TanStack Start, Vite 8 — but there are ZERO route/entry/layout files (no `.tsx` anywhere in tracked docs/), no `dev`/`start` script, and `build` runs only `fumadocs-mdx && check:catalog` (content validation, no site). Nothing deploys anywhere. For a shadcn-inspired component library, the browsable docs site is the product's front door; today the framework is a dead weight of locked dependencies with nothing to render. The repo's Modern-first mandate (AGENTS.md) and the maintainer's multi-project reuse goal make BUILD (not delete) the right disposition — this plan builds the shell and deploy; Plan 028 fills in per-component reference pages.

## Current state

- `docs/package.json` scripts (complete):

```json
  "scripts": {
    "postinstall": "fumadocs-mdx",
    "types:check": "fumadocs-mdx && tsc --noEmit",
    "check:catalog": "bun run scripts/check-catalog.ts",
    "build": "fumadocs-mdx && bun run check:catalog"
  },
```

  Deps: `@tanstack/react-start`, `@vitejs/plugin-react`, `fumadocs-core`, `fumadocs-mdx`, `fumadocs-ui`, `react` 19, `react-dom`, `vite` 8; dev: `@types/*`, `typescript` 7.
- `docs/vite.config.ts` (entire file):

```ts
import react from '@vitejs/plugin-react'
import { tanstackStart } from '@tanstack/react-start/plugin/vite'
import mdx from 'fumadocs-mdx/vite'
import { defineConfig } from 'vite'

export default defineConfig({ plugins: [mdx(await import('./source.config')), tanstackStart({ prerender: { enabled: true } }), react()] })
```

- `docs/source.config.ts`: `defineDocs({ dir: 'content/docs' })`.
- Content: `docs/content/docs/{index,components,application-patterns,interaction,quality-migrations}.mdx` + `meta.json` listing those five pages. Static assets: `docs/public/component-previews/*.svg` (fixed to be visible by Plan 022).
- `docs/tsconfig.json` `include` covers only `source.config.ts` + `scripts/**` — vite.config.ts and any future app code are unchecked.
- `docs/scripts/catalog.test.ts` — a `bun:test` that runs the checker and asserts the literal "catalog covers 18 public components with 18 stories"; executed by NOTHING (no `bun test` in any workflow or script).
- `.github/workflows/docs.yml`: types:check, build (= mdx+catalog), lookbook preview check, double-render determinism. No vite build, no artifact, no deploy job; no Pages config in the repo.
- Repo conventions: bun as the JS runtime (mise pins bun 1.3.14); trunk-only; Conventional Commits + DCO.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Install | `cd docs && bun install --frozen-lockfile` | exit 0 |
| Typecheck | `cd docs && bun run types:check` | exit 0 |
| Dev server | `cd docs && bun run dev` | serves locally (manual spot-check) |
| Site build | `cd docs && bun run build` | exit 0, emits static output |
| Workflow lint | `mise x -- actionlint` | exit 0 |

## Scope

**In scope**:
- `docs/` app shell: routes, entries, layout, root styles (new `.tsx`/`.css` files), `package.json` scripts, `tsconfig.json` include, `vite.config.ts` if needed
- `.github/workflows/docs.yml` (build + deploy additions)
- `docs/scripts/catalog.test.ts` disposition (Step 4)

**Out of scope**:
- Per-component reference pages and preview embedding — Plan 028.
- MDX content rewrites beyond what routing requires (frontmatter tweaks OK).
- The Rust crates entirely.
- Custom visual design beyond fumadocs-ui defaults + a phosphor-accent tweak (keep it minimal; design iteration later).

## Git workflow

- Directly on `main`; commits: `feat(docs): stand up the fumadocs site shell`, `ci(docs): build and deploy the site`.

## Steps

### Step 1: Scaffold the app shell

Follow the current fumadocs + TanStack Start integration docs (fumadocs.dev — the fumadocs-mdx vite plugin + `defineDocs` setup is already half-done here; consult the framework's TanStack Start guide for the exact file set). Expected shape (adjust to the framework's current conventions — the docs are authoritative, not this sketch):

- `docs/src/routes/__root.tsx` — root layout wiring fumadocs-ui provider + RootProvider, dark default theme (phosphor-friendly).
- `docs/src/routes/docs/$.tsx` (catch-all docs route) — fumadocs page renderer over the `source` loader from `source.config`.
- `docs/src/routes/index.tsx` — redirect or landing that links into `/docs`.
- Client/server entries per TanStack Start convention.
- `package.json`: add `"dev": "vite dev"`, change `"build": "fumadocs-mdx && bun run check:catalog && vite build"` (keep the catalog gate IN the build), keep `types:check`.
- `tsconfig.json`: include `src/**` and `vite.config.ts`; add JSX settings per the framework guide.

**Verify**: `bun run types:check` → exit 0; `bun run dev` serves and `/docs` renders the five MDX pages with navigation (manual check — describe what you saw in the report); `bun run build` → exit 0 and emits the prerendered static output directory (locate it — TanStack Start prerender output; note the path for Step 3).

### Step 2: Wire the SVG previews as usable assets

Confirm `docs/public/` is served/copied by the build (vite convention: yes). Add one image embed to `components.mdx` (e.g. the List preview) to prove the path works end-to-end — full per-component embedding is Plan 028:

```mdx
![List selection preview](/component-previews/list-selection.svg)
```

**Verify**: dev server shows the image; built output contains the SVG; `bun run build` (catalog gate) still green — note check-catalog.ts requires story ids in backticks; adding an image doesn't disturb that.

### Step 3: CI build + deploy

Extend `docs.yml`: after the existing gates, run the site build and deploy to GitHub Pages (unless repo settings dictate otherwise — check `gh api repos/tailrocks/termrock --jq .has_pages` and existing Pages config; if inspection is impossible, default to the standard Pages actions and note it):

```yaml
      - working-directory: docs
        run: bun run build
      - uses: actions/upload-pages-artifact@v3
        with: { path: docs/<prerender-output-dir> }
  deploy-pages:
    needs: docs-required
    if: github.ref == 'refs/heads/main'
    permissions: { pages: write, id-token: write }
    environment: { name: github-pages }
    runs-on: ubuntu-latest
    steps:
      - uses: actions/deploy-pages@v4
```

Set the site base path if Pages serves under `/termrock/` (framework-specific config — the deploy is broken without it; verify against the built output's asset URLs).

**Verify**: `actionlint` → exit 0. Full CI verification requires a push; state in the report that the deploy job's first run must be watched, and what URL to expect.

### Step 4: Give `catalog.test.ts` a life or a funeral

The count-assertion test is executed by nothing. Simplest coherent fix: delete `catalog.test.ts` and move a dynamic count line into `check-catalog.ts`'s success output (it already prints the counts — the test only re-asserted them). If instead the team wants `bun test` in CI, wire `bun test` into docs.yml and make the assertion dynamic (read the component count from the checker's exit data). Pick deletion unless a second `bun:test` exists to justify the harness (there isn't — verified).

**Verify**: `ls docs/scripts/` reflects the choice; `bun run build` green.

## Test plan

- `types:check` + `build` + dev-server manual render check.
- One embedded preview proves the asset pipeline.
- `actionlint` on the workflow.

## Done criteria

- [ ] `bun run dev` serves the site; all five MDX pages render with nav
- [ ] `bun run build` emits deployable static output AND still runs the catalog gate
- [ ] `docs.yml` builds the site and deploys on main (workflow lints clean; first-run watch noted)
- [ ] `tsconfig.json` covers all TS/TSX in docs/
- [ ] `catalog.test.ts` deleted-or-wired (no dead test remains)
- [ ] `plans/README.md` status row updated

## STOP conditions

- fumadocs-ui/TanStack Start versions in `bun.lock` are incompatible with the current integration guide (API drift in the framework) — report the mismatch + closest working recipe before writing piles of glue.
- The prerender output cannot serve under a Pages base path without framework support — report; deploying broken asset URLs is worse than not deploying.
- `bun install --frozen-lockfile` fails needing lockfile changes — adding the shell should not need new deps (everything is locked already); if a peer dep is genuinely missing, report it before touching the lockfile.

## Maintenance notes

- Plan 028 builds the per-component pages on this shell — keep the docs route generic (catch-all over `content/docs`) so new pages are pure content.
- The site build now runs in CI on every push — if it slows the docs job, split site-build/deploy into its own workflow later.
- Keep the phosphor-dark default theme in sync with Plan 010's slate preset story (a future site theme toggle can mirror the lookbook's).
