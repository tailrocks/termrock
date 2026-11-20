//! Line-edit fields: the `EditField` substrate, its pointer and selection
//! helpers and the value parsers.
//!
//! `termrock::widgets::{edit, field_common}` are `edit` and `field_common`
//! here; the `termrock` facade re-exports them at those paths.

pub mod edit;
pub mod field_common;
#[doc(hidden)]
pub mod text_select;
