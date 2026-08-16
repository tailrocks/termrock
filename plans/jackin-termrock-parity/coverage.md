# Coverage Ledger — jackin-termrock-parity

Item: roadmap/jackin-termrock-parity/README.md, working tree (uncommitted;
created and finalized 2026-08-16), ingested 2026-08-16.
Override: none — item is READY.

## Screens

Headless by explicit item declaration (§Screens): no screens exist; all
capabilities route through flows W1/W2 or committed artifacts. No `S#` rows.

## Capabilities

| ID | Capability | Item anchor | Spec | Plans | Status |
|----|------------|-------------|------|-------|--------|
| F1 | Verify all functionality jackin needs exists in current termrock | §Capabilities/1 | spec/parity-inventory.md | 005 | covered |
| F2 | List all components from the jackin project | §Capabilities/2 | spec/parity-inventory.md | 005 | covered |
| F3 | Deep per-component design comparison, one subagent per verification | §Capabilities/3 | spec/comparison-verdicts.md | 008 | covered |
| F4 | Jackin-era design kept, current improvements merged on top, never discarded | §Capabilities/4, §Intent | spec/comparison-verdicts.md | 009 | covered |
| F5 | PNG baselines: jackin-used subset, all states, phosphor, pure-Rust pipeline, plain git | §Capabilities/5 | spec/render-pipeline.md, spec/baselines.md | 001, 002, 004 | covered |
| F6 | CI regenerates affected renders per PR; bless-required verification | §Capabilities/6 | spec/ci-gate.md | 003 | covered |
| F7 | Classify every jackin custom component; promote generic gaps into widgets | §Capabilities/7 | spec/parity-inventory.md | 006, 010 | covered |
| F8 | Per-widget comparison reports under roadmap/…/comparisons/ | §Capabilities/8 | spec/comparison-verdicts.md | 008 | covered |

## Flows

| ID | Flow | Screens touched | Spec | Plans | Status |
|----|------|-----------------|------|-------|--------|
| W1 | PR design-verification (regenerate → pixel-compare → bless-or-fail → review), failure points a–c | headless | spec/ci-gate.md | 003 | covered |
| W2 | Per-component verdict (side-harness old + HEAD renders → comparison doc → batch verdicts → merge/restore/accept), failure points a–b | headless | spec/comparison-verdicts.md | 007, 008, 009 | covered |

## Must-not anchors

| ID | Statement | Reason | Registry |
|----|-----------|--------|----------|
| N1 | No unreviewed visual divergence; every difference restored or verdict-accepted | nothing drifts silently | spec/README.md |
| N2 | No git-LFS for baselines | pointer-only PR diffs defeat review | spec/README.md |
| N3 | No PNG byte-equality CI gate; decoded-pixel equality at zero tolerance | encoder churn rewrites bytes without pixel change | spec/README.md |

## Quality bar

| ID | Statement anchor | Spec scenario(s) | Status |
|----|------------------|------------------|--------|
| B1 | §Quality bar/1 — every subset widget restored, merged, or accepted; zero unreviewed diffs, zero lost improvements | comparison-verdicts.md: "Merge keeps the improvement", "No application without a recorded verdict" | covered (009) |
| B2 | §Quality bar/2 — real shaping + rasterization of the true cell grid | render-pipeline.md: "Panel story renders", "Italic survives", "Double render identical" | covered (001) |
| B3 | §Quality bar/3 — every subset widget rendered in all its states | baselines.md: "Every subset story has a baseline", "TextInput focused story exists" | covered (002, 004) |
| B4 | §Quality bar/4 — CI makes look-and-feel unbreakable-by-accident | ci-gate.md: "Pixel drift fails with bless instruction", "Nondeterminism is not blessable" | covered (003) |

## Decisions (constraints)

| ID | Decision | Dated | Constrains |
|----|----------|-------|------------|
| D1 | Design conflicts resolved per-component | 2026-08-16 | W2, F3, F4 |
| D2 | Termrock-side scope only; jackin migration separate | 2026-08-16 | all slicing |
| D3 | Comparison baseline = old-rev (5ff94ee) per-widget renders | 2026-08-16 | W2, F5 |
| D4 | PNG coverage = jackin-used subset only (16 families + chrome) | 2026-08-16 | F5, B3 |
| D5 | CI gate bless-required | 2026-08-16 | W1, F6 |
| D6 | Phosphor theme only for baselines | 2026-08-16 | F5 |
| D7 | Classify all jackin customs; promote generic | 2026-08-16 | F7 |
| D8 | Verdicts via comparison docs + dated Decisions in item | 2026-08-16 | W2, F8 |
| D9 | Pipeline = pure-Rust rasterizer (vendored font + swash-class shaping + tiny-skia-class raster) | 2026-08-16 | F5, F6, B2 |
| D10 | Old-rev capture via side harness against public constructors | 2026-08-16 | W2 |
| D11 | Per-component verdicts are the visual authority | 2026-08-16 | N1, B1 |
| D12 | Verdicts merge current improvements onto jackin-era base; merge is the expected default | 2026-08-16 | W2, F4, B1 |

## External references & integrations

| ID | Reference | Kind | Research topics |
|----|-----------|------|-----------------|
| R1 | /Users/donbeave/Projects/tailrocks/jackin-project/jackin | source repo (read-only evidence) | tui-png-baselines ch. 03 facts; item §References facts |
| R2 | termrock current repo (this repo) | target | tui-png-baselines ch. 03 |
| R3 | ~~libghostty~~ | dropped (ruled out 2026-08-16) | tui-png-baselines ch. 01 |
| R4 | Plain-git PNG storage + GitHub PR image diffs | platform | tui-png-baselines ch. 04 |
| R5 | CI: generated ci.yml → tailrocks/velnor-actions@d6ebc786; docs.yml standalone precedent | integration | tui-png-baselines ch. 04; Q2 |
| R6 | research/tui-png-baselines (6 chapters, vetted 2026-08-16) | research | — |

## Assumptions

| ID | Assumption | Why safe | Falsified by | Status |
|----|------------|----------|---------------|--------|
| A1 | `png` crate emits deterministic bytes at a fixed version with fixed options | no time/random inputs documented; gate compares pixels (N3), so byte drift cannot break CI | double-encode diff in the determinism self-test failing | holds |
| A2 | Old rev 5ff94ee keeps building unmodified with today's toolchain | built clean 2026-08-16 (ch. 03 Q4, exit 0) | side-harness build failure against the pin | holds |
| A3 | macOS-blessed PNGs match Linux CI renders (cross-OS bit-identity of the pure-Rust stack) | zero OS-text-stack inputs by construction (ch. 04 §1); cross-arch identity measured (ch. 06 §4); only libm/allocator axis untested | first Linux CI run diffing a macOS-blessed baseline; fallback = bless in a pinned Linux container or CI-side bless artifact | holds |

## Research questions

| ID | Question | Research topic | Status |
|----|----------|----------------|--------|
| Q1 | Cross-arch bit-identity of pure-Rust raster stack (swash/tiny-skia class) | tui-png-baselines ch. 06 §4 — measured bit-identical (aarch64 vs x86_64/Rosetta); residual cross-OS axis carried as A3 | closed |
| Q2 | velnor-actions ci-code.yml extensibility / standalone-workflow placement for the PNG job | tui-png-baselines ch. 05 — gate rides workspace nextest via `mise run ci`/`test` (goldens precedent); no workflow change needed | closed |
| Q3 | ~~resvg SVG→PNG byte-identity~~ | — | dropped (direction A chosen) |
