//! Minimal JSON string escaping for the dependency-free catalog CLI.

pub(crate) fn json_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\u{08}' => escaped.push_str("\\b"),
            '\u{0c}' => escaped.push_str("\\f"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character < '\u{20}' => {
                use std::fmt::Write as _;
                write!(escaped, "\\u{:04x}", character as u32)
                    .expect("writing to String cannot fail");
            }
            character => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::json_escape;

    #[test]
    fn escapes_quotes_backslashes_and_line_breaks() {
        assert_eq!(json_escape("a\"b\\c\n"), "a\\\"b\\\\c\\n");
    }

    #[test]
    fn escapes_every_json_control_character() {
        assert_eq!(json_escape("\u{0}\u{1f}\t"), "\\u0000\\u001f\\t");
    }

    #[test]
    fn escaped_fields_form_valid_json_shape() {
        let title = json_escape("quote \" and\nline");
        assert_eq!(
            format!(r#"{{"title":"{title}"}}"#),
            "{\"title\":\"quote \\\" and\\nline\"}"
        );
    }
}
