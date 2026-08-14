# Coverage matrix — every component → audit disposition → owning plan(s)

Proof of scope for the 2026-08-14 look-and-feel audit (commit `605217aa`).
Nine audit passes covered every rendering surface in `crates/termrock`.
"Clean" = audited, no finding beyond what foundation plans fix globally
(all widgets inherit plans 002/003/004 token+recipe changes).

## The 48 priority components (user list)

| # | Component | File | Disposition → plans |
|---|-----------|------|---------------------|
| 1 | Setup wizard | patterns/setup_wizard.rs | cancel slab, flat KV rows, dead ascii ternaries, inert colorless → 010 |
| 2 | Settings screen | patterns/settings_screen.rs | banner, dual footers, ThemePicker double border → 010 |
| 3 | Metrics dashboard | patterns/metrics_dashboard.rs | hand-drawn tiles, health-role series, focus recolor → 010 |
| 4 | Auth Entry | patterns/auth_entry.rs | no Form, invisible pending, dup errors → 010 |
| 5 | VirtualList | widgets/virtual_list.rs | sticky rows lose selection → 006 |
| 6 | VirtualGrid | widgets/virtual_grid.rs | cursor==range==Accent, invisible unfocused cursor → 006 |
| 7 | TreeTable | widgets/tree_table.rs | chrome override, inverted gutter, cursor underline, focus header → 004,005,006,012 |
| 8 | Toolbar | widgets/toolbar.rs | ActionFocused chip focus, color-only fallback gating → 007,008 |
| 9 | TokenField | widgets/token_field.rs | label underline, no well → 005,008 |
| 10 | Toast | widgets/toast.rs | status rail second border → 007 |
| 11 | Timeline | widgets/timeline.rs | hand-rolled chrome match, `›` marker → 006 |
| 12 | ThemePicker | widgets/theme_picker.rs | always-focused panel, Selection fill rows → 006,010 |
| 13 | TextInput | widgets/text_input.rs | label underline, no field focus cue, Input>Surface bg → 005,008 (002 Input→Sunken) |
| 14 | TextArea | widgets/text_area.rs | soft-wrap full-row selection, raw colorless flag → 008 |
| 15 | Tag | widgets/tag_chip.rs | remove-part underline, Selection fill → 005,006,007 |
| 16 | Tabs | widgets/tabs.rs | label underline+reverse, gray chips, heavy rule glyph → 002,005,008 |
| 17 | Surface | widgets/surface.rs | Selected=neon fill, no overlay-focused variants → 004 |
| 18 | Stepper | widgets/stepper.rs | underline cursor, reversed accent slab → 005,008 |
| 19 | StatusIndicator | widgets/status_indicator.rs | whole-string status color → 007 |
| 20 | Stack / Inline | layout/stack.rs | clean paint; density-gap defaults recorded as follow-up (README) |
| 21 | SplitPane | widgets/split_pane.rs | heavy-glyph focus (border law) → 006 |
| 22 | Spinner | widgets/spinner.rs | ActivityIndicator loudness, ascii seeding → 007,008 |
| 23 | Slider | widgets/slider.rs | label underline, accent track, Slider≠RangeSlider chrome → 005,008 |
| 24 | Sidebar | widgets/sidebar.rs | Selection slab + Focus slab, `›`/`•` markers, OR-sticky focus → 006,010 |
| 25 | ShortcutHint | widgets/kbd.rs + hint_bar.rs | clean; adoption gap elsewhere → 009 |
| 26 | Select | widgets/select.rs | label underline, chrome override, focused trigger loses well → 005,006,008 |
| 27 | Section | widgets/section.rs | clean (inherits 002 tones) |
| 28 | SearchInput | widgets/search_input.rs | label underline, neon filter chips, NoResults=Warning → 005,007,008 |
| 29 | ResizablePanelGroup | widgets/resizable_panel_group.rs | heavy-glyph focus → 006 |
| 30 | RangeSlider | widgets/slider.rs | divergent thumb chrome → 008 |
| 31 | RadioGroup | widgets/controls.rs | focus/hover underline, legend underline → 005,008 |
| 32 | QuickOpen | widgets/quick_open.rs | Selection tab fill, chrome override, raw flags → 006,008 |
| 33 | Picker | widgets/picker.rs | dead ternary, raw colorless → 006,008 |
| 34 | PasswordInput | widgets/password_input.rs | flat strength hint, ad-hoc reveal → 008 (label via TextInput 005) |
| 35 | Panel | widgets/panel.rs | Surface-filled overlays, no elevation input → 004 |
| 36 | NumberInput | widgets/number_input.rs | label underline, color-only errors → 005,008 |
| 37 | MenuBar | widgets/menu_bar.rs | Selection slab, mnemonic accent flood, destructive-loses-danger → 005,006,007,008 |
| 38 | LogPane | widgets/log_pane.rs | permanent accent follow indicator, no ascii → 013 |
| 39 | LoadingView | widgets/view_state.rs | full cyan line → 007 |
| 40 | List | widgets/list.rs | focus underline + accent label repaint → 004,005,006,012 |
| 41 | KeyboardHelp | widgets/keyboard_help.rs | conflict underline, Selection fill row → 005,006 |
| 42 | KeyValueTable | widgets/key_value_table.rs | chrome match copy, `›` marker, status overwrites selection → 006 |
| 43 | Histogram | widgets/charts.rs (Histogram) | clean; chart defaults → 007 |
| 44 | DetailTable | widgets/detail_table.rs | `▸` marker const, single-state selection → 006 |
| 45 | Collapsible | widgets/collapsible.rs | clean (reference implementation) |
| 46 | ChoiceDialog | widgets/dialog.rs (ChoiceDialog) | Info=Focused border, no backdrop, flat footer → 004,009 |
| 47 | Chip | widgets/tag_chip.rs | as Tag → 005,006,007 |
| 48 | Breadcrumbs | widgets/breadcrumbs.rs | always-on current underline → 005 |

