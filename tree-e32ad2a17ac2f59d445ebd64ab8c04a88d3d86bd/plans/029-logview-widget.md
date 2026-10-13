# Plan 029 (rewritten 2026-07-16, round 3): Complete `LogPane` — the shipped widget's residual gaps vs the original LogView spec

> **SUPERSESSION NOTE**: The original Plan 029 specified a `LogView` widget.
> Commit `ccf0646` shipped `widgets/log_pane.rs` (`LogPane`/`LogPaneState`)
> implementing the core concept (tail-follow scrollback, follow indicator,
> themed viewport, story/preview/contract row). This rewrite covers ONLY the
> residual gaps. The original build-from-scratch plan is superseded.
>
> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on. STOP
> conditions are binding. Update the plans/README.md row when done.
>
> **Drift check (run first)**: `git diff --stat 5c4758b..HEAD -- crates/termrock/src/widgets/log_pane.rs crates/termrock/src/widgets/viewport.rs crates/termrock/src/ansi_text.rs`
> Concurrent executors active — re-locate by symbol.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: LOW-MED (one item — Step 1 — is a maintainer decision, not code)
- **Depends on**: none
- **Category**: tech-debt
- **Planned at**: commit `5c4758b`, 2026-07-16

## Why this matters

`LogPane` shipped the right concept but skipped named parts of the spec, and one skip was an explicit STOP-condition violation: the widget **owns** its line buffer (`LogPaneState { lines: Vec<Line<'static>>, .. }` with `append`/`clear`/ring-buffer), where the ownership doctrine and the original plan mandated borrowed `&[Line]` with a STOP-and-report if owned storage seemed necessary. That decision was made silently and must be ratified or reverted — it also broke the family signature (`handle_key(&mut self, key)` with no data parameter, unlike every other stateful widget). Beyond that: no wheel/scroll API (`scroll_by`), no `Home` (jump to oldest), no public `follow()`, contract row says mouse "caller-owned" but offers no method to own it against; no scrolled-back ("▲ +N") non-color indicator (only the following-state cue exists); no ANSI ingest helper (the widget renders logs but offers no path from SGR-colored bytes to `Line`s); the buffer defaults to unbounded (`max_lines: None`); width is still re-measured O(content) per frame via `Viewport`'s `max_line_width` (Plan 016 owns the Viewport-internal fix — this plan only must not add more O(content) work); and there is no hot-path allocation test.

## Current state

- `crates/termrock/src/widgets/log_pane.rs` (~line 14): `LogPaneState { lines: Vec<Line<'static>>, /* tail/follow state */, max_lines: Option<usize> }`; `LogPaneState::new()` → `max_lines: None`; `with_max_lines(usize)` builder; `append(impl Into<Line<'static>>)` with drain-on-overflow; `handle_key(&mut self, key)` (~:86, has the Release guard) covering Up/Down/j/k/PageUp/PageDown/End — **no Home**; render (~:145) delegates to `Viewport` passing `&state.lines`; follow indicator `" ⇣ following"` (~:163).
- Family exemplars for the missing APIs: `TreeState::scroll_by(delta: isize, len) -> bool` shape (post-011 canonical), `tests/tree_hot_path.rs` (stats_alloc pattern), `tests/viewport_hot_path.rs` (landed with plan 015).
- `ansi_text::styled_spans(input: &str, default_style: Style) -> Vec<Span>` exists; no `Line`-producing ingest helper.
- Lookbook: one story (`log-pane-follow`), interactor with dead mouse path (`handle_mouse` returns false).
- Contract row (`docs/api/component-contracts.json` LogPane): mouse `caller-owned`; unicode claimed `covered` without a wide-char story line (see plans 023/030-pattern).
- Conventions: breaking changes → migration file; `missing_docs = deny` (real docs, not stubs); `public-api.txt` regen; previews re-render when stories change.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Widget tests | `cargo test -p termrock log_pane` | all pass |
| Hot path | `cargo test -p termrock --test log_pane_hot_path` | passes (new) |
| Previews | `cargo run -p termrock-lookbook -- render --out docs/public/component-previews && cargo run -p termrock-lookbook -- check --dir docs/public/component-previews` | exit 0 |
| Full gate | `mise run gate` | exit 0 |

## Scope

**In scope**: `log_pane.rs`, `ansi_text.rs` (one helper), lookbook story/interactor additions, contract row, hot-path test file, migration file if Step 1 changes the API, `public-api.txt` regen.

