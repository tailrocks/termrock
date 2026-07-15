use super::{ClipboardWrite, PointerShape, Request};
use base64::Engine as _;

#[must_use]
pub fn encode(request: Request<'_>) -> Vec<u8> {
    match request {
        Request::Pointer(shape) => encode_pointer(shape),
        Request::Clipboard(request) => encode_clipboard(request),
        Request::HyperlinkOpen { id, url } => encode_hyperlink_open(id, url),
        Request::HyperlinkClose => encode_hyperlink_close(),
    }
}

#[must_use]
pub fn encode_pointer(shape: PointerShape) -> Vec<u8> {
    format!("\x1b]22;{}\x1b\\", shape.name()).into_bytes()
}
#[must_use]
pub fn encode_hyperlink_open(id: Option<&str>, url: &str) -> Vec<u8> {
    format!(
        "\x1b]8;{};{url}\x1b\\",
        id.map_or(String::new(), |id| format!("id={id}"))
    )
    .into_bytes()
}
#[must_use]
pub fn encode_hyperlink_close() -> Vec<u8> {
    b"\x1b]8;;\x1b\\".to_vec()
}
#[must_use]
pub fn encode_clipboard(request: ClipboardWrite<'_>) -> Vec<u8> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(request.text.as_bytes());
    format!("\x1b]52;{};{encoded}\x07", request.selection).into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encodes_known_requests_exactly() {
        assert_eq!(
            encode_pointer(PointerShape::Pointer),
            b"\x1b]22;pointer\x1b\\"
        );
        assert_eq!(
            encode_hyperlink_open(Some("docs"), "https://example.invalid"),
            b"\x1b]8;id=docs;https://example.invalid\x1b\\"
        );
        assert_eq!(encode_hyperlink_close(), b"\x1b]8;;\x1b\\");
        assert_eq!(
            encode_clipboard(ClipboardWrite {
                selection: "c",
                text: "hello"
            }),
            b"\x1b]52;c;aGVsbG8=\x07"
        );
    }
}
