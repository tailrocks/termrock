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

/// Closest grapheme boundary at or before `byte` (clamped to line).
pub(crate) fn boundary_at_or_before(line: &str, byte: usize) -> usize {
    let byte = byte.min(line.len());
    if is_boundary(line, byte) {
        return byte;
    }
    previous_boundary(line, byte).unwrap_or(0)
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

/// True if grapheme is considered a "word" character (alphanumeric or `_`).
pub(crate) fn is_word_grapheme(g: &str) -> bool {
    g.chars()
        .next()
        .is_some_and(|c| c.is_alphanumeric() || c == '_')
}

/// Previous word-start boundary (Emacs/Vim-style word left).
pub(crate) fn previous_word_boundary(line: &str, byte: usize) -> usize {
    let byte = boundary_at_or_before(line, byte.min(line.len()));
    if byte == 0 {
        return 0;
    }
    let mut pos = byte;
    // 1) skip whitespace left
    while let Some(prev) = previous_boundary(line, pos) {
        let g = &line[prev..pos];
        if g.chars().all(char::is_whitespace) {
            pos = prev;
        } else {
            break;
        }
    }
    // 2) consume word or non-word cluster leftward
    if let Some(prev) = previous_boundary(line, pos) {
        let g = &line[prev..pos];
        if g.chars().all(char::is_whitespace) {
            return boundary_at_or_before(line, pos);
        }
        let word = is_word_grapheme(g);
        pos = prev;
        while let Some(p) = previous_boundary(line, pos) {
            let gg = &line[p..pos];
            if gg.chars().all(char::is_whitespace) {
                break;
            }
            if is_word_grapheme(gg) == word {
                pos = p;
            } else {
                break;
            }
        }
    }
    boundary_at_or_before(line, pos)
}

/// Next word-end / next word-start boundary (word right).
pub(crate) fn next_word_boundary(line: &str, byte: usize) -> usize {
    let byte = boundary_at_or_after(line, byte.min(line.len()));
    if byte >= line.len() {
        return line.len();
    }
    let mut pos = byte;
    // skip whitespace
    while let Some(next) = next_boundary(line, pos) {
        let g = &line[pos..next];
        if g.chars().all(char::is_whitespace) {
            pos = next;
        } else {
            break;
        }
    }
    if pos >= line.len() {
        return line.len();
    }
    // consume word or non-word cluster
    if let Some(next) = next_boundary(line, pos) {
        let g = &line[pos..next];
        let word = is_word_grapheme(g);
        pos = next;
        while let Some(n) = next_boundary(line, pos) {
            let gg = &line[pos..n];
            if is_word_grapheme(gg) == word && !gg.chars().all(char::is_whitespace) {
                pos = n;
            } else {
                break;
            }
        }
    }
    boundary_at_or_after(line, pos)
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
