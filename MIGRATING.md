# TermRock migration index

TermRock optimizes for the best current API and architecture, not backward
compatibility. Consumers pin reviewed full Git revisions and migrate forward
without compatibility shims or parallel legacy paths. TermRock keeps executor,
output, validation, wording, and application models consumer-owned unless a
migration explicitly changes that boundary.

Migration versions correspond to immutable Git tags from `v0.7.0` onward.
Release-boundary rules and tag ownership are documented in
[`RELEASING.md`](RELEASING.md).

Apply every migration after the consumer's pinned version in numeric order:

| Sequence | Version | Migration |
|---:|---|---|
| 0001 | `v0.7.0` | [Canonical namespaces](migrations/0001-v0.7.0-canonical-namespaces.md) |
| 0002 | `v0.8.0` | [Canonical widget contracts](migrations/0002-v0.8.0-canonical-widget-contracts.md) |
| 0003 | `v0.9.0` | [Styled tab glyphs](migrations/0003-v0.9.0-styled-tab-glyphs.md) |
| 0004 | `v0.9.0` | [Typed OSC requests](migrations/0004-v0.9.0-typed-osc-requests.md) |
| 0005 | `v0.9.0` | [Unknown key handling](migrations/0005-v0.9.0-unknown-key-handling.md) |
| 0006 | `v0.9.0` | [Unified key vocabulary](migrations/0006-v0.9.0-unified-key-vocabulary.md) |
| 0007 | `v0.9.0` | [Constructible theme](migrations/0007-v0.9.0-constructible-theme.md) |
| 0008 | `v0.9.0` | [Semantic theme palette](migrations/0008-v0.9.0-semantic-theme-palette.md) |
| 0009 | `v0.9.0` | [Neutral event contract](migrations/0009-v0.9.0-neutral-event-contract.md) |
| 0010 | `v0.9.0` | [Canonical module homes](migrations/0010-v0.9.0-canonical-module-homes.md) |
| 0011 | `v0.10.0` | [Trailing metadata cells](migrations/0011-v0.10.0-trailing-metadata-cells.md) |
| 0012 | `v0.10.0` | [Widget construction and growth](migrations/0012-v0.10.0-widget-construction-and-growth.md) |
| 0013 | `v0.10.0` | [Content measurement revisions](migrations/0013-v0.10.0-content-measurement-revisions.md) |
| 0014 | `v0.10.0` | [Scroll and hover unification](migrations/0014-v0.10.0-scroll-and-hover-unification.md) |
| 0015 | `v0.10.0` | [Independent terminal session options](migrations/0015-v0.10.0-independent-session-options.md) |
| 0016 | `v0.11.0` | [Ordinary vs strong text and Viewport emphasis](migrations/0016-v0.11.0-text-strong-and-viewport-emphasis.md) |
| 0017 | `v0.11.0` | [First-class scrollable block helpers](migrations/0017-v0.11.0-scrollable-block-helpers.md) |
| 0018 | `v0.11.0` | [Theme-explicit scroll and typed dialog input](migrations/0018-v0.11.0-theme-explicit-scroll.md) |
| 0019 | `v0.11.0` | [Bounded LogPane scrollback](migrations/0019-v0.11.0-bounded-log-pane-scrollback.md) |
| 0020 | `v0.11.0` | [Explicit LogPane oldest navigation](migrations/0020-v0.11.0-log-pane-oldest-navigation.md) |
| 0021 | `v0.11.0` | [Responsive Progress percentage](migrations/0021-v0.11.0-responsive-progress-percentage.md) |
| 0022 | `v0.11.0` | [Paste payloads in neutral events](migrations/0022-v0.11.0-paste-payload.md) |
| 0023 | `v0.11.0` | [List multi-select contract alignment](migrations/0023-v0.11.0-list-multiselect-contract.md) |
| 0024 | `v0.11.0` | [Closure runner and immutable frame time](migrations/0024-v0.11.0-closure-runner-and-frame-time.md) |
| 0025 | `v0.11.0` | [Runtime-configurable keymaps](migrations/0025-v0.11.0-runtime-configurable-keymaps.md) |
| 0026 | `v0.11.0` | [Scoped per-frame focus ring](migrations/0026-v0.11.0-scoped-focus-ring.md) |
| 0027 | `v0.11.0` | [TextInput insertion boundary repair](migrations/0027-v0.11.0-text-input-boundary-repair.md) |
| 0028 | `v0.11.0` | [VirtualGrid widget](migrations/0028-v0.11.0-virtual-grid.md) |
| 0029 | `v0.12.0` | [Experience layer](migrations/0029-v0.12.0-experience-layer.md) |
| 0030 | `v0.12.0` | [Theme system and patterns](migrations/0030-v0.12.0-theme-system-and-patterns.md) |
| 0031 | `v0.12.0` | [Foundation kernel](migrations/0031-v0.12.0-foundation-kernel.md) |
| 0032 | `v0.12.0` | [Fail-safe ApprovalCard interaction](migrations/0032-v0.12.0-fail-safe-approval-card.md) |
| 0033 | `v0.12.0` | [VirtualGrid resident projection contract](migrations/0033-v0.12.0-virtual-grid-resident-projection.md) |
| 0034 | `v0.12.0` | [Unified InteractionScene](migrations/0034-v0.12.0-unified-interaction-scene.md) |
| 0035 | `v0.12.0` | [Variable-height transcript](migrations/0035-v0.12.0-variable-height-transcript.md) |
| 0036 | `v0.12.0` | [Responsive workspace tree](migrations/0036-v0.12.0-responsive-workspace-tree.md) |
| 0037 | `v0.12.0` | [DesignSystem quiet phosphor](migrations/0037-v0.12.0-design-system-quiet-phosphor.md) |
| 0038 | `v0.12.0` | [Universal collection intents](migrations/0038-v0.12.0-universal-intents.md) |
| 0039 | `v0.12.0` | [Composed row anatomy](migrations/0039-v0.12.0-composed-row-anatomy.md) |
| 0040 | `v0.12.0` | [Agent Workbench pattern](migrations/0040-v0.12.0-agent-workbench.md) |
| 0041 | `v0.12.0` | [Design inspector](migrations/0041-v0.12.0-design-inspector.md) |
| 0042 | `v0.12.0` | [Capability preview host](migrations/0042-v0.12.0-capability-preview-host.md) |
| 0043 | `v0.12.0` | [Unified OverlayStack](migrations/0043-v0.12.0-overlay-stack.md) |
| 0044 | `v0.12.0` | [Responsive layout system](migrations/0044-v0.12.0-responsive-layout.md) |
| 0045 | `v0.12.0` | [Flagship PromptComposer](migrations/0045-v0.12.0-prompt-composer.md) |
| 0046 | `v0.12.0` | [Permission and trust surface](migrations/0046-v0.12.0-permission-trust.md) |
| 0047 | `v0.12.0` | [Data presentation foundation](migrations/0047-v0.12.0-data-presentation.md) |
| 0048 | `v0.12.0` | [Component quality standard](migrations/0048-v0.12.0-component-quality-standard.md) |
| 0049 | `v0.12.0` | [Streaming performance kits](migrations/0049-v0.12.0-streaming-performance.md) |
| 0050 | `v0.12.0` | [Terminal capability architecture](migrations/0050-v0.12.0-terminal-capabilities.md) |
| 0051 | `v0.12.0` | [Component documentation standard](migrations/0051-v0.12.0-component-documentation-standard.md) |
| 0052 | `v0.12.0` | [Terminal-native primitives](migrations/0052-v0.12.0-terminal-primitives.md) |
| 0053 | `v0.12.0` | [Controls, navigation, overlays](migrations/0053-v0.12.0-controls-navigation-overlays.md) |
| 0054 | `v0.12.0` | [Data and review surfaces](migrations/0054-v0.12.0-data-review-surfaces.md) |
| 0055 | `v0.12.0` | [Source registry CLI](migrations/0055-v0.12.0-source-registry-cli.md) |
| 0056 | `v0.12.0` | [Application blocks](migrations/0056-v0.12.0-application-blocks.md) |
| 0057 | `v0.12.0` | [Category-leading List redesign](migrations/0057-v0.12.0-list-category-leading.md) |
| 0058 | `v0.12.0` | [Category-leading Tree redesign](migrations/0058-v0.12.0-tree-category-leading.md) |
| 0059 | `v0.12.0` | [OverlayStack helpers (drawer/popover/tooltip/jump)](migrations/0059-v0.12.0-overlay-stack-helpers.md) |
| 0060 | `v0.13.0` | [Crate root re-export purge](migrations/0060-v0.13.0-root-reexport-purge.md) |
| 0061 | `v0.13.0` | [DesignSystem sole paint](migrations/0061-v0.13.0-design-system-sole-paint.md) |
| 0062 | `v0.13.0` | [InteractionScene sole focus (collections)](migrations/0062-v0.13.0-scene-sole-focus.md) |
| 0063 | `v0.13.0` | [Agent dual chrome cutover](migrations/0063-v0.13.0-agent-dual-cutover.md) |
| 0064 | `v0.13.0` | [Transcript sole stream](migrations/0064-v0.13.0-transcript-sole-stream.md) |
| 0065 | `v0.13.0` | [OverlayStack sole authority](migrations/0065-v0.13.0-overlay-stack-sole.md) |
| 0066 | `v0.13.0` | [Lookbook HostFrame](migrations/0066-v0.13.0-lookbook-host-frame.md) |
| 0067 | `v0.13.0` | [Form scene-owned field focus](migrations/0067-v0.13.0-form-scene-focus.md) |
| 0068 | `v0.13.0` | [DataTable cursor vs scene focus](migrations/0068-v0.13.0-data-table-cursor.md) |
| 0069 | `v0.13.0` | [Menu/Sidebar cursor](migrations/0069-v0.13.0-menu-cursor.md) |
| 0070 | `v0.13.0` | [ObjectInspector cursor vs scene focus](migrations/0070-v0.13.0-object-inspector-cursor.md) |
| 0071 | `v0.13.0` | [LogStream premium redesign](migrations/0071-v0.13.0-log-stream-premium.md) |
| 0072 | `v0.13.0` | [DiffReview hunk cursor](migrations/0072-v0.13.0-diff-review-cursor.md) |
| 0073 | `v0.13.0` | [Dialog/ChoiceDialog action cursor](migrations/0073-v0.13.0-dialog-action-cursor.md) |

Each breaking or dramatic public change adds the next zero-padded file and an
index row in the same commit. Existing migration files describe historical
boundaries and are not rewritten to describe a later API. Agents encountering
an incompatibility should locate the consumer's pinned version, then walk these
files sequentially until reaching the target revision.
