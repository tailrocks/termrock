# Jackin TermRock usage inventory

This is the R1 evidence snapshot for Jackin’s TermRock dependency.

- Jackin: `9e211559` (clean tree)
- TermRock: `e68ed5a6`
- Captured: 2026-08-17
- Recount: 1,067 matching lines across 134 Rust files. The spec’s earlier 137-file figure drifted by three files; the live recount is authoritative.
- Crates: jackin-console 68 files; jackin-capsule 34; jackin-launch 22; jackin-tui 4; jackin-oppicker 3; jackin 2; jackin-xtask 1.

Reference totals use `rg -c 'termrock::' --type rust` matching lines. Per-item counts are advisory occurrences of `termrock::<mod>::<Item>` plus bare item occurrences in the evidence tree; generic names can be inflated. Every item retains a concrete Jackin exemplar.

## Theme

| Usage | Jackin exemplar | Ref count |
|---|---|---:|
| `termrock::Theme` | `crates/jackin-console/src/tui/run.rs:329` | 319 |
| `Theme::default()` | `crates/jackin-console/src/tui/run.rs:329` | 351 |
| StatusBar role override | `crates/jackin-console/src/tui/run.rs:329` | 1 |
| StatusBar role override | `crates/jackin-capsule/src/tui/components/chrome.rs:190` | 1 |
| StatusBar role override | `crates/jackin-launch/src/tui/view.rs:435` | 1 |
| Diff role remap | `crates/jackin-launch/src/tui/run.rs:988` | 2 |

## `style`

| API item | Jackin exemplar | Ref count |
|---|---|---:|
| `Role` | `crates/jackin-capsule/tests/status_bar.rs:70` | 599 |
| `faded` | `crates/jackin-launch/src/tui/components/progress_rail.rs:285` | 1 |

## `scroll`

| API item | Jackin exemplar | Ref count |
|---|---|---:|
| `DialogScroll` | `crates/jackin-capsule/src/daemon/tests.rs:4610` | 59 |
| `ScrollAxes` | `crates/jackin-capsule/src/tui/view.rs:74` | 123 |
| `ScrollAxis` | `crates/jackin-capsule/src/tui/view.rs:117` | 7 |
| `ScrollDelta` | `crates/jackin-console/src/tui/input/mouse/modal_scroll.rs:11` | 2 |
| `ScrollbarGeometry` | `crates/jackin-capsule/src/tui/view.rs:118` | 3 |
| `ScrollbarSpec` | `crates/jackin-capsule/src/tui/view.rs:116` | 3 |
| `TailScroll` | `crates/jackin-capsule/src/tui/view.rs:109` | 14 |
| `apply_scroll_delta` | `crates/jackin-console/src/tui/layout.rs:258` | 7 |
| `apply_scroll_delta_unclamped` | `crates/jackin-console/src/tui/update.rs:861` | 1 |
| `apply_term_width_scroll_delta` | `crates/jackin-console/src/tui/update.rs:873` | 1 |
| `clamp_scroll_offset` | `crates/jackin-console/src/tui/sidebar_layout.rs:532` | 8 |
| `cursor_follow_offset` | `crates/jackin-console/src/tui/focus.rs:39` | 1 |
| `dialog_scroll_axes` | `crates/jackin-capsule/src/tui/components/dialog_widgets.rs:404` | 14 |
| `effective_offset` | `crates/jackin-tui/src/operator_info.rs:363` | 3 |
| `horizontal_scrollbar_area` | `crates/jackin-console/src/tui/screens/workspaces/view.rs:519` | 3 |
| `is_scrollable` | `crates/jackin-console/src/tui/sidebar_layout.rs:544` | 17 |
| `max_line_width` | `crates/jackin-console/src/tui/screens/workspaces/view.rs:422` | 9 |
| `max_offset` | `crates/jackin-console/src/tui/list_geometry.rs:148` | 7 |
| `max_offset_u16` | `crates/jackin-console/src/tui/input/mouse.rs:87` | 5 |
| `mouse_scroll_delta` | `crates/jackin-launch/src/tui/subscriptions.rs:269` | 3 |
| `mouse_scroll_delta_with_step` | `crates/jackin-console/src/tui/update.rs:445` | 2 |
| `offset_for_track_position_u16` | `crates/jackin-launch/src/tui/components/build_log_dialog.rs:136` | 2 |
| `render_line_with_fixed_prefix_scroll` | `crates/jackin-console/src/tui/screens/workspaces/view.rs:548` | 1 |
| `render_lines_with_offset_in_area` | `crates/jackin-console/src/tui/components/confirm_save.rs:23` | 2 |
| `render_scrollbar` | `crates/jackin-capsule/src/tui/view.rs:113` | 3 |
| `scroll_hint_spans` | `crates/jackin-capsule/src/tui/components/dialog/hint.rs:61` | 13 |
| `scrollbar_offset_for_track_position` | `crates/jackin-capsule/src/tui/daemon/mouse_input.rs:312` | 3 |
| `tail_vertical_thumb` | `crates/jackin-capsule/src/tui/pane_snapshot.rs:122` | 4 |
| `vertical_scrollbar_area` | `crates/jackin-capsule/src/tui/view.rs:106` | 10 |
| `viewport_height` | `crates/jackin-console/src/tui/layout.rs:286` | 20 |
| `viewport_width` | `crates/jackin-console/src/tui/layout.rs:281` | 27 |

