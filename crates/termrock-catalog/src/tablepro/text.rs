// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/ui/text.rs (MIT),
// https://github.com/donbeave/terminal-components-claude

//! TablePro text helpers (fuzzy rank, middle truncate). Catalog chrome uses
//! [`crate::text`]; this module is the source matching/ranking used by
//! completion and the switcher.

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

fn width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

/// Truncate keeping both ends: `very_long_identifier_name` → `very_l…_name`.
#[must_use]
pub fn truncate_middle(s: &str, max: usize) -> String {
    if width(s) <= max {
        return s.to_owned();
    }
    if max < 5 {
        return crate::text::truncate(s, max);
    }
    let keep_end = (max - 1) / 3;
    let keep_start = max - 1 - keep_end;
    let gs: Vec<&str> = s.graphemes(true).collect();
    let mut head = String::new();
    let mut w = 0;
    for g in &gs {
        let gw = width(g);
        if w + gw > keep_start {
            break;
        }
        head.push_str(g);
        w += gw;
    }
    let mut tail = String::new();
    let mut w = 0;
    for g in gs.iter().rev() {
        let gw = width(g);
        if w + gw > keep_end {
            break;
        }
        tail.insert_str(0, g);
        w += gw;
    }
    format!("{head}…{tail}")
}

/// Fuzzy match `word` against `label`: prefix, then substring, then subsequence.
/// Returns a penalty (lower is better) and byte offsets of matched characters.
#[must_use]
pub fn fuzzy(label: &str, word: &str) -> Option<(u32, Vec<usize>)> {
    if word.is_empty() {
        return Some((0, vec![]));
    }
    let l = label.to_lowercase();
    let w = word.to_lowercase();
    if l.starts_with(&w) {
        return Some((0, (0..w.len()).collect()));
    }
    if let Some(p) = l.find(&w) {
        let boundary = p == 0 || matches!(l.as_bytes()[p - 1], b'_' | b'.');
        return Some((if boundary { 10 } else { 30 }, (p..p + w.len()).collect()));
    }
    let mut matched = Vec::new();
    let mut li = 0;
    let lb = l.as_bytes();
    for wc in w.bytes() {
        while li < lb.len() && lb[li] != wc {
            li += 1;
        }
        if li >= lb.len() {
            return None;
        }
        matched.push(li);
        li += 1;
    }
    Some((60 + (matched.last().copied().unwrap_or(0) as u32), matched))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn middle_truncation() {
        assert_eq!(
            truncate_middle("very_long_identifier_name", 12),
            "very_lon…ame"
        );
        assert_eq!(truncate_middle("short", 12), "short");
    }

    #[test]
    fn fuzzy_prefix_wins() {
        let (pen, m) = fuzzy("orders", "ord").unwrap();
        assert_eq!(pen, 0);
        assert_eq!(m, vec![0, 1, 2]);
        assert!(fuzzy("orders", "xyz").is_none());
    }
}