**Out of scope**: `Viewport` internals (Plan 016 owns the O(content) width measure); Plan 034/TextArea; search/filter in logs.

## Git workflow

- Directly on `main`; `git commit -s -m "feat(log-pane): complete the scrollback contract"` (+`!` and migration if Step 1 reverts ownership).

## Steps

### Step 1: Ratify or revert the owned buffer — REPORT FIRST

This is the escalated STOP condition from the original plan. Present both options to the maintainer via your report BEFORE implementing either (if running unattended: implement (a) ratify — it's shipped, non-breaking, and the ring-buffer is genuinely useful — and record the decision):
(a) **Ratify**: add a decision paragraph to `AGENTS.md`'s ownership section or the widget's module doc: "LogPane is the deliberate exception: an append-oriented scrollback owns its ring buffer because callers stream lines in rather than projecting state per frame." Keep `handle_key(&mut self, key)` (no data param — the data is internal; document why the family shape differs).
(b) **Revert**: borrowed `&[Line]` per the original spec — breaking, migration file, `append`/ring-buffer moves to a standalone `LogBuffer` utility.

**Verify**: decision recorded (doc text committed) or report filed.

### Step 2: Complete the interaction surface

Add: `scroll_by(&mut self, delta: isize) -> bool` (wheel; len is internal under ratified ownership), public `follow(&mut self)` + `is_following(&self) -> bool` (if not already public — check), `Home` key arm (jump to oldest). Change the default construction to bounded: `new()` keeps unbounded? NO — flip the default to a documented sane bound (e.g. 10_000 lines) with `unbounded()` as the explicit opt-out builder; unbounded-by-default plus O(content) measure is a memory/CPU footgun. (Breaking default-behavior change: include in the migration/notes.) Add the scrolled-back non-color cue: when not following, indicator shows `" ⇡ +N"` (N = lines below the view) alongside the existing following cue. Wire the lookbook interactor's mouse path (wheel → `scroll_by`).

**Verify**: widget tests extended (Home, scroll_by clamps, follow/unfollow transitions, bounded-default eviction, scrolled-back indicator cell content) → `cargo test -p termrock log_pane` all pass.

### Step 3: ANSI ingest helper

In `ansi_text.rs`: `pub fn line_from_ansi(input: &str, default_style: Style) -> Line<'static>` wrapping `styled_spans` (documented as the parse-once-at-append path). Test: SGR-colored input → styled spans in the Line; plain input passthrough.

**Verify**: `cargo test -p termrock ansi` → new tests pass.

### Step 4: Hot-path proof + story coverage

`crates/termrock/tests/log_pane_hot_path.rs` modeled on `tree_hot_path.rs`/`viewport_hot_path.rs`: 10k appended lines, 40-row viewport, warmed 100 renders at tail → bounded allocations. NOTE: until Plan 016 lands, `Viewport`'s `max_line_width` full scan makes a strict zero-alloc bound unreachable — set the budget to what the slice-fix (015) allows and add a `// TIGHTEN after plan 016` comment with the target. Add a `log-pane/scrolled` story variant (scrolled-back indicator + a CJK/emoji line, covering the claimed unicode axis); re-render previews.

**Verify**: hot-path test green (and demonstrably fails with an artificially unbounded budget — state observed numbers); previews deterministic; `cd docs && bun run build` green; `mise run gate` green.

## Done criteria

- [x] Ownership decision recorded (doc or report)
- [x] `scroll_by`/`follow`/`Home`/bounded-default/scrolled-back cue shipped + tested; interactor wheel works
- [x] `line_from_ansi` exists + tested
- [x] `log_pane_hot_path` test green with stated budget; `log-pane/scrolled` story + preview committed
- [x] Contract row accurate (mouse still caller-owned is now defensible — scroll_by exists; unicode demonstrated)
- [x] `mise run gate` → exit 0; `plans/README.md` row updated

## STOP conditions

- Maintainer rejects both ownership options (wants a third shape) — stop, capture requirements.
- The bounded-default flip breaks a shipped consumer pattern visible in-repo — report before changing the default.

## Maintenance notes

- Plan 016 tightens the hot-path budget (the TIGHTEN comment is the tracker).
- If Plan 031's frame clock lands, no LogPane change needed (no time-based behavior here).
