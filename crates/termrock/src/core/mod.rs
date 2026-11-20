//! The modules of `termrock-core` under their `core::` paths, and the text
//! buffer of `termrock-text` as `core::text`. The list is explicit: `field` and
//! `item` are public in `termrock-core` for the crates above it, and are not
//! part of this facade.

pub use termrock_core::{
    data, edit, event, focus, hints, hit, id, intent, keyboard, keyed, keymap, list, nav, pointer,
    refusal, response, scroll, selection, shape, typeahead,
};

pub use termrock_text::buffer as text;
