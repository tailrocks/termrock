//! Typed OSC requests and pure encoders. Consumers own emission policy.

use crate::Rgb;
use base64::Engine as _;

pub const RESET: &str = "\x1b[0m";
pub const POINTER_HAND: &str = "\x1b]22;pointer\x1b\\";
pub const POINTER_DEFAULT: &str = "\x1b]22;default\x1b\\";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request<'a> {
    Pointer(&'a str),
    Clipboard(&'a str),
    HyperlinkOpen(&'a str),
    HyperlinkClose,
}

#[must_use]
pub fn encode(request: Request<'_>) -> Vec<u8> {
    match request {
        Request::Pointer(shape) => format!("\x1b]22;{shape}\x1b\\").into_bytes(),
        Request::Clipboard(payload) => encode_osc52_clipboard_write(payload),
        Request::HyperlinkOpen(href) => format!("\x1b]8;;{href}\x1b\\").into_bytes(),
        Request::HyperlinkClose => b"\x1b]8;;\x1b\\".to_vec(),
    }
}

#[must_use]
pub fn encode_osc52_clipboard_write(payload: &str) -> Vec<u8> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(payload.as_bytes());
    format!("\x1b]52;c;{encoded}\x07").into_bytes()
}

pub fn emit_osc8_open(buf: &mut Vec<u8>, href: &str) {
    buf.extend(encode(Request::HyperlinkOpen(href)));
}
pub fn emit_osc8_close(buf: &mut Vec<u8>) {
    buf.extend(encode(Request::HyperlinkClose));
}
pub fn move_to(buf: &mut Vec<u8>, row: u16, col: u16) {
    buf.extend_from_slice(format!("\x1b[{};{}H", row + 1, col + 1).as_bytes());
}
pub fn fg(buf: &mut Vec<u8>, rgb: Rgb) {
    buf.extend_from_slice(format!("\x1b[38;2;{};{};{}m", rgb.r, rgb.g, rgb.b).as_bytes());
}
