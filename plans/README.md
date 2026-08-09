# Implementation plans

Historical plans **001–038** are complete (removed after verification).

## Design history

| Doc | Role |
|-----|------|
| [`docs/design/shadcn-tui-direction.md`](../docs/design/shadcn-tui-direction.md) | Landscape research (executed 0029–0030) |
| [`docs/design/architecture-foundation.md`](../docs/design/architecture-foundation.md) | Hybrid kernel + progressive capability (0031) |
| [`docs/design/shadcn-quality-roadmap.md`](../docs/design/shadcn-quality-roadmap.md) | Full R1–R8 recommendations |

## Open executable plans (do in order)

| Plan | Title | Priority | Status | Depends | Migration |
|------|-------|----------|--------|---------|-----------|
| **039** | Fail-safe ApprovalCard + VirtualGrid contracts | P0 | TODO | — | 0032 |
| **040** | Unified InteractionScene | P0 | TODO | 039 | 0033 |
| **041** | Variable-height streaming transcript engine | P1 | TODO | 040 | 0034 |
| **042** | Responsive workspace tree (not flat rect recipes) | P1 | TODO | 040–041 | 0035 |
| **043** | DesignTokens drive paint + phosphor quiet canvas | P1 | TODO | 040 | 0036 |
| **044** | Universal intents for all collections | P1 | TODO | 039–040 | 0037 |

## Follow-on plan IDs (not authored yet)

| ID | Title | After |
|----|-------|-------|
| 045 | Composed row / panel anatomy | 043 |
| 046 | Agent workbench flagship (scene + transcript + safe approval) | 039–041 |
| 047 | Source registry CLI spike (`termrock add`) | 044+ |
| 048 | Lookbook → Studio inspector (tokens, scene, capability) | 040–043 |
Status: `TODO` · `IN PROGRESS` · `DONE` · `BLOCKED` · `REJECTED`.
