//! Transitional (#444): the modules of `termrock-core` keep their `core::` paths
//! through these re-exports. #452 removes them.

pub use termrock_core::{
    data, edit, event, focus, hints, hit, id, intent, keyboard, keyed, keymap, list, nav, pointer,
    refusal, response, scroll, selection, shape, typeahead,
};

pub mod text;

#[cfg(test)]
#[path = "pointer/nested_scrollbar_tests.rs"]
mod nested_scrollbar_tests;
