#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use crate::{
    style::{BadgeKind, DesignSystem, Role, RolePalette},
    text::{CellAlignment, LinePlacement, display_cols, paint_line_overflow, truncate_cols},
};
use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Widget,
};
use ratatui_widgets::paragraph::Paragraph;
use unicode_width::UnicodeWidthStr;

/// One footer-hint span shared by terminal surfaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HintSpan<'a> {
    /// A statically borrowed key glyph.
    Key(&'a str),
    /// An owned key glyph computed at runtime.
    DynKey(String),
    /// Statically borrowed hint text.
    Text(&'a str),
    /// Owned hint text computed at runtime.
    Dyn(String),
    /// A separator between a key and its label.
    Sep,
    /// A separator between adjacent hint groups.
    GroupSep,
}

/// Blank join painted between adjacent hint groups.
///
/// Groups are separated by blank space rather than a glyph so the eye reads them
/// as two clusters, not two facts. Its width matches the dotted separator, so
/// both wrap identically.
pub const HINT_GROUP_JOIN: &str = "   ";

/// Display columns every hint separator occupies, dotted or blank.
pub const HINT_SEPARATOR_COLS: usize = 3;

impl HintSpan<'_> {
    /// Display-column width contribution of this span.
    #[must_use]
    pub fn display_cols(&self) -> usize {
        match self {
            Self::Key(key) => UnicodeWidthStr::width(*key),
            Self::DynKey(key) => UnicodeWidthStr::width(key.as_str()),
            Self::Text(text) => 1 + UnicodeWidthStr::width(*text),
            Self::Dyn(text) => 1 + UnicodeWidthStr::width(text.as_str()),
            Self::Sep | Self::GroupSep => HINT_SEPARATOR_COLS,
        }
    }
}

/// Total display-column width of a hint-span sequence.
#[must_use]
pub fn hint_row_cols(spans: &[HintSpan<'_>]) -> usize {
    spans.iter().map(HintSpan::display_cols).sum()
}

#[derive(Debug, Clone, Copy)]
/// A key glyph and label shown in a [`HintBar`].
pub struct Hint<'a> {
    /// Key chord advertised by the hint.
    pub chord: &'a str,
    /// Caller-visible label.
    pub label: &'a str,
    /// Lower values are retained first when narrow layouts drop hints.
    pub priority: u8,
    /// Whether the hint participates in layout and rendering.
    pub visible: bool,
}

#[derive(Debug, Clone, Copy)]
/// A wrapping row of keyboard hints.
pub struct HintBar<'a> {
    hints: &'a [Hint<'a>],
    separator: &'a str,
    leading_spacer: bool,
    alignment: CellAlignment,
    system: &'a DesignSystem,
    /// Status sentence pinned to the right edge (text-secondary).
    right: Option<&'a str>,
}

impl<'a> HintBar<'a> {
    #[must_use]
    /// Creates a hint bar over borrowed hints with canonical spacing.
    pub const fn new(hints: &'a [Hint<'a>], system: &'a DesignSystem) -> Self {
        Self {
            hints,
            separator: system.glyphs.meta_join(),
            leading_spacer: false,
            alignment: CellAlignment::Left,
            system,
            right: None,
        }
    }

    /// Status sentence on the footer's right edge.
    #[must_use]
    pub const fn right(mut self, right: &'a str) -> Self {
        self.right = Some(right);
        self
    }

    #[must_use]
    /// Sets separator text rendered between groups.
    pub const fn separator(mut self, separator: &'a str) -> Self {
        self.separator = separator;
        self
    }

    /// Adds one painted blank row before the hints.
    #[must_use]
    pub const fn leading_spacer(mut self, leading_spacer: bool) -> Self {
        self.leading_spacer = leading_spacer;
        self
    }

    /// Places the hint rows against an edge of their area.
    ///
    /// This is the only alignment control for footer hints: the rich-span
    /// painter [`render_hint_bar`] resolves through the same path.
    #[must_use]
    pub const fn alignment(mut self, alignment: CellAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    /// Returns the rows required to paint this bar at `width`.
    ///
    /// Footer hints are one row; overflow drops from the right.
    #[must_use]
    pub fn measured_height(&self, _width: u16) -> u16 {
        1u16.saturating_add(u16::from(self.leading_spacer))
    }

    /// Hints that fit, most important first, dropping from the right.
    fn fitted(&self, width: u16) -> Vec<&'a Hint<'a>> {
        let mut hints: Vec<&Hint<'a>> = self.hints.iter().filter(|hint| hint.visible).collect();
        hints.sort_by_key(|h| h.priority);
        let right_w = self
            .right
            .map(|r| (UnicodeWidthStr::width(r) as u16).saturating_add(3))
            .unwrap_or(0);
        let mut x = 1u16;
        let mut out = Vec::new();
        for hint in hints {
            let w = (UnicodeWidthStr::width(hint.chord) as u16)
                .saturating_add(1)
                .saturating_add(UnicodeWidthStr::width(hint.label) as u16)
                .saturating_add(2);
            if x.saturating_add(w).saturating_add(right_w) > width {
                break;
            }
            out.push(hint);
            x = x.saturating_add(w);
        }
        out
    }
}

