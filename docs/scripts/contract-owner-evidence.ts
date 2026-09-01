import type { AxisApplicability, AxisName, AxisStatus } from './catalog-data'

export type OwnerAxisEvidence = Readonly<{
  applicability: AxisApplicability
  status: AxisStatus
  tests: readonly string[]
  reason: string
}>

export type OwnerEvidence = Readonly<{
  id: string
  axes: Readonly<Partial<Record<AxisName, OwnerAxisEvidence>>>
}>

type Code = 'r' | 'rp' | 'c' | 'cp' | 'caller' | 'na'
const behaviorAxes = ['keyboard', 'mouse', 'focus', 'disabled', 'overlay', 'escape'] as const

function behavior(
  id: string,
  tests: readonly string[],
  overrides: Readonly<Partial<Record<(typeof behaviorAxes)[number], Code>>> = {},
): OwnerEvidence {
  const axes = Object.fromEntries(behaviorAxes.map((axis) => {
    const code = overrides[axis] ?? 'r'
    const applicability = code === 'r' || code === 'rp' ? 'required' : 'conditional'
    const status: AxisStatus = code === 'na'
      ? 'not_applicable'
      : code === 'caller'
        ? 'caller_owned'
        : code === 'cp' || code === 'rp'
          ? 'partial'
          : 'covered'
    const reason = code === 'na'
      ? `Owner-local typed API and behavioral tests prove ${id} owns no ${axis} state or event path.`
      : code === 'caller'
        ? `${id} exposes semantic composition data; the enclosing host owns the ${axis} path.`
      : code === 'cp' || code === 'rp'
          ? `Owner-local behavior proves one ${axis} path; alternate paths remain unproven.`
          : `Owner-local behavioral tests exercise the declared ${axis} contract.`
    return [axis, { applicability, status, tests, reason }]
  }))
  return { id, axes }
}

function specific(
  id: string,
  tests: readonly string[],
  claims: Readonly<Partial<Record<AxisName, Code>>>,
): OwnerEvidence {
  const axes = Object.fromEntries(Object.entries(claims).map(([axis, code]) => {
    if (code === undefined) throw new Error(`${id}.${axis}: missing owner evidence code`)
    const applicability = code === 'r' || code === 'rp' ? 'required' : 'conditional'
    const status: AxisStatus = code === 'na'
      ? 'not_applicable'
      : code === 'caller'
        ? 'caller_owned'
        : code === 'cp' || code === 'rp'
          ? 'partial'
          : 'covered'
    const reason = code === 'na'
      ? `Owner-local typed API and behavioral tests prove ${id} owns no ${axis} state or event path.`
      : code === 'caller'
        ? `${id} exposes semantic composition data; the enclosing host owns the ${axis} path.`
      : code === 'cp' || code === 'rp'
          ? `Owner-local behavior proves one ${axis} path; alternate paths remain unproven.`
          : `Owner-local behavioral tests exercise the declared ${axis} contract.`
    return [axis, { applicability, status, tests, reason }]
  }))
  return { id, axes }
}

const widget = (file: string, symbol: string): string => `crates/termrock/src/widgets/${file}#${symbol}`
const interaction = (file: string, symbol: string): string => `crates/termrock/src/interaction/${file}#${symbol}`
const crateRoot = (file: string, symbol: string): string => `crates/termrock/src/${file}#${symbol}`
const integration = (file: string, symbol: string): string => `crates/termrock/tests/${file}#${symbol}`
const pattern = (file: string, symbol: string): string => `crates/termrock/src/patterns/${file}#${symbol}`
const scroll = (file: string, symbol: string): string => `crates/termrock/src/scroll/${file}#${symbol}`

