//! Render context and its collaborators: the per-frame `RenderCtx`, reveal,
//! tooltips and popups, measuring, row painting, edge fades, the specimen trait
//! and the column solver.
//!
//! `termrock::ui::<module>` is the module of the same name here, except that
//! `columns` is `termrock::ui::columns`; the `termrock` facade re-exports
//! each at that path.

pub mod columns;
pub mod ctx;
pub mod fade;
#[doc(hidden)]
pub mod fitting;
pub mod measure;
pub mod popup;
pub mod reveal;
pub mod row;
pub mod scrollbar;
pub mod specimen;
pub mod tooltip;
