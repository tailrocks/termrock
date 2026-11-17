use termrock::terminal::is_spoofing_format;

pub fn shown(raw: &str, max_chars: usize) -> String {
    let mut out = String::new();
    let mut chars = 0;
    let mut gap = false;
    let mut visible = false;
    for c in raw.chars() {
        if c.is_whitespace() {
            gap = chars > 0;
            continue;
        }
        if is_hidden(c) {
            continue;
        }
        let needed = if gap { 2 } else { 1 };
        if chars + needed > max_chars {
            break;
        }
        if gap {
            out.push(' ');
            gap = false;
        }
        visible |= !is_joiner(c);
        out.push(c);
        chars += needed;
    }
    if visible { out } else { String::new() }
}

pub fn escaped(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for c in raw.chars() {
        if is_hidden(c) {
            out.extend(c.escape_unicode());
        } else {
            out.push(c);
        }
    }
    out
}

pub fn is_hidden(c: char) -> bool {
    c.is_control() || is_spoofing_format(c) || is_invisible(c)
}

fn is_invisible(c: char) -> bool {
    matches!(
        c,
        '\u{00ad}'
            | '\u{034f}'
            | '\u{115f}'..='\u{1160}'
            | '\u{17b4}'..='\u{17b5}'
            | '\u{180b}'..='\u{180f}'
            | '\u{200b}'
            | '\u{2060}'..='\u{2064}'
            | '\u{206a}'..='\u{206f}'
            | '\u{2800}'
            | '\u{3164}'
            | '\u{feff}'
            | '\u{ffa0}'
            | '\u{fff9}'..='\u{fffb}'
    )
}

fn is_joiner(c: char) -> bool {
    matches!(c, '\u{200c}' | '\u{200d}' | '\u{fe00}'..='\u{fe0f}' | '\u{e0100}'..='\u{e01ef}')
}

#[cfg(test)]
mod tests {
    use super::escaped;

    #[test]
    fn escaped_passes_visible_text_through_unchanged() {
        assert_eq!(escaped("gateway builder 42"), "gateway builder 42");
    }

    #[test]
    fn escaped_renders_control_characters_as_debug_escapes() {
        let shown = escaped("red\u{1b}[31m");
        assert!(!shown.contains('\u{1b}'), "{shown:?}");
        assert_eq!(shown, "red\\u{1b}[31m");
    }

    #[test]
    fn escaped_renders_invisible_and_spoofing_characters_as_debug_escapes() {
        let shown = escaped("a\u{200b}b\u{202e}c");
        assert!(!shown.chars().any(super::is_hidden), "{shown:?}");
        assert_eq!(shown, "a\\u{200b}b\\u{202e}c");
    }

    #[test]
    fn escaped_output_never_contains_raw_hidden_characters() {
        let shown = escaped("\u{7}bell\u{200b}zwsp\u{feff}bom");
        assert!(!shown.chars().any(super::is_hidden), "{shown:?}");
        assert!(shown.ends_with("bom"), "{shown:?}");
    }

    #[test]
    fn escaped_renders_unicode_fillers_as_debug_escapes() {
        let shown = escaped("a\u{115f}b\u{1160}c\u{2800}d\u{3164}e\u{ffa0}f");
        assert!(!shown.chars().any(super::is_hidden), "{shown:?}");
        assert_eq!(shown, "a\\u{115f}b\\u{1160}c\\u{2800}d\\u{3164}e\\u{ffa0}f");
    }
}