impl Widget for &HintBar<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let spacer_rows = u16::from(self.leading_spacer).min(area.height);
        if spacer_rows > 0 {
            for x in area.left()..area.right() {
                buffer[(x, area.top())]
                    .set_symbol(" ")
                    .set_style(self.system.key_hint_action());
            }
        }
        let y = area.y.saturating_add(spacer_rows);
        if y >= area.bottom() {
            return;
        }
        // junie: hints start at x+1; status wins the right edge.
        let mut right_w = 0u16;
        if let Some(right) = self.right {
            let w = UnicodeWidthStr::width(right) as u16;
            if area.width > w + 2 {
                buffer.set_string(
                    area.right().saturating_sub(w).saturating_sub(1),
                    y,
                    right,
                    self.system.junie_theme().secondary(),
                );
                right_w = w + 3;
            }
        }
        let mut x = area.x.saturating_add(1);
        for hint in self.fitted(area.width) {
            let kw = UnicodeWidthStr::width(hint.chord) as u16;
            let w = kw
                .saturating_add(1)
                .saturating_add(UnicodeWidthStr::width(hint.label) as u16)
                .saturating_add(2);
            if x.saturating_add(w).saturating_add(right_w) > area.right() {
                break;
            }
            buffer.set_string(x, y, hint.chord, self.system.key_hint_key());
            buffer.set_string(
                x.saturating_add(kw).saturating_add(1),
                y,
                hint.label,
                self.system.key_hint_action(),
            );
            x = x.saturating_add(w);
        }
        let _ = self.alignment;
    }
}

/// Paints hint rows through the shared line painter: one alignment path, one
/// contraction rule, wherever hints appear.
fn paint_hint_lines(
    buffer: &mut Buffer,
    area: Rect,
    lines: &[Line<'_>],
    alignment: CellAlignment,
    system: &DesignSystem,
) {
    let mut scratch = String::new();
    let placement = LinePlacement::contracting(system.glyphs.ellipsis()).align(alignment);
    for (index, line) in lines.iter().take(usize::from(area.height)).enumerate() {
        let row = Rect::new(
            area.x,
            area.y
                .saturating_add(u16::try_from(index).unwrap_or(u16::MAX)),
            area.width,
            1,
        );
        paint_line_overflow(
            buffer,
            row,
            line,
            system.key_hint_action(),
            placement,
            &mut scratch,
        );
    }
}

impl Widget for HintBar<'_> {
    #[expect(
        clippy::needless_borrows_for_generic_args,
        reason = "explicitly delegate the owned contract to the borrowed renderer"
    )]
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

