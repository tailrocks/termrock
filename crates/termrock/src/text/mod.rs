//! Product-neutral terminal text measurement, sanitization, and windows.
pub use crate::ansi_text::{
    AnsiLine, AnsiParseOptions, AnsiSegment, AnsiStream, AnsiText, AnsiTextMode, AnsiTextState,
    is_paint_safe, line_from_ansi, lines_for_log, parse_lines, parse_to_line, strip_bytes,
    strip_str, styled_spans,
};
use std::borrow::Cow;

use ratatui_core::{buffer::Buffer, layout::Rect, style::Style, text::Line};
use unicode_segmentation::UnicodeSegmentation;
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

/// Truncate to display columns without splitting a grapheme, appending the
/// caller-selected Unicode or ASCII ellipsis when contraction is required.
#[must_use]
pub fn truncate_cols<'a>(s: &'a str, max_cols: usize, ellipsis: &str) -> Cow<'a, str> {
    if display_cols(s) <= max_cols {
        return Cow::Borrowed(s);
    }
    let ellipsis_width = display_cols(ellipsis);
    if ellipsis_width > max_cols {
        return Cow::Owned(take_display_cols(ellipsis, max_cols));
    }
    let budget = max_cols - ellipsis_width;
    let mut out = String::new();
    let mut used = 0;
    for grapheme in s.graphemes(true) {
        let width = display_cols(grapheme);
        if used + width > budget {
            break;
        }
        out.push_str(grapheme);
        used += width;
    }
    out.push_str(ellipsis);
    Cow::Owned(out)
}

/// Substring of `s` covering display columns `[skip, skip + width)`,
/// skipping terminal control bytes and preserving only complete grapheme clusters.
#[must_use]
pub fn display_cols_slice(s: &str, skip: usize, width: usize) -> String {
    let mut out = String::new();
    display_cols_slice_into(s, skip, width, &mut out);
    out
}