## All other widgets

| Widget file | Disposition → plans |
|---|---|
| accent_rail.rs | no quiet tier, off-palette actor hues → 007 |
| accordion.rs | clean (delegates to Collapsible) |
| action_bar.rs | cursor=ActionFocused slab, style-override drops cursor → 007 |
| agent.rs (TokenMeter) | truncates numbers, no meter form → 007 |
| agent_blocks.rs (ModeRibbon) | always-accent mode chip → 007 |
| alert_dialog.rs | flat hints, confirm phrase discarded → 009 |
| attachment_chips.rs | inherits tag_chip fixes → 005,006,007 |
| badge.rs | focus underline; Outline label=Border color; whole-badge tone → 005,007 |
| blocks.rs | clean (marker type, no paint) |
| breadcrumbs, button_group | button_group black-on-tint focus contrast bug → 007 |
| callout.rs | title underline, tone border/rail/glyph triple → 005,007 |
| card.rs | clean (Panel-composed); container-language reference in 009 |
| carousel.rs | ladder skips Text, raw empty state, BOLD-only focus, un-gated arrows → 013 |
| charts.rs | Accent as data default + 5th series, flat legends, radar-axis underline → 005,007 |
| checkpoint_timeline.rs | single-string rows, accent selection/header → 007,012 |
| citation.rs | legit link underline; policy routing → 005 |
| code_block.rs | search/diff-fallback underline, hardcoded ANSI token colors + style-sniffing → 005 |
| collapsible, composed_row | composed_row single-style paint → 012 |
| connectivity.rs | single-string status row, un-gated `✓` → 007,012 |
| content.rs | H1 label underline (double-paints with rule row) → 005; else clean |
| context_meter.rs | accent actions line, reverse in color path → 007 |
| controls.rs (Checkbox/Switch/Fieldset) | focus/hover underlines → 005,008 |
| data_table.rs | cell Selection fill, cursor underline, per-row system clone, chrome override → 004,005,006,012 |
| data_view.rs | clean (model only) |
| date_time_picker.rs | ragged grid, 5 decorations, green reverse blocks, today-underline → 005,008 |
| dependency_graph.rs | accent filter row → 007; ascii-gated glyphs OK |
| design_inspector.rs | hardcoded chrome report, no active tab → 011 |
| diagnostic.rs | caret rows legit; single-string rows, caret color ignores severity → 007,012 |
| dialog.rs | backdrop dead, Info=Focused, flat footer → 004,009 |
| diff.rs | word-diff underline (color path), cursor line loses word diff, `›` marker → 005,006,007 |
| drawer.rs | 4-side border at dock edge, in-body double-bold title → 009 |
| dropdown_menu.rs | chrome override, destructive-loses-danger-when-active, mark precedence → 004,006,007 |
| edit_core.rs | clean (no paint) |
| empty_state.rs | FirstUse accent title → 007; adoption gaps elsewhere → 009,013 |
| error_state.rs | no container, whole-line danger, un-gated glyphs → 007,009 |
| event_stream.rs | chrome match copy, whole-line severity, single-string rows → 006,007,012 |
| field_row.rs | good citizen; label/value tone inversion + two-line mode → 008 |
| file_picker.rs | chrome override, no footer hint (discarded), whole-row error, Surface fill overlay → 004,006,007,009 |
| file_tree.rs | accent/warning/danger chrome rows → 007 |
| form.rs | phantom blank rows, value hit-region off-by-one, no wells → 008 |
| form_wizard.rs | Surface overlay fill, hand-rolled Next chip → 004,007 |
| fullscreen_viewer.rs | chrome-focus underlines ×3, Selection action chip → 005,006,007 |
| hex_viewer.rs | chrome match copy, accent filter, single-string rows → 006,007,012 |
| highlighted_text.rs | match underline, Warning matches, selected→Accent flips → 005,007 |
| hint_bar.rs | clean (canonical); adoption → 009 |
| history_picker.rs | chrome override, pin column shift, flat hints → 004,006,009 |
| icon.rs | clean (reference for glyph discipline) |
| identity.rs | mono underlines, status roles as identity hues → 005,007 |
| image_surface.rs | no glyph/ascii policy, word-only lifecycle → 013 |
| input_group.rs | dead underline block, always-accent addons → 005,007 |
| input_otp.rs | triple-cue slot, no well → 005,008 |
| jump_overlay.rs | N ActionFocused slabs, no prefix masking, no dim → 007 |
| kbd.rs | clean |
| key_value_list.rs | full-row selection underline; href legit → 005 |
| keybinding_recorder.rs | REVERSED recording slab, Kbd chip discarded, flat 4-tier muted → 013 |
| label.rs | clean |
| link.rs | focus-triggered underline; LinkStyle policy home → 005 |
| loading_overlay.rs | destructive `set_char` wash, cyan labels → 007 |
| log_stream.rs | chrome match copy, whole-line level color, spans flattened, single-string rows → 006,007,012 |
| markdown.rs | underline_row block selection, unconditional em-underline; links legit → 005 |
| mention.rs | Focus-as-text cursor row, `›` marker → 006,007 |
| menu_nav.rs | Selection cursor row, ragged highlight edge → 006,009 |
| message_thread.rs | unconditional accent footer → 007 |
| model_mode_selectors.rs | Focus rows, accent query line, warning-hides-selection, indent drift → 006,007,009 |
| multi_select.rs | label underline, chrome override, no gutter, desc at primary tone → 004,005,006,009 |
| notification_center.rs | Selection row, severity discarded, ragged edge, raw epoch → 006,009 |
| number_input, object_inspector | object_inspector: chrome copy, `·`/`›` markers, single-string rows → 006,007,012 |
| pagination.rs | active-page underline, color-path reverse → 005,007 |
| panel, password_input, path_input | path_input: label+destructive underlines → 005,008 |
| permission.rs | risk-blind Focused border, rail+border double chrome, unaligned muted wall, color-only egress → 007,009 |
| picker, popover | popover: in-body double-bold title, missing header rule, dead colorless arms → 009 |
| preview_card.rs | meta at body tone, pinned=BorderFocused, hand-rolled states → 013 |
| primitives.rs (Button/IconButton) | enabled-state underlines, recipe bypass → 004,005,007 |
| progress.rs | full-width accent running fill → 007 |
| progress_steps.rs | Selection row + whole-row status → 006,007 |
| prompt_composer.rs | Selection sites, always-accent prompt, busy verb, meter width → 006,007 |
| prompt_queue_model.rs | clean (model) |
| question_flow.rs | Focus option rows, multi-choice loses cursor mark, flat hints → 006,007,009 |
| quick_open, resizable_panel_group | above |
| review.rs | ≥10 accent regions/frame, emoji in glyph cluster → 007 |
| scroll_area.rs | un-gated `↓` glyph → 007 |
| search_results.rs | selected row loses highlights, `*`/`›` markers, single-string rows → 006,007,012 |
| section, segmented_control | segmented: focus/hover underlines → 005 |
| select, selection.rs | selection.rs clean (model) |
| semantic_status.rs | Running=Accent, Online≈Running glyphs → 007 |
| separator.rs | double/heavy/`:` rule glyphs → 007 |
| skeleton.rs | clean |
| slash_command_menu.rs | delegates to completion_menu → 006,009 |
| completion_menu.rs | Selection row, hover=Focus, always-BorderFocused while non-focusable, empty top-left → 006,007,009 |
| command_palette.rs | Selection badges, ignores recipe gutter, manual border title → 006,009 |
| slider, spinner, split_pane, status_bar | status_bar: 4-color default band → 007 |
| status_indicator, stepper | above |
| streaming_markdown.rs | `▌` caret collision, colorless dropped, error strip clobbers row → 006,007 |
| surface, table | table: cell underline, no ASCII sort glyph, no column tones → 005,006,012 |
| tabs, tag_chip, terminal_output | terminal_output: chrome copy, single-string rows → 006,012 |
| text.rs | em/code/highlight underlines → 005 |
| text_area, text_input, theme_picker, timeline, toast, toggle | toggle: focus/hover underlines, Solid pressed=Accent → 005,007 |
| token_field, tool_call_card | tool_call_card: Focus action chip → 007 |
| toolbar, tooltip | tooltip: bare Plain/Shortcut variants → 009 |
| trace_waterfall.rs | saturated bars + names, accent filter, single-string rows → 007,012 |
| transcript.rs | selected block accent flood, third gutter variant → 006,007 |
| tree.rs | selection/hover underlines, LinkHover on rows → 004,005 |
| tree_navigation.rs | two full-row slabs, one-style rows, Focus filter row → 006,007 |
| tree_table, view_state, viewport, virtualizer | viewport/virtualizer clean (engine) |
| virtual_grid, virtual_list | above |