/// Render the shared rich hint vocabulary centered in the supplied area.
pub fn render_hint_bar(
    frame: &mut ratatui_core::terminal::Frame<'_>,
    area: Rect,
    spans: &[HintSpan<'_>],
    system: &DesignSystem,
) {
    let line = Line::from(styled_hint_spans(spans, system, |color| color));
    paint_hint_lines(
        frame.buffer_mut(),
        area,
        std::slice::from_ref(&line),
        CellAlignment::Center,
        system,
    );
}

/// Convert rich hint spans into their canonical styled terminal spans.
pub fn styled_hint_spans(
    spans: &[HintSpan<'_>],
    system: &DesignSystem,
    remap: impl Fn(Color) -> Color,
) -> Vec<Span<'static>> {
    let key = remap_style(system.key_hint_key(), &remap);
    let text = remap_style(system.key_hint_action(), &remap);
    let dim = remap_style(system.style(Role::HintDim), &remap);
    let sep = remap_style(system.style(Role::HintSeparator), &remap);
    let mut out = Vec::with_capacity(spans.len());
    for span in spans {
        match span {
            HintSpan::Key(value) => out.push(Span::styled((*value).to_owned(), key)),
            HintSpan::DynKey(value) => out.push(Span::styled(value.clone(), key)),
            HintSpan::Text(value) => out.push(Span::styled(format!(" {value}"), text)),
            HintSpan::Dyn(value) => out.push(Span::styled(format!(" {value}"), dim)),
            HintSpan::Sep => out.push(Span::styled(system.glyphs.meta_join(), sep)),
            HintSpan::GroupSep => out.push(Span::raw(HINT_GROUP_JOIN)),
        }
    }
    out
}

fn remap_style(mut style: Style, remap: &impl Fn(Color) -> Color) -> Style {
    if let Some(color) = style.fg {
        style = style.fg(remap(color));
    }
    if let Some(color) = style.bg {
        style = style.bg(remap(color));
    }
    if let Some(color) = style.underline_color {
        style = style.underline_color(remap(color));
    }
    style
}

/// Wrap semantic hint groups without splitting a key/label pair.
#[must_use]
pub fn wrapped_hint_lines(
    spans: &[HintSpan<'_>],
    width: u16,
    system: &DesignSystem,
) -> Vec<Line<'static>> {
    #[derive(Clone, Copy)]
    enum Separator {
        Group,
        Dot,
    }
    struct Chunk {
        spans: Vec<Span<'static>>,
        width: usize,
        separator: Separator,
    }

    let mut chunks = Vec::new();
    let mut current = Vec::new();
    let mut current_width = 0;
    let mut separator = Separator::Group;
    let flush = |chunks: &mut Vec<Chunk>,
                 current: &mut Vec<Span<'static>>,
                 current_width: &mut usize,
                 separator| {
        if !current.is_empty() {
            chunks.push(Chunk {
                spans: std::mem::take(current),
                width: *current_width,
                separator,
            });
            *current_width = 0;
        }
    };
    for span in spans {
        match span {
            HintSpan::Sep | HintSpan::GroupSep => {
                flush(&mut chunks, &mut current, &mut current_width, separator);
                separator = if matches!(span, HintSpan::Sep) {
                    Separator::Dot
                } else {
                    Separator::Group
                };
            }
            _ => {
                current_width += span.display_cols();
                current.extend(styled_hint_spans(
                    std::slice::from_ref(span),
                    system,
                    |color| color,
                ));
            }
        }
    }
    flush(&mut chunks, &mut current, &mut current_width, separator);

    let mut lines = Vec::new();
    let mut row = Vec::new();
    let mut row_width: usize = 0;
    for chunk in chunks {
        let separator_width = usize::from(!row.is_empty()) * HINT_SEPARATOR_COLS;
        if !row.is_empty()
            && row_width
                .saturating_add(separator_width)
                .saturating_add(chunk.width)
                > usize::from(width)
        {
            lines.push(Line::from(std::mem::take(&mut row)));
            row_width = 0;
        }
        if !row.is_empty() {
            match chunk.separator {
                Separator::Dot => {
                    row.extend(styled_hint_spans(&[HintSpan::Sep], system, |color| color));
                }
                Separator::Group => row.push(Span::raw(HINT_GROUP_JOIN)),
            }
            row_width += HINT_SEPARATOR_COLS;
        }
        row.extend(chunk.spans);
        row_width += chunk.width;
    }
    if !row.is_empty() {
        lines.push(Line::from(row));
    }
    if lines.is_empty() {
        lines.push(Line::raw(""));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_wrapping_keeps_key_and_label_together() {
        let spans = [
            HintSpan::Key("Enter"),
            HintSpan::Text("select"),
            HintSpan::GroupSep,
            HintSpan::Key("Esc"),
            HintSpan::Text("cancel"),
        ];
        let lines = wrapped_hint_lines(&spans, 15, &crate::style::DesignSystem::default());
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].to_string(), "Enter select");
        assert_eq!(lines[1].to_string(), "Esc cancel");
    }

    #[test]
    fn hint_row_width_covers_empty_and_mixed_spans() {
        assert_eq!(hint_row_cols(&[]), 0);
        assert_eq!(
            hint_row_cols(&[HintSpan::Key("↵"), HintSpan::Text("go"), HintSpan::Sep]),
            7
        );
    }

    #[test]
    fn measured_height_is_one_row() {
        let system = crate::style::DesignSystem::default();
        let hints = [
            Hint {
                chord: "Enter",
                label: "select",
                priority: 1,
                visible: true,
            },
            Hint {
                chord: "Esc",
                label: "cancel",
                priority: 2,
                visible: true,
            },
            Hint {
                chord: "?",
                label: "help",
                priority: 3,
                visible: true,
            },
        ];
        let bar = HintBar::new(&hints, &system);
        assert_eq!(bar.measured_height(20), 1);
        assert_eq!(bar.measured_height(80), 1);
    }

    #[test]
    fn leading_spacer_is_measured_and_painted() {
        let system = crate::style::DesignSystem::default();
        let hints = [Hint {
            chord: "Esc",
            label: "close",
            priority: 1,
            visible: true,
        }];
        let bar = HintBar::new(&hints, &system).leading_spacer(true);
        assert_eq!(bar.measured_height(40), 2);

        let area = Rect::new(0, 0, 40, 2);
        let mut buffer = Buffer::filled(area, ratatui_core::buffer::Cell::new("x"));
        (&bar).render(area, &mut buffer);
        assert!((area.left()..area.right()).all(|x| buffer[(x, area.top())].symbol() == " "));
        assert_eq!(buffer[(1, 1)].symbol(), "E");
    }

    #[test]
    fn drops_from_the_right_keeps_most_important() {
        let system = crate::style::DesignSystem::default();
        let hints = [
            Hint {
                chord: "Esc",
                label: "Cancel",
                priority: 1,
                visible: true,
            },
            Hint {
                chord: "Enter",
                label: "Confirm",
                priority: 2,
                visible: true,
            },
            Hint {
                chord: "?",
                label: "Help",
                priority: 3,
                visible: true,
            },
        ];
        let bar = HintBar::new(&hints, &system);
        let area = Rect::new(0, 0, 18, 1);
        let mut buffer = Buffer::empty(area);
        (&bar).render(area, &mut buffer);
        let row: String = (0..area.width).map(|x| buffer[(x, 0)].symbol()).collect();
        assert!(row.contains("Esc"), "{row}");
        assert!(row.contains("Cancel"), "{row}");
        assert!(!row.contains("Help"), "{row}");
    }

    #[test]
    fn key_is_bold_primary_action_is_muted() {
        let system = crate::style::DesignSystem::default();
        let hints = [Hint {
            chord: "Esc",
            label: "Cancel",
            priority: 1,
            visible: true,
        }];
        let bar = HintBar::new(&hints, &system);
        let area = Rect::new(0, 0, 20, 1);
        let mut buffer = Buffer::empty(area);
        (&bar).render(area, &mut buffer);
        assert_eq!(buffer[(1, 0)].symbol(), "E");
        assert_eq!(buffer[(1, 0)].fg, system.key_hint_key().fg.unwrap());
        assert!(
            buffer[(1, 0)]
                .modifier
                .contains(ratatui_core::style::Modifier::BOLD)
        );
        let action_x = 1 + 4; // "Esc" + space
        assert_eq!(buffer[(action_x, 0)].symbol(), "C");
        assert_eq!(
            buffer[(action_x, 0)].fg,
            system.key_hint_action().fg.unwrap()
        );
    }

    #[test]
    fn right_status_wins_the_edge() {
        let system = crate::style::DesignSystem::default();
        let hints = [Hint {
            chord: "Esc",
            label: "Cancel",
            priority: 1,
            visible: true,
        }];
        let bar = HintBar::new(&hints, &system).right("Cell saved");
        let area = Rect::new(0, 0, 40, 1);
        let mut buffer = Buffer::empty(area);
        (&bar).render(area, &mut buffer);
        let row: String = (0..area.width).map(|x| buffer[(x, 0)].symbol()).collect();
        assert!(row.contains("Cell saved"), "{row}");
        assert!(row.trim_end().ends_with("Cell saved"), "{row}");
        let start = row.find("Cell saved").unwrap() as u16;
        assert_eq!(
            buffer[(start, 0)].fg,
            system.junie_theme().secondary().fg.unwrap()
        );
    }
}
