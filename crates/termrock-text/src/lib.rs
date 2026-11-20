//! Text foundations: cell width, truncation and wrapping (`cells`) and the
//! line-indexed text buffer behind the editors (`buffer`).
//!
//! `termrock::ui::text` is `cells` and `termrock::core::text` is `buffer`; the
//! the `termrock` facade re-exports both at those paths.

pub mod buffer;
pub mod cells;
#[doc(hidden)]
pub mod text_index;
