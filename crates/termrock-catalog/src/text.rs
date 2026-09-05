// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/ui/text.rs (MIT).

//! Display-width helpers used by catalog chrome (not a widget).

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Display width in terminal cells.
#[must_use]
pub fn width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

/// Truncate to `max` display cells, appending `…` when cut.
#[must_use]
pub fn truncate(s: &str, max: usize) -> String {
    if width(s) <= max {
        return s.to_owned();
    }
    if max == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut w = 0;
    for g in s.graphemes(true) {
        let gw = width(g);
        if w + gw > max - 1 {
            break;
        }
        out.push_str(g);
        w += gw;
    }
    out.push('…');
    out
}

/// Left-align in `w` cells (pad or truncate).
#[must_use]
pub fn fit(s: &str, w: usize) -> String {
    let mut t = truncate(s, w);
    while width(&t) < w {
        t.push(' ');
    }
    t
}

/// Wrap `text` to `max` display cells.
#[must_use]
pub fn wrap(text: &str, max: usize) -> Vec<String> {
    if max == 0 {
        return vec![String::new()];
    }
    let mut lines = Vec::new();
    let mut cur = String::new();
    let mut cur_w = 0;
    for word in text.split(' ') {
        let ww = width(word);
        if cur.is_empty() {
            cur.push_str(word);
            cur_w = ww;
            continue;
        }
        if cur_w + 1 + ww <= max {
            cur.push(' ');
            cur.push_str(word);
            cur_w += 1 + ww;
        } else {
            lines.push(cur);
            cur = word.to_owned();
            cur_w = ww;
        }
    }
    if !cur.is_empty() || lines.is_empty() {
        lines.push(cur);
    }
    lines
}
