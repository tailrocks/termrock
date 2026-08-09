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
| [`docs/design/component-prompt-library.md`](../docs/design/component-prompt-library.md) | **164 agent prompts** (global contract + per-component tasks) |
| [`docs/design/source-owned-registry.md`](../docs/design/source-owned-registry.md) | **Source-owned registry + CLI architecture** (shadcn-class distribution) |
| [`docs/design/termrock-studio.md`](../docs/design/termrock-studio.md) | **TermRock Studio** (Storybook/DevTools-class lookbook evolution) |
| [`docs/design/termrock-agent.md`](../docs/design/termrock-agent.md) | **`@termrock/agent`** agent component collection + AgentWorkbench |
| [`docs/design/prompt-composer.md`](../docs/design/prompt-composer.md) | **PromptComposer** flagship agent input surface |
| [`docs/design/permission-trust.md`](../docs/design/permission-trust.md) | **Permission & trust** surface (queue, provenance, stale safety) |
| [`docs/design/data-presentation.md`](../docs/design/data-presentation.md) | **Data presentation** (DataTable, virtualization kits, 1M-row targets) |
| [`docs/design/component-quality-standard.md`](../docs/design/component-quality-standard.md) | **Component quality standard** (contracts, lints, CI evidence) |
| [`docs/design/streaming-performance.md`](../docs/design/streaming-performance.md) | **Streaming / large-data performance** (budgets, coalesce, follow) |
| [`docs/design/terminal-capability-architecture.md`](../docs/design/terminal-capability-architecture.md) | **Terminal capabilities** (profiles, doctor, graceful degrade) |
| [`docs/design/component-documentation-standard.md`](../docs/design/component-documentation-standard.md) | **Component docs standard** (shadcn-depth handbook template) |
| [`docs/design/competitive-tui-research.md`](../docs/design/competitive-tui-research.md) | **Competitive TUI research** (matrix + 10 exceed opportunities) |
| [`docs/design/showcase-workbench.md`](../docs/design/showcase-workbench.md) | **Flagship showcase** AI/dev workbench (IA, mockups, dogfood law) |

### Using the component prompt library

1. Paste the **Global implementation contract** from the library (top of the file).
2. Paste **one** numbered component prompt (or a grouped pair, e.g. Tag+Chip).
3. Prefer the library’s **Recommended build order**: foundation wave → representative components → developer tools → AI signature → application blocks.
4. Plans **050–053** are the executable wave wrappers; the prompt library is the per-component task source.

## Open executable plans (do in order)

| Plan | Title | Priority | Effort | Status | Depends | Migration |
|------|-------|----------|--------|--------|---------|-----------|
| [039](039-safe-interaction-baseline.md) | Fail-safe ApprovalCard + VirtualGrid contracts | P0 | L | DONE | — | 0032–0033 |
| [040](040-unified-interaction-scene.md) | Unified InteractionScene | P0 | L | DONE | 039 | 0034 |
| [041](041-variable-height-transcript-engine.md) | Variable-height streaming transcript | P1 | L | DONE | 040 | 0035 |
| [042](042-responsive-workspace-blocks.md) | Responsive workspace tree + patterns | P1 | L | DONE | 040–041 | 0036 |
| [043](043-token-driven-phosphor-system.md) | Token-driven quiet phosphor hierarchy | P1 | L | DONE | 040 | 0037 |
| [044](044-universal-intent-collections.md) | Universal intents for collections | P1 | M | DONE | 039–040 | 0038 |
| [045](045-composed-row-panel-anatomy.md) | Priority-aware row/panel anatomy | P2 | M | DONE | 041, 043 | 0039 |
| [046](046-agent-workbench-flagship.md) | Agent Workbench flagship | P2 | L | DONE | 039–045 | 0040 |
| [047](047-source-registry-cli-spike.md) | Safe source-registry CLI spike | P3 | L | DONE | 046 + approval | 0055 |
| [048](048-lookbook-studio-inspector.md) | Lookbook → executable Studio | P3 | L | DONE | 040, 043–046 | 0041 |
| [049](049-capability-aware-preview-host.md) | Capability-aware preview/media host | P3 | L | DONE | 042–043, 048 | 0042 |
| [050](050-terminal-native-primitives.md) | Primitives, content, and feedback | P2 | L | DONE | 043, 045, 048–049 | 0052 |
| [051](051-controls-navigation-overlays.md) | Controls, navigation, and overlays | P2 | L | DONE | 040, 043–045, 048, 050 | 0053 |
| [052](052-data-review-surfaces.md) | Scalable data/log/review surfaces | P2 | L | DONE | 041, 043–045, 048, 050–051 | 0054 |
| [053](053-application-block-collection.md) | Source-ownable application blocks | P2 | L | DONE | 042, 046–047, 049–052 | 0056 |

Plan 047 is DONE (offline `termrock-cli` + fixtures; migration `0055`).
Historical plan bodies may still say `0040`; live tip ids `0052`–`0056` are authoritative.

## Execution notes

Primary sequence: `039 → 040 → 041 → 042 → 043 → 044 → 045 → 046 → 047 → 048 → 049 → 050 → 051 → 052 → 053`.
Although 043/044 can begin earlier by dependency, “all in order” deliberately
keeps one green migration boundary at a time. Plan 047 requires explicit
maintainer approval for a workspace CLI.

Plans were reconciled to audit commit `16b0ee8` on 2026-08-09. The audited
checkout was `feat/experience-layer-shadcn-tui`, conflicting with the direct-
`main` rule. Executors must STOP until the operator reconciles the branch.
Advisor plans do not authorize switching, merging, committing, or pushing it.
The bootstrap gate is also red: fmt check fails and direct clippy reports 26
errors plus one warning. Workspace tests remain green (388 tests, 18 suites).
Plan 039 Step 0 owns restoring the green baseline.

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
- Binding component anatomy exposes missing Button/control/menu/overlay/data
  families and proves current application patterns are geometry, not blocks.

## Rejected boundaries

- CSS/retained DOM; product-branded runtime widgets; TermRock-owned model,
  process, permission, file/network, persistence, or secret effects.
- Mandatory heavy parsers/media decoders.
- Silent source overwrite, registry template execution, or remote marketplace
  work in the CLI spike.
- Backward-compatibility facades; each break gets one canonical replacement.

Status: `TODO` · `IN PROGRESS` · `DONE` · `BLOCKED` · `REJECTED`.
