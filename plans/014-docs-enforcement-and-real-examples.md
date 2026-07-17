# Plan 014: Enforce public-API docs and replace stub examples with a real interactive on-ramp

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat da54a03..HEAD -- crates/termrock/examples/ crates/termrock/README.md Cargo.toml`
> Earlier plans changed widget APIs — that is expected; this plan is written
> against the POST-011/013 API, so read the current signatures as you go. STOP
> only if the examples/ layout itself changed.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: LOW-MED (missing_docs backfill is large but mechanical; examples are additive)
- **Depends on**: plans/011-event-model-convergence.md, plans/013-construction-idiom-and-widget-traits.md (write examples against the final API, once)
- **Category**: docs
- **Planned at**: commit `da54a03`, 2026-07-16

## Why this matters

For a library whose stated value is reuse across projects, the on-ramp is missing twice: (1) `missing_docs` is not enabled — public items can ship undocumented and CI stays green (`RUSTDOCFLAGS='-D warnings'` catches broken links, not absent docs); there are zero doctests, and the crate README is 9 lines. (2) All seven examples render the same single `List` into an off-screen buffer once via a shared `support::render()`; the ones named after architectures are hollow — `tea.rs` defines an enum and never uses it; `component.rs` implements `Component<(), ()>` trivially and drops it. A prospective consumer learns nothing about composing widgets, feeding events, theming, or running a loop.

## Current state

- `Cargo.toml` `[workspace.lints.rust]` — only `unsafe_code = "forbid"` and `rust_2018_idioms`; no `missing_docs`.
- Examples (all of them): `buffer_only.rs`, `direct.rs`, `flux.rs`, `tea.rs`, `component.rs`, `crossterm_manual.rs`, `crossterm_managed.rs`. Representative content (verbatim):

```rust
// examples/tea.rs — entire file
mod support;
enum Message {
    Select,
}
fn main() {
    let _message = Message::Select;
    support::render();
}
```

```rust
// examples/component.rs — entire file
mod support;
use termrock::runtime::Component;
struct Screen;
impl Component<(), ()> for Screen {
    fn handle_event(&mut self, (): &()) -> Option<()> {
        Some(())
    }
}
fn main() {
    let _screen = Screen;
    support::render();
}
```

- `examples/support/mod.rs` — builds two `ListRow`s, renders one `List` into a `Buffer`, asserts selection. That's the entire shared body.
- `crates/termrock/Cargo.toml` declares `crossterm_manual`/`crossterm_managed` with `required-features = ["crossterm"]`; CI checks examples under both feature sets (`cargo check --workspace --examples --locked` and `--features crossterm`).
- `crates/termrock/README.md` — 9 lines; mentions the runtime module and points at repo docs.
- The crossterm `Session` (terminal lifecycle owner: raw mode, alt screen, mouse capture, bracketed paste, rollback on failed entry — per `COMPONENTS.md`) exists precisely for examples like this.
- Repo conventions: docs changes ship with the change; Conventional Commits + DCO.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Docs build (strict) | `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --no-deps --locked` | exit 0 |
| Doctests | `cargo test --doc --workspace --locked` | all pass |
| Example checks | `cargo check --workspace --examples --features crossterm --locked` | exit 0 |
| Run the demo | `cargo run -p termrock --example showcase --features crossterm` | interactive TUI opens/quits with `q` |
| Tests | `cargo test --workspace --all-features --locked` | all pass |

## Scope

**In scope**:
- `Cargo.toml` (workspace lints)
- Doc comments across `crates/termrock/src/` (backfill)
- `crates/termrock/examples/` (rewrite)
- `crates/termrock/README.md`
- Doctests on core types

**Out of scope**:
- `runtime` module docs beyond what exists — Plan 018 decides its fate first; do NOT write examples that use `Component`/`drive_frame` (they'd cement an abstraction under review). If `component.rs`/`flux.rs`/`tea.rs` names can't be honestly filled without runtime, DELETE those examples (forward-only: no hollow placeholders) and note it.
- The docs site content (`docs/content/`) — separate pipeline.

## Git workflow

- Directly on `main`; suggested commits: `docs(api): enforce missing_docs and backfill` then `docs(examples): replace stubs with runnable showcase`.

## Steps

### Step 1: Turn on `missing_docs` as warn, measure

Add to `[workspace.lints.rust]`: `missing_docs = "warn"`. Count: `cargo doc --workspace --all-features --no-deps --locked 2>&1 | grep -c "missing documentation"`.

**Verify**: build succeeds; record the count in your report.

### Step 2: Backfill

Document every flagged public item. Quality bar per item: one sentence saying what it IS plus (for non-obvious items) one sentence on when to use it. For widget structs, the doc names its State type and its story ID in the lookbook (grep `lib.rs` in the lookbook for story IDs). For enums, document variants that aren't self-evident. Match the existing voice — see `list.rs`/`toast.rs` doc comments for register (descriptive, terminal-domain-specific, no marketing).

Then flip `missing_docs = "warn"` → `"deny"`.

**Verify**: `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --no-deps --locked` → exit 0 with `deny(missing_docs)` active.

### Step 3: Doctests on the core types

Add compiling doctests (each becomes the canonical usage snippet) to: `Theme` (build default, override one role — post-008 API), `List` + `ListState` (construct post-013 idiom, feed a `KeyEvent`, match the `Outcome`), `Keymap` (define a binding table, dispatch a chord), `Toast` (builder chain), `osc::encode` (encode a pointer request; assert bytes). Five doctests minimum. They run under `cargo test --doc` which Plan 001 wired into CI.

**Verify**: `cargo test --doc --workspace --locked` → ≥5 doctests pass.

### Step 4: Replace the stub examples with one real showcase

Delete `buffer_only.rs`, `direct.rs`, `flux.rs`, `tea.rs`, `component.rs` and `support/` (they demonstrate nothing; forward-only says no placeholders). Keep/merge `crossterm_manual.rs` semantics into a new `examples/showcase.rs` (`required-features = ["crossterm"]`, registered in `Cargo.toml`; remove the deleted examples' `[[example]]` entries — note `buffer_only` etc. had none, only the crossterm pair are registered):

`showcase.rs` contents (~150 lines, fully commented as a tutorial):
1. Open a `crossterm::Session` (the managed path from `crossterm_managed.rs`).
2. Build `Theme::default()`; build a screen with `Tabs` (two tabs), a `List` (6 rows) in a `Panel`, a `StatusBar`, and a `HintBar` driven by a real `Keymap` binding table (`q` quit, arrows navigate, `Enter` activate, `t` toggle theme between phosphor and slate if Plan 010 landed — otherwise omit).
3. Event loop: read crossterm events, convert via the neutral `input::Event` (post-011), route keys through the keymap + `ListState::handle_key`, mouse through `hover`/`click`.
4. On activate: show a `Toast`.
5. Clean exit restores the terminal (Session handles it — say so in comments).

Keep `crossterm_managed.rs` if `showcase.rs` fully covers it — prefer deleting to duplicating (one great example beats two overlapping ones); keep `crossterm_manual.rs` only if it demonstrates the non-Session manual path meaningfully — otherwise fold a "manual lifecycle" comment into showcase and delete it too. Final state: ONE example (`showcase`), maybe two.

**Verify**: `cargo check --workspace --examples --features crossterm --locked` → exit 0; `cargo run -p termrock --example showcase --features crossterm` manually spot-run if a TTY is available (in CI/headless, the check suffices — note it in the report).

### Step 5: Crate README

Expand `crates/termrock/README.md` to ~40 lines: what it is (2 sentences), the pin-a-revision install snippet (copy from root README), a 15-line quick-start (borrowed from the `List` doctest), theming pointer (`Theme`, presets), the showcase example invocation, link to `MIGRATING.md` + the "Modern-first, pre-stable API" policy (quote its no-guarantees sentence so consumers see it at the crate level).

**Verify**: `cd docs && bun run build` → exit 0 (README isn't consumed by the catalog, but keep the gate green); `cargo package -p termrock --locked --allow-dirty` → exit 0 (README ships in the package).

## Test plan

- ≥5 doctests (Step 3) running in `cargo test --doc`.
- Example compile checks under both feature sets.
- No behavioral source changes — suite must stay green untouched.

## Done criteria

- [x] `missing_docs = "deny"` active workspace-wide; strict docs build green
- [x] ≥5 doctests pass
- [x] Stub examples deleted; `examples/showcase.rs` compiles under `--features crossterm`; `examples/` contains no file that only calls a shared render stub
- [x] Crate README ≥ 30 lines including quick-start and instability notice
- [x] `cargo test --workspace --all-features --locked` → all pass
- [x] `plans/README.md` status row updated

## STOP conditions

- The missing_docs count exceeds ~400 items — report the count and do the backfill in module-sized commits rather than one; if it exceeds what fits the session, land `warn` + partial backfill and mark the plan IN PROGRESS with the remaining module list.
- Writing the showcase reveals the post-011 event wiring is awkward enough to need API changes — do NOT change the API; report the friction verbatim (that report is a Plan-018 input).
- `Session` misbehaves in the example (terminal not restored on panic/exit) — report; that's a library bug discovery, not an example problem.

## Maintenance notes

- `deny(missing_docs)` now forces docs on every future public item at compile time — the review burden drops to doc *quality*.
- The showcase is the de-facto integration test of the whole consumer path; when Plans 018-021 land features, extend it rather than adding parallel demo files.
- The five doctests are canonical snippets — docs-site content should embed/mirror them rather than hand-writing drifting copies.