## Patterns (all 36)

Structural note: plan **016** owns the widgets-vs-examples split for ALL
patterns (primitive promotions, zero-raw-paint charter, gates, reference
paints for the 4 geometry-only recipes); 010/013 rows below are the paint
sweeps that ride on it.

| Pattern | Disposition → plans |
|---|---|
| activity_shelf | chip status rainbow, REVERSED slab → 010,013 |
| agent_shell, studio_shell, resource_browser, ops_dashboard | geometry-only; ops Tab dead-ends → 013 (GAP-29); reference paints = README follow-up (GAP-30) |
| agent_status_header | 5-hue row, separator hues, wrong drop order → 013 |
| agent_workbench | gold master; one dead statement → 010 |
| app_dashboard | hardcoded ascii, KPI placeholder → 010 |
| app_shell | gold master (clean) |
| approval_queue | 🔒 emoji jitter, dim safety banner, row hues, colorless order → 013 |
| auth_entry, background_task_panel | bg_task: trailing `·`, ANSI dropped, no liveness → 013 |
| connection_manager | REVERSED status slabs, one-style rows, `>` markers → 010 |
| database_workbench | whole-strip accent tabs, 1-of-5 focus borders, double border → 013 |
| error_recovery | color-only Success line, no ascii knob → 013 |
| file_manager | inert colorless, double borders → 013 |
| git_workbench | permanent accent branch row, confirm overpaint, un-gated `↑↓` → 010,013 |
| help_center | 4 raw empties, internals in status bar → 013 |
| integration_status | row hues, accent tabs, un-gated `↗` → 013 |
| metrics_dashboard, observability_dashboard | observability: counts discarded, live-state word-only → 010,013 |
| plan_review | 6-pane accent slabs, risk-row hues, colorless 2/22 → 013 |
| process_table | reference idiom; catalog nits → 013 |
| project_launcher | best EmptyState use; 3 raw empties, double border → 013 |
| prompt_queue | confirm overpaint, warning summary → 013 |
| query_editor | title overpaint, `›` stamps editor cell, dev strings, results discarded → 013 |
| result_grid | title-line hues, blank binary cells, dead role() → 013 |
| schema_browser | dead kind→hue API, brightness-only focus → 013 |
| session_picker | known slab + 5-hue preview, curly quotes, third confirm copy → 010,013 |
| settings_screen, setup_wizard | → 010 |
| subagent_card | second whole-row status site → 013 |
| task_rail | colorless-as-ascii conflation, warning footer → 013 |
| terminal_run_card | Running=Focused border, colorless removes glyph → 013 |
| working_state_card | Waiting==Running under reduced motion, phase hues → 013 |

## Style + infrastructure

| Surface | → plans |
|---|---|
| style/mod.rs, palette.rs, appearance.rs | 002 |
| style/tokens.rs (recipes, SelectionChrome) | 004 |
| style/quantize.rs, glyph.rs, preview_host.rs, capability/boundary.rs | 003 |
| style/motion.rs, density.rs | clean (motion = color remap only) |
| termrock-lookbook (host, svg, frame, demo, app, stories, interactors) | 011 |
| docs preview gates (previews.spec.ts, check-preview-metrics.ts, CI) | 011 |
| design docs (4 SoT files + anatomy/quality standards) | 001 |

Non-visual modules (runtime, input, keymap, interaction, osc, perf, scroll,
ansi_text engine, registry, termrock-cli) — out of look-and-feel scope;
ansi_text underline handling audited LEGIT (SGR passthrough).
