//! Typed terminal requests and pure OSC encoders. Consumers own emission.

mod encode;
mod request;

pub use encode::{
    encode, encode_clipboard, encode_hyperlink_close, encode_hyperlink_open, encode_pointer,
};
pub use request::{ClipboardWrite, HyperlinkRegion, PointerShape, Request};
