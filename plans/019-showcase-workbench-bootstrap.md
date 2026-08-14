# Plan 019: Bootstrap `termrock-showcase` — the real-work application that proves the library

> **Executor instructions**: This plan EXECUTES an existing SoT —
> `docs/design/showcase-workbench.md` (1787 lines; Draft design SoT). Read
> it IN FULL before starting; its §"PR Plan" S0–S9 + gap gates G1–G5d are
> the step list, its SKD-1…SKD-18 decisions are binding, its success
> criteria are the done criteria. This plan file adds only sequencing,
> repo-law wiring, and verification — it does not restate the spec.
> Update `plans/README.md` when done (or per-stage).
>
> **Drift check (run first)**: confirm `crates/termrock-showcase` still does
> not exist and `docs/design/showcase-api-gaps.md` still lists GAP-WB-2,
> GAP-MT-1, GAP-MD-1, GAP-TC-1, GAP-TR-1, GAP-DF-1, GAP-SUB-1, GAP-ACT-1,
> GAP-CM-1, GAP-REC-1 as open. If the crate exists, switch to reconcile
> mode: diff its state against S0-S9 and continue from the first unmet gate.

## Status

- **Priority**: P2 (after the visual suite 001-017 — the showcase must
  demo the REDESIGNED look, not today's)
- **Effort**: XL (multi-stage; each S-stage is a commit-sized unit)
- **Risk**: MED (new crate; public-API-only constraint turns gaps into
  library work — that is the point)
- **Depends on**: plans/002-017 substantially DONE (the wow depends on the
  redesign); plans/016 (promoted primitives available)
- **Category**: direction / dogfood
- **Planned at**: commit `d09bd2fe`, 2026-08-14

## Why this matters

The goal's "WOW effect in a real-work TUI application" has a fully specced
but unimplemented answer in-repo: `termrock-showcase`, a standalone binary
(mock agent runtime, public APIs only) whose triple purpose is Demo (<2 min
to wow), Dogfood (every weakness found = a missing public primitive → fix
the library), and Recording corpus (headless replay as quality gates). The
spec explicitly rejected "lookbook stories only" as the end-to-end proof
(alternative F), so plan 018's docs scenes complement, not replace, this.

## Execution frame (the spec owns the detail)

1. **S0 review gate first**: the SoT is `Status: Draft — implement after S0
   review + G1`. Stage 0 = operator sign-off on the spec's SKD decisions
   (10 min read of §Key decisions + §Success criteria). Record sign-off in
   this plan's README row note; if any SKD is overturned, amend the SoT doc
   first (same commit).
2. **Follow S0–S9 in order**, interleaving gap gates G1–G5d exactly as the
   spec's dependency graph orders them. Each stage: one commit, Conventional
   Commits, `git commit -s`, `mise run check` green before commit.
3. **Workspace wiring** (S0): add `crates/termrock-showcase` to workspace
   members; binary target; NOT published (add to release exclusions;
   check `PUBLISH-AUDIT.md`/release tooling for the exclusion mechanism and
   follow it).
4. **Gap protocol** (binding, from the SoT): any missing capability found
   while building = file the gap in `showcase-api-gaps.md`, implement the
   PUBLIC primitive in `termrock` (with story/contract/migration per repo
   law), then consume it. FORBIDDEN: `pub(crate)` reach-ins, copied private
   widgets, local Permission/Approval forks, silent auto-Allow.
5. **The redesigned look is the contract**: the showcase renders through
   `DesignSystem::phosphor()` post-plans-002+; its screens must pass the
   same design gates (accent budget, one focused border, info budget from
   plan 017 — wire `design_gate`-style assertions over showcase scenes as
   they land).
6. **Recordings** (S8/G-REC): the 12 `rec/*` scenario recordings become CI
   acceptance tests per the spec; wire into `mise run gate` behind a
   feature/cfg so the trunk gate stays fast (document the knob).

## Verification per stage

| Stage cluster | Verify |
|---|---|
| S0 workspace + shell | `cargo run -p termrock-showcase` boots to the workbench frame; `mise run check` green |
| S1-S3 transcript/composer/runtime | MVP script: hello stream + tool card renders; Esc peels one layer |
| S4-S5 permission/plan/diff | High-risk permission defaults to Deny; Enter ≠ Allow; diff hunk nav |
| S6-S7 sessions/narrow | 40×16 keeps submit + read usable |
| S8 recordings | `rec/*` replays pass headless |
| S9 polish | <2 min demo script walkthrough documented in the crate README |

## Done criteria (= the SoT's success criteria, mechanically checked)

- [ ] MVP list at `showcase-workbench.md` §Success criteria all pass.
- [ ] `rg -n "pub(crate)|ApprovalCard|PromptBox" crates/termrock-showcase/src` → 0.
- [ ] Zero private-chrome workarounds: showcase imports only `termrock::{widgets,patterns,style,layout,runtime,input,interaction}` public items.
- [ ] Every gap opened during the build is either closed (library primitive shipped, migration filed) or logged open in `showcase-api-gaps.md` with a priority.
- [ ] Showcase scenes pass accent-budget / one-border / info-budget assertions.
- [ ] `mise run gate` green; README rows updated per stage.

## STOP conditions

- S0 sign-off overturns SKD-1 (separate crate) or SKD-2 (mock runtime) —
  stop; that's a different architecture.
- A gap requires a kernel-level capability (new event source, scene-layer
  API) larger than one migration — file it and stop that stage; the
  library plan comes first.
- Recording format (GAP-REC-1) blocks S8 — ship S0-S7 and leave S8 staged;
  don't invent an ad-hoc format.

## Maintenance notes

- The showcase is the standing dogfood: every future library redesign runs
  its demo script as the human acceptance test.
- Registry future: the spec anticipates adopting registry blocks once
  distribution ships — revisit after `source-owned-registry.md` lands.