/// Writes the display-column window of `s` into a reusable buffer.
///
/// `out` is cleared first. Control bytes and partial wide characters are
/// omitted using the same rules as [`display_cols_slice`].
pub fn display_cols_slice_into(s: &str, skip: usize, width: usize, out: &mut String) {
    let mut col = 0usize;
    out.clear();
    for grapheme in s.graphemes(true) {
        let sanitized = grapheme.chars().any(is_terminal_control_char).then(|| {
            grapheme
                .chars()
                .filter(|ch| !is_terminal_control_char(*ch))
                .collect::<String>()
        });
        let grapheme = sanitized.as_deref().unwrap_or(grapheme);
        let w = UnicodeWidthStr::width(grapheme);
        if col >= skip && col + w <= skip + width {
            out.push_str(grapheme);
        }
        col += w;
        if col >= skip + width {
            break;
        }
    }
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
/// A source-byte segment and its measured display-column placement.
pub struct FixedPrefixSegment {
    /// Inclusive UTF-8 byte offset in the source string.
    pub start_byte: usize,
    /// Exclusive UTF-8 byte offset in the source string.
    pub end_byte: usize,
    /// Zero-based output display column.
    pub target_col: usize,
    /// Width of the segment in terminal display columns.
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

/// Expand ASCII tabs to spaces (`tab_width` columns, default 4 when 0 → 4).
///
/// Control characters other than tab are dropped (copy-safe plain text path).
#[must_use]
pub fn expand_tabs(s: &str, tab_width: usize) -> String {
    let tab_w = if tab_width == 0 { 4 } else { tab_width };
    let mut out = String::with_capacity(s.len());
    let mut col = 0usize;
    for c in s.chars() {
        if c == '\t' {
            let spaces = tab_w - (col % tab_w);
            for _ in 0..spaces {
                out.push(' ');
            }
            col += spaces;
            continue;
        }
        if is_terminal_control_char(c) {
            continue;
        }
        out.push(c);
        col += unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
    }
    out
}

/// Soft-wrap `s` into display-column lines of at most `width` columns.
///
/// Grapheme-safe: never splits combining marks or wide cells. Empty input
/// yields one empty line when `width > 0` and a single empty string otherwise.
/// Prefer this for body prose; pair with [`take_display_cols`] for clip-only.
#[must_use]
pub fn wrap_display_cols(s: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }
    if s.is_empty() {
        return vec![String::new()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut used = 0usize;
    for grapheme in s.graphemes(true) {
        let sanitized: String = grapheme
            .chars()
            .filter(|c| !is_terminal_control_char(*c))
            .collect();
        if sanitized.is_empty() {
            continue;
        }
        let w = UnicodeWidthStr::width(sanitized.as_str());
        if w == 0 {
            // zero-width joiners already in grapheme — keep attached
            current.push_str(&sanitized);
            continue;
        }
        if w > width {
            // Oversized grapheme alone: flush and place on own line (may overflow paint clip).
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
                used = 0;
            }
            lines.push(sanitized);
            continue;
        }
        if used + w > width && !current.is_empty() {
            lines.push(std::mem::take(&mut current));
            used = 0;
        }
        current.push_str(&sanitized);
        used += w;
    }
    if !current.is_empty() || lines.is_empty() {
        lines.push(current);
    }
    lines
}

/// Truncate `s` to `max_cols` with an ellipsis policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TruncateMode {
    /// Keep prefix, ellipsis at end.
    #[default]
    End,
    /// Ellipsis at start, keep suffix.
    Start,
    /// Keep head and tail with ellipsis in the middle.
    Middle,
}

/// Truncate to `max_cols` using `ellipsis` (display width counted). Empty if
/// `max_cols` is zero.
#[must_use]
pub fn truncate_display_cols(
    s: &str,
    max_cols: usize,
    mode: TruncateMode,
    ellipsis: &str,
) -> String {
    if max_cols == 0 {
        return String::new();
    }
    let full = display_cols(s);
    if full <= max_cols {
        // Still strip controls for copy-safe consistency.
        return take_display_cols(s, max_cols);
    }
    let ell_w = display_cols(ellipsis);
    if ell_w >= max_cols {
        return take_display_cols(ellipsis, max_cols);
    }
    let budget = max_cols.saturating_sub(ell_w);
    match mode {
        TruncateMode::End => {
            let mut out = take_display_cols(s, budget);
            out.push_str(ellipsis);
            out
        }
        TruncateMode::Start => {
            // Keep suffix of `budget` cols.
            let total = display_cols(s);
            let skip = total.saturating_sub(budget);
            let mut out = String::from(ellipsis);
            out.push_str(&display_cols_slice(s, skip, budget));
            out
        }
        TruncateMode::Middle => {
            let head = budget / 2;
            let tail = budget.saturating_sub(head);
            let total = display_cols(s);
            let mut out = take_display_cols(s, head);
            out.push_str(ellipsis);
            let skip = total.saturating_sub(tail);
            out.push_str(&display_cols_slice(s, skip, tail));
            out
        }
    }
}

/// Horizontal alignment of painted content inside its rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CellAlignment {
    /// Align content to the left edge.
    #[default]
    Left,
    /// Center content in the resolved width.
    Center,
    /// Align content to the right edge.
    Right,
}

/// How painted content treats a rectangle that is too small.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum CellOverflow {
    /// Clip at a display-column boundary (default).
    #[default]
    Clip,
    /// Clip and mark the contraction with the caller's ellipsis glyph.
    Ellipsis,
}

/// Alignment, overflow policy, and ellipsis glyph for one painted line.
///
/// Right-aligned content contracts from its head (`…5678`) so the meaningful
/// tail — the part alignment pulled to the edge — survives; every other
/// alignment contracts from its tail (`1234…`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinePlacement<'a> {
    /// Horizontal alignment inside the paint rectangle.
    pub alignment: CellAlignment,
    /// Contraction policy applied when content exceeds the rectangle.
    pub overflow: CellOverflow,
    /// Glyph painted where content was contracted.
    pub ellipsis: &'a str,
}

impl<'a> LinePlacement<'a> {
    /// Left-aligned placement that clips without an ellipsis marker.
    #[must_use]
    pub const fn clipped(ellipsis: &'a str) -> Self {
        Self {
            alignment: CellAlignment::Left,
            overflow: CellOverflow::Clip,
            ellipsis,
        }
    }

    /// Left-aligned placement that marks contraction with `ellipsis`.
    #[must_use]
    pub const fn contracting(ellipsis: &'a str) -> Self {
        Self {
            alignment: CellAlignment::Left,
            overflow: CellOverflow::Ellipsis,
            ellipsis,
        }
    }

