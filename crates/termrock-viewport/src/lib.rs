//! The text viewport and its operations, the scroll panel and the scroll view.
//!
//! `termrock::ui::scroll_view` and `termrock::widgets::{panel, viewport,
//! viewport_ops}` are the modules of the same name here; the `termrock` facade
//! re-exports each at that path.

#[cfg(test)]
mod nested_scrollbar_tests;
pub mod panel;
pub mod scroll_view;
pub mod viewport;
pub mod viewport_ops;
#[doc(hidden)]
pub mod viewport_specimen;
