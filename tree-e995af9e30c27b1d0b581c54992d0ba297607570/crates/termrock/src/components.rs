//! High-level terminal components built from the stable widget primitives.

pub mod button_strip;
pub mod confirm_dialog;
pub mod diff_view;
pub mod error_dialog;
pub mod filter_input;
pub mod hint_bar;
pub mod panel;
pub mod save_discard_dialog;
pub mod select_list;
pub mod status_popup;
pub mod tab_strip;
pub mod text_input;

pub use crate::interaction::{
    ButtonFocus, FocusOwner, HoverTracker, ModalClickResult, ModalStack, classify_click,
    render_backdrop,
};
pub use crate::layout::{
    DIALOG_HORIZONTAL_SCROLL_STEP, DialogBodyScroll, DialogBorder, ScrollAxes, ScrollAxis,
    dialog_inner_chunks, dialog_inner_height, dialog_scroll_axes, mouse_scroll_delta,
    render_dialog_shell, render_scrollable_dialog_body, scroll_hint_spans,
};
pub use crate::scroll::{
    SCROLLBAR_HORIZONTAL_THUMB, SCROLLBAR_TRACK, ScrollableList, ScrollbarStyle,
    apply_scroll_delta, apply_scroll_delta_unclamped, apply_term_width_scroll_delta,
    clamp_scroll_offset, effective_offset, horizontal_scrollbar_area, is_scrollable, line_width,
    render_horizontal_scrollbar, render_line_with_fixed_prefix_scroll,
    render_lines_with_offset_in_area, render_scrollable_block, render_selected_lines_in_area,
    render_vertical_scrollbar, render_vertical_scrollbar_in_area,
    render_vertical_scrollbar_in_area_with_style, render_vertical_scrollbar_with_style,
    rendered_cursor_follow_offset as cursor_follow_offset,
    rendered_max_line_width as max_line_width, rendered_max_offset as max_offset,
    scrollbar_offset_for_track_position, vertical_scrollbar_area, viewport_height, viewport_width,
};
pub use crate::widgets::ModalBackdrop;
pub use button_strip::{
    ButtonStrip, ButtonStripItem, button_rects, button_strip_line, button_style,
};
pub use confirm_dialog::{
    CONFIRM_KEYMAP, ConfirmAction, ConfirmFocus, ConfirmKind, ConfirmState, confirm_button_hit,
    confirm_hint_spans, render_confirm_dialog, required_height as confirm_required_height,
    width_pct as confirm_width_pct,
};
pub use diff_view::{DiffViewState, SinglePaneKind, diff_view_hint_spans, render_diff_view};
pub use error_dialog::{
    ERROR_POPUP_KEYMAP, ErrorPopupAction, ErrorPopupRow, ErrorPopupState, error_popup_hint_spans,
    estimated_message_rows, hyperlink_regions as error_popup_hyperlink_regions,
    render_error_dialog, render_error_dialog_in, required_height,
    row_value_rect_groups as error_popup_row_value_rect_groups,
    row_value_rects as error_popup_row_value_rects,
};
pub use filter_input::{FilterInput, filter_input_line, render_filter_input};
pub use hint_bar::{
    HintBar, line as hint_line, render_hint_bar, render_wrapped_hint_bar, styled_hint_spans,
    wrapped_height, wrapped_lines,
};
pub use panel::{Panel, PanelFocus, modal_block, panel_body_area, unfocused_block};
pub use save_discard_dialog::{
    SAVE_DISCARD_KEYMAP, SaveDiscardAction, SaveDiscardChoice, SaveDiscardFocus, SaveDiscardState,
    render_save_discard_dialog, save_discard_hint_spans,
};
pub use select_list::{
    PickerRow, SELECT_LIST_KEYMAP, SelectListAction, SelectListState, render_picker_lines,
    render_picker_list, render_select_list, select_list_hint_spans,
};
pub use status_popup::{StatusPopupState, render_status_popup};
pub use tab_strip::{TabStrip, tab_cell_style, tab_label_line, tab_underline_line};
pub use text_input::{
    BorderStyle, TEXT_INPUT_KEYMAP, TextField, TextInputAction, TextInputState,
    render_labeled_text_input_dialog, render_text_input, text_input_hint_spans,
    text_input_prompt_rect,
};