export const ownerEvidence: readonly OwnerEvidence[] = [
  behavior('FocusGraph', [
    interaction('focus_graph.rs', 'hybrid_skips_spatial_inside_roving'),
    interaction('focus_graph.rs', 'focus_at_pointer_does_not_require_activate'),
    interaction('focus_graph.rs', 'trap_restores_opener'),
    interaction('focus_graph.rs', 'reconcile_drops_disabled'),
  ], { overlay: 'na', escape: 'na' }),
  behavior('InteractionScene', [
    interaction('scene.rs', 'focus_preserved_by_stable_id_across_reorder'),
    interaction('scene.rs', 'outside_click_only_applies_top_policy'),
    interaction('scene.rs', 'disabled_and_hidden_never_focus_or_hit_action'),
    interaction('scene.rs', 'nested_menus_peel_one_layer_per_escape'),
    interaction('scene.rs', 'dismiss_top_restores_focus_return'),
  ]),
  behavior('OverlayStack', [
    interaction('overlay_stack.rs', 'dismissable_pointer_press_release_and_double_guard'),
    interaction('overlay_stack.rs', 'story_opener_focus_restoration'),
    interaction('overlay_stack.rs', 'story_spec_constructors_for_all_kinds'),
    interaction('overlay_stack.rs', 'story_policy_table_covers_all_kinds'),
    interaction('overlay_stack.rs', 'double_escape_same_stack_peels_one_per_call'),
  ], { keyboard: 'caller', disabled: 'na' }),
  behavior('RovingFocusGroup', [
    interaction('roving.rs', 'home_end_and_intent'),
    interaction('roving.rs', 'skips_disabled_and_wraps'),
    interaction('roving.rs', 'reconcile_after_disable_active'),
  ], { mouse: 'na', overlay: 'na', escape: 'na' }),
  behavior('SemanticScene', [
    interaction('scene.rs', 'semantic_focus_neighbor_and_by_role'),
    interaction('scene.rs', 'semantic_register_many_and_focus_nodes'),
    interaction('scene.rs', 'semantic_snapshot_roundtrip_text'),
  ], { keyboard: 'na', mouse: 'na', overlay: 'na', escape: 'na' }),
  behavior('UiContext', [
    crateRoot('context.rs', 'focus_request_via_context'),
    crateRoot('context.rs', 'context_routes_overlay_pointer_gesture_and_one_escape_layer'),
  ], { keyboard: 'caller', disabled: 'caller' }),
  behavior('Backdrop', [
    widget('dialog.rs', 'backdrop_from_design_system_dims'),
    interaction('overlay_stack.rs', 'dim_policy_washes_the_whole_layer_not_just_the_overlay'),
  ], { keyboard: 'na', mouse: 'na', focus: 'na', disabled: 'na', overlay: 'caller', escape: 'na' }),
  behavior('Callout', [widget('callout.rs', 'semantic_callout_and_alert')], {
    keyboard: 'na', mouse: 'na', focus: 'na', disabled: 'na', overlay: 'na', escape: 'na',
  }),
  behavior('LoadingOverlay', [
    widget('loading_overlay.rs', 'blocking_swallows_keys'),
    widget('loading_overlay.rs', 'pointer_outside_is_outside'),
    widget('loading_overlay.rs', 'blocking_paint_after_min_show'),
    widget('loading_overlay.rs', 'cancellable_esc_requests_cancel'),
  ], { focus: 'na', disabled: 'na' }),
  behavior('MessageDialog', [
    widget('dialog.rs', 'message_details_start_after_wrapped_body'),
    widget('dialog.rs', 'dialog_opens_on_overlay_stack_with_opener_restore'),
  ], { keyboard: 'caller', mouse: 'caller', focus: 'caller', disabled: 'na', overlay: 'caller', escape: 'caller' }),
  behavior('Toast', [
    widget('toast.rs', 'never_focusable'),
    widget('toast.rs', 'single_toast_kinds_paint'),
  ], { keyboard: 'na', mouse: 'na', focus: 'na', disabled: 'na', overlay: 'na', escape: 'na' }),
  behavior('ToastStack', [
    widget('toast.rs', 'esc_dismisses_the_newest_toast'),
    widget('toast.rs', 'anchors_and_margins_resolve_inside_the_outer_area'),
    widget('toast.rs', 'stack_paint_and_announcement'),
    widget('toast.rs', 'never_focusable'),
  ], { mouse: 'na', focus: 'na', disabled: 'na' }),
  behavior('Tooltip', [
    widget('tooltip.rs', 'disabled_never_shows'),
    widget('tooltip.rs', 'never_focusable_const'),
    widget('tooltip.rs', 'overlay_no_input_ownership'),
    widget('tooltip.rs', 'overlay_size_spends_one_chrome_ring_and_declares_it_minimum'),
  ], { keyboard: 'na', mouse: 'caller', focus: 'na', disabled: 'c', escape: 'na' }),
  behavior('AlertDialog', [
    widget('alert_dialog.rs', 'every_dismiss_and_focus_path_matrix'),
    widget('alert_dialog.rs', 'click_safe_and_confirm_paths'),
    widget('alert_dialog.rs', 'initial_focus_is_safe_cancel'),
    widget('alert_dialog.rs', 'typed_gate_blocks_confirm_until_match'),
    widget('alert_dialog.rs', 'overlay_open_alert_kind_and_opener_restore'),
    widget('alert_dialog.rs', 'locked_esc_trapped'),
  ], { disabled: 'c' }),
  behavior('ChoiceDialog', [
    widget('dialog.rs', 'choice_dialog_skips_disabled_actions_and_returns_semantic_outcomes'),
    widget('dialog.rs', 'choice_dialog_mouse_outcomes_follow_enabled_painted_regions'),
    widget('dialog.rs', 'choice_dialog_accepts_input_gate'),
    widget('dialog.rs', 'dialog_opens_on_overlay_stack_with_opener_restore'),
    widget('dialog.rs', 'confirm_only_esc_traps_without_cancel_action'),
  ], { disabled: 'c' }),
  behavior('CommandPalette', [
    widget('command_palette.rs', 'nested_page_and_esc_layers'),
    widget('command_palette.rs', 'mouse_hit_activates_same_enabled_command_as_keyboard'),
    widget('command_palette.rs', 'accepts_input_gate'),
    widget('command_palette.rs', 'disabled_not_activated'),
    widget('command_palette.rs', 'overlay_focus_restore_and_fullscreen'),
  ], { disabled: 'c' }),
  behavior('CompletionMenu', [
    widget('completion_menu.rs', 'keyboard_tab_enter_esc_via_intents'),
    widget('completion_menu.rs', 'mouse_click_commits_selected_hit'),
    widget('completion_menu.rs', 'active_descendant_not_focus_trap'),
    widget('completion_menu.rs', 'open_on_overlay_stack_and_dismiss'),
    widget('completion_menu.rs', 'fullscreen_open_on_stack'),
  ], { focus: 'caller', disabled: 'c' }),
  behavior('Dialog', [
    widget('dialog.rs', 'enter_does_not_submit_from_body_zone'),
    widget('dialog.rs', 'choice_dialog_mouse_outcomes_follow_enabled_painted_regions'),
    widget('dialog.rs', 'choice_dialog_skips_disabled_actions_and_returns_semantic_outcomes'),
    widget('dialog.rs', 'dialog_opens_on_overlay_stack_with_opener_restore'),
    widget('dialog.rs', 'confirm_only_esc_traps_without_cancel_action'),
  ], { disabled: 'c' }),
  behavior('Drawer', [
    widget('drawer.rs', 'esc_and_intent_close'),
    widget('drawer.rs', 'mouse_resize_uses_the_painted_handle_and_input_gate'),
    widget('drawer.rs', 'open_close_restores_opener'),
    widget('drawer.rs', 'modal_vs_non_modal_policy'),
    widget('drawer.rs', 'fullscreen_on_tiny_bounds'),
  ], { disabled: 'c' }),
  behavior('DropdownMenu', [
    widget('dropdown_menu.rs', 'open_close_and_activate'),
    widget('dropdown_menu.rs', 'mouse_activates_only_enabled_painted_menu_rows'),
    widget('dropdown_menu.rs', 'checkbox_radio_destructive_disabled'),
    widget('dropdown_menu.rs', 'exhaustive_nested_overlay_stack'),
    widget('dropdown_menu.rs', 'nested_submenu_and_layer_dismiss'),
  ], { disabled: 'c' }),
  behavior('FullscreenViewer', [
    widget('fullscreen_viewer.rs', 'search_and_help_keys'),
    widget('fullscreen_viewer.rs', 'mouse_uses_painted_action_regions_and_disabled_gate'),
    widget('fullscreen_viewer.rs', 'semantic_registers'),
    widget('fullscreen_viewer.rs', 'open_close_restores_opener_focus'),
    widget('fullscreen_viewer.rs', 'escape_help_then_search_then_demote'),
    widget('fullscreen_viewer.rs', 'nested_overlay_stack_escape_one_layer'),
  ], { disabled: 'c' }),
  behavior('HistoryPicker', [
    widget('history_picker.rs', 'select_applies_value_and_discards_draft'),
    widget('history_picker.rs', 'mouse_hit_selects_the_painted_history_entry'),
    widget('history_picker.rs', 'accepts_input_gate'),
    widget('history_picker.rs', 'overlay_restore_focus'),
    widget('history_picker.rs', 'draft_preserved_on_cancel'),
  ], { disabled: 'na' }),
  behavior('JumpOverlay', [
    widget('jump_overlay.rs', 'badge_key_activates_and_closes'),
    widget('jump_overlay.rs', 'click_activates'),
    widget('jump_overlay.rs', 'jump_targets_from_semantic_scene'),
    widget('jump_overlay.rs', 'jump_opens_fullscreen_layer_and_esc_restores_opener'),
    widget('jump_overlay.rs', 'esc_clears_prefix_then_dismisses'),
  ], { disabled: 'c' }),
  behavior('KeyboardHelp', [
    widget('keyboard_help.rs', 'modal_esc_closes'),
    widget('keyboard_help.rs', 'mouse_hit_moves_the_modal_help_cursor'),
    widget('keyboard_help.rs', 'semantic_registers'),
    widget('keyboard_help.rs', 'overlay_dismiss_restores_focus'),
  ], { disabled: 'na' }),
  behavior('MenuBar', [
    widget('menu_bar.rs', 'bar_roving_and_switch_while_open'),
    widget('menu_bar.rs', 'mouse_bar_hit_opens_the_same_menu_model_as_keyboard'),
    widget('menu_bar.rs', 'disabled_skipped_on_roving'),
    widget('menu_bar.rs', 'overlay_stack_nested_dismiss_restores_focus'),
    widget('menu_bar.rs', 'nested_submenu_and_layer_dismiss'),
  ], { disabled: 'c' }),
  behavior('NotificationCenter', [
    widget('notification_center.rs', 'keyboard_nav_and_esc_close'),
    widget('notification_center.rs', 'mouse_row_hit_selects_and_marks_read'),
    widget('notification_center.rs', 'semantic_registers'),
    widget('notification_center.rs', 'overlay_open_close'),
  ], { disabled: 'na' }),
  behavior('PermissionPrompt', [
    widget('permission.rs', 'default_permission_intent_has_no_y_grant'),
    widget('permission.rs', 'mouse_confirm_uses_hit_regions'),
    widget('permission.rs', 'default_focus_is_never_allow'),
    widget('permission.rs', 'accepts_input_false_blocks_all_grants'),
    widget('permission.rs', 'permission_overlay_opens_alert_for_high_risk'),
    widget('permission.rs', 'esc_cancels_without_grant_and_advances_queue'),
  ], { disabled: 'c' }),
  behavior('Picker', [
    widget('picker.rs', 'unicode_query_navigation_activation_and_two_stage_escape_are_disjoint'),
    widget('picker.rs', 'mouse_activation_delegates_to_painted_list_geometry'),
    widget('picker.rs', 'disabled_and_separator_rows_never_become_fallbacks'),
    widget('picker.rs', 'picker_overlay_helpers_open_and_dismiss'),
    widget('picker.rs', 'accepts_input_gate'),
  ], { disabled: 'c' }),
  behavior('Popover', [
    widget('popover.rs', 'esc_requests_close'),
    widget('popover.rs', 'outside_click_dismisses_non_modal'),
    widget('popover.rs', 'open_close_restores_opener_focus'),
    widget('popover.rs', 'disabled_popover_rejects_input_and_registers_disabled_semantics'),
    widget('popover.rs', 'modal_open_uses_focus_trap_policy'),
  ], { mouse: 'caller', disabled: 'c' }),
  behavior('PreviewCard', [
    widget('preview_card.rs', 'pin_open_enter'),
    widget('preview_card.rs', 'delay_then_show_no_focus_theft'),
    widget('preview_card.rs', 'semantic_registers_unpinned_not_focusable'),
    widget('preview_card.rs', 'overlay_no_input_when_unpinned'),
    widget('preview_card.rs', 'overlay_pinned_uses_popover'),
    widget('preview_card.rs', 'pin_survives_pointer_leave'),
  ], { keyboard: 'c', mouse: 'caller', focus: 'caller', disabled: 'na', escape: 'c' }),
  behavior('QuickOpen', [
    widget('quick_open.rs', 'provider_switch_preserves_query'),
    widget('quick_open.rs', 'mouse_result_hit_activates_the_canonical_provider_item'),
    widget('quick_open.rs', 'accepts_input_gate'),
    widget('quick_open.rs', 'overlay_fullscreen_and_restore'),
  ], { disabled: 'na' }),
  behavior('SlashCommandMenu', [
    widget('slash_command_menu.rs', 'key_nav_selection'),
    widget('slash_command_menu.rs', 'mouse_uses_completion_menu_painted_hit_regions'),
    widget('slash_command_menu.rs', 'bridge_from_command_entries'),
    widget('slash_command_menu.rs', 'overlay_opens'),
    widget('slash_command_menu.rs', 'draft_sync_opens_and_closes'),
    widget('slash_command_menu.rs', 'commit_command_without_args_closes'),
  ], { focus: 'caller', disabled: 'c' }),
  behavior('Accordion', [
    widget('accordion.rs', 'home_end_roving'),
    widget('accordion.rs', 'mouse_toggles_and_focuses'),
    widget('accordion.rs', 'focus_independent_of_open'),
    widget('accordion.rs', 'disabled_not_activated'),
  ], { disabled: 'c', overlay: 'na', escape: 'na' }),
  behavior('Alert', [
    widget('callout.rs', 'alert_action_activation'),
    widget('callout.rs', 'disabled_alert_is_noninteractive_and_semantically_disabled'),
    widget('callout.rs', 'unfocused_alert_ignores_keys'),
    widget('callout.rs', 'alert_esc_dismisses'),
  ], { disabled: 'c', overlay: 'na' }),
  behavior('Breadcrumbs', [
    widget('breadcrumbs.rs', 'breadcrumbs_navigate'),
    widget('breadcrumbs.rs', 'mouse_navigate'),
    widget('breadcrumbs.rs', 'single_tab_stop_internal_move'),
    widget('breadcrumbs.rs', 'disabled_segment_ignored'),
    widget('breadcrumbs.rs', 'editable_esc_cancels'),
  ], { disabled: 'c', overlay: 'na', escape: 'c' }),
  behavior('Carousel', [
    widget('carousel.rs', 'mouse_footer_matches_keyboard_and_input_gate'),
    widget('carousel.rs', 'next_prev_wrap_and_activate'),
  ], { disabled: 'na', overlay: 'na' }),
  behavior('Collapsible', [
    widget('collapsible.rs', 'expand_collapse_keys'),
    widget('collapsible.rs', 'mouse_toggle_on_trigger'),
    widget('collapsible.rs', 'unfocused_ignores_keys'),
    widget('collapsible.rs', 'disabled_ignores_keys_and_mouse'),
  ], { disabled: 'c', overlay: 'na', escape: 'na' }),
  behavior('EmptyState', [
    widget('empty_state.rs', 'primary_activation_enter'),
    widget('empty_state.rs', 'mouse_primary_activation'),
    widget('empty_state.rs', 'tab_cycles_actions'),
    widget('empty_state.rs', 'semantic_registers'),
  ], { disabled: 'na', overlay: 'na', escape: 'na' }),
  behavior('ErrorState', [
    widget('error_state.rs', 'retry_activation_and_safety_line'),
    widget('error_state.rs', 'mouse_retry'),
    widget('error_state.rs', 'tab_cycles_recovery'),
    widget('error_state.rs', 'semantic_registers'),
  ], { disabled: 'na', overlay: 'na', escape: 'na' }),
  behavior('List', [
    widget('list.rs', 'keyboard_skips_non_items_and_returns_stable_ids'),
    widget('list.rs', 'render_reveals_selection_and_mouse_uses_painted_regions'),
    widget('list.rs', 'phosphor_selection_is_a_gutter_not_neon'),
    widget('list.rs', 'handle_intent_moves_and_activates_without_raw_keys'),
    widget('list.rs', 'list_state_accessors_preserve_semantic_ownership'),
  ], { disabled: 'c', overlay: 'na' }),
  behavior('NavigationList', [
    widget('sidebar.rs', 'handle_intent_move_skips_collapsed_children'),
    widget('sidebar.rs', 'mouse_route'),
    widget('sidebar.rs', 'route_distinct_from_focus'),
    widget('sidebar.rs', 'disabled_not_routed'),
    widget('sidebar.rs', 'filter_search'),
  ], { disabled: 'c', overlay: 'caller', escape: 'c' }),
  behavior('Pagination', [
    widget('pagination.rs', 'keys_bracket_nav'),
    widget('pagination.rs', 'paint_full_and_mouse'),
    widget('pagination.rs', 'presentation_by_width'),
    widget('pagination.rs', 'loading_disables_nav'),
    widget('pagination.rs', 'jump_entry'),
    widget('pagination.rs', 'disabled_state_rejects_keyboard_mouse_and_registers_semantics'),
    widget('pagination.rs', 'escape_closes_only_the_jump_entry_layer'),
  ], { disabled: 'c', overlay: 'na', escape: 'c' }),
  behavior('SearchResults', [
    widget('search_results.rs', 'nav_open_preview_group'),
    widget('search_results.rs', 'mouse_hit_uses_painted_row_identity_and_input_gate'),
    widget('search_results.rs', 'accepts_input_gate'),
    widget('search_results.rs', 'cancel_while_loading'),
  ], { focus: 'caller', disabled: 'c', overlay: 'na' }),
  behavior('Sidebar', [
    widget('sidebar.rs', 'sidebar_route_change_is_explicit'),
    widget('sidebar.rs', 'mouse_route'),
    widget('sidebar.rs', 'route_distinct_from_focus'),
    widget('sidebar.rs', 'disabled_not_routed'),
    widget('sidebar.rs', 'apply_width_to_rail_and_drawer_chords'),
    widget('sidebar.rs', 'sidebar_escape_blurs_without_changing_route'),
  ], { disabled: 'c', overlay: 'caller' }),
  behavior('Tabs', [
    widget('tabs.rs', 'automatic_activation_on_arrow'),
    widget('tabs.rs', 'mouse_select'),
    widget('tabs.rs', 'every_active_cue_marks_the_active_tab_differently'),
    widget('tabs.rs', 'disabled_tabs_are_not_focusable_or_activatable'),
    widget('tabs.rs', 'narrow_select_presentation'),
    widget('tabs.rs', 'overflow_presentation'),
    widget('tabs.rs', 'escape_closes_only_the_overflow_layer'),
  ], { disabled: 'c', overlay: 'caller', escape: 'c' }),
  behavior('Tree', [
    widget('tree.rs', 'handle_intent_expands_collapses_and_cancels'),
    integration('tree.rs', 'painted_disclosure_and_selected_row_have_distinct_mouse_outcomes'),
    widget('tree.rs', 'keyboard_skips_loading_disabled'),
  ], { disabled: 'c', overlay: 'na' }),
  behavior('TreeNavigation', [
    widget('tree_navigation.rs', 'enter_sets_route_on_leaf'),
    widget('tree_navigation.rs', 'paint_and_mouse'),
    widget('tree_navigation.rs', 'route_distinct_from_focus'),
    widget('tree_navigation.rs', 'disabled_nodes_are_skipped_and_cannot_be_activated_directly'),
    widget('tree_navigation.rs', 'context_menu'),
    widget('tree_navigation.rs', 'escape_cancels_one_navigation_layer_without_changing_route'),
  ], { disabled: 'c', overlay: 'caller' }),
  ...([
    ['ActionLink', 'link.rs', 'action_link_not_navigation'],
    ['AgentModeSelector', 'model_mode_selectors.rs', 'accepts_input_gate'],
    ['AttachmentChip', 'attachment_chips.rs', 'attachment_remove_via_tag'],
    ['Button', 'primitives.rs', 'button_enter_activates_once_when_accepts_input'],
    ['ButtonGroup', 'button_group.rs', 'roving_moves_cursor'],
    ['Checkbox', 'controls.rs', 'checkbox_space_toggles_outcome'],
    ['Chip', 'primitives.rs', 'chip_toggle_space'],
    ['Combobox', 'combobox.rs', 'draft_active_value_separate'],
    ['DateTimePicker', 'date_time_picker.rs', 'open_calendar_nav_keys'],
    ['FilePicker', 'file_picker.rs', 'open_directory_and_confirm_file'],
    ['Form', 'form.rs', 'enter_activates_host_focused_field'],
    ['FormWizard', 'form_wizard.rs', 'focus_field_after_step'],
    ['IconButton', 'primitives.rs', 'icon_button_toggle_and_badge'],
    ['InputGroup', 'input_group.rs', 'field_typing_and_addon_action'],
    ['InputOtp', 'input_otp.rs', 'type_digits_auto_advance_and_complete'],
    ['KeybindingRecorder', 'keybinding_recorder.rs', 'record_single_chord_sequence_commit'],
    ['Link', 'link.rs', 'activate_marks_visited'],
    ['ModelSelector', 'model_mode_selectors.rs', 'accepts_input_gate'],
    ['MultiSelect', 'multi_select.rs', 'toggle_membership_highlight_distinct'],
    ['NumberInput', 'number_input.rs', 'draft_separate_from_committed'],
    ['PasswordInput', 'password_input.rs', 'outcome_submitted_has_no_secret_payload'],
    ['PasteChip', 'attachment_chips.rs', 'paste_expand_collapse_esc'],
    ['PathInput', 'path_input.rs', 'history_and_submit'],
    ['PromptComposer', 'prompt_composer.rs', 'draft_survives_accepts_input_gate_for_overlay_takeover'],
    ['QuestionFlow', 'question_flow.rs', 'single_choice_answer_and_advance'],
    ['RadioGroup', 'controls.rs', 'radio_follow_focus_selects_on_move'],
    ['RangeSlider', 'slider.rs', 'range_slider_move_thumbs'],
    ['SearchInput', 'search_input.rs', 'history_up_down'],
    ['SegmentedControl', 'segmented_control.rs', 'select_on_arrow_follow_focus'],
    ['Select', 'select.rs', 'open_highlight_distinct_from_value'],
    ['Slider', 'slider.rs', 'slider_arrow_steps'],
    ['Stepper', 'stepper.rs', 'menu_toggle_and_activate'],
    ['Switch', 'controls.rs', 'switch_toggles'],
    ['Tag', 'tag_chip.rs', 'tag_remove_and_part_focus'],
    ['TextArea', 'text_area.rs', 'edit_and_cursor_contract_table'],
    ['TextInput', 'text_input.rs', 'keyboard_owns_edit_submit_cancel_and_validation'],
    ['ThemePicker', 'theme_picker.rs', 'navigation_and_confirm_index'],
    ['Toggle', 'toggle.rs', 'single_toggle_space_flips'],
    ['ToggleGroup', 'toggle.rs', 'group_roving'],
    ['TokenField', 'token_field.rs', 'left_from_draft_to_token_and_right_back'],
    ['TokenStrip', 'tag_chip.rs', 'token_strip_wrap_and_roving'],
    ['Toolbar', 'toolbar.rs', 'arrows_move_cursor_when_focused'],
  ] as const).map(([id, file, symbol]) => specific(
    id,
    [widget(file, symbol)],
    { keyboard: 'r', focus: 'r' },
  )),
  specific('ActionBar', [], { keyboard: 'caller', focus: 'caller', mouse: 'caller' }),
  specific('ConfirmPrompt', [], { keyboard: 'caller', focus: 'caller', mouse: 'caller', escape: 'caller' }),
  specific('Form', [], { mouse: 'caller' }),
  specific('InputOtp', [], { mouse: 'na' }),
  specific('KeybindingRecorder', [], { mouse: 'na' }),
  ...([
    ['ActionLink', 'link.rs', 'mouse_click_activates'],
    ['AgentModeSelector', 'model_mode_selectors.rs', 'model_and_mode_mouse_confirm_only_hit_options'],
    ['ModelSelector', 'model_mode_selectors.rs', 'model_and_mode_mouse_confirm_only_hit_options'],
    ['AttachmentChip', 'attachment_chips.rs', 'attachment_and_paste_mouse_use_painted_hit_geometry'],
    ['PasteChip', 'attachment_chips.rs', 'attachment_and_paste_mouse_use_painted_hit_geometry'],
    ['Button', 'primitives.rs', 'button_mouse_down_up_activates'],
    ['IconButton', 'primitives.rs', 'icon_button_mouse_uses_hit_slop'],
    ['ButtonGroup', 'button_group.rs', 'mouse_activates'],
    ['Checkbox', 'controls.rs', 'checkbox_mouse_toggles'],
    ['RadioGroup', 'controls.rs', 'radio_group_paint_and_mouse'],
    ['Switch', 'controls.rs', 'switch_pointer_up_in_region_toggles'],
    ['Chip', 'tag_chip.rs', 'tag_chip_and_strip_mouse_use_painted_regions'],
    ['Tag', 'tag_chip.rs', 'tag_chip_and_strip_mouse_use_painted_regions'],
    ['TokenStrip', 'tag_chip.rs', 'tag_chip_and_strip_mouse_use_painted_regions'],
    ['Combobox', 'combobox.rs', 'mouse_focuses_field_and_opens_menu_from_painted_geometry'],
    ['DateTimePicker', 'date_time_picker.rs', 'mouse_select_day'],
    ['FilePicker', 'file_picker.rs', 'mouse_breadcrumb_and_double_click'],
    ['FormWizard', 'form_wizard.rs', 'paint_and_mouse_nav'],
    ['Link', 'link.rs', 'mouse_click_activates'],
    ['MultiSelect', 'multi_select.rs', 'mouse_toggle'],
    ['NumberInput', 'number_input.rs', 'mouse_stepper_hits'],
    ['PasswordInput', 'password_input.rs', 'mouse_reveal_uses_explicit_policy_and_exact_hit_region'],
    ['PathInput', 'path_input.rs', 'mouse_browse'],
    ['PromptComposer', 'prompt_composer.rs', 'mode_model_status_hits'],
    ['QuestionFlow', 'question_flow.rs', 'mouse_selects_only_painted_question_options'],
    ['SearchInput', 'search_input.rs', 'mouse_filter_chip'],
    ['SegmentedControl', 'segmented_control.rs', 'mouse_selects'],
    ['Select', 'select.rs', 'mouse_select_option'],
    ['Slider', 'slider.rs', 'slider_mouse_click_track'],
    ['Stepper', 'stepper.rs', 'mouse_activates_only_painted_step_hit'],
    ['TextArea', 'text_area.rs', 'scrollbars_stay_inside_panel_and_own_press_drag_geometry'],
    ['TextInput', 'text_input.rs', 'mouse_places_cursor'],
    ['Toggle', 'toggle.rs', 'mouse_toggles'],
    ['ToggleGroup', 'toggle.rs', 'mouse_toggles'],
    ['TokenField', 'token_field.rs', 'mouse_focuses_only_painted_token_or_draft_regions'],
    ['Toolbar', 'toolbar.rs', 'mouse_activates_hit'],
  ] as const).map(([id, file, symbol]) => specific(id, [widget(file, symbol)], { mouse: 'r' })),
  specific('ChromeRow', [widget('chrome_row.rs', 'unicode_body_and_prefix_use_display_columns')], {
    unicode: 'r', cjk: 'r', combining: 'r', emoji: 'r',
  }),
  specific('Inline', [crateRoot('layout/stack.rs', 'direction_for_width_responsive')], { unicode: 'na', cjk: 'na', combining: 'na', emoji: 'na' }),
  specific('WorkSurface', [crateRoot('layout/work_surface.rs', 'collapsed_region_is_zero_sized')], { unicode: 'na', cjk: 'na', combining: 'na', emoji: 'na' }),
  specific('Workspace', [crateRoot('layout/workspace.rs', 'rects_stay_inside_parent_on_tiny_area')], { unicode: 'na', cjk: 'na', combining: 'na', emoji: 'na' }),
  ...([
    ['FileTree', 'file_tree.rs', 'fuzz_kinds_git'],
    ['LogPane', 'log_pane.rs', 'ascii_profile_replaces_follow_and_scroll_chrome'],
    ['MessageThread', 'message_thread.rs', 'ascii_projection_builds_ascii_chrome_without_rewriting_host_copy'],
    ['Transcript', 'transcript.rs', 'ascii_and_unicode_paint_kind_prefix'],
    ['Viewport', 'viewport.rs', 'ascii_profile_uses_ascii_title_ellipsis'],
  ] as const).map(([id, file, symbol]) => specific(id, [widget(file, symbol)], { ascii_fallback: 'r' })),
  specific('ScrollArea', [
    widget('scroll_area.rs', 'visual_bars_and_new_content_paint'),
    scroll('render/tests.rs', 'ascii_profile_paints_single_cell_ascii_track_and_thumbs'),
  ], { ascii_fallback: 'r' }),
  specific('VirtualList', [
    widget('virtual_list.rs', 'async_placeholder_and_loading'),
    scroll('render/tests.rs', 'ascii_profile_paints_single_cell_ascii_track_and_thumbs'),
  ], { ascii_fallback: 'r' }),
  ...([
    ['CodeBlock', 'code_block.rs', 'large_window_paint_cheap'],
    ['DiagnosticView', 'diagnostic.rs', 'sustained_list_paint'],
    ['DiffView', 'diff.rs', 'sustained_viewport_paint'],
    ['EventStream', 'event_stream.rs', 'sustained_append_viewport_bound'],
    ['HexViewer', 'hex_viewer.rs', 'sustained_paint_window_only'],
    ['LogPane', 'log_pane.rs', 'default_history_is_bounded_and_unbounded_is_explicit'],
    ['LogStream', 'log_stream.rs', 'sustained_viewport_paint'],
    ['MarkdownView', 'markdown.rs', 'long_doc_measure_paint_benchmark'],
    ['TerminalOutput', 'terminal_output.rs', 'sustained_viewport_paint'],
  ] as const).map(([id, file, symbol]) => specific(id, [widget(file, symbol)], { large_data: 'r' })),
  specific('Transcript', [
    widget('transcript.rs', 'variable_height_viewport_maps_across_blocks'),
    widget('transcript.rs', 'anchor_survives_append_and_resize_height'),
  ], { large_data: 'cp' }),
  specific('DetailTable', [widget('detail_table.rs', 'wrap_and_both_axis_scroll_are_bounded_and_unicode_safe')], { large_data: 'na' }),
  ...([
    ['DataTable', 'data_table.rs', 'load_states_paint_explicit_ascii_no_color_cues'],
    ['TreeTable', 'tree_table.rs', 'non_ready_load_state_precedes_the_empty_projection_fallback'],
    ['ObjectInspector', 'object_inspector.rs', 'root_load_states_paint_before_the_empty_object_fallback'],
  ] as const).map(([id, file, symbol]) => specific(id, [
    widget('data_view.rs', 'load_state_chrome_is_shared_ascii_and_non_color_semantics'),
    widget(file, symbol),
  ], { loading: 'r', empty: 'r', error: 'r' })),
  specific('Card', [
    widget('card.rs', 'card_loading_body'),
    widget('card.rs', 'card_empty_and_error_body_modes_paint_their_state_copy'),
  ], { loading: 'r', empty: 'r', error: 'r' }),
  specific('ActivityIndicator', [
    widget('spinner.rs', 'every_phase_declares_its_channel_and_cadence'),
    widget('spinner.rs', 'spinner_and_activity_indicator_resize_cjk_combining_and_ascii_safe'),
  ], { loading: 'r', streaming: 'r', error: 'na', disabled: 'na' }),
  specific('Spinner', [
    widget('spinner.rs', 'every_phase_declares_its_channel_and_cadence'),
    widget('spinner.rs', 'dot_pulse_uses_raster_safe_frames_and_motion_off_is_static'),
    widget('spinner.rs', 'fuzz_phases_ticks'),
  ], { loading: 'r', streaming: 'r', error: 'na', disabled: 'na' }),
  specific('ProgressBar', [
    widget('progress.rs', 'indeterminate_tick_is_deterministic_and_tiny_areas_are_safe'),
    widget('progress.rs', 'status_complete_and_failed_paint'),
  ], { loading: 'r', error: 'r', streaming: 'na', disabled: 'na' }),
  specific('Skeleton', [
    widget('skeleton.rs', 'shimmer_only_full_motion'),
    widget('skeleton.rs', 'tiny_size_safe'),
  ], { loading: 'r', error: 'na', streaming: 'na', disabled: 'na' }),
  specific('WorkingStateCard', [
    pattern('working_state_card.rs', 'paint_expanded_and_collapsed'),
    pattern('working_state_card.rs', 'reduced_motion_running_presence_is_tick_static'),
  ], { loading: 'r', error: 'na', streaming: 'na', disabled: 'na' }),
  specific('ActivityShelf', [pattern('activity_shelf.rs', 'paint_presentations')], {
    loading: 'na', error: 'r', streaming: 'na', disabled: 'na',
  }),
  specific('AgentStatusHeader', [pattern('agent_status_header.rs', 'fuzz_work_connection')], {
    loading: 'na', error: 'rp', streaming: 'rp', disabled: 'na',
  }),
  specific('BackgroundTaskPanel', [
    pattern('background_task_panel.rs', 'paint_pane_and_rail'),
    pattern('background_task_panel.rs', 'bounded_output_drops_oldest'),
    pattern('background_task_panel.rs', 'follow_toggle'),
  ], { loading: 'na', error: 'r', streaming: 'r', disabled: 'na' }),
  specific('ConnectionManager', [
    pattern('connection_manager.rs', 'dual_presentation_paint'),
    pattern('connection_manager.rs', 'status_kind_fuzz'),
    pattern('connection_manager.rs', 'disabled_cannot_connect'),
  ], { loading: 'na', error: 'r', streaming: 'na', disabled: 'r' }),
  specific('IntegrationStatus', [
    pattern('integration_status.rs', 'paint_all_presentations'),
    pattern('integration_status.rs', 'fuzz_kinds_health'),
    pattern('integration_status.rs', 'disable_enable'),
  ], { loading: 'na', error: 'r', streaming: 'na', disabled: 'r' }),
  ...(['OfflineBanner', 'OfflineChrome', 'OfflineSurface'] as const).map((id) => specific(id, [
    widget('connectivity.rs', 'full_surface_shows_queue_caps_preserve'),
    widget('connectivity.rs', 'offline_surfaces_resize_cjk_combining_and_ascii_safe'),
  ], { loading: 'na', error: 'r', streaming: 'na', disabled: 'na' })),
  specific('ProgressSteps', [
    widget('progress_steps.rs', 'ascii_marks'),
    widget('progress_steps.rs', 'status_marks_are_non_color'),
  ], { loading: 'na', error: 'r', streaming: 'na', disabled: 'na' }),
  specific('StatusBar', [widget('status_bar.rs', 'semantic_status_owns_glyph_over_custom_slot_glyph')], {
    loading: 'na', error: 'r', streaming: 'na', disabled: 'na',
  }),
  specific('StatusIndicator', [
    widget('status_indicator.rs', 'all_kinds_have_glyph_label_role'),
    widget('status_indicator.rs', 'paint_includes_non_color_glyph'),
  ], { loading: 'na', error: 'r', streaming: 'na', disabled: 'na' }),
  specific('StatusStrip', [widget('status_strip.rs', 'semantic_segments_supply_a_non_color_glyph')], {
    loading: 'na', error: 'r', streaming: 'na', disabled: 'na',
  }),
  specific('TaskRail', [pattern('task_rail.rs', 'paint_all_and_narrow')], {
    loading: 'na', error: 'r', streaming: 'na', disabled: 'na',
  }),
  ...([
    ['ActivityShelf', pattern('activity_shelf.rs', 'resize_cjk_combining_and_ascii_safe')],
    ['AgentStatusHeader', pattern('agent_status_header.rs', 'resize_cjk_combining_and_ascii_safe')],
    ['BackgroundTaskPanel', pattern('background_task_panel.rs', 'resize_cjk_combining_and_ascii_safe')],
    ['ConnectionManager', pattern('connection_manager.rs', 'resize_cjk_combining_and_ascii_safe')],
    ['IntegrationStatus', pattern('integration_status.rs', 'resize_cjk_combining_and_ascii_safe')],
    ['ProgressSteps', widget('progress_steps.rs', 'resize_cjk_combining_and_ascii_safe')],
    ['StatusBar', widget('status_bar.rs', 'resize_cjk_combining_and_ascii_safe')],
    ['StatusIndicator', widget('status_indicator.rs', 'resize_cjk_combining_and_ascii_safe')],
    ['StatusStrip', widget('status_strip.rs', 'resize_cjk_combining_and_ascii_safe')],
    ['TaskRail', pattern('task_rail.rs', 'resize_cjk_combining_and_ascii_safe')],
    ['WorkingStateCard', pattern('working_state_card.rs', 'resize_cjk_combining_and_ascii_safe')],
  ] as const).map(([id, test]) => specific(id, [test], {
    resize: 'r', responsive: 'r', tiny_terminal: 'r', unicode: 'r', cjk: 'r', combining: 'r', ascii_fallback: 'r',
  })),
  ...(['ActivityIndicator', 'Spinner'] as const).map((id) => specific(id, [
    widget('spinner.rs', 'spinner_and_activity_indicator_resize_cjk_combining_and_ascii_safe'),
  ], { resize: 'r', responsive: 'r', tiny_terminal: 'r', unicode: 'r', cjk: 'r', combining: 'r', ascii_fallback: 'r' })),
  ...(['OfflineBanner', 'OfflineChrome', 'OfflineSurface'] as const).map((id) => specific(id, [
    widget('connectivity.rs', 'offline_surfaces_resize_cjk_combining_and_ascii_safe'),
    widget('connectivity.rs', 'tiny_empty_safe'),
  ], { resize: 'r', responsive: 'r', tiny_terminal: 'r', unicode: 'r', cjk: 'r', combining: 'r', emoji: 'r', ascii_fallback: 'r' })),
  specific('ProgressBar', [
    widget('progress.rs', 'cjk_and_combining_labels_resize_on_grapheme_boundaries'),
    widget('progress.rs', 'indeterminate_tick_is_deterministic_and_tiny_areas_are_safe'),
  ], { resize: 'r', responsive: 'r', tiny_terminal: 'r', unicode: 'r', cjk: 'r', combining: 'r', ascii_fallback: 'r' }),
  specific('Skeleton', [
    widget('skeleton.rs', 'tiny_size_safe'),
    widget('skeleton.rs', 'capability_tiny_stories_contract'),
    widget('skeleton.rs', 'legacy_new_paints_staggered_lines'),
    widget('skeleton.rs', 'ascii_fill'),
  ], { resize: 'r', responsive: 'r', tiny_terminal: 'r', unicode: 'r', cjk: 'na', combining: 'na', emoji: 'na', ascii_fallback: 'r' }),
  specific('StatusStrip', [widget('status_strip.rs', 'resize_cjk_combining_and_ascii_safe')], { emoji: 'r' }),
  ...([
    ['ActivityShelf', pattern('activity_shelf.rs', 'paint_perf_budget')],
    ['BackgroundTaskPanel', pattern('background_task_panel.rs', 'paint_perf_budget')],
    ['ConnectionManager', pattern('connection_manager.rs', 'paint_perf')],
    ['IntegrationStatus', pattern('integration_status.rs', 'paint_perf')],
    ['ProgressSteps', widget('progress_steps.rs', 'paint_perf_smoke')],
    ['TaskRail', pattern('task_rail.rs', 'paint_perf_budget')],
  ] as const).map(([id, test]) => specific(id, [test], { large_data: 'r' })),
  ...(['ActivityIndicator', 'AgentStatusHeader', 'OfflineBanner', 'OfflineChrome', 'OfflineSurface', 'ProgressBar', 'Skeleton', 'Spinner', 'StatusBar', 'StatusIndicator', 'StatusStrip', 'WorkingStateCard'] as const)
    .map((id) => specific(id, [], { large_data: 'na' })),
  specific('ActivityShelf', [pattern('activity_shelf.rs', 'mouse_activates_chip')], { mouse: 'r' }),
  specific('AgentStatusHeader', [pattern('agent_status_header.rs', 'mouse_action')], { mouse: 'r' }),
  specific('ConnectionManager', [pattern('connection_manager.rs', 'mouse_connect')], { mouse: 'r' }),
  specific('OfflineBanner', [widget('connectivity.rs', 'mouse_retry_on_banner')], { mouse: 'r' }),
  ...([
    ['ActivityShelf', pattern('activity_shelf.rs', 'keyboard_nav_activate_dismiss')],
    ['AgentStatusHeader', pattern('agent_status_header.rs', 'arrow_moves_action_focus')],
    ['BackgroundTaskPanel', pattern('background_task_panel.rs', 'selected_output_copy_stays_visible_without_claiming_second_focus')],
    ['ConnectionManager', pattern('connection_manager.rs', 'disabled_cannot_connect')],
    ['IntegrationStatus', pattern('integration_status.rs', 'egress_warning_focus')],
    ['ProgressSteps', widget('progress_steps.rs', 'interactive_nav_and_retry')],
    ['TaskRail', pattern('task_rail.rs', 'keyboard_cancel_retry_tab_input')],
    ['WorkingStateCard', pattern('working_state_card.rs', 'inspect_default_before_cancel')],
  ] as const).map(([id, test]) => specific(id, [test], { focus: 'r' })),
  ...(['OfflineBanner', 'OfflineSurface'] as const).map((id) => specific(id, [
    widget('connectivity.rs', 'auth_required_prefers_sign_in'),
    widget('connectivity.rs', 'retry_key_and_preserve_defaults'),
  ], { focus: 'r' })),
  ...([
    ['BackgroundTaskPanel', pattern('background_task_panel.rs', 'paint_pane_and_rail')],
    ['ConnectionManager', pattern('connection_manager.rs', 'dual_presentation_paint')],
    ['IntegrationStatus', pattern('integration_status.rs', 'paint_all_presentations')],
    ['StatusStrip', widget('status_strip.rs', 'colorless_strips_paint_no_hue_at_all')],
    ['WorkingStateCard', pattern('working_state_card.rs', 'reference_minimum_and_below_minimum_keep_status_anatomy')],
    ['ActivityIndicator', widget('spinner.rs', 'activity_indicator_is_rail_glyph_and_verb_in_ascii_colorless_mode')],
  ] as const).map(([id, test]) => specific(id, [test], { no_color: 'r' })),
  specific('StatusIndicator', [widget('status_indicator.rs', 'labeled_ascii_status_keeps_rail_glyph_and_verb')], { no_color: 'rp' }),
  specific('ActivityShelf', [pattern('activity_shelf.rs', 'paint_presentations')], { no_color: 'rp', color_ladder: 'rp' }),
  ...(['OfflineBanner', 'OfflineChrome', 'OfflineSurface'] as const).map((id) => specific(id, [
    widget('connectivity.rs', 'offline_surfaces_resize_cjk_combining_and_ascii_safe'),
  ], { no_color: 'rp', color_ladder: 'rp' })),
  specific('BackgroundTaskPanel', [pattern('background_task_panel.rs', 'paint_pane_and_rail')], { color_ladder: 'rp' }),
  specific('ConnectionManager', [pattern('connection_manager.rs', 'dual_presentation_paint')], { color_ladder: 'rp' }),
  specific('IntegrationStatus', [pattern('integration_status.rs', 'paint_all_presentations')], { color_ladder: 'rp' }),
  specific('StatusIndicator', [widget('status_indicator.rs', 'paint_includes_non_color_glyph')], { color_ladder: 'rp' }),
  specific('WorkingStateCard', [pattern('working_state_card.rs', 'paint_expanded_and_collapsed')], { color_ladder: 'rp' }),
  specific('MultiSelect', [widget('multi_select.rs', 'esc_keeps_selection')], { escape: 'r' }),
  specific('Select', [widget('select.rs', 'esc_closes_without_commit')], { escape: 'r' }),
]
