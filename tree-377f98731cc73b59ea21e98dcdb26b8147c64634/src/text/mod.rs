//! Product-neutral terminal text measurement, sanitization, and windows.

pub use crate::ansi_text::{strip_bytes, styled_spans};
pub use crate::geometry::{
    FixedPrefixSegment, display_cols, display_cols_slice, fixed_prefix_scroll_segments,
    is_terminal_control_char, leading_space_cols, padded_line_display_cols,
    sanitize_terminal_title, take_display_cols,
};