    /// Places content against a different edge of the rectangle.
    #[must_use]
    pub const fn align(mut self, alignment: CellAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    /// Chooses whether contraction is silent or marked.
    #[must_use]
    pub const fn overflow(mut self, overflow: CellOverflow) -> Self {
        self.overflow = overflow;
        self
    }
}

/// Paints one row of plain `text`, contracting with `ellipsis` when the text is
/// wider than `area`.
///
/// This is the sanctioned painter for titles, labels, and single-line values:
/// it never splits a grapheme cluster and never leaves a silent hard cut.
pub fn paint_text(buffer: &mut Buffer, area: Rect, text: &str, style: Style, ellipsis: &str) {
    if area.is_empty() {
        return;
    }
    let width = usize::from(area.width);
    let fitted = truncate_cols(text, width, ellipsis);
    let safe = if fitted.chars().any(is_terminal_control_char) {
        Cow::Owned(
            fitted
                .chars()
                .filter(|ch| !is_terminal_control_char(*ch))
                .collect::<String>(),
        )
    } else {
        fitted
    };
    buffer.set_stringn(area.x, area.y, safe.as_ref(), width, style);
}

/// Paints a styled `line` into `area`, preserving per-span styles across the
/// alignment offset and the contraction boundary.
///
/// `scratch` is a caller-owned buffer reused across rows so a table body does
/// not allocate per cell.
pub fn paint_line_overflow(
    buffer: &mut Buffer,
    area: Rect,
    line: &Line<'_>,
    style: Style,
    placement: LinePlacement<'_>,
    scratch: &mut String,
) {
    if area.is_empty() {
        return;
    }
    buffer.set_style(area, style);
    let line_width = line
        .spans
        .iter()
        .map(|span| display_cols(span.content.as_ref()))
        .sum::<usize>();
    let width = usize::from(area.width);
    let contracts = matches!(placement.overflow, CellOverflow::Ellipsis) && line_width > width;
    let ellipsis_cols = if contracts {
        display_cols(placement.ellipsis).min(width)
    } else {
        0
    };
    let budget = width.saturating_sub(ellipsis_cols);
    if budget == 0 {
        if ellipsis_cols > 0 {
            buffer.set_stringn(area.x, area.y, placement.ellipsis, width, style);
        }
        return;
    }
    // Right-aligned content keeps its tail; every other alignment keeps its head.
    let leading_ellipsis = contracts && matches!(placement.alignment, CellAlignment::Right);
    let skip = if leading_ellipsis {
        line_width.saturating_sub(budget)
    } else {
        0
    };
    let visible = line_width.saturating_sub(skip).min(budget);
    let pad = match placement.alignment {
        CellAlignment::Left => 0,
        CellAlignment::Center => budget.saturating_sub(visible) / 2,
        CellAlignment::Right => budget.saturating_sub(visible),
    };
    let content_x = if leading_ellipsis { ellipsis_cols } else { 0 } + pad;
    paint_line_window(
        buffer,
        area,
        line,
        style,
        (skip, budget),
        content_x,
        scratch,
    );
    if contracts {
        let ellipsis_x = if leading_ellipsis {
            0
        } else {
            content_x + visible
        };
        let x = area
            .x
            .saturating_add(u16::try_from(ellipsis_x).unwrap_or(u16::MAX));
        buffer.set_stringn(x, area.y, placement.ellipsis, ellipsis_cols, style);
    }
}

/// Paints the `[skip, skip + budget)` display-column window of `line` starting
/// at `content_x` cells into `area`.
fn paint_line_window(
    buffer: &mut Buffer,
    area: Rect,
    line: &Line<'_>,
    style: Style,
    window: (usize, usize),
    content_x: usize,
    scratch: &mut String,
) {
    let (skip, budget) = window;
    let end = skip.saturating_add(budget);
    let mut logical_col = 0usize;
    for span in &line.spans {
        let span_width = display_cols(span.content.as_ref());
        let span_end = logical_col + span_width;
        if span_end <= skip {
            logical_col = span_end;
            continue;
        }
        if logical_col >= end {
            break;
        }
        let local_skip = skip.saturating_sub(logical_col);
        let take = span_width
            .saturating_sub(local_skip)
            .min(end - logical_col.max(skip));
        display_cols_slice_into(span.content.as_ref(), local_skip, take, scratch);
        let target = content_x + logical_col.max(skip) - skip;
        buffer.set_stringn(
            area.x
                .saturating_add(u16::try_from(target).unwrap_or(u16::MAX)),
            area.y,
            scratch.as_str(),
            take,
            style.patch(span.style),
        );
        logical_col = span_end;
    }
}

/// Contracts a path to `budget` display columns by dropping leading segments,
/// so the discriminating tail (`…/widgets/quick_open.rs`) stays readable.
///
/// When not even one leading `…/` plus the final segment fits, the directories
/// go entirely and the filename alone is contracted from its middle: a
/// ten-column column shows `quick_o…rs`, not `src/w…en.rs`. Middle contraction
/// is what keeps both the start of a name and its extension.
#[must_use]
pub fn truncate_path<'a>(path: &'a str, budget: usize, ellipsis: &str) -> Cow<'a, str> {
    if display_cols(path) <= budget {
        return Cow::Borrowed(path);
    }
    let ellipsis_cols = display_cols(ellipsis);
    if ellipsis_cols < budget {
        // Longest suffix that starts at a separator and still fits.
        for (index, _) in path.match_indices('/') {
            let suffix = &path[index..];
            if ellipsis_cols + display_cols(suffix) <= budget {
                let mut out = String::with_capacity(ellipsis.len() + suffix.len());
                out.push_str(ellipsis);
                out.push_str(suffix);
                return Cow::Owned(out);
            }
        }
    }
    // Directories cannot help at this width; keep the name itself.
    let name = path.rsplit('/').next().unwrap_or(path);
    Cow::Owned(truncate_display_cols(
        name,
        budget,
        TruncateMode::Middle,
        ellipsis,
    ))
}