## `widgets`

| API item | Jackin exemplar | Ref count |
|---|---|---:|
| `Action` | `crates/jackin-capsule/src/tui/components/dialog.rs:1039` | 346 |
| `ActionBar` | `crates/jackin-console/src/tui/components/mount_dst_choice.rs:24` | 8 |
| `ActionBarState` | `crates/jackin-console/src/tui/components/mount_dst_choice.rs:24` | 8 |
| `Anchor` | `crates/jackin-capsule/src/tui/view.rs:344` | 4 |
| `Backdrop` | `crates/jackin-capsule/src/tui/view.rs:204` | 3 |
| `ChoiceDialog` | `crates/jackin-capsule/src/tui/components/dialog.rs:1060` | 8 |
| `ChoiceDialogState` | `crates/jackin-capsule/src/tui/components/dialog.rs:1052` | 12 |
| `DetailCapability` | `crates/jackin-tui/src/operator_info.rs:19` | 10 |
| `DetailRow` | `crates/jackin-tui/src/operator_info.rs:19` | 7 |
| `DetailTable` | `crates/jackin-tui/src/operator_info.rs:19` | 6 |
| `DetailTableOutcome` | `crates/jackin-tui/src/operator_info.rs:19` | 3 |
| `DetailTableState` | `crates/jackin-launch/src/tui/components/prompts.rs:241` | 12 |
| `Dialog` | `crates/jackin-capsule/src/tui/components/dialog.rs:1056` | 272 |
| `DiffKind` | `crates/jackin-launch/src/tui/run.rs:835` | 6 |
| `DiffLine` | `crates/jackin-launch/src/tui/run.rs:835` | 2 |
| `DiffState` | `crates/jackin-launch/src/tui/run.rs:835` | 3 |
| `DiffView` | `crates/jackin-launch/src/tui/run.rs:835` | 2 |
| `EditAction` | `crates/jackin-capsule/src/tui/components/dialog/input.rs:51` | 2 |
| `HintSpan` | `crates/jackin-capsule/src/tui/view.rs:76` | 628 |
| `List` | `crates/jackin-console/src/tui/screens/workspaces/view.rs:871` | 195 |
| `ListRow` | `crates/jackin-console/src/tui/screens/workspaces/view.rs:862` | 30 |
| `ListState` | `crates/jackin-console/src/tui/components/github_picker.rs:8` | 40 |
| `MessageDialog` | `crates/jackin-launch/src/tui/components/prompts.rs:13` | 7 |
| `Panel` | `crates/jackin-launch/src/tui/components/prompts.rs:356` | 21 |
| `PanelEmphasis` | `crates/jackin-capsule/src/tui/components/dialog.rs:1058` | 63 |
| `Progress` | `crates/jackin-launch/src/tui/components/progress_rail.rs:44` | 6 |
| `ProgressKind` | `crates/jackin-launch/src/tui/components/progress_rail.rs:45` | 1 |
| `RowRole` | `crates/jackin-console/src/tui/screens/workspaces/view.rs:866` | 18 |
| `Severity` | `crates/jackin-capsule/src/tui/view.rs:343` | 63 |
| `StatusBar` | `crates/jackin/src/console/adapter/run.rs:445` | 25 |
| `StatusBarState` | `crates/jackin/src/console/adapter/run.rs:429` | 5 |
| `StatusSlot` | `crates/jackin/src/console/adapter/run.rs:400` | 145 |
| `TAB_GAP` | `crates/jackin-capsule/src/tui/components/dialog_widgets.rs:583` | 4 |
| `Tab` | `crates/jackin-console/src/tui/components/editor_rows.rs:211` | 258 |
| `Tabs` | `crates/jackin-console/src/tui/components/editor_rows.rs:220` | 7 |
| `TabsState` | `crates/jackin-console/src/tui/components/editor_rows.rs:223` | 3 |
| `TextInput` | `crates/jackin-launch/src/tui/components/prompts.rs:13` | 162 |
| `TextInputOutcome` | `crates/jackin-launch/src/tui/run.rs:18` | 18 |
| `TextInputState` | `crates/jackin-capsule/src/tui/components/dialog/input.rs:20` | 150 |
| `Toast` | `crates/jackin-capsule/src/tui/view.rs:343` | 1 |
| `Validation` | `crates/jackin-capsule/src/tui/components/dialog_widgets.rs:21` | 18 |
| `Viewport` | `crates/jackin-launch/src/tui/components/build_log_dialog.rs:232` | 8 |
| `lay_out_tabs` | `crates/jackin-capsule/src/tui/components/status_bar.rs:10` | 6 |
| `render_hint_bar` | `crates/jackin-launch/src/tui/view.rs:78` | 11 |
| `tab_at_column` | `crates/jackin-console/src/tui/layout.rs:206` | 1 |
| `wrapped_hint_lines` | `crates/jackin-console/src/tui/view.rs:344` | 3 |

