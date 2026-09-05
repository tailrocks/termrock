# Phase 4/5 Port Plan — workstream batches

Rule: one batch = one implementation agent; batches never share files.
Each port follows reference-spec.md §4.x (exact geometry/keys/states) + §5/§6
composition + §7 kernel conventions, with junie tokens from Phase 3.
Verification: harness scenario (verify/junie) or lookbook story + text-grid
diff; behavioral tests for keys/mouse/focus/boundaries per GOAL.

## Direct-match mapping (junie → termrock files)

| junie (spec §) | termrock files |
|---|---|
| Button (4.1) | primitives.rs (Button/IconButton/ActivationState), button_group.rs, toolbar.rs |
| Chips (4.2) | tag_chip.rs, token_field.rs, attachment_chips.rs, badge.rs |
| Choice (4.3) | controls.rs (checkbox/radio/switch/select/multiselect), toggle.rs |
| Completion (4.4) | completion_menu.rs, slash_command_menu.rs, combobox.rs |
| Dialog (4.5) | dialog.rs, alert_dialog.rs, confirm_prompt.rs |
| EmptyState (4.6) | empty_state.rs |
| TextInput (4.7) | text_input.rs, number_input.rs, password_input.rs, path_input.rs, search_input.rs, input_group.rs, edit_core.rs |
| TextArea (4.8) | text_area.rs |
| Select (4.9) | select.rs, picker.rs (popup id), multi_select.rs |
| List (4.10) | list.rs, virtual_list.rs, selection.rs, row_chrome.rs, sidebar.rs |
| Tree (4.11) | tree.rs, file_tree.rs, tree_table.rs, tree_navigation.rs |
| Tabs (4.12) | tabs.rs |
| Picker (4.13) | quick_open.rs, command_palette.rs, history_picker.rs |
| Progress (4.14) | progress.rs, spinner.rs |
| Segments (4.15) | segmented_control.rs, status_strip.rs, chrome_row.rs |
| Keyhint (4.16) | hint_bar.rs, kbd.rs, keyboard_help.rs |
| Scrollbar (4.17) | scroll_area.rs, viewport.rs |
| Table (4.18) | table.rs, data_table.rs, detail_table.rs, key_value_table.rs, table_chrome.rs, data_view.rs |
| CodeEditor (4.19) | code_block.rs |
| DataGrid (4.20) | virtual_grid.rs, virtualizer.rs |
| Props (4.21) | key_value_list.rs, key_value_table.rs (props rows), field_row.rs |
| ScrollPanel (4.22) | panel.rs |
| Panel/card/frame | panel.rs, card.rs, surface.rs, section.rs |
| Select-popup/anchored | popover.rs, drawer.rs |
| Key hints footer | status_bar.rs |

## Batches (disjoint files)

- B1 button: primitives.rs, button_group.rs, toolbar.rs, action_bar.rs
- B2 choice: controls.rs, toggle.rs, slider.rs(derive), date_time_picker.rs(derive)
- B3 input: text_input.rs, number_input.rs, password_input.rs, path_input.rs,
  search_input.rs, input_group.rs, edit_core.rs, form.rs, label.rs,
  field_message.rs, field_row.rs
- B4 textarea: text_area.rs
- B5 select: select.rs, multi_select.rs, combobox.rs, picker.rs
- B6 list: list.rs, virtual_list.rs, selection.rs, row_chrome.rs, sidebar.rs
- B7 tree: tree.rs, file_tree.rs, tree_table.rs, tree_navigation.rs, file_picker.rs
- B8 tabs+segments: tabs.rs, segmented_control.rs, status_strip.rs, chrome_row.rs
- B9 dialog: dialog.rs, alert_dialog.rs, confirm_prompt.rs, popover.rs, drawer.rs
- B10 table: table.rs, data_table.rs, detail_table.rs, key_value_table.rs,
  table_chrome.rs, data_view.rs, virtual_grid.rs, virtualizer.rs
- B11 panel+scroll: panel.rs, card.rs, surface.rs, section.rs, scroll_area.rs,
  viewport.rs, collapsible.rs, accordion.rs, resizable_panel_group.rs, split_pane.rs
- B12 progress: progress.rs, spinner.rs, skeleton.rs, loading_overlay.rs,
  progress_steps.rs(derive), stepper.rs(derive)
- B13 hints+status: hint_bar.rs, kbd.rs, keyboard_help.rs, status_bar.rs,
  status_indicator.rs, semantic_status.rs, breadcrumbs.rs(derive)
- B14 code+completion: code_block.rs, completion_menu.rs, slash_command_menu.rs
- B15 empty/error: empty_state.rs, error_state.rs, connectivity.rs, view_state.rs,
  diagnostic.rs(derive)
- B16 pickers: quick_open.rs, command_palette.rs, history_picker.rs,
  search_results.rs, jump_overlay.rs(derive)
- B17 text: text.rs, content.rs, highlighted_text.rs, markdown.rs,
  streaming_markdown.rs, log_pane.rs, log_stream.rs, terminal_output.rs,
  event_stream.rs, transcript.rs, message_thread.rs (visual language pass)
- B18 chips+misc: tag_chip.rs, token_field.rs, attachment_chips.rs, badge.rs,
  identity.rs, mention.rs, citation.rs, link.rs, icon.rs
- B19 compose/apps (Phase 7): showcase, lookbook, lookbook-web, cli, examples,
  patterns/* (35 files)

## Order

1. After Phase 3 lands: B13 (hints/status — cheapest, validates tokens),
   B1 (button), B12 (progress), B15 (empty) in parallel — foundation feel.
2. Then B3, B5, B6, B8, B9 in parallel.
3. Then B2, B4, B7, B10, B11, B14, B16, B18.
4. B17 language pass + B19 apps last.

Every batch: fresh verifier agent afterwards (not the implementer), harness
scenarios added, behavioral tests updated. Derive-classified widgets inside a
batch (e.g. slider) follow Phase 5 multi-proposal process when nontrivial.
