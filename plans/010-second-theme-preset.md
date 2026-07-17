# Plan 010: Ship a second built-in theme preset that proves consumer rebranding works end-to-end

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat da54a03..HEAD -- crates/termrock/src/style/ crates/termrock-lookbook/`
> Plans 008/009 must already have landed (constructible Theme, palette inside
> style). If they haven't, STOP — this plan cannot work without them.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW (additive preset + lookbook flag)
- **Depends on**: plans/008-theme-constructor-and-role-threading.md, plans/009-brand-constant-purge.md
- **Category**: direction
- **Planned at**: commit `da54a03`, 2026-07-16

## Why this matters

After Plans 008/009 a consumer *can* build a custom `Theme` — but nothing demonstrates it, and nothing in CI would catch a widget regressing into a hardcoded color. A second built-in preset does three jobs: (1) living documentation of "how do I rebrand this library"; (2) a regression tripwire — render every lookbook story under both themes and any widget that ignores the theme becomes visible; (3) per AGENTS.md's "Modern-first, pre-stable API" section, the phosphor look stays the default while the library proves it is "adoptable by projects with entirely different brands."

## Current state

- `Theme` (post-008): constructible via `tailrocks_phosphor()`, `with_role(Role, Style)`, `from_fn(...)`; `Default` = phosphor. Roles: 37 variants (22 original + 15 added by Plan 008).
- Lookbook: `crates/termrock-lookbook/src/main.rs` — subcommands `render --out <dir>`, `check --dir <dir>`, `list --format json` (used by `docs/scripts/check-catalog.ts`); stories in `stories.rs` construct `Theme::default()` per story; deterministic SVG pipeline via `svg.rs`; preview goldens in `docs/public/component-previews/` checked by `docs.yml`.
- `docs.yml` renders twice and `diff -r`s the two outputs to prove determinism.
- Repo conventions: additive public API still needs `docs/api/public-api.txt` regeneration (Plan 003 gate); Conventional Commits + DCO.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Tests | `cargo test --workspace --all-features --locked` | all pass |
| Render previews | `cargo run -p termrock-lookbook -- render --out docs/public/component-previews` | exit 0 |
| Check previews | `cargo run -p termrock-lookbook -- check --dir docs/public/component-previews` | exit 0 |
| Clippy | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | exit 0 |

## Scope

**In scope**:
- `crates/termrock/src/style/mod.rs` (one new preset fn)
- `crates/termrock-lookbook/src/` (a theme dimension for stories/render)
- `docs/public/component-previews/` if the render output set changes
- `docs/api/public-api.txt` regeneration

**Out of scope**:
- Changing the default theme — `Default` stays phosphor, non-negotiable (AGENTS.md).
- An interactive theme switcher in the lookbook TUI — Plan 020's spike.
- New roles or widget changes — if a widget renders identically under both themes, that's a Plan-008 gap: STOP and report it.

## Git workflow

- Directly on `main`; `git commit -s -m "feat(style): add slate theme preset and dual-theme preview proof"`.

## Steps

### Step 1: Design and add the preset

Add `Theme::slate()` in `style/mod.rs`: a deliberately different, still-tasteful palette (cool grays + blue accent + amber warning; e.g. accent `Color::Rgb(96,165,250)`, selection bg `Color::Rgb(30,41,59)`, text `Color::Rgb(226,232,240)`, danger `Color::Rgb(248,113,113)`) covering **all 37 roles** with values visibly distinct from phosphor for at least: `Accent`, `Selection`, `BorderFocused`, `TabActive`, `HintText`, `DiffAdded`, `DiffRemoved`. Document it as "the neutrality proof and rebranding reference — copy this function into your app and adjust."

Add a unit test: for a chosen list of roles (the seven above), `Theme::slate().style(r) != Theme::tailrocks_phosphor().style(r)`.

**Verify**: `cargo test -p termrock style` → passes.

### Step 2: Give the lookbook a theme dimension

In the lookbook, thread a `--theme <phosphor|slate>` flag through `render` (default `phosphor`, keeping current goldens byte-identical) and render slate previews to a sibling directory or `--theme slate --out <dir>` invocation. Choose the minimal mechanism the existing CLI parsing supports (read `main.rs`'s arg handling first — it is hand-rolled; extend it in the same style).

Update `docs.yml` determinism block to also render slate once:

```yaml
      - run: cargo run -p termrock-lookbook -- render --theme slate --out target/render-slate >/dev/null
```

and add a cheap tripwire: a lookbook test (in `crates/termrock-lookbook/src/tests.rs`, following its existing test style) that renders one representative story (e.g. the `List` selection story) under both themes into buffers and asserts the selected-row cells differ in style — the "no widget ignores the theme" canary.

**Verify**: `cargo test -p termrock-lookbook` → new test passes; both render commands exit 0.

### Step 3: Documentation touch

In `crates/termrock/README.md` (currently ~9 lines), add a short "Theming" section: default is phosphor; `Theme::slate()` exists as the rebranding reference; three-line example using `with_role`. Regenerate `docs/api/public-api.txt`.

**Verify**: `cd docs && bun run build` → exit 0.

## Test plan

- Unit: slate-vs-phosphor role divergence (Step 1).
- Integration: dual-theme story render canary (Step 2).
- Suite: `cargo test --workspace --all-features --locked` all pass; preview check unchanged for phosphor goldens.

## Done criteria

- [x] `Theme::slate()` public, documented, all roles covered
- [x] Dual-theme canary test exists and passes
- [x] `cargo run -p termrock-lookbook -- render --theme slate --out target/render-slate` → exit 0
- [x] Phosphor goldens in `docs/public/component-previews/` unchanged (`git diff --stat` empty for that dir unless the set intentionally grew)
- [x] `cargo test --workspace --all-features --locked` → all pass
- [x] `plans/README.md` status row updated

## STOP conditions

- Any widget renders byte-identically under both themes in the canary → a theming bypass survived Plans 008/009; report the widget.
- The lookbook CLI's arg handling can't accommodate a flag without restructuring `main.rs` significantly → report; Plan 020 restructures the lookbook and this step may belong there.

## Maintenance notes

- When new roles are added, `slate()` must be extended in the same commit (the from-fn coverage test will catch omissions if written against `Theme::roles()`).
- Plan 020 (storybook spike) adds the interactive theme switcher on top of this preset.
