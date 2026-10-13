//! Product-neutral terminal text measurement, sanitization, and windows.

pub use crate::ansi_text::{strip_bytes, styled_spans};
use unicode_width::UnicodeWidthStr;

/// True for any C0 / C1 control byte or DEL (`0x7f`).
#[must_use]
pub fn is_terminal_control_char(c: char) -> bool {
    let code = c as u32;
    code < 0x20 || c == '\x7f' || (0x80..0xa0).contains(&code)
}

/// Display-column width of `s`, excluding terminal control bytes.
#[must_use]
pub fn display_cols(s: &str) -> usize {
    if s.chars().any(is_terminal_control_char) {
        let sanitized: String = s
            .chars()
            .filter(|c| !is_terminal_control_char(*c))
            .collect();
        UnicodeWidthStr::width(sanitized.as_str())
    } else {
        UnicodeWidthStr::width(s)
    }
}

/// Take the longest prefix of `s` whose display width fits inside
/// `max_cols`, skipping control bytes.
#[must_use]
pub fn take_display_cols(s: &str, max_cols: usize) -> String {
    use unicode_width::UnicodeWidthChar;
    let mut out = String::new();
    let mut used = 0usize;
    for c in s.chars() {
        if is_terminal_control_char(c) {
            continue;
        }
        let width = c.width().unwrap_or(0);
        if used + width > max_cols {
            break;
        }
        out.push(c);
        used += width;
    }
    out
}

/// Substring of `s` covering display columns `[skip, skip + width)`,
/// skipping terminal control bytes and preserving only complete characters.
#[must_use]
pub fn display_cols_slice(s: &str, skip: usize, width: usize) -> String {
    use unicode_width::UnicodeWidthChar;
    let mut col = 0usize;
    let mut out = String::new();
    for ch in s.chars() {
        if is_terminal_control_char(ch) {
            continue;
        }
        let w = ch.width().unwrap_or(0);
        if col >= skip && col + w <= skip + width {
            out.push(ch);
        }
        col += w;
        if col >= skip + width {
            break;
        }
    }
    out
}

/// Leading ASCII-space count for text rows that need symmetric trailing
/// scroll padding. Controls are ignored.
#[must_use]
pub fn leading_space_cols<S>(parts: impl IntoIterator<Item = S>) -> usize
where
    S: AsRef<str>,
{
    let mut count = 0;
    for part in parts {
        for ch in part.as_ref().chars() {
            if is_terminal_control_char(ch) {
                continue;
            }
            if ch != ' ' {
                return count;
            }
            count += 1;
        }
    }
    count
}

/// Display-column width for a row plus matching trailing indentation padding.
#[must_use]
pub fn padded_line_display_cols<I, S>(parts: I) -> usize
where
    I: IntoIterator<Item = S> + Clone,
    S: AsRef<str>,
{
    parts
        .clone()
        .into_iter()
        .map(|part| display_cols(part.as_ref()))
        .sum::<usize>()
        + leading_space_cols(parts)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedPrefixSegment {
    pub start_byte: usize,
    pub end_byte: usize,
    pub target_col: usize,
    pub display_cols: usize,
}

/// Visible byte ranges for a horizontally scrolled line whose prefix remains
/// fixed while the suffix scrolls by display columns.
#[must_use]
pub fn fixed_prefix_scroll_segments(
    text: &str,
    base_col: usize,
    fixed_prefix_cols: usize,
    scroll_cols: usize,
    viewport_cols: usize,
) -> Vec<FixedPrefixSegment> {
    use unicode_width::UnicodeWidthChar;

    let prefix_cols = fixed_prefix_cols.min(viewport_cols);
    let suffix_cols = viewport_cols.saturating_sub(prefix_cols);
    let suffix_start = fixed_prefix_cols.saturating_add(scroll_cols);
    let suffix_end = suffix_start.saturating_add(suffix_cols);
    let mut segments: Vec<FixedPrefixSegment> = Vec::new();
    let mut col = base_col;

    for (start_byte, ch) in text.char_indices() {
        if is_terminal_control_char(ch) {
            continue;
        }
        let end_byte = start_byte + ch.len_utf8();
        let width = ch.width().unwrap_or(0);
        if width == 0 {
            if let Some(last) = segments.last_mut()
                && last.end_byte == start_byte
            {
                last.end_byte = end_byte;
            }
            continue;
        }

        let target_col = if col < prefix_cols && col + width <= prefix_cols {
            col
        } else if col >= suffix_start && col + width <= suffix_end {
            prefix_cols + (col - suffix_start)
        } else {
            col += width;
            continue;
        };
        if target_col + width <= viewport_cols {
            segments.push(FixedPrefixSegment {
                start_byte,
                end_byte,
                target_col,
                display_cols: width,
            });
        }
        col += width;
    }

    segments
}

/// Collapse a terminal-window title to one printable line.
#[must_use]
pub fn sanitize_terminal_title(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    let mut prev_space = true;
    for ch in title.chars() {
        if ch.is_control() || ch == '\u{7f}' || ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    if out.ends_with(' ') {
        out.pop();
    }
    out
}
