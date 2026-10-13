//! termrock: shared TUI widgets, theme, and render helpers.
//!
//! **Architecture Invariant:** T1.
//! Entry point: [`Theme`] — shared TUI theme tokens.

pub mod ansi_text;
pub mod geometry;
pub mod input;
pub mod interaction;
pub mod keymap;
pub mod layout;
pub mod osc;
pub mod runtime;
pub mod scroll;
pub mod style;
pub mod text;
pub mod widgets;

#[cfg(feature = "crossterm")]
pub mod crossterm;

pub use style as theme;
pub use style::Theme;

pub use geometry::{
    FixedPrefixSegment, HintSpan, TAB_GAP, TabCell, centered_rect, display_cols,
    display_cols_slice, fixed_prefix_scroll_segments, hint_row_cols, is_terminal_control_char,
    lay_out_tabs, leading_space_cols, padded_line_display_cols, sanitize_terminal_title,
    tab_at_column, take_display_cols,
};
pub use scroll::{TailScroll, is_scrollable, max_line_width, max_offset};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerShape {
    Default,
    Pointer,
    Text,
    EwResize,
    NsResize,
    Grabbing,
}

impl PointerShape {
    #[must_use]
    pub const fn as_osc22_name(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Pointer => "pointer",
            Self::Text => "text",
            Self::EwResize => "ew-resize",
            Self::NsResize => "ns-resize",
            Self::Grabbing => "grabbing",
        }
    }
}

#[must_use]
pub const fn clickable_pointer_shape(clickable: bool) -> PointerShape {
    if clickable {
        PointerShape::Pointer
    } else {
        PointerShape::Default
    }
}

#[must_use]
pub fn osc22_pointer_shape(shape: PointerShape) -> String {
    format!("\x1b]22;{}\x1b\\", shape.as_osc22_name())
}
