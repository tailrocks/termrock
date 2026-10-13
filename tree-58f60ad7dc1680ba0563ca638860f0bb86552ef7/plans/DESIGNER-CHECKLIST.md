# The designer micro-detail checklist — 49 checks, each with an owner

Compiled 2026-08-14 from the repo's own design SoTs (per-item citations in
the micro-detail research digest; terminal-physics translations included).
Every item now has an owning plan. Use this as the review lens for any
future component work: a PR that touches a widget answers every applicable
row.

## A. States

| # | Check | Owner |
|---|-------|-------|
| S1 | All 9 states render distinctly (matrix provable from buffers) | 021 (gate `state_matrix_distinct`) |
| S2 | Hover = HoverTint wash on the element rect, never underline/recolor | 002/004/005 |
| S3 | Hover-revealed actions also reveal on focus/key | 021 Step 3 |
| S4 | Focus = border role swap; one bright border per scene | 004/006/013 |
| S5 | Underline only on hovered links + content | 001/005/015 |
| S6 | Pressed/armed frame distinct from focus (60-120ms budget) | 021 Step 2 |
| S7 | Disabled: non-color cue, omitted-not-greyed actions, skips focus cycle | 008/016/021 |
| S8 | Selected survives unfocus + mono; glyph not overloaded | 003/004/006 |
| S9 | Read-only ≠ disabled | 021 Step 4 |

## B. Spacing

| # | Check | Owner |
|---|-------|-------|
| S10 | Text never adjacent to a border glyph, at every width | 022 Steps 1-2 (gate) |
| S11 | Insets from the Density table, not literals | 022 Step 6 |
| S12 | Stack/Inline/Grid gaps from SpacingScale | 022 Step 6 |
| S13 | Blank rows are intentional Canvas bands | 015/022 |
| S14 | Body column ~80 cols centered, Canvas side-bands | 017 (law) + 019 showcase proof |
| S15 | Composer docks ≥2 blank rows below last message | 019 (showcase composes it; widget rule in law) |
| S16 | Overlay bodies: uniform 1-cell pad + header rule | 009/022 |

## C. Alignment & truncation

| # | Check | Owner |
|---|-------|-------|
| S17 | Fixed-width leading slots; no ragged state shifts | 009/013/022 |
| S18 | Numerics right-aligned + tone-stepped | 012/022 (CellAlignment shared) |
| S19 | Grapheme-safe truncation everywhere | 022 Steps 1,3 |
| S20 | One ASCII-gated ellipsis mark | 020/022 |
| S21 | Ids middle-truncate; timestamps relative + faint | 017/022 |
| S22 | Truncation honest: `+N more`, counts, chevrons | 022 Steps 2-3 |
| S23 | Narrow contraction drops by priority before clipping | 012/013/016 |
| S24 | Usable at ≤20×5 or documented LineMode | 022 Step 6 |

## D. Motion

| # | Check | Owner |
|---|-------|-------|
| S25 | Durations in the binding table; out faster than in | 014 |
| S26 | No Elastic/Bounce/Back easings | 014 |
| S27 | Tweens retarget, never restart | 014 |
| S28 | Gutter snaps; tabs cross-fade; borders blend fg only | 014 Step 3b |
| S29 | Motion = information; reduced = static, zero loss | 014 |
| S30 | Flicker-free: diff preserved, idle 0fps, sync frames | 014 Step 1 |
| S31 | Ambient loops wall-clock phased, ≤1/3 amplitude | 014 |

## E. Affordances

| # | Check | Owner |
|---|-------|-------|
| S32 | Every mouse action has a key path; hit rects match paint | 021 Steps 1,4 |
| S33 | Form value hit-region on the right row | 008 |
| S34 | One glyph per job (pointer/marker/match/selection) | 003/005/006 |
| S35 | Removed detail keeps a visible one-keypress path | 017 |
| S36 | Disclosure/overflow affordances ASCII-gated | 007/009/022 |
| S37 | One recipe per family (chip/kbd/row/overlay) | 006/009/015 |

## F. Feedback & microcopy

| # | Check | Owner |
|---|-------|-------|
| S38 | Empty ≠ loading ≠ error, each with guidance + action | 009/013/022 |
| S39 | Errors name cause + one recovery; no generic strings | 020 (voice) |
| S40 | Spinner + verb + elapsed + delayed interrupt hint | 007/014 (hint delay in 014 Step 4 spinners row) |
| S41 | Optimistic render + first-token masking + token batching | 014 (pipeline) + 019 (flow proof) |
| S42 | Toast TTL 4s, pause on unfocus, esc/× dismiss | 021 Step 6 |
| S43 | One case system (sentence case; no caps-as-structure) | 020 |
| S44 | One key notation + hint punctuation | 020 |

## G. Degradation

| # | Check | Owner |
|---|-------|-------|
| S45 | Every state survives NO_COLOR/mono | 003 |
| S46 | Every glyph: catalog + ASCII + 1-col width test | 003/008/013 |
| S47 | 256/ANSI16 keep ladder + semantics | 003 |
| S48 | Contrast floor per role pair, every preset | 017 |
| S49 | Resize reflow safety incl. overlays | 022 Step 6 |

N/A in terminal physics (do not re-litigate): sub-cell radii, real
shadows, per-cell scale/alpha, scroll momentum, cursor-shape changes,
hover-only patterns, 60fps easing fidelity, continuous gradients, native
select/find under alt-screen (rebuilt equivalents instead) — rationale in
`docs/design/web-premium-tui-law.md` §9.