## `text`

| API item | Jackin exemplar | Ref count |
|---|---|---:|
| `display_cols` | `crates/jackin-capsule/tests/status_bar.rs:75` | 49 |
| `display_cols_slice` | `crates/jackin-launch/src/tui/components/failure_dialog.rs:340` | 1 |
| `leading_space_cols` | `crates/jackin-console/src/tui/mount_display.rs:13` | 2 |
| `sanitize_terminal_title` | `crates/jackin-capsule/src/tui/title.rs:12` | 4 |
| `take_display_cols` | `crates/jackin-capsule/src/tui/components/dialog_widgets/usage.rs:822` | 1 |

## `osc`

| API item | Jackin exemplar | Ref count |
|---|---|---:|
| `ClipboardSelection` | `crates/jackin-launch/src/standalone_dialog_sink.rs:70` | 4 |
| `ClipboardWrite` | `crates/jackin-capsule/src/tui/view.rs:413` | 8 |
| `PointerShape` | `crates/jackin-capsule/src/tui/model.rs:13` | 46 |
| `encode_clipboard` | `crates/jackin-capsule/src/tui/view.rs:413` | 4 |
| `encode_hyperlink_close` | `crates/jackin-tui/src/operator_info.rs:578` | 2 |
| `encode_hyperlink_open` | `crates/jackin-tui/src/operator_info.rs:574` | 2 |
| `encode_pointer` | `crates/jackin-launch/src/standalone_dialog_sink.rs:56` | 6 |

## `layout`

| API item | Jackin exemplar | Ref count |
|---|---|---:|
| `DialogSpec` | `crates/jackin-capsule/src/tui/components/modal_rects.rs:283` | 3 |
| `Placement` | `crates/jackin-launch/src/tui/components/dialog.rs:31` | 9 |
| `bottom_rows` | `crates/jackin-launch/src/tui/components/footer.rs:214` | 6 |
| `render_dialog_shell` | `crates/jackin-capsule/src/tui/components/dialog_widgets.rs:560` | 22 |
| `render_scrollable_dialog_body` | `crates/jackin-capsule/src/tui/components/dialog_widgets.rs:602` | 4 |
| `resolve_dialog` | `crates/jackin-capsule/src/tui/components/modal_rects.rs:281` | 3 |

