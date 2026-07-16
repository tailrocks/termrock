# Plan 022: Fix the invisible-text SVG previews and harden the lookbook's JSON/CLI pipeline

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat c51e11c..HEAD -- crates/termrock-lookbook/ docs/public/component-previews/`
> On mismatch with "Current state" excerpts, STOP.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED (regenerates all committed preview SVGs — large gated diff)
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `c51e11c`, 2026-07-16

## Why this matters

The committed SVG previews — the library's shipped visual reference — render component *content* invisibly: `color_to_css` maps `Color::Reset` to `#000000` for foregrounds too, and the page background is forced `#000000`, so every cell whose fg is the terminal default (all `Style::new()` content text) is black-on-black. Shipped files prove it: `panel-focused.svg`, `viewport-both-axes.svg`, `split-pane-horizontal.svg`, `dialog-message.svg` all contain `<text ... fill="#000000">` glyph runs. The determinism and drift gates pass because the wrong output is stable. Alongside: the `list --format json` output that CI's catalog gate `JSON.parse`s is hand-assembled with `format!` and zero escaping (first `"` in a story title breaks CI opaquely); the drift-gate failure message tells users to run a command that doesn't exist against a directory that doesn't exist; `lib.rs` carries a dead duplicate story catalog; and none of these paths have tests — which is exactly why the black-on-black defect shipped.

## Current state

- `crates/termrock-lookbook/src/svg.rs` — the color mapping (~line 232-240):

```rust
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        Color::Reset => "#000000".into(),
        Color::Indexed(_) => "#ffffff".into(),
```

  and the emit loop (~185-212): page is `style="background:#000000"` + full-page `<rect ... fill="#000000"/>`; backgrounds are skipped when `bg != "#000000"` fails (so Reset-bg is correctly treated as the black page); foregrounds go straight through `let fg = color_to_css(cell.fg);` into `<text ... fill="{fg}">`.
- `escape_xml` (~243-249): handles `&`, `<`, `>`, `"` — correct for double-quoted attributes; untested.
- `manifest.json` assembly (svg.rs ~101-113): `format!(r#"  {{"id":"{}","file":"{}"}}"#, ...)` — unescaped; the file is consumed by NOTHING (verified: `check_svgs` filters to `.svg` only, no docs script reads it).
- Drift-gate error (svg.rs ~151-153):

```rust
        Err(concat!(
            "tui lookbook previews are out of date; regenerate with ",
            "`cargo run -p termrock-lookbook -- docs/public/tui-lookbook`",
        )
```

  That invocation fails with USAGE (`main.rs:40`: `usage: termrock-lookbook <terminal|list|render|check>`) and `docs/public/tui-lookbook` doesn't exist (real dir: `docs/public/component-previews`). Same stale command at `crates/termrock-lookbook/AGENTS.md` step 5; `README.md:9` names `tui-lookbook/`; `README.md:20` links `src/stories/` + `src/stories/tests.rs`, neither exists.
- `list --format json` (main.rs ~191-201):

```rust
                    format!(
                        r#"{{"id":"{}","title":"{}","component":"{}"}}"#,
                        story.id, story.title, story.component
                    )
```

  parsed by `docs/scripts/check-catalog.ts:5-7` (`JSON.parse` on the command's stdout).
- `crates/termrock-lookbook/src/lib.rs` — `pub const STORIES: &[StoryMetadata]` (18 entries id/title/component) + `pub fn stories()`: zero consumers anywhere (binary uses `stories::stories()` from `stories.rs`; grep for `StoryMetadata`/`termrock_lookbook` finds nothing). Dead duplicate catalog.
- Dead code in `main.rs` (~357-359): `sidebar_content_rows`/`sidebar_viewport_rows` computed then discarded via `let _ = (...)` under a "Vertical scrollbar" comment.
- Gates that protect this work: `docs.yml` runs `check --dir docs/public/component-previews` + double-render `diff -r`; `cd docs && bun run build` runs the catalog checker.
- Repo conventions: Conventional Commits + DCO; trunk-only `main`.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Lookbook tests | `cargo test -p termrock-lookbook` | all pass |
| Re-render previews | `cargo run -p termrock-lookbook -- render --out docs/public/component-previews` | exit 0 |
| Drift check | `cargo run -p termrock-lookbook -- check --dir docs/public/component-previews` | exit 0 |
| Determinism | render twice to temp dirs + `diff -r` | no diff |
| Catalog gate | `cd docs && bun install --frozen-lockfile && bun run build` | exit 0 |
| Workspace | `cargo test --workspace --all-features --locked` | all pass |

## Scope

**In scope**:
- `crates/termrock-lookbook/src/{svg.rs, main.rs, lib.rs, stories.rs (only if lib.rs deletion needs a type moved)}`
- `crates/termrock-lookbook/{AGENTS.md, README.md}`
- `docs/public/component-previews/*.svg` (regenerated) + `manifest.json` (removed or kept per Step 4)

**Out of scope**:
- `run_terminal` internals beyond deleting the dead scrollbar bindings (Plan 018 owns the rewrite).
- Story content/sizes (Plan 023 owns axis-coverage stories).
- `docs/scripts/check-catalog.ts` (its parse is correct; the producer is fixed here).

## Git workflow

- Directly on `main`; suggested commits: `fix(lookbook): render default-foreground text visibly in SVG previews` then `fix(lookbook): escape JSON output and correct stale runbook commands`.

## Steps

### Step 1: Fix the Reset-foreground mapping

In `svg.rs`, split fg/bg default handling: keep the bg path as-is (Reset bg = the black page, already correct via the `bg != "#000000"` skip), and at the fg call site map `Color::Reset` to the lookbook's default foreground — use the phosphor text tone (match what the terminal renders: pick `#e6e6e6` or the theme's `Text` role color; inspect `Theme::default().style(Role::Text)` — currently `BOLD_WHITE` = `#ffffff` — and use `#ffffff`). Implement as a `color_to_css_fg(color)` wrapper (Reset → `#ffffff`) leaving `color_to_css` for bg, or an explicit parameter — smallest clear change wins.

