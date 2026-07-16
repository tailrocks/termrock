# Plan 036: Replace the placeholder rustdoc stubs that launder the docs gate; guard against them in CI

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat 5c4758b..HEAD -- crates/termrock/src/`
> Concurrent executors are active — re-count the stubs at start (Step 1); the
> counts below are the planning-time baseline, not a criterion.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: LOW (docs-only; zero behavior change)
- **Depends on**: none (014 landed; this fixes its quality gap)
- **Category**: docs
- **Planned at**: commit `5c4758b`, 2026-07-16

## Why this matters

Plan 014 turned on `missing_docs = "deny"` — and the backfill satisfied the lint with generated placeholders: at planning time, **142** occurrences of `/// Documentation for \`item\`.` and **189** of `/// Performs the \`<name>\` operation.` across `crates/termrock/src`, plus template-mangled lines like `/// The \`Determinate { fraction\` value.` and `/// Selects the \`Paste\` behavior.` on enum variants. The gate is green while hundreds of public items teach the reader nothing — worse than absent docs, because the gate now claims coverage. For a reuse-focused component library, rustdoc IS the product surface. This plan replaces every stub with a real sentence and adds a CI tripwire so placeholder phrasing can never satisfy the gate again.

## Current state

- Counting commands (run these to get the live inventory):

```
grep -rn "Documentation for \`" crates/termrock/src --include="*.rs" | wc -l    # 142 at planning
grep -rn "Performs the \`" crates/termrock/src --include="*.rs" | wc -l          # 189 at planning
grep -rn "Selects the \`" crates/termrock/src --include="*.rs" | wc -l           # variant stubs
grep -rn "The \`[A-Za-z]* {" crates/termrock/src --include="*.rs"                # mangled struct-variant stubs
grep -rn "Creates a new value with canonical defaults\|Data carried by \`\|Available \`" crates/termrock/src --include="*.rs"   # more template phrasings
```

- Representative offenders (verbatim, current):
  - `widgets/list.rs` `ListState` — every field: `/// Documentation for \`item\`.` on `selected`, `hovered`, `focused`, `offset`, `viewport_height`, `regions`, `selection`, `check_regions`.
  - `widgets/progress.rs:12-24` — `/// Selects the \`Determinate\` behavior.` + mangled `/// The \`Determinate { fraction\` value.` (and the `Indeterminate { tick` twin). Note the fields UNDER them have real docs ("Completed fraction; rendering clamps…") — the stub layer sits on top.
  - `widgets/selection.rs` — `/// Performs the \`checked\` operation.`, `/// Data carried by \`Selection\`.`, `/// Creates a new value with canonical defaults.` — though `toggle` has a real doc ("Toggle a stable identity, preserving check order.") — match that register.
  - `input/event.rs` — `/// Selects the \`Key\` behavior.` etc. on every `Event` variant; `/// Documentation for \`item\`.` on `Resize` fields.
- The GOOD register to match (real docs already in the codebase): `selection.rs`'s `toggle` doc; `progress.rs`'s `fraction` field doc; the pre-wave docs on `Toast`/`Panel`/`sanitize_terminal_title`. One sentence: what it IS; second sentence when non-obvious: when to use it.
- Also in scope (from the round-3 audit): `crates/termrock/README.md` documents `runtime::drive_frame`/`Component`/`View`/`Subscription`/`UpdateResult` as settled "shared update-loop contracts" while plan 018 is chartered to decide that module's fate — soften with a provisional note.
- Doc gates: `RUSTDOCFLAGS='-D warnings' cargo doc` and `missing_docs = "deny"` already in CI; `mise run check` runs the suite.
- Repo conventions: docs ship with the change; Conventional Commits + DCO; trunk-only.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Stub inventory | the grep set above | counts → 0 by the end |
| Docs build | `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --no-deps --locked` | exit 0 |
| Fast check | `mise run check` | exit 0 |
| Doctests | `cargo test --doc --workspace --locked` | all pass |