## `input`

| API item | Jackin exemplar | Ref count |
|---|---|---:|
| `KeyBinding` | `crates/jackin-capsule/src/tui/keymap.rs:12` | 207 |
| `KeyChord` | `crates/jackin-capsule/src/tui/keymap/tests.rs:11` | 415 |
| `KeyCode` | `crates/jackin-console/src/tui/keymap/tests.rs:15` | 1759 |
| `KeyEvent` | `crates/jackin-launch/src/tui/subscriptions.rs:629` | 220 |
| `KeyModifiers` | `crates/jackin-capsule/src/tui/scroll_input.rs:10` | 164 |
| `Keymap` | `crates/jackin-capsule/src/tui/keymap/tests.rs:13` | 75 |
| `MouseEventKind` | `crates/jackin-capsule/src/tui/scroll_input.rs:10` | 147 |
| `Visibility` | `crates/jackin-capsule/src/tui/keymap.rs:12` | 184 |

## `keymap`

| API item | Jackin exemplar | Ref count |
|---|---|---:|
| `KeyBinding` | `crates/jackin-console/src/tui/keymap.rs:12` | 207 |
| `KeyChord` | `crates/jackin-console/src/tui/keymap/tests.rs:16` | 415 |
| `Keymap` | `crates/jackin-launch/src/tui/keymap/tests.rs:12` | 75 |
| `SCROLL_HINT_KEYMAP` | `crates/jackin-launch/src/tui/keymap.rs:130` | 3 |
| `Visibility` | `crates/jackin-console/src/tui/keymap.rs:12` | 184 |
| `chord_glyph` | `crates/jackin-capsule/src/tui/components/dialog/hint.rs:25` | 2 |
| `glyph` | `crates/jackin-capsule/src/tui/keymap.rs:13` | 139 |
| `raw_bytes_to_chord` | `crates/jackin-capsule/src/tui/keymap.rs:17` | 23 |

## `interaction`

| API item | Jackin exemplar | Ref count |
|---|---|---:|
| `FocusRing` | `crates/jackin-tui/src/runtime/focus.rs:6` | 6 |
| `HitRegion` | `crates/jackin-capsule/src/tui/daemon/mouse_input.rs:8` | 10 |
| `ModalClickResult` | `crates/jackin-console/src/tui/run.rs:437` | 3 |
| `ModalStack` | `crates/jackin-tui/src/runtime/modal_flow.rs:6` | 3 |
| `Outcome` | `crates/jackin-capsule/src/tui/components/dialog.rs:1066` | 81 |
| `classify_click` | `crates/jackin-capsule/src/tui/components/dialog.rs:938` | 3 |

## `ansi_text`

| API item | Jackin exemplar | Ref count |
|---|---|---:|
| `line_from_ansi` | `crates/jackin-launch/src/tui/components/build_log_dialog.rs:266` | 1 |

## Jackin-owned custom components

### `Widget` implementations

| Component | Jackin evidence |
|---|---|
| `CustomPaneBlit` | `crates/jackin-capsule/benches/pane_body.rs:52` |
| `PaneBodyWidget` | `crates/jackin-capsule/src/tui/components/pane.rs:48` |
| `StatusBarWidget` | `crates/jackin-capsule/src/tui/components/chrome.rs:128` |
| `PaneBorderWidget` | `crates/jackin-capsule/src/tui/components/chrome.rs:256` |
| `BottomChromeWidget` | `crates/jackin-capsule/src/tui/components/chrome.rs:291` |
| `DialogBottomChromeWidget` | `crates/jackin-capsule/src/tui/components/chrome.rs:326` |
| `BrandHeader` | `crates/jackin-console/src/tui/components/brand_header.rs:14` |

`CustomPaneBlit` is bench-only; all other rows are production surface.

### Function-style component entry points

