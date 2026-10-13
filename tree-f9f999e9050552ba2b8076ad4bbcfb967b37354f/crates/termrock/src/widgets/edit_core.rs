//! Shared single-line grapheme editing primitives.

use std::{borrow::Cow, ops::Range};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LineDelta {
    Inserted { range: Range<usize> },
    Deleted { at: usize, text: String },
}

pub(crate) fn is_boundary(line: &str, byte: usize) -> bool {
    byte == line.len() || line.grapheme_indices(true).any(|(index, _)| index == byte)
}

pub(crate) fn previous_boundary(line: &str, byte: usize) -> Option<usize> {
    line.get(..byte)?
        .grapheme_indices(true)
        .next_back()
        .map(|(index, _)| index)
}

pub(crate) fn next_boundary(line: &str, byte: usize) -> Option<usize> {
    line.get(byte..)?
        .graphemes(true)
        .next()
        .map(|grapheme| byte + grapheme.len())
}

pub(crate) fn boundary_at_or_after(line: &str, byte: usize) -> usize {
    line.grapheme_indices(true)
        .map(|(index, _)| index)
        .chain(core::iter::once(line.len()))
        .find(|boundary| *boundary >= byte)
        .unwrap_or(line.len())
}

pub(crate) fn insert_char(
    line: &mut String,
    byte: &mut usize,
    character: char,
) -> Option<LineDelta> {
    if character.is_control() || !is_boundary(line, *byte) {
        return None;
    }
    let insertion = *byte;
    let logical_end = insertion + character.len_utf8();
    line.insert(insertion, character);
    *byte = boundary_at_or_after(line, logical_end);
    Some(LineDelta::Inserted {
        range: insertion..logical_end,
    })
}

pub(crate) fn insert_inline(line: &mut String, byte: &mut usize, text: &str) -> Option<LineDelta> {
    if !is_boundary(line, *byte) {
        return None;
    }
    let filtered = if text.chars().any(char::is_control) {
        Cow::Owned(
            text.chars()
                .filter(|character| !character.is_control())
                .collect::<String>(),
        )
    } else {
        Cow::Borrowed(text)
    };
    if filtered.is_empty() {
        return None;
    }
    let logical_end = *byte + filtered.len();
    line.insert_str(*byte, &filtered);
    *byte = boundary_at_or_after(line, logical_end);
    Some(LineDelta::Inserted {
        range: (logical_end - filtered.len())..logical_end,
    })
}

pub(crate) fn backspace(line: &mut String, byte: &mut usize) -> Option<LineDelta> {
    let previous = previous_boundary(line, *byte)?;
    let text = line[previous..*byte].to_owned();
    line.drain(previous..*byte);
    *byte = previous;
    Some(LineDelta::Deleted { at: previous, text })
}

pub(crate) fn delete(line: &mut String, byte: usize) -> Option<LineDelta> {
    let next = next_boundary(line, byte)?;
    let text = line[byte..next].to_owned();
    line.drain(byte..next);
    Some(LineDelta::Deleted { at: byte, text })
}

pub(crate) fn byte_at_display_column(line: &str, goal: usize) -> usize {
    let mut column = 0;
    let mut boundary = 0;
    for (byte, grapheme) in line.grapheme_indices(true) {
        let next = column + crate::text::display_cols(grapheme);
        if next > goal {
            break;
        }
        column = next;
        boundary = byte + grapheme.len();
    }
    boundary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insertion_repairs_globally_merged_graphemes() {
        let mut combining = "\u{301}x".to_owned();
        let mut byte = 0;
        assert!(insert_char(&mut combining, &mut byte, 'e').is_some());
        assert_eq!((combining.as_str(), byte), ("e\u{301}x", 3));

        let mut joined = "👩\u{200d}".to_owned();
        let mut byte = joined.len();
        assert!(insert_char(&mut joined, &mut byte, '🔬').is_some());
        assert_eq!(byte, joined.len());
        assert!(is_boundary(&joined, byte));
    }

    #[test]
    fn deltas_restore_exact_line_without_snapshot_diffing() {
        let original = "a🧪b";
        let mut line = original.to_owned();
        let mut byte = 1;
        let inserted = insert_inline(&mut line, &mut byte, "東京").unwrap();
        if let LineDelta::Inserted { range } = inserted {
            line.replace_range(range, "");
        }
        assert_eq!(line, original);
        let mut byte = 5;
        let deleted = backspace(&mut line, &mut byte).unwrap();
        if let LineDelta::Deleted { at, text } = deleted {
            line.insert_str(at, &text);
        }
        assert_eq!(line, original);
    }
}
