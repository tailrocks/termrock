use super::{ClipboardWrite, PointerShape, Request};
use crate::text::is_terminal_control_char;
use base64::Engine as _;
use std::fmt::Write as _;

const MAX_CLIPBOARD_BYTES: usize = 100_000;

#[must_use]
/// Encodes a typed terminal OSC request into bytes ready for the output stream.
///
/// # Examples
///
/// ```
/// use termrock::osc::{PointerShape, Request, encode};
///
/// assert_eq!(
///     encode(Request::Pointer(PointerShape::Pointer)),
///     b"\x1b]22;pointer\x1b\\",
/// );
/// ```
pub fn encode(request: Request<'_>) -> Vec<u8> {
    match request {
        Request::Pointer(shape) => encode_pointer(shape),
        Request::Clipboard(request) => encode_clipboard(request),
        Request::HyperlinkOpen { id, url } => encode_hyperlink_open(id, url),
        Request::HyperlinkClose => encode_hyperlink_close(),
    }
}

#[must_use]
/// Performs the `encode_pointer` operation.
pub fn encode_pointer(shape: PointerShape) -> Vec<u8> {
    format!("\x1b]22;{}\x1b\\", shape.name()).into_bytes()
}
#[must_use]
/// Encodes an OSC 8 hyperlink after validating its scheme and neutralizing
/// terminal control characters.
///
/// An empty vector means the request was rejected and callers must emit
/// nothing. Supported schemes are `http`, `https`, `mailto`, and `file`.
pub fn encode_hyperlink_open(id: Option<&str>, url: &str) -> Vec<u8> {
    let Some((scheme, _)) = url.split_once(':') else {
        return Vec::new();
    };
    if !["http", "https", "mailto", "file"]
        .iter()
        .any(|allowed| scheme.eq_ignore_ascii_case(allowed))
    {
        return Vec::new();
    }

    let url = percent_encode_terminal_controls(url);
    let id = id.map_or_else(String::new, |id| {
        id.chars()
            .filter(|&character| {
                !is_terminal_control_char(character) && character != ';' && character != ':'
            })
            .collect()
    });
    format!(
        "\x1b]8;{};{url}\x1b\\",
        if id.is_empty() {
            String::new()
        } else {
            format!("id={id}")
        }
    )
    .into_bytes()
}

fn percent_encode_terminal_controls(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for character in value.chars() {
        if character == ' ' || is_terminal_control_char(character) {
            let mut bytes = [0; 4];
            for byte in character.encode_utf8(&mut bytes).bytes() {
                write!(encoded, "%{byte:02X}").expect("writing to String cannot fail");
            }
        } else {
            encoded.push(character);
        }
    }
    encoded
}

#[must_use]
/// Performs the `encode_hyperlink_close` operation.
pub fn encode_hyperlink_close() -> Vec<u8> {
    b"\x1b]8;;\x1b\\".to_vec()
}
#[must_use]
/// Encodes an OSC 52 clipboard write.
///
/// An empty vector means the request exceeded 100,000 source bytes and was
/// rejected; callers must emit nothing.
pub fn encode_clipboard(request: ClipboardWrite<'_>) -> Vec<u8> {
    if request.text.len() > MAX_CLIPBOARD_BYTES {
        return Vec::new();
    }
    let encoded = base64::engine::general_purpose::STANDARD.encode(request.text.as_bytes());
    format!("\x1b]52;{};{encoded}\x07", request.selection.letter()).into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::osc::ClipboardSelection;

    #[test]
    fn encodes_known_requests_exactly() {
        assert_eq!(
            encode_pointer(PointerShape::Pointer),
            b"\x1b]22;pointer\x1b\\"
        );
        assert_eq!(
            encode_pointer(PointerShape::EwResize),
            b"\x1b]22;ew-resize\x1b\\"
        );
        assert_eq!(
            encode_hyperlink_open(Some("docs"), "https://example.invalid"),
            b"\x1b]8;id=docs;https://example.invalid\x1b\\"
        );
        assert_eq!(encode_hyperlink_close(), b"\x1b]8;;\x1b\\");
        assert_eq!(
            encode_clipboard(ClipboardWrite {
                selection: ClipboardSelection::Clipboard,
                text: "hello"
            }),
            b"\x1b]52;c;aGVsbG8=\x07"
        );
    }

    #[test]
    fn hyperlink_url_control_bytes_are_percent_encoded() {
        let encoded = encode_hyperlink_open(None, "https://example.invalid/\x1b/\x07");
        let encoded = String::from_utf8(encoded).unwrap();

        assert!(encoded.contains("%1B"));
        assert!(encoded.contains("%07"));
        assert_eq!(encoded.matches("\x1b]8;").count(), 1);
        assert_eq!(encoded.matches("\x1b\\").count(), 1);
    }

    #[test]
    fn hyperlink_disallowed_scheme_is_rejected() {
        assert!(encode_hyperlink_open(None, "javascript:").is_empty());
    }

    #[test]
    fn hyperlink_id_strips_separators_and_controls() {
        assert_eq!(
            encode_hyperlink_open(Some("do;c:s\x1b"), "https://example.invalid"),
            b"\x1b]8;id=docs;https://example.invalid\x1b\\"
        );
    }

    #[test]
    fn clipboard_selection_is_typed() {
        assert_eq!(
            encode_clipboard(ClipboardWrite {
                selection: ClipboardSelection::Clipboard,
                text: "hello",
            }),
            b"\x1b]52;c;aGVsbG8=\x07"
        );
        assert_eq!(
            encode_clipboard(ClipboardWrite {
                selection: ClipboardSelection::Primary,
                text: "hello",
            }),
            b"\x1b]52;p;aGVsbG8=\x07"
        );
    }

    #[test]
    fn clipboard_oversized_write_is_rejected() {
        let text = "x".repeat(MAX_CLIPBOARD_BYTES + 1);
        assert!(
            encode_clipboard(ClipboardWrite {
                selection: ClipboardSelection::Clipboard,
                text: &text,
            })
            .is_empty()
        );
    }
}