| Entry point | Jackin evidence |
|---|---|
| `CommandPalette` | `crates/jackin-capsule/src/tui/components/palette.rs:30` |
| `ProgressRail` | `crates/jackin-launch/src/tui/components/progress_rail.rs:26` |
| `DigitalRain` | `crates/jackin-launch/src/tui/components/rain.rs:182` |
| `github_context_view_from_state` | `crates/jackin-capsule/src/tui/components/dialog/github_context.rs:20` |
| `CapsuleModalRect` | `crates/jackin-capsule/src/tui/components/modal_rects.rs:135` |
| `status_bar_plan` | `crates/jackin-capsule/src/tui/components/status_bar.rs:213` |
| `agent_picker_label` | `crates/jackin-console/src/tui/components/agent_choice.rs:79` |
| `auth_source_picker_state` | `crates/jackin-console/src/tui/components/auth_panel.rs:70` |
| `render_brand_header` | `crates/jackin-console/src/tui/components/brand_header.rs:50` |
| `confirm_save_hint_spans` | `crates/jackin-console/src/tui/components/confirm_save.rs:241` |
| `debug_run_info_state` | `crates/jackin-console/src/tui/components/container_info.rs:14` |
| `text_input_hint_spans` | `crates/jackin-console/src/tui/components/dialogs.rs:25` |
| `cursor_span` | `crates/jackin-console/src/tui/components/editor_rows.rs:26` |
| `secret_display` | `crates/jackin-console/src/tui/components/env_value.rs:6` |
| `error_popup_state` | `crates/jackin-console/src/tui/components/error_popup.rs:8` |
| `page_rows_for_modal` | `crates/jackin-console/src/tui/components/file_browser.rs:42` |
| `git_prompt_rect` | `crates/jackin-console/src/tui/components/file_browser/git_prompt.rs:146` |
| `listing_rect` | `crates/jackin-console/src/tui/components/file_browser/render.rs:35` |
| `tab_bar_footer_items` | `crates/jackin-console/src/tui/components/footer_hints/common.rs:16` |
| `editor_screen_footer_items` | `crates/jackin-console/src/tui/components/footer_hints/editor.rs:40` |
| `modal_footer_items` | `crates/jackin-console/src/tui/components/footer_hints/modals.rs:141` |
| `settings_general_row_footer_items` | `crates/jackin-console/src/tui/components/footer_hints/settings.rs:18` |
| `workspace_screen_footer_items` | `crates/jackin-console/src/tui/components/footer_hints/workspace.rs:220` |
| `github_open_plan` | `crates/jackin-console/src/tui/components/github_picker.rs:31` |
| `ConsoleModalRect` | `crates/jackin-console/src/tui/components/modal_rects.rs:236` |
| `MountDestinationChoice` | `crates/jackin-console/src/tui/components/mount_dst_choice.rs:94` |
| `render_mount_header` | `crates/jackin-console/src/tui/components/mount_rows.rs:19` |
| `push_op_breadcrumb_spans` | `crates/jackin-console/src/tui/components/op_breadcrumb.rs:14` |
| `sentinel_line` | `crates/jackin-console/src/tui/components/op_picker/lines.rs:19` |
| `render_picker` | `crates/jackin-console/src/tui/components/op_picker/render.rs:21` |
| `RolePicker` | `crates/jackin-console/src/tui/components/role_picker.rs:124` |
| `editor_exit_save_discard_state` | `crates/jackin-console/src/tui/components/save_discard.rs:6` |
| `workspace_create_display_name` | `crates/jackin-console/src/tui/components/save_preview.rs:66` |
| `ScopePicker` | `crates/jackin-console/src/tui/components/scope_picker.rs:78` |
| `SourcePicker` | `crates/jackin-console/src/tui/components/source_picker.rs:85` |
| `status_popup_state` | `crates/jackin-console/src/tui/components/status_popup.rs:8` |
| `WorkdirPicker` | `crates/jackin-console/src/tui/components/workdir_pick.rs:134` |
| `warp_intro` | `crates/jackin-launch/src/animation.rs:308` |
| `prelaunch_select_choice` | `crates/jackin-launch/src/progress.rs:311` |
| `render_launch_frame` | `crates/jackin-launch/src/tui/view.rs:25` |
| `ready_blocking_subscription` | `crates/jackin-oppicker/src/adapters.rs:60` |
| `clamp_dialog_scroll` | `crates/jackin-tui/src/operator_info.rs:357` |