Add the test FIRST (red→green) in svg.rs's test module: render a 1-cell buffer containing a default-style glyph through `buffer_to_svg`-equivalent path and assert the output contains `fill="#ffffff"` (not `#000000`) for the `<text>` element.

**Verify**: new test red before, green after. `cargo test -p termrock-lookbook` → all pass.

### Step 2: Regenerate all previews

```
cargo run -p termrock-lookbook -- render --out docs/public/component-previews
cargo run -p termrock-lookbook -- check --dir docs/public/component-previews
```

Inspect the diff: expect fg fills flipping `#000000` → `#ffffff` on content text across ~19 SVGs; backgrounds unchanged. Spot-open `panel-focused.svg` and confirm `grep -o 'fill="#000000"' docs/public/component-previews/panel-focused.svg | wc -l` drops to 0 (or only genuine black-styled cells remain — check each survivor).

**Verify**: `check` exits 0; double-render determinism diff empty; `cd docs && bun run build` → exit 0.

### Step 3: Escape the JSON producers

Add a tiny `json_escape(&str) -> String` (escape `\`, `"`, and control chars < 0x20 as `\uXXXX`) in the lookbook (no new deps) and route BOTH `list --format json` (main.rs) and the manifest assembly through it. Test: a synthetic story title containing `"` and `\n` round-trips through a real `JSON.parse`-equivalent — simplest in Rust: assert the escaped output, plus one integration-style test spawning nothing (don't shell out): assert `json_escape(r#"a"b"#) == r#"a\"b"#` and full-line format.

**Verify**: `cargo test -p termrock-lookbook` → new tests pass; `cd docs && bun run build` → exit 0 (the real parser still accepts real output).

### Step 4: Delete dead surface + fix stale docs

- Delete `lib.rs`'s `STORIES`/`StoryMetadata`/`stories()` (the whole dead catalog; if `lib.rs` becomes empty, remove the lib target from `Cargo.toml` — check `[lib]`/auto-discovery: the binary must keep building. If main.rs does `use termrock_lookbook::...` anywhere, it doesn't (verified — it uses internal modules); proceed).
- `manifest.json`: nothing consumes it — stop generating it (delete the assembly block in svg.rs) and `git rm docs/public/component-previews/manifest.json`. (If you find a consumer grep missed, STOP instead.)
- Fix the drift-gate error message to: `` regenerate with `cargo run -p termrock-lookbook -- render --out docs/public/component-previews` ``.
- Fix `AGENTS.md` step 5 (same command), `README.md:9` (`component-previews/`), `README.md:20` table row (point at `stories.rs` and `src/tests.rs`).
- Delete the dead `sidebar_content_rows`/`sidebar_viewport_rows` bindings (main.rs ~357-359).

**Verify**: `grep -rn "tui-lookbook\|StoryMetadata\|manifest.json" crates/termrock-lookbook/ docs/scripts/` → no hits (except this plan's own text if grep hits plans/ — scope the grep); `cargo test --workspace --all-features --locked` → all pass; `check` gate green.

### Step 5: Backfill the missing test net

Add tests (svg.rs test module + a new `main` args test if the parser is reachable — it lives in `main.rs`, so either extract `parse_args` into a testable fn in a module or test via the escape/emit helpers only; prefer the small extraction):

- `escape_xml`: `&`, `<`, `>`, `"` each escaped; `'` untouched (documented: attributes are double-quoted).
- Wide-char cell (e.g. `日`) produces a single `<text>` at the correct x (pin current placement behavior).
- `json_escape` cases (Step 3).
- Default-fg visibility (Step 1's test).

**Verify**: `cargo test -p termrock-lookbook` → ≥6 new tests pass.

## Test plan

Steps 1/3/5 define it: ~6-8 new tests, red→green for the two fixes. The SVG regen diff in Step 2 is itself reviewed evidence.

## Done criteria

- [ ] No `fill="#000000"` on default-foreground content text in regenerated previews (spot-grep 3 files)
- [ ] `list --format json` and any remaining JSON emission escaped + tested
- [ ] Stale `tui-lookbook` command/dir references gone from error message, AGENTS.md, README.md
- [ ] `lib.rs` dead catalog + `manifest.json` generation removed
- [ ] `cargo test --workspace --all-features --locked`, preview `check`, determinism diff, `bun run build` — all green
- [ ] `plans/README.md` status row updated

## STOP conditions

- A consumer of `manifest.json` or `termrock_lookbook::STORIES` surfaces that grep missed — report it; deletion halts for that item.
- Regenerated SVGs change more than fg fills (layout/geometry diffs) — the renderer changed underneath this plan (check for concurrent plan landings); reconcile before committing goldens.
- The `Role::Text` default is not white after Plan 008/010 landed with theme changes — pick the actual default-theme text color and note it.

## Maintenance notes

- Rule: SVG fg and bg default-color semantics differ — any future `color_to_css` edit must keep the split; the Step-1 test enforces it.
- Plan 020 (storybook knobs) and Plan 023 (axis stories) will regenerate previews again — no conflict, later regens inherit the fix.
- If story metadata ever needs a machine-readable manifest again, emit it via the (now-tested) `json_escape` path and give it a consumer + gate in the same commit.