## Scope

**In scope**:
- Doc comments across `crates/termrock/src/**` (and `crates/termrock-lookbook/src/**` if the stub greps hit it — check)
- `crates/termrock/README.md` (runtime paragraph note only)
- The CI tripwire (Step 3): `mise.toml` gate task or a tiny script + `rust.yml` step

**Out of scope**:
- ANY code change. If a doc you're writing reveals a symbol that shouldn't exist or behaves surprisingly, note it in the report — don't touch the code.
- Rewriting real (non-stub) docs, even improvable ones.
- New doctests beyond what exists (014 delivered those).

## Git workflow

- Directly on `main`; module-batched commits: `docs(widgets): replace placeholder rustdoc with real descriptions`, `docs(input): …`, etc.; final `ci: fail on placeholder doc phrasings`.

## Steps

### Step 1: Inventory and batch

Run the grep set; produce the per-file list. Batch by module (widgets, input, keymap, style, scroll, layout, interaction, osc, text, runtime, crossterm, ansi_text; lookbook if hit). Expected total ≈ 350–400 items.

### Step 2: Write real docs, module by module

Rules per item: name what the item IS in domain terms (one sentence), add "when to use" only where non-obvious; enum variants describe the CASE not "selects the behavior"; struct fields describe the VALUE and its units/invariants ("Zero-based viewport top offset in rows"); never restate the signature; match the register of the exemplar docs named above. Delete the mangled double-lines on `Progress` variants (the real field docs beneath already carry the content — the variant line should say e.g. "A known-fraction progress bar."). Commit per batch; `mise run check` between batches.

**Verify per batch**: greps for the batch's files → 0 stubs; `cargo doc` strict → exit 0.

### Step 3: CI tripwire

Add to the `gate`/`check` task (or a dedicated `rust.yml` step) a guard that fails on the stub phrasings:

```bash
! grep -rEn "Documentation for \`|Performs the \`[a-z_]+\` operation|Selects the \`[A-Za-z]+\` behavior|Creates a new value with canonical defaults|Data carried by \`|Available \`[A-Za-z]+\` choices" crates/ --include="*.rs"
```

(Tune the patterns to exactly the generated phrasings found in Step 1; anchor tightly enough that legitimate prose can't false-positive — check the final pattern against the whole tree before wiring.)

**Verify**: the guard passes on the cleaned tree; temporarily add one stub line locally → guard fails → remove.

### Step 4: README runtime note

In `crates/termrock/README.md`, amend the runtime paragraph with one sentence: "The `runtime` contracts are provisional and under active design review; expect this surface to change (see `plans/018`)." Keep the rest.

**Verify**: `cargo package -p termrock --locked --allow-dirty` → exit 0 (README ships).

## Test plan

No behavior tests. Gates: stub greps zero, strict docs build, doctests still pass, tripwire bite-proof demonstrated.

## Done criteria

- [ ] All stub-phrase greps return 0 across `crates/`
- [ ] `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --no-deps --locked` → exit 0
- [ ] CI tripwire wired and demonstrated to bite
- [ ] README runtime paragraph carries the provisional note
- [ ] `mise run gate` → exit 0
- [ ] `plans/README.md` status row updated

## STOP conditions

- The stub count is wildly different from the baseline (concurrent executor already sweeping) — check `git log --oneline -10` for a docs sweep commit; if one exists, reconcile scope with what remains instead of re-writing.
- A public item cannot be honestly documented because its purpose is unclear from code + tests — list it in the report (that's an API-design finding), write the best-effort doc, don't guess semantics into existence.

## Maintenance notes

- The tripwire makes `missing_docs = deny` mean what it claims. If a future codegen/scaffold tool emits new placeholder phrasings, add them to the pattern.
- Plan 028's per-component pages source usage snippets from these docs — quality here compounds there.
