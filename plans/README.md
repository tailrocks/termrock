# Implementation plans

Historical plans **001–038** are complete (removed after verification).

## Design history

| Doc | Role |
|-----|------|
| [`docs/design/shadcn-tui-direction.md`](../docs/design/shadcn-tui-direction.md) | Landscape research (executed 0029–0030) |
| [`docs/design/architecture-foundation.md`](../docs/design/architecture-foundation.md) | Hybrid kernel + progressive capability (0031) |
| [`docs/design/shadcn-quality-roadmap.md`](../docs/design/shadcn-quality-roadmap.md) | Full R1–R8 recommendations |
| [`docs/design/terminal-design-system.md`](../docs/design/terminal-design-system.md) | Full token + recipe system (DesignSystem) |
| [`docs/design/product-audit.md`](../docs/design/product-audit.md) | Product/architecture audit |
| [`docs/design/component-anatomy-spec.md`](../docs/design/component-anatomy-spec.md) | Component anatomy/behavior/state catalog |

## Open executable plans (do in order)

| Plan | Title | Priority | Effort | Status | Depends | Migration |
|------|-------|----------|--------|--------|---------|-----------|
| [039](039-safe-interaction-baseline.md) | Fail-safe ApprovalCard + VirtualGrid contracts | P0 | L | TODO | — | 0032 |
| [040](040-unified-interaction-scene.md) | Unified InteractionScene | P0 | L | TODO | 039 | 0033 |
| [041](041-variable-height-transcript-engine.md) | Variable-height streaming transcript | P1 | L | TODO | 040 | 0034 |
| [042](042-responsive-workspace-blocks.md) | Responsive workspace tree + patterns | P1 | L | TODO | 040–041 | 0035 |
| [043](043-token-driven-phosphor-system.md) | Token-driven quiet phosphor hierarchy | P1 | L | TODO | 040 | 0036 |
| [044](044-universal-intent-collections.md) | Universal intents for collections | P1 | M | TODO | 039–040 | 0037 |
| [045](045-composed-row-panel-anatomy.md) | Priority-aware row/panel anatomy | P2 | M | TODO | 041, 043 | 0038 |
| [046](046-agent-workbench-flagship.md) | Agent Workbench flagship | P2 | L | TODO | 039–045 | 0039 |
| [047](047-source-registry-cli-spike.md) | Safe source-registry CLI spike | P3 | L | TODO | 046 + approval | 0040* |
| [048](048-lookbook-studio-inspector.md) | Lookbook → executable Studio | P3 | L | TODO | 040, 043–046 | 0041 |
| [049](049-capability-aware-preview-host.md) | Capability-aware preview/media host | P3 | L | TODO | 042–043, 048 | 0042 |

`0040*` exists only if the registry spike ships a public CLI/schema. A failed
spike is REJECTED/BLOCKED with evidence, never misrepresented as a migration.

## Execution notes

Primary sequence: `039 → 040 → 041 → 042 → 043 → 044 → 045 → 046 → 047 → 048 → 049`.
Although 043/044 can begin earlier by dependency, “all in order” deliberately
keeps one green migration boundary at a time. Plan 047 requires explicit
maintainer approval for a workspace CLI.

Plans were reconciled to audit commit `5657c10` on 2026-08-09. The audited
checkout was `feat/experience-layer-shadcn-tui`, conflicting with the direct-
`main` rule. Executors must STOP until the operator reconciles the branch.
Advisor plans do not authorize switching, merging, committing, or pushing it.

## Selected findings

- ApprovalCard defaults to approval; narrow paint may hide selected action.
- VirtualGrid invents rows/hits, ignores disabled rows, drops IDs, mispaints
  ranges, and performs avoidable viewport × resident work.
- Focus, scene, overlays, Escape, Keymap, and hints are parallel truths; a
  non-dismissible top layer can expose a lower layer to Escape.
- StreamView is one-row-only/not ID-anchored; WorkSurface can escape its parent.
- Tokens do not universally paint; phosphor hierarchy is flat; row/panel
  anatomy cannot reduce gracefully.
- Static lookbook claims are not executable interaction/design evidence.
- ImageSurface is a placeholder/protocol hint, not a safe media lifecycle.
- Clipping can split graphemes; text key-kind/newline contracts diverge.

## Rejected boundaries

- CSS/retained DOM; product-branded runtime widgets; TermRock-owned model,
  process, permission, file/network, persistence, or secret effects.
- Mandatory heavy parsers/media decoders.
- Silent source overwrite, registry template execution, or remote marketplace
  work in the CLI spike.
- Backward-compatibility facades; each break gets one canonical replacement.

Status: `TODO` · `IN PROGRESS` · `DONE` · `BLOCKED` · `REJECTED`.