/// States how many rows a surface cut, in the one voice that says it.
///
/// A list that stops painting is telling the operator "that's all of it"
/// unless it says otherwise. Plan 022 fixed four such surfaces by writing
/// `format!("+{hidden} more")` four times, which is a template, not an
/// authority — the fifth surface to clip (`integration_status`) simply went
/// silent. One function, one gate (`design_gate.rs::one_overflow_note`), and
/// the phrasing stays a single decision (plans/017 §B2, plans/022 Step 3).
///
/// Returns `None` when nothing was hidden, so callers spend no row on it.
#[must_use]
pub fn more_note(hidden: usize) -> Option<String> {
    (hidden > 0).then(|| format!("+{hidden} more"))
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

#[cfg(test)]
mod tests {
    use ratatui_core::text::Span;

    use super::*;

    fn row_text(buffer: &Buffer, row: u16) -> String {
        (0..buffer.area.width)
            .map(|x| buffer[(x, row)].symbol())
            .collect()
    }

    /// Row content without the blank continuation cell of a wide grapheme.
    fn row_glyphs(buffer: &Buffer, row: u16) -> String {
        row_text(buffer, row).replace(' ', "")
    }

    #[test]
    fn paint_text_contracts_wide_titles_without_splitting_a_cell() {
        let area = Rect::new(0, 0, 8, 1);
        let mut buffer = Buffer::empty(area);
        paint_text(&mut buffer, area, "日本語のタイトル", Style::default(), "…");
        let painted = row_glyphs(&buffer, 0);
        assert_eq!(painted, "日本語…");
        assert!(display_cols(&painted) <= 8);
    }

    #[test]
    fn paint_text_keeps_zwj_sequences_whole() {
        let area = Rect::new(0, 0, 3, 1);
        let mut buffer = Buffer::empty(area);
        // Family emoji is one grapheme cluster: it survives or it goes, never splits.
        paint_text(&mut buffer, area, "👨‍👩‍👧‍👦👨‍👩‍👧‍👦", Style::default(), "…");
        let painted = row_glyphs(&buffer, 0);
        assert!(painted.ends_with('…'), "{painted:?}");
        assert_eq!(painted.matches('\u{200d}').count(), 3, "{painted:?}");
    }

    #[test]
    fn right_aligned_lines_contract_from_the_head() {
        let area = Rect::new(0, 0, 5, 1);
        let mut buffer = Buffer::empty(area);
        let line = Line::from(vec![Span::raw("1234 5678")]);
        let mut scratch = String::new();
        paint_line_overflow(
            &mut buffer,
            area,
            &line,
            Style::default(),
            LinePlacement::contracting("…").align(CellAlignment::Right),
            &mut scratch,
        );
        assert_eq!(row_text(&buffer, 0), "…5678");
    }

    #[test]
    fn left_aligned_lines_contract_from_the_tail_and_keep_span_styles() {
        use ratatui_core::style::Color;
        let area = Rect::new(0, 0, 6, 1);
        let mut buffer = Buffer::empty(area);
        let line = Line::from(vec![
            Span::styled("ab", Style::default().fg(Color::Red)),
            Span::raw("cdefgh"),
        ]);
        let mut scratch = String::new();
        paint_line_overflow(
            &mut buffer,
            area,
            &line,
            Style::default(),
            LinePlacement::contracting("…"),
            &mut scratch,
        );
        assert_eq!(row_text(&buffer, 0), "abcde…");
        assert_eq!(buffer[(0, 0)].fg, Color::Red);
        assert_eq!(buffer[(2, 0)].fg, Color::Reset);
    }

    #[test]
    fn centered_lines_stay_centered_when_they_fit() {
        let area = Rect::new(0, 0, 7, 1);
        let mut buffer = Buffer::empty(area);
        let line = Line::from(vec![Span::raw("abc")]);
        let mut scratch = String::new();
        paint_line_overflow(
            &mut buffer,
            area,
            &line,
            Style::default(),
            LinePlacement::clipped("…").align(CellAlignment::Center),
            &mut scratch,
        );
        assert_eq!(row_text(&buffer, 0), "  abc  ");
    }

    #[test]
    fn paths_drop_leading_segments_before_they_lose_the_filename() {
        assert_eq!(
            truncate_path("src/widgets/quick_open.rs", 20, "…"),
            "…/quick_open.rs"
        );
        assert_eq!(
            truncate_path("src/widgets/quick_open.rs", 40, "…"),
            "src/widgets/quick_open.rs"
        );
        // No separator fits: the directories go and the name keeps its head
        // and its extension.
        let bare = truncate_path("averylongfilename.rs", 12, "…");
        assert!(bare.starts_with('a') && bare.ends_with(".rs"));
        assert_eq!(display_cols(&bare), 12);
        let squeezed = truncate_path("src/widgets/quick_open.rs", 10, "…");
        assert!(
            squeezed.starts_with("quic") && squeezed.ends_with(".rs"),
            "{squeezed}"
        );
        assert_eq!(display_cols(&squeezed), 10);
    }

    #[test]
    fn reusable_display_slice_matches_allocating_variant() {
        let text = "a\u{1b}界bc";
        let mut out = String::from("stale capacity");
        display_cols_slice_into(text, 1, 3, &mut out);
        assert_eq!(out, display_cols_slice(text, 1, 3));
        assert_eq!(out, "界b");
    }

    #[test]
    fn display_width_handles_wide_combining_control_and_empty_text() {
        for (text, width) in [
            ("ascii", 5),
            ("日本語", 6),
            ("🧪", 2),
            ("e\u{301}", 1),
            ("a\u{1b}b", 2),
            ("", 0),
        ] {
            assert_eq!(display_cols(text), width, "{text:?}");
        }
    }

    #[test]
    fn display_prefix_never_splits_wide_characters() {
        for (text, width, expected) in [
            ("abc", 2, "ab"),
            ("日本", 3, "日"),
            ("🧪x", 2, "🧪"),
            ("e\u{301}x", 1, "e\u{301}"),
            ("a\u{7f}b", 2, "ab"),
            ("", 4, ""),
        ] {
            let taken = take_display_cols(text, width);
            assert_eq!(taken, expected, "{text:?} at {width}");
            assert!(display_cols(&taken) <= width);
        }
    }

    #[test]
    fn display_slices_drop_partial_wide_characters() {
        for (text, skip, width, expected) in [
            ("abcdef", 2, 3, "cde"),
            ("日本", 1, 1, ""),
            ("日本", 0, 2, "日"),
            ("e\u{301}x", 0, 1, "e\u{301}"),
            ("\u{7}\u{80}\u{7f}ab", 0, 2, "ab"),
            ("🧪x", 0, 0, ""),
            ("abc", 0, 20, "abc"),
            ("", 0, 4, ""),
        ] {
            let slice = display_cols_slice(text, skip, width);
            assert_eq!(slice, expected, "{text:?} [{skip}..{}]", skip + width);
            assert!(display_cols(&slice) <= width);
        }
    }

    #[test]
    fn control_boundaries_match_terminal_ranges() {
        for (ch, expected) in [
            ('\u{1f}', true),
            ('\u{20}', false),
            ('\u{7e}', false),
            ('\u{7f}', true),
            ('\u{80}', true),
            ('\u{9f}', true),
            ('\u{a0}', false),
        ] {
            assert_eq!(
                is_terminal_control_char(ch),
                expected,
                "U+{:04X}",
                ch as u32
            );
        }
    }

    #[test]
    fn terminal_titles_collapse_controls_and_whitespace() {
        assert_eq!(
            sanitize_terminal_title(" \u{1b}build\u{7}\n\tready\u{9b} "),
            "build ready"
        );
        assert_eq!(sanitize_terminal_title("\u{1b}\u{7}\n"), "");
    }

    #[test]
    fn indentation_measurement_matches_trailing_padding_contract() {
        assert_eq!(leading_space_cols(["  one", "two"]), 2);
        assert_eq!(leading_space_cols(["", "   "]), 3);
        assert_eq!(padded_line_display_cols(["  one"]), 7);
    }

    #[test]
    fn expand_tabs_and_strip_controls() {
        assert_eq!(expand_tabs("a\tb", 4), "a   b");
        assert_eq!(expand_tabs("a\u{1b}b", 4), "ab");
    }

    #[test]
    fn wrap_display_cols_cjk_and_ascii() {
        let lines = wrap_display_cols("hello world", 5);
        assert_eq!(lines, vec!["hello", " worl", "d"]);
        let cjk = wrap_display_cols("日本語です", 4);
        assert!(cjk.iter().all(|l| display_cols(l) <= 4));
        assert!(cjk.len() >= 2);
    }

    #[test]
    fn truncate_modes() {
        assert_eq!(
            truncate_display_cols("abcdef", 5, TruncateMode::End, "…"),
            "abcd…"
        );
        let start = truncate_display_cols("abcdef", 5, TruncateMode::Start, "…");
        assert!(start.starts_with('…') || start.starts_with("..."));
        assert_eq!(display_cols(&start), 5);
        let mid = truncate_display_cols("abcdefghij", 7, TruncateMode::Middle, "…");
        assert!(mid.contains('…') || mid.contains("..."));
        assert!(display_cols(&mid) <= 7);
    }

    #[test]
    fn truncate_cols_is_grapheme_safe_and_borrows_exact_fit() {
        let exact = truncate_cols("hello", 5, "…");
        assert!(matches!(exact, Cow::Borrowed("hello")));
        let cjk = truncate_cols("日本語", 5, "…");
        assert!(display_cols(&cjk) <= 5);
        assert!(cjk.ends_with('…'));
        let combining = truncate_cols("e\u{301}clair", 4, "…");
        assert!(display_cols(&combining) <= 4);
        assert!(combining.starts_with("e\u{301}"));
    }

    #[test]
    fn fixed_prefix_segments_cover_scroll_and_combining_boundaries() {
        let fit = fixed_prefix_scroll_segments("ab", 0, 1, 0, 2);
        assert_eq!(fit.len(), 2);
        assert_eq!((fit[0].target_col, fit[1].target_col), (0, 1));

        let past_end = fixed_prefix_scroll_segments("ab", 0, 1, 10, 2);
        assert_eq!(past_end.len(), 1);
        assert_eq!(past_end[0].start_byte, 0);

        let combining = fixed_prefix_scroll_segments("e\u{301}x", 0, 1, 0, 2);
        assert_eq!(combining[0].end_byte, "e\u{301}".len());
        assert_eq!(combining[1].target_col, 1);

        let no_prefix = fixed_prefix_scroll_segments("ab", 0, 0, 1, 1);
        assert_eq!(no_prefix.len(), 1);
        assert_eq!(no_prefix[0].target_col, 0);
    }
}
