//! The code editor, the text area and the diff view.
//!
//! `termrock::widgets::{code, diff, textarea}` are the modules of the same name
//! here; the `termrock` facade re-exports each at that path.

pub mod code;
mod code_specimen;
pub mod diff;
// Installs the counting allocator for this crate's own test binary; the allocation-budget
// tests of `code` and `textarea` are the only users.
#[cfg(test)]
mod test_alloc;
pub mod textarea;
