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
| 0074 | `v0.13.0` | [CommandPalette / Picker premium](migrations/0074-v0.13.0-command-palette-premium.md) |
| 0075 | `v0.13.0` | [PromptComposer accepts_input](migrations/0075-v0.13.0-prompt-composer-accepts-input.md) |
| 0076 | `v0.13.0` | [Button premium redesign](migrations/0076-v0.13.0-button-premium.md) |
| 0077 | `v0.13.0` | [PermissionPrompt premium](migrations/0077-v0.13.0-permission-prompt-premium.md) |
| 0078 | `v0.13.0` | [TextArea accepts_input](migrations/0078-v0.13.0-text-area-accepts-input.md) |
| 0079 | `v0.13.0` | [SemanticScene public tree](migrations/0079-v0.13.0-semantic-scene.md) |
| 0080 | `v0.13.0` | [EventResult + typed outcomes](migrations/0080-v0.13.0-event-result.md) |
| 0081 | `v0.13.0` | [FocusGraph](migrations/0081-v0.13.0-focus-graph.md) |
| 0082 | `v0.13.0` | [RovingFocusGroup](migrations/0082-v0.13.0-roving-focus-group.md) |
| 0083 | `v0.13.0` | [CollectionState](migrations/0083-v0.13.0-collection-state.md) |
| 0084 | `v0.13.0` | [SelectionModel](migrations/0084-v0.13.0-selection-model.md) |
| 0085 | `v0.13.0` | [ScrollArea](migrations/0085-v0.13.0-scroll-area.md) |
| 0086 | `v0.13.0` | [Virtualizer](migrations/0086-v0.13.0-virtualizer.md) |
| 0087 | `v0.13.0` | [OverlayStack premium](migrations/0087-v0.13.0-overlay-stack-premium.md) |
| 0088 | `v0.13.0` | [TextArea ScrollArea viewport](migrations/0088-v0.13.0-text-area-scroll-area.md) |
| 0089 | `v0.13.0` | [DismissableLayer](migrations/0089-v0.13.0-dismissable-layer.md) |
| 0090 | `v0.13.0` | [Responsive contraction](migrations/0090-v0.13.0-responsive-contraction.md) |
| 0091 | `v0.13.0` | [TerminalCapabilities](migrations/0091-v0.13.0-terminal-capabilities.md) |
| 0092 | `v0.13.0` | [FrameClock / Presence](migrations/0092-v0.13.0-frame-clock-presence.md) |
| 0093 | `v0.13.0` | [ComponentContract registry](migrations/0093-v0.13.0-component-contract-registry.md) |
| 0094 | `v0.13.0` | [AppShell](migrations/0094-v0.13.0-app-shell.md) |
| 0095 | `v0.13.0` | [Surface](migrations/0095-v0.13.0-surface.md) |
| 0096 | `v0.13.0` | [Panel + Card](migrations/0096-v0.13.0-panel-card.md) |
| 0097 | `v0.13.0` | [Stack / Inline](migrations/0097-v0.13.0-stack-inline.md) |
| 0098 | `v0.13.0` | [Grid](migrations/0098-v0.13.0-grid.md) |
| 0099 | `v0.13.0` | [Center](migrations/0099-v0.13.0-center.md) |
| 0100 | `v0.13.0` | [DesignSystem recipes & presets](migrations/0100-v0.13.0-design-system-recipes.md) |
| 0101 | `v0.13.0` | [UiContext](migrations/0101-v0.13.0-ui-context.md) |
| 0102 | `v0.13.0` | [SemanticScene premium](migrations/0102-v0.13.0-semantic-scene-premium.md) |
| 0103 | `v0.13.0` | [Panel / Card anatomy](migrations/0103-v0.13.0-panel-card-anatomy.md) |
| 0104 | `v0.13.0` | [Stack / Inline premium](migrations/0104-v0.13.0-stack-inline-premium.md) |
| 0105 | `v0.13.0` | [Grid premium](migrations/0105-v0.13.0-grid-premium.md) |
| 0106 | `v0.13.0` | [Center premium](migrations/0106-v0.13.0-center-premium.md) |
| 0107 | `v0.13.0` | [Section](migrations/0107-v0.13.0-section.md) |
| 0108 | `v0.13.0` | [Separator](migrations/0108-v0.13.0-separator.md) |
| 0109 | `v0.13.0` | [Toolbar](migrations/0109-v0.13.0-toolbar.md) |
| 0110 | `v0.13.0` | [StatusBar premium](migrations/0110-v0.13.0-status-bar.md) |
| 0111 | `v0.13.0` | [ResizablePanelGroup](migrations/0111-v0.13.0-resizable-panel-group.md) |
| 0112 | `v0.13.0` | [Collapsible](migrations/0112-v0.13.0-collapsible.md) |
| 0113 | `v0.13.0` | [Accordion](migrations/0113-v0.13.0-accordion.md) |
| 0114 | `v0.13.0` | [Text](migrations/0114-v0.13.0-text.md) |
| 0115 | `v0.13.0` | [Heading & Paragraph](migrations/0115-v0.13.0-heading-paragraph.md) |
| 0116 | `v0.13.0` | [Label & Description](migrations/0116-v0.13.0-label-description.md) |
| 0117 | `v0.13.0` | [Icon & Glyph](migrations/0117-v0.13.0-icon-glyph.md) |
| 0118 | `v0.13.0` | [Badge](migrations/0118-v0.13.0-badge.md) |
| 0119 | `v0.13.0` | [Tag & Chip](migrations/0119-v0.13.0-tag-chip.md) |
| 0120 | `v0.13.0` | [Kbd & ShortcutHint](migrations/0120-v0.13.0-kbd-shortcut-hint.md) |
| 0121 | `v0.13.0` | [Link & ActionLink](migrations/0121-v0.13.0-link-action-link.md) |
| 0122 | `v0.13.0` | [CodeBlock premium](migrations/0122-v0.13.0-code-block.md) |
| 0123 | `v0.13.0` | [Markdown premium](migrations/0123-v0.13.0-markdown.md) |
| 0124 | `v0.13.0` | [AnsiText](migrations/0124-v0.13.0-ansi-text.md) |
| 0125 | `v0.13.0` | [KeyValueList](migrations/0125-v0.13.0-key-value-list.md) |
| 0126 | `v0.13.0` | [AvatarGlyph & Identity](migrations/0126-v0.13.0-avatar-identity.md) |
| 0127 | `v0.13.0` | [HighlightedText & MatchRanges](migrations/0127-v0.13.0-highlighted-text.md) |
| 0128 | `v0.13.0` | [Button anatomy](migrations/0128-v0.13.0-button.md) |
| 0129 | `v0.13.0` | [IconButton](migrations/0129-v0.13.0-icon-button.md) |
| 0130 | `v0.13.0` | [ButtonGroup](migrations/0130-v0.13.0-button-group.md) |
| 0131 | `v0.13.0` | [Toggle & ToggleGroup](migrations/0131-v0.13.0-toggle.md) |
| 0132 | `v0.13.0` | [Checkbox](migrations/0132-v0.13.0-checkbox.md) |
| 0133 | `v0.13.0` | [RadioGroup](migrations/0133-v0.13.0-radio-group.md) |
| 0134 | `v0.13.0` | [Switch](migrations/0134-v0.13.0-switch.md) |
| 0135 | `v0.13.0` | [SegmentedControl](migrations/0135-v0.13.0-segmented-control.md) |
| 0136 | `v0.13.0` | [Slider & RangeSlider](migrations/0136-v0.13.0-slider.md) |
| 0137 | `v0.13.0` | [Field, Fieldset & Form](migrations/0137-v0.13.0-form-field-fieldset.md) |
| 0138 | `v0.13.0` | [TextInput](migrations/0138-v0.13.0-text-input.md) |
| 0139 | `v0.13.0` | [TextArea](migrations/0139-v0.13.0-text-area.md) |
| 0140 | `v0.13.0` | [PasswordInput](migrations/0140-v0.13.0-password-input.md) |
| 0141 | `v0.13.0` | [NumberInput](migrations/0141-v0.13.0-number-input.md) |
| 0142 | `v0.13.0` | [SearchInput](migrations/0142-v0.13.0-search-input.md) |
| 0143 | `v0.13.0` | [PathInput](migrations/0143-v0.13.0-path-input.md) |
| 0144 | `v0.13.0` | [TokenField](migrations/0144-v0.13.0-token-field.md) |
| 0145 | `v0.13.0` | [Select](migrations/0145-v0.13.0-select.md) |
| 0146 | `v0.13.0` | [MultiSelect](migrations/0146-v0.13.0-multi-select.md) |
| 0147 | `v0.13.0` | [Combobox & Autocomplete](migrations/0147-v0.13.0-combobox-autocomplete.md) |
| 0148 | `v0.13.0` | [FilePicker](migrations/0148-v0.13.0-file-picker.md) |
| 0149 | `v0.13.0` | [DateTimePicker](migrations/0149-v0.13.0-date-time-picker.md) |
| 0150 | `v0.13.0` | [KeybindingRecorder](migrations/0150-v0.13.0-keybinding-recorder.md) |
| 0151 | `v0.13.0` | [FormWizard](migrations/0151-v0.13.0-form-wizard.md) |
| 0152 | `v0.13.0` | [Tabs](migrations/0152-v0.13.0-tabs.md) |
| 0153 | `v0.13.0` | [Sidebar & NavigationList](migrations/0153-v0.13.0-sidebar-navigation-list.md) |
| 0154 | `v0.13.0` | [TreeNavigation](migrations/0154-v0.13.0-tree-navigation.md) |
| 0155 | `v0.13.0` | [Breadcrumbs](migrations/0155-v0.13.0-breadcrumbs.md) |
| 0156 | `v0.13.0` | [Pagination](migrations/0156-v0.13.0-pagination.md) |
| 0157 | `v0.13.0` | [MenuBar](migrations/0157-v0.13.0-menu-bar.md) |
| 0158 | `v0.13.0` | [CommandPalette](migrations/0158-v0.13.0-command-palette.md) |
| 0159 | `v0.13.0` | [QuickOpen](migrations/0159-v0.13.0-quick-open.md) |
| 0160 | `v0.13.0` | [JumpMode and FocusLens](migrations/0160-v0.13.0-jump-mode-focus-lens.md) |
| 0161 | `v0.13.0` | [Stepper](migrations/0161-v0.13.0-stepper.md) |
| 0162 | `v0.13.0` | [HistoryPicker](migrations/0162-v0.13.0-history-picker.md) |
| 0163 | `v0.13.0` | [KeyboardHelp](migrations/0163-v0.13.0-keyboard-help.md) |
| 0164 | `v0.13.0` | [Tooltip](migrations/0164-v0.13.0-tooltip.md) |
| 0165 | `v0.13.0` | [Popover](migrations/0165-v0.13.0-popover.md) |
| 0166 | `v0.13.0` | [DropdownMenu and ContextMenu](migrations/0166-v0.13.0-dropdown-context-menu.md) |
| 0167 | `v0.13.0` | [CompletionMenu](migrations/0167-v0.13.0-completion-menu.md) |
| 0168 | `v0.13.0` | [Dialog](migrations/0168-v0.13.0-dialog.md) |
| 0169 | `v0.13.0` | [AlertDialog](migrations/0169-v0.13.0-alert-dialog.md) |
| 0170 | `v0.13.0` | [Drawer and Sheet](migrations/0170-v0.13.0-drawer-sheet.md) |
| 0171 | `v0.13.0` | [FullscreenViewer and SemanticZoom](migrations/0171-v0.13.0-fullscreen-viewer-semantic-zoom.md) |
| 0172 | `v0.13.0` | [PreviewCard](migrations/0172-v0.13.0-preview-card.md) |
| 0173 | `v0.13.0` | [Alert and Callout](migrations/0173-v0.13.0-alert-callout.md) |
| 0174 | `v0.13.0` | [Toast](migrations/0174-v0.13.0-toast.md) |
| 0175 | `v0.13.0` | [NotificationCenter](migrations/0175-v0.13.0-notification-center.md) |
| 0176 | `v0.13.0` | [Spinner and ActivityIndicator](migrations/0176-v0.13.0-spinner-activity-indicator.md) |
| 0177 | `v0.13.0` | [ProgressBar](migrations/0177-v0.13.0-progress-bar.md) |
| 0178 | `v0.13.0` | [ProgressSteps](migrations/0178-v0.13.0-progress-steps.md) |
| 0179 | `v0.13.0` | [Skeleton](migrations/0179-v0.13.0-skeleton.md) |
| 0180 | `v0.13.0` | [StatusIndicator](migrations/0180-v0.13.0-status-indicator.md) |
| 0181 | `v0.13.0` | [EmptyState](migrations/0181-v0.13.0-empty-state.md) |
| 0182 | `v0.13.0` | [ErrorState and Recovery](migrations/0182-v0.13.0-error-state-recovery.md) |
| 0183 | `v0.13.0` | [LoadingOverlay and BusyBoundary](migrations/0183-v0.13.0-loading-overlay-busy-boundary.md) |
| 0184 | `v0.13.0` | [Offline and ReconnectingState](migrations/0184-v0.13.0-offline-reconnecting-state.md) |
| 0185 | `v0.13.0` | [List collection view](migrations/0185-v0.13.0-list-collection-view.md) |
| 0186 | `v0.13.0` | [VirtualList](migrations/0186-v0.13.0-virtual-list.md) |
| 0187 | `v0.13.0` | [Tree hierarchy](migrations/0187-v0.13.0-tree-hierarchy.md) |
| 0188 | `v0.13.0` | [Table presentation](migrations/0188-v0.13.0-table-presentation.md) |
| 0189 | `v0.13.0` | [DataTable interactive](migrations/0189-v0.13.0-data-table-interactive.md) |
| 0190 | `v0.13.0` | [TreeTable hierarchy columns](migrations/0190-v0.13.0-tree-table.md) |
| 0191 | `v0.13.0` | [KeyValueTable dense detail](migrations/0191-v0.13.0-key-value-table.md) |
| 0192 | `v0.13.0` | [ObjectInspector typed expandable](migrations/0192-v0.13.0-object-inspector.md) |
| 0193 | `v0.13.0` | [Timeline chronological events](migrations/0193-v0.13.0-timeline.md) |
| 0194 | `v0.13.0` | [EventStream structured viewer](migrations/0194-v0.13.0-event-stream.md) |
| 0195 | `v0.13.0` | [LogStream professional viewer](migrations/0195-v0.13.0-log-stream.md) |
| 0196 | `v0.13.0` | [DiffView unified/split renderer](migrations/0196-v0.13.0-diff-view.md) |
| 0197 | `v0.13.0` | [DiffReview interactive patch review](migrations/0197-v0.13.0-diff-review.md) |
| 0198 | `v0.13.0` | [Diagnostic and CodeFrame](migrations/0198-v0.13.0-diagnostic-code-frame.md) |
| 0199 | `v0.13.0` | [TerminalOutput safe command presentation](migrations/0199-v0.13.0-terminal-output.md) |
| 0200 | `v0.13.0` | [HexViewer virtualized binary inspector](migrations/0200-v0.13.0-hex-viewer.md) |
| 0201 | `v0.13.0` | [Charts visualization family](migrations/0201-v0.13.0-charts-viz-family.md) |
| 0202 | `v0.13.0` | [FileTree filesystem explorer](migrations/0202-v0.13.0-file-tree.md) |
| 0203 | `v0.13.0` | [ProcessTable process / task monitor](migrations/0203-v0.13.0-process-table.md) |
| 0204 | `v0.13.0` | [QueryEditor query workbench](migrations/0204-v0.13.0-query-editor.md) |
| 0205 | `v0.13.0` | [ResultGrid query results](migrations/0205-v0.13.0-result-grid.md) |
| 0206 | `v0.13.0` | [SchemaBrowser catalog navigator](migrations/0206-v0.13.0-schema-browser.md) |
| 0207 | `v0.13.0` | [SearchResults grouped hits](migrations/0207-v0.13.0-search-results.md) |
| 0208 | `v0.13.0` | [MetricsDashboard observability block](migrations/0208-v0.13.0-metrics-dashboard.md) |
| 0209 | `v0.13.0` | [TraceWaterfall span latency](migrations/0209-v0.13.0-trace-waterfall.md) |
| 0210 | `v0.13.0` | [DependencyGraph constrained deps](migrations/0210-v0.13.0-dependency-graph.md) |
| 0211 | `v0.13.0` | [PromptComposer flagship agent input](migrations/0211-v0.13.0-prompt-composer.md) |
| 0212 | `v0.13.0` | [AttachmentChip and PasteChip](migrations/0212-v0.13.0-attachment-paste-chips.md) |
| 0213 | `v0.13.0` | [FileMention and EntityMention](migrations/0213-v0.13.0-file-entity-mention.md) |
| 0214 | `v0.13.0` | [SlashCommandMenu](migrations/0214-v0.13.0-slash-command-menu.md) |
| 0215 | `v0.13.0` | [ModelSelector and AgentModeSelector](migrations/0215-v0.13.0-model-mode-selectors.md) |
| 0216 | `v0.13.0` | [MessageThread conversation transcript](migrations/0216-v0.13.0-message-thread.md) |
| 0217 | `v0.13.0` | [StreamingMarkdown incomplete-fence stream](migrations/0217-v0.13.0-streaming-markdown.md) |
| 0218 | `v0.13.0` | [SourceCitation and CitationList](migrations/0218-v0.13.0-source-citation.md) |
| 0219 | `v0.13.0` | [ToolCallCard and elevated ToolStatus](migrations/0219-v0.13.0-tool-call-card.md) |
| 0220 | `v0.13.0` | [TerminalRunCard shell command card](migrations/0220-v0.13.0-terminal-run-card.md) |
| 0221 | `v0.13.0` | [ActivityShelf concurrent activity strip](migrations/0221-v0.13.0-activity-shelf.md) |
| 0222 | `v0.13.0` | [TaskRail ActivityModel side panel](migrations/0222-v0.13.0-task-rail.md) |
| 0223 | `v0.13.0` | [SubagentCard delegated agent work](migrations/0223-v0.13.0-subagent-card.md) |
| 0224 | `v0.13.0` | [BackgroundTaskPanel long-job monitor](migrations/0224-v0.13.0-background-task-panel.md) |
| 0225 | `v0.13.0` | [ContextMeter budget display](migrations/0225-v0.13.0-context-meter.md) |
| 0226 | `v0.13.0` | [PermissionPrompt Global trust surface](migrations/0226-v0.13.0-permission-prompt.md) |
| 0227 | `v0.13.0` | [QuestionFlow multi-question HITL](migrations/0227-v0.13.0-question-flow.md) |
| 0228 | `v0.13.0` | [PlanReview interactive plan document](migrations/0228-v0.13.0-plan-review.md) |
| 0229 | `v0.13.0` | [CheckpointTimeline session history](migrations/0229-v0.13.0-checkpoint-timeline.md) |
| 0230 | `v0.13.0` | [SessionPicker agent sessions](migrations/0230-v0.13.0-session-picker.md) |
| 0231 | `v0.13.0` | [PromptQueue agent prompt queue](migrations/0231-v0.13.0-prompt-queue.md) |
| 0232 | `v0.13.0` | [AgentStatusHeader agent chrome](migrations/0232-v0.13.0-agent-status-header.md) |
| 0233 | `v0.13.0` | [IntegrationStatus MCP/plugins](migrations/0233-v0.13.0-integration-status.md) |
| 0234 | `v0.13.0` | [WorkingStateCard agent work summary](migrations/0234-v0.13.0-working-state-card.md) |
| 0235 | `v0.13.0` | [ApprovalQueue human decisions inbox](migrations/0235-v0.13.0-approval-queue.md) |
| 0236 | `v0.13.0` | [AgentWorkbench elevated composition block](migrations/0236-v0.13.0-agent-workbench.md) |
| 0237 | `v0.13.0` | [SettingsScreen elevated searchable settings](migrations/0237-v0.13.0-settings-screen.md) |
| 0238 | `v0.13.0` | [SetupWizard first-run onboarding block](migrations/0238-v0.13.0-setup-wizard.md) |
| 0239 | `v0.13.0` | [ConnectionManager connection inventory block](migrations/0239-v0.13.0-connection-manager.md) |
| 0240 | `v0.13.0` | [DatabaseWorkbench data composition block](migrations/0240-v0.13.0-database-workbench.md) |
| 0241 | `v0.13.0` | [GitWorkbench source-control composition block](migrations/0241-v0.13.0-git-workbench.md) |
| 0242 | `v0.13.0` | [Logs ObservabilityDashboard composition block](migrations/0242-v0.13.0-logs-observability-dashboard.md) |
| 0243 | `v0.13.0` | [FileManager composition block](migrations/0243-v0.13.0-file-manager.md) |
| 0244 | `v0.13.0` | [ProjectLauncher composition block](migrations/0244-v0.13.0-project-launcher.md) |
| 0245 | `v0.13.0` | [HelpCenter / CommandReference composition block](migrations/0245-v0.13.0-help-center-command-reference.md) |
| 0246 | `v0.13.0` | [ErrorRecovery / CrashReport composition block](migrations/0246-v0.13.0-error-recovery-crash-report.md) |
| 0247 | `v0.13.0` | [shadcn gap: InputOtp, Carousel, InputGroup](migrations/0247-v0.13.0-shadcn-gap-input-otp-carousel-input-group.md) |
| 0248 | `v0.13.0` | [AuthEntry signup-blocks TUI composition](migrations/0248-v0.13.0-auth-entry-signup-blocks.md) |
| 0249 | `v0.13.0` | [AuthEntry login blocks + email-only](migrations/0249-v0.13.0-auth-entry-login-email-only.md) |
| 0250 | `v0.13.0` | [Sidebar collapse filter + sectioned nav](migrations/0250-v0.13.0-sidebar-collapse-filter.md) |
| 0251 | `v0.13.0` | [AppDashboard + first-party blocks matrix](migrations/0251-v0.13.0-app-dashboard-blocks.md) |
| 0252 | `v0.13.0` | [Chart area fill for shadcn charts](migrations/0252-v0.13.0-chart-area-fill.md) |
| 0253 | `v0.13.0` | [BarSeries stacked + bipolar negatives](migrations/0253-v0.13.0-bar-stacked-negative.md) |
| 0254 | `v0.13.0` | [Chart line linear/step interpolation](migrations/0254-v0.13.0-chart-line-interpolation.md) |
| 0255 | `v0.13.0` | [SegmentedMeter pie peers](migrations/0255-v0.13.0-segmented-meter-pie.md) |
| 0256 | `v0.13.0` | [MetricRadar multi-axis (radar peer)](migrations/0256-v0.13.0-metric-radar.md) |
| 0257 | `v0.13.0` | [Composites move widgets→patterns](migrations/0257-v0.13.0-composites-to-patterns.md) |
| 0258 | `v0.13.0` | [Widgets/patterns boundary fix](migrations/0258-v0.13.0-widgets-patterns-boundary-fix.md) |
| 0259 | `v0.13.0` | [Neutral panel presets + example nav](migrations/0259-v0.13.0-neutral-panel-presets-and-example-nav.md) |
| 0260 | `v0.13.0` | [Building-block classification rule](migrations/0260-v0.13.0-building-block-classification-rule.md) |
| 0261 | `v0.13.0` | [Surface ladder and semantic role expansion](migrations/0261-v0.13.0-surface-ladder-and-role-expansion.md) |
| 0262 | `v0.13.0` | [Border shape token](migrations/0262-v0.13.0-border-shape-token.md) |
| 0263 | `v0.13.0` | [Spacing activation](migrations/0263-v0.13.0-spacing-activation.md) |
| 0264 | `v0.13.0` | [Recipe-enforced controls, chips, tints, and meters](migrations/0264-v0.13.0-recipe-enforcement-and-chips.md) |
| 0265 | `v0.13.0` | [Overlay elevation and shared shells](migrations/0265-v0.13.0-overlay-elevation.md) |
| 0266 | `v0.13.0` | [Motion, text, and glyph kit](migrations/0266-v0.13.0-motion-text-glyph-kit.md) |
| 0267 | `v0.13.0` | [Tree tone tiers and pinned horizontal scroll](migrations/0267-v0.13.0-tree-tone-and-scroll.md) |
| 0268 | `v0.13.0` | [Composition and measurement primitives](migrations/0268-v0.13.0-composition-measurement-primitives.md) |
| 0269 | `v0.13.0` | [Transcript actor rails and active presence](migrations/0269-v0.13.0-transcript-actor-rails.md) |
| 0270 | `v0.13.0` | [ToolCallCard actor rail and compact row](migrations/0270-v0.13.0-tool-call-card-actor-rail.md) |
| 0271 | `v0.13.0` | [Agent pattern-card presence](migrations/0271-v0.13.0-agent-pattern-card-presence.md) |
| 0272 | `v0.13.0` | [Composer and permission trust chrome](migrations/0272-v0.13.0-composer-permission-chrome.md) |
| 0273 | `v0.13.0` | [Plan review golden composition chrome](migrations/0273-v0.13.0-plan-review-golden-chrome.md) |
| 0274 | `v0.13.0` | [Table recipe rows and raised headers](migrations/0274-v0.13.0-table-recipe-rows.md) |
| 0275 | `v0.13.0` | [TreeTable recipe rows and pointer hover](migrations/0275-v0.13.0-tree-table-recipe-rows.md) |
| 0276 | `v0.13.0` | [Overlay and picker recipe rows](migrations/0276-v0.13.0-overlay-picker-recipe-rows.md) |
| 0277 | `v0.13.0` | [Full-row diff semantic tints](migrations/0277-v0.13.0-diff-row-tints.md) |
| 0278 | `v0.13.0` | [Typed Form values through FieldRow](migrations/0278-v0.13.0-form-field-row-values.md) |
| 0279 | `v0.13.0` | [Visual cascade completion](migrations/0279-v0.13.0-visual-cascade-completion.md) |
| 0280 | `v0.13.0` | [Portable frame time](migrations/0280-v0.13.0-portable-frame-time.md) |
| 0281 | `v0.14.0` | [Craft helpers: label painter, path contraction, inset token](migrations/0281-v0.14.0-craft-helpers.md) |
| 0282 | `v0.14.0` | [Honest capability ladder: quantizer, projections, glyph de-collision](migrations/0282-v0.14.0-honest-capability-ladder.md) |
| 0283 | `v0.14.0` | [The contrast floor](migrations/0283-v0.14.0-contrast-floor.md) |
| 0284 | `v0.14.0` | [Graphite role ladder: text tiers, accent de-collapse, retired tab-underline roles](migrations/0284-v0.14.0-graphite-role-ladder.md) |
| 0285 | `v0.14.0` | [Row rhythm and alignment](migrations/0285-v0.14.0-row-rhythm-and-alignment.md) |
| 0286 | `v0.14.0` | [One voice: the microcopy standard](migrations/0286-v0.14.0-one-voice-microcopy.md) |
| 0287 | `v0.14.0` | [Light-preset contrast and the faint tier](migrations/0287-v0.14.0-light-preset-contrast.md) |
| 0288 | `v0.14.0` | [One paint authority for selection, focus, and elevation](migrations/0288-v0.14.0-selection-focus-paint-authority.md) |
| 0289 | `v0.14.0` | [Motion: demand-driven pipeline, policy tiers, layer-1 animation](migrations/0289-v0.14.0-motion-pipeline-and-policy.md) |
| 0290 | `v0.14.0` | [The information budget](migrations/0290-v0.14.0-information-budget.md) |
| 0291 | `v0.14.0` | [One scrollbar language](migrations/0291-v0.14.0-one-scrollbar-language.md) |
| 0292 | `v0.14.0` | [Bordered overlays reserve their gutter](migrations/0292-v0.14.0-overlay-content-inset.md) |
| 0293 | `v0.14.0` | [Honest truncation: chrome labels, paths, held-back errors](migrations/0293-v0.14.0-honest-truncation.md) |
| 0294 | `v0.14.0` | [Silent clips say what they held back](migrations/0294-v0.14.0-silent-clips.md) |
| 0295 | `v0.14.0` | [Underline means "link", and nothing else](migrations/0295-v0.14.0-underline-free-interaction.md) |
| 0296 | `v0.14.0` | [One selection language for every collection](migrations/0296-v0.14.0-one-selection-language.md) |
| 0297 | `v0.14.0` | [Skeleton sweeps instead of pulsing](migrations/0297-v0.14.0-skeleton-shimmer-sweep.md) |
| 0298 | `v0.14.0` | [Status lives in the glyph](migrations/0298-v0.14.0-status-in-the-glyph.md) |
| 0299 | `v0.14.0` | [The design system carries the frame tick](migrations/0299-v0.14.0-design-system-carries-the-tick.md) |
| 0300 | `v0.14.0` | [The accent budget](migrations/0300-v0.14.0-accent-budget.md) |
| 0301 | `v0.14.0` | [Stream rows keep their words](migrations/0301-v0.14.0-stream-rows-keep-their-words.md) |
| 0302 | `v0.14.0` | [Overlay affordances restored, and the neon-fill gate goes live](migrations/0302-v0.14.0-overlay-affordances-restored.md) |
| 0303 | `v0.14.0` | [Spinner phases declare a motion channel](migrations/0303-v0.14.0-spinner-channels.md) |
| 0304 | `v0.14.0` | [The underline scan covers the builder too](migrations/0304-v0.14.0-underline-scan-covers-the-builder.md) |
| 0305 | `v0.14.0` | [A live status breathes in its own cell](migrations/0305-v0.14.0-status-breathes-in-its-cell.md) |
| 0306 | `v0.14.0` | [Toasts fade in, errors do not](migrations/0306-v0.14.0-toast-entrance-motion.md) |
| 0307 | `v0.14.0` | [Empty bodies speak one language](migrations/0307-v0.14.0-empty-bodies-speak-one-language.md) |
| 0308 | `v0.14.0` | [Overlay hints share one vocabulary](migrations/0308-v0.14.0-overlay-hints-share-one-vocabulary.md) |
| 0309 | `v0.14.0` | [Danger is quiet by default](migrations/0309-v0.14.0-danger-is-quiet-by-default.md) |
| 0310 | `v0.14.0` | [Floating surfaces actually float](migrations/0310-v0.14.0-floating-surfaces-float.md) |
| 0311 | `v0.14.0` | [Picker columns hold still, and notifications get a clock](migrations/0311-v0.14.0-picker-columns-and-a-clock.md) |
| 0312 | `v0.14.0` | [Control chrome parity](migrations/0312-v0.14.0-control-chrome-parity.md) |
| 0313 | `v0.14.0` | [Rows get anatomy](migrations/0313-v0.14.0-rows-get-anatomy.md) |
| 0314 | `v0.14.0` | [Examples only compose](migrations/0314-v0.14.0-examples-only-compose.md) |
| 0315 | `v0.14.0` | [Examples stop hand-rolling chrome](migrations/0315-v0.14.0-examples-stop-hand-rolling.md) |
| 0316 | `v0.14.0` | [One token family, one focus vocabulary](migrations/0316-v0.14.0-one-token-family.md) |
| 0317 | `v0.14.0` | [Every state paints, and Space works](migrations/0317-v0.14.0-every-state-paints.md) |
| 0318 | `v0.14.0` | [Tables say where they are cut](migrations/0318-v0.14.0-tables-say-where-they-are-cut.md) |
| 0319 | `v0.14.0` | [Empty states, one-column glyphs, honest card chrome](migrations/0319-v0.14.0-empty-states-and-one-column-glyphs.md) |
| 0320 | `v0.14.0` | [Transitions reach the widgets](migrations/0320-v0.14.0-transitions-reach-the-widgets.md) |
| 0321 | `v0.14.0` | [The catalog tells the truth](migrations/0321-v0.14.0-the-catalog-tells-the-truth.md) |
| 0322 | `v0.14.0` | [Overlays survive any terminal](migrations/0322-v0.14.0-overlays-survive-any-terminal.md) |
| 0323 | `v0.14.0` | [One modal geometry](migrations/0323-v0.14.0-one-modal-geometry.md) |
| 0324 | `v0.14.0` | [The default frame goes on a diet](migrations/0324-v0.14.0-the-default-frame-goes-on-a-diet.md) |
| 0325 | `v0.14.0` | [Departures, edges, and acknowledgements](migrations/0325-v0.14.0-departures-edges-and-acknowledgements.md) |
| 0326 | `v0.14.0` | [One ellipsis and five visible elevation rungs](migrations/0326-v0.14.0-one-ellipsis-five-elevation-rungs.md) |
| 0327 | `v0.14.0` | [Tabs restore the focus-aware selection rule](migrations/0327-v0.14.0-tabs-restore-focus-aware-rule.md) |
| 0328 | `v0.14.0` | [ChoiceDialog measures actions before stacking](migrations/0328-v0.14.0-choice-dialog-measured-actions.md) |
| 0329 | `v0.14.0` | [Tabs rule keeps the selection wash](migrations/0329-v0.14.0-tabs-rule-keeps-selection-wash.md) |
| 0330 | `v0.14.0` | [ChoiceDialog centers actions](migrations/0330-v0.14.0-choice-dialog-centers-actions.md) |
| 0331 | `v0.14.0` | [Jackin parity handoff](migrations/0331-v0.14.0-jackin-parity-handoff.md) |
| 0332 | `v0.14.0` | [FilePicker preview generations](migrations/0332-v0.14.0-file-picker-preview-generations.md) |
| 0333 | `v0.14.0` | [SearchResults status authority](migrations/0333-v0.14.0-search-results-status-authority.md) |
| 0334 | `v0.14.0` | [SegmentedControl selection authority](migrations/0334-v0.14.0-segmented-control-selection-authority.md) |
| 0335 | `v0.14.0` | [KeyEvent phase predicates](migrations/0335-v0.14.0-key-event-phase-predicates.md) |
| 0336 | `v0.14.0` | [Scrolled-region gutter contract](migrations/0336-v0.14.0-scrolled-region-gutter-contract.md) |
| 0367 | `v0.14.0` | [Stateful selectable Viewport](migrations/0367-v0.14.0-viewport-selection-state.md) |
| 0367 | `v0.14.0` | [Collection virtual windows clamp their start](migrations/0367-v0.14.0-collection-window-start-invariant.md) |
| 0368 | `v0.14.0` | [TreeTable loading rows reject hierarchy actions](migrations/0368-v0.14.0-tree-table-loading-hierarchy-guard.md) |
| 0369 | `v0.14.0` | [TreeTable collapse validates parent ancestry](migrations/0369-v0.14.0-tree-table-collapse-ancestor-validation.md) |
| 0370 | `v0.14.0` | [TreeTable movement recovers from non-selectable cursors](migrations/0370-v0.14.0-tree-table-move-from-invalid-cursor.md) |
| 0371 | `v0.14.0` | [Roving movement repairs disabled active IDs once](migrations/0371-v0.14.0-roving-disabled-active-movement.md) |
| 0372 | `v0.14.0` | [Roving reconciliation clears stale typeahead](migrations/0372-v0.14.0-roving-reconcile-clears-stale-typeahead.md) |
| 0373 | `v0.14.0` | [Roving movement stops after missing-active recovery](migrations/0373-v0.14.0-roving-missing-active-recovery.md) |
| 0374 | `v0.14.0` | [Roving typeahead folds Unicode case](migrations/0374-v0.14.0-roving-unicode-typeahead-folding.md) |
| 0375 | `v0.14.0` | [Roving typeahead retries after the active row](migrations/0375-v0.14.0-roving-typeahead-forward-retry.md) |
| 0376 | `v0.14.0` | [TreeTable preserves virtual selection identity](migrations/0376-v0.14.0-tree-table-virtual-selection-identity.md) |
| 0377 | `v0.14.0` | [TreeTable activation requires selected identity](migrations/0377-v0.14.0-tree-table-virtual-activation-identity.md) |
| 0378 | `v0.14.0` | [TreeTable invalidates stale row geometry](migrations/0378-v0.14.0-tree-table-invalidate-stale-row-geometry.md) |
| 0379 | `v0.14.0` | [TreeTable Up stops before the first selectable row](migrations/0379-v0.14.0-tree-table-up-leading-nonselectable.md) |
| 0380 | `v0.14.0` | [TreeTable Down stops after the last selectable row](migrations/0380-v0.14.0-tree-table-down-trailing-nonselectable.md) |
| 0381 | `v0.14.0` | [TreeTable gap movement retains virtual scroll fallback](migrations/0381-v0.14.0-tree-table-gap-scroll-fallback.md) |
| 0382 | `v0.14.0` | [TreeTable scrolls all-nonselectable projections](migrations/0382-v0.14.0-tree-table-empty-enabled-scroll.md) |
| 0383 | `v0.14.0` | [TreeTable pages empty virtual projections](migrations/0383-v0.14.0-tree-table-empty-projection-page.md) |
| 0384 | `v0.14.0` | [TreeTable routes empty virtual keys](migrations/0384-v0.14.0-tree-table-empty-key-routing.md) |
| 0385 | `v0.14.0` | [TreeTable honors error retryability](migrations/0385-v0.14.0-tree-table-error-retryability.md) |
| 0386 | `v0.14.0` | [TreeTable cycles nav mode on empty projections](migrations/0386-v0.14.0-tree-table-empty-nav-mode.md) |
| 0387 | `v0.14.0` | [TreeTable key routing for empty projections](migrations/0387-v0.14.0-tree-table-key-empty-projection.md) |
| 0388 | `v0.14.0` | [TreeTable routes shifted page keys](migrations/0388-v0.14.0-tree-table-shifted-page-routing.md) |
| 0389 | `v0.14.0` | [List one-shot intents reject key repeats](migrations/0389-v0.14.0-list-intent-repeat-gates.md) |
| 0390 | `v0.14.0` | [Virtual windows preserve active identity](migrations/0390-v0.14.0-virtual-window-active-identity.md) |
| 0391 | `v0.14.0` | [Collection full-window reconciliation repairs stale active IDs](migrations/0391-v0.14.0-collection-full-window-reconcile.md) |
| 0392 | `v0.14.0` | [Partial collection windows expose active-ID policy](migrations/0392-v0.14.0-collection-partial-active-policy.md) |
| 0393 | `v0.14.0` | [List virtual-window mode transitions synchronize collection metadata](migrations/0393-v0.14.0-clear-virtual-window-state.md) |
| 0394 | `v0.14.0` | [Full collection viewport resets virtual-window metadata](migrations/0394-v0.14.0-set-viewport-clears-window.md) |
| 0398 | `v0.14.0` | [paint-only Accordion](migrations/0398-v0.14.0-paint-only-accordion.md) |
| 0399 | `v0.14.0` | [paint-only ButtonGroup](migrations/0399-v0.14.0-paint-only-button-group.md) |
| 0400 | `v0.14.0` | [paint-only links and controls](migrations/0400-v0.14.0-paint-only-links-controls.md) |
| 0401 | `v0.14.0` | [Timeline paint contract and Skeleton consolidation](migrations/0401-v0.14.0-paint-only-timeline-skeleton.md) |
| 0402 | `v0.14.0` | [EmptyState uses one stateful paint path](migrations/0402-v0.14.0-paint-only-empty-state.md) |
| 0403 | `v0.14.0` | [ErrorState and StatusIndicator use one paint path](migrations/0403-v0.14.0-paint-only-error-status.md) |
| 0404 | `v0.14.0` | [Slider family and Tooltip are paint-only](migrations/0404-v0.14.0-paint-only-slider-tooltip.md) |
| 0405 | `v0.14.0` | [SegmentedControl and ToggleGroup are paint-only](migrations/0405-v0.14.0-paint-only-segmented-toggle.md) |
| 0406 | `v0.14.0` | [Stateful paint vocabulary replaces render aliases](migrations/0406-v0.14.0-unify-stateful-paint-vocabulary.md) |
| 0407 | `v0.14.0` | [AgentWorkbench removes the legacy task-list fallback and SettingsShell aliases](migrations/0407-v0.14.0-remove-agent-workbench-task-list-fallback.md) |
| 0408 | `v0.14.0` | [Legacy layout and input aliases removed](migrations/0408-v0.14.0-remove-legacy-layout-and-input-aliases.md) |
| 0409 | `v0.14.0` | [LogStream and list-row migration shims removed](migrations/0409-v0.14.0-remove-log-and-list-migration-shims.md) |
| 0410 | `v0.14.0` | [Panel and Card use explicit paint](migrations/0410-v0.14.0-remove-panel-card-widget-impls.md) |
| 0413 | `v0.14.0` | [Tree virtual windows use absolute scroll geometry](migrations/0413-v0.14.0-tree-virtual-window-absolute-offset.md) |
| 0414 | `v0.14.0` | [InteractionScene gates pointer hits by input ownership](migrations/0414-v0.14.0-interaction-scene-modal-pointer-ownership.md) |
| 0415 | `v0.14.0` | [CommandPalette keeps keyword ranges out of label highlights](migrations/0415-v0.14.0-command-palette-keyword-highlight-ranges.md) |
| 0416 | `v0.14.0` | [SearchResults ignores stale completions](migrations/0416-v0.14.0-search-results-ignore-stale-completions.md) |
| 0417 | `v0.14.0` | [Text-area Escape cancel is one-shot](migrations/0417-v0.14.0-one-shot-cancel-intent.md) |
| 0418 | `v0.14.0` | [Keymap resolution uses canonical key events](migrations/0418-v0.14.0-canonical-keymap-resolution.md) |
| 0419 | `v0.14.0` | [Nested focus traps restore their outer boundary](migrations/0419-v0.14.0-nested-focus-trap-restoration.md) |
| 0421 | `v0.14.0` | [HistoryPicker Escape cancellation is one-shot](migrations/0421-v0.14.0-history-picker-one-shot-cancel.md) |

Each breaking or dramatic public change adds the next zero-padded file and an
index row in the same commit. Existing migration files describe historical
boundaries and are not rewritten to describe a later API. Agents encountering
an incompatibility should locate the consumer's pinned version, then walk these
files sequentially until reaching the target revision.
