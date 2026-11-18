//! Transitional: the text buffer lives in `termrock-text` (#444). The test
//! module stays here until `termrock-core` can hold it (it drives `core::edit`).

pub use termrock_text::buffer::*;

#[cfg(test)]
mod tests;
