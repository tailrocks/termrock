//! One row, several tiers.
//!
//! The data widgets — log streams, event streams, traces, inspectors — build a
//! row out of facts of different weight: a timestamp, a source, a level, a
//! message, a count. Built as one `format!` and painted with one style, those
//! five facts arrive as five equals, and the reader has to parse the row
//! instead of scanning it.
//!
//! [`TieredRow`] keeps the joined string those widgets need for wrapping,
//! horizontal scrolling and clipping, and remembers where each fact sits so
//! the tones can be put back on afterwards. The geometry stays exactly what it
//! was; only the voice changes.
use ratatui_core::{buffer::Buffer, layout::Rect, style::Style};

use crate::text::display_cols;

/// A run of columns that carries its own tone.
#[derive(Debug, Clone, Copy)]
struct TierSpan {
    start: usize,
    cols: usize,
    style: Style,
}

/// A row assembled from tiers: joined text plus the tone of each part.
#[derive(Debug, Clone)]
pub(crate) struct TieredRow {
    text: String,
    cols: usize,
    separator: &'static str,
    tiers: Vec<TierSpan>,
}

impl Default for TieredRow {
    fn default() -> Self {
        Self::with_separator(" ")
    }
}

impl TieredRow {
    /// A row whose parts are separated by `separator` (usually one space).
    pub(crate) fn with_separator(separator: &'static str) -> Self {
        Self {
            text: String::new(),
            cols: 0,
            separator,
            tiers: Vec::new(),
        }
    }

    /// Appends a part after one separating space, in the row's base tone.
    pub(crate) fn push_plain(&mut self, text: &str) {
        self.append(text, None);
    }

    /// Appends a part after one separating space, in its own tone.
    pub(crate) fn push(&mut self, text: &str, style: Style) {
        self.append(text, Some(style));
    }

    /// Appends a part with no separating space (a suffix such as `×3`).
    pub(crate) fn push_joined(&mut self, text: &str, style: Option<Style>) {
        if text.is_empty() {
            return;
        }
        self.record(text, style);
    }

    fn append(&mut self, text: &str, style: Option<Style>) {
        if text.is_empty() {
            return;
        }
        if !self.text.is_empty() {
            self.text.push_str(self.separator);
            self.cols += display_cols(self.separator);
        }
        self.record(text, style);
    }

    fn record(&mut self, text: &str, style: Option<Style>) {
        let start = self.cols;
        let cols = display_cols(text);
        self.text.push_str(text);
        self.cols += cols;
        if let Some(style) = style
            && cols > 0
        {
            self.tiers.push(TierSpan { start, cols, style });
        }
    }

    /// The joined row, for wrapping / clipping / horizontal scrolling.
    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    /// Puts each tier's tone back on the painted row.
    ///
    /// `skip` is the horizontal scroll offset already applied to `row`, in
    /// display columns; parts scrolled off the left are dropped and a part
    /// straddling the edge keeps only its visible tail. Existing backgrounds
    /// survive, so a selection wash painted under the row is not undone.
    pub(crate) fn paint_tiers(&self, buffer: &mut Buffer, row: Rect, skip: usize) {
        if row.width == 0 || row.height == 0 {
            return;
        }
        for tier in &self.tiers {
            let end = tier.start + tier.cols;
            if end <= skip {
                continue;
            }
            let visible_start = tier.start.max(skip) - skip;
            let visible_cols = end - tier.start.max(skip);
            let Ok(x0) = u16::try_from(visible_start) else {
                continue;
            };
            let x = row.x.saturating_add(x0);
            if x >= row.right() {
                continue;
            }
            let width = u16::try_from(visible_cols)
                .unwrap_or(u16::MAX)
                .min(row.right().saturating_sub(x));
            for dx in 0..width {
                let cell = &mut buffer[(x.saturating_add(dx), row.y)];
                let ground = cell.style().bg;
                let mut style = tier.style;
                if style.bg.is_none()
                    && let Some(bg) = ground
                {
                    style = style.bg(bg);
                }
                cell.set_style(style);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui_core::style::Color;

    use super::*;

    fn row() -> TieredRow {
        let mut row = TieredRow::default();
        row.push("12:00", Style::default().fg(Color::Blue));
        row.push_plain("message body");
        row.push("x3", Style::default().fg(Color::Green));
        row
    }

    #[test]
    fn joined_text_and_tier_columns_agree() {
        let row = row();
        assert_eq!(row.text(), "12:00 message body x3");
        assert_eq!(row.cols, display_cols(row.text()));
    }

    #[test]
    fn tones_land_on_their_own_parts_only() {
        let row = row();
        let area = Rect::new(0, 0, 24, 1);
        let mut buffer = Buffer::empty(area);
        buffer.set_stringn(0, 0, row.text(), 24, Style::default().fg(Color::White));
        row.paint_tiers(&mut buffer, area, 0);
        assert_eq!(buffer[(0, 0)].style().fg, Some(Color::Blue), "timestamp");
        assert_eq!(buffer[(6, 0)].style().fg, Some(Color::White), "message");
        assert_eq!(buffer[(19, 0)].style().fg, Some(Color::Green), "count");
    }

    #[test]
    fn scrolled_rows_keep_the_visible_tail_of_a_straddling_part() {
        let row = row();
        let area = Rect::new(0, 0, 24, 1);
        let mut buffer = Buffer::empty(area);
        let scrolled: String = row.text().chars().skip(3).collect();
        buffer.set_stringn(0, 0, &scrolled, 24, Style::default().fg(Color::White));
        row.paint_tiers(&mut buffer, area, 3);
        // "00" is what is left of the timestamp.
        assert_eq!(buffer[(0, 0)].style().fg, Some(Color::Blue));
        assert_eq!(buffer[(1, 0)].style().fg, Some(Color::Blue));
        assert_eq!(buffer[(2, 0)].style().fg, Some(Color::White));
    }

    #[test]
    fn a_selection_wash_survives_the_tones() {
        let row = row();
        let area = Rect::new(0, 0, 24, 1);
        let mut buffer = Buffer::empty(area);
        buffer.set_style(area, Style::default().bg(Color::Magenta));
        buffer.set_stringn(0, 0, row.text(), 24, Style::default().bg(Color::Magenta));
        row.paint_tiers(&mut buffer, area, 0);
        assert_eq!(buffer[(0, 0)].style().bg, Some(Color::Magenta));
    }
}
