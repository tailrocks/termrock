//! **ChromeRow** — the one-line strips a pane grows when it is doing something.
//!
//! Filtering, renaming, confirming: each is a single row that appears above or
//! below a pane's body while a mode is active. Written by hand they drift —
//! one paints its query in the accent, the next in a warning, a third on a
//! recessed ground with a live caret — and the reader has to learn each pane's
//! private vocabulary.
//!
//! One row type instead: a glyph carries what mode this is, the body reads as
//! text, and a query row sits in a well with the caret as the only live cell
//! (plans/007's rule, generalised).
use ratatui_core::{buffer::Buffer, layout::Rect, style::Style};

use crate::style::{DesignSystem, Role};
use crate::text::{display_cols, take_display_cols};

/// What kind of inline chrome the row is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ChromeRowKind {
    /// A query the operator is typing: recessed ground, live caret.
    #[default]
    Query,
    /// A mode the pane is in (rename, move, select): stated, not shouted.
    Mode,
    /// Something the operator should read before continuing.
    Notice,
}

impl ChromeRowKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Query => "query",
            Self::Mode => "mode",
            Self::Notice => "notice",
        }
    }
}

/// One line of inline pane chrome.
#[derive(Debug, Clone, Copy)]
pub struct ChromeRow<'a> {
    system: &'a DesignSystem,
    kind: ChromeRowKind,
    prefix: &'a str,
    body: &'a str,
    tone: Option<Role>,
    caret: bool,
}

impl<'a> ChromeRow<'a> {
    /// A query row: `/{query}` on a recessed ground with a live caret.
    #[must_use]
    pub const fn query(query: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            system,
            kind: ChromeRowKind::Query,
            prefix: "/",
            body: query,
            tone: None,
            caret: true,
        }
    }

    /// A mode row: `{prefix} {body}`, quiet, with an optional caret.
    #[must_use]
    pub const fn mode(prefix: &'a str, body: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            system,
            kind: ChromeRowKind::Mode,
            prefix,
            body,
            tone: None,
            caret: false,
        }
    }

    /// A notice row: a glyph plus a sentence.
    #[must_use]
    pub const fn notice(glyph: &'a str, body: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            system,
            kind: ChromeRowKind::Notice,
            prefix: glyph,
            body,
            tone: None,
            caret: false,
        }
    }

    /// Overrides the glyph's tone (a rename is not a failure).
    #[must_use]
    pub const fn tone(mut self, role: Role) -> Self {
        self.tone = Some(role);
        self
    }

    /// Whether the row ends in a live caret (an editable row does).
    #[must_use]
    pub const fn caret(mut self, caret: bool) -> Self {
        self.caret = caret;
        self
    }

    /// The composed text, without the caret.
    #[must_use]
    pub fn text(&self) -> String {
        match self.kind {
            ChromeRowKind::Query => format!("{}{}", self.prefix, self.body),
            _ => format!("{} {}", self.prefix, self.body),
        }
    }

    /// Paints the row into one line of `area`.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let system = self.system;
        let cols = usize::from(area.width);
        let mut line = self.text();
        if self.caret {
            line.push('_');
        }

        // A query is a well: the ground says "type here" so the text does not
        // have to shout it.
        let mut body: Style = system.style(Role::Text);
        if matches!(self.kind, ChromeRowKind::Query)
            && let Some(bg) = system.style(Role::Sunken).bg
        {
            body = body.bg(bg);
        }
        buffer.set_stringn(area.x, area.y, take_display_cols(&line, cols), cols, body);

        // The prefix carries the mode; the words stay in the body tone.
        let prefix_cols = display_cols(self.prefix).min(cols);
        let prefix_tone = system.style(self.tone.unwrap_or(match self.kind {
            ChromeRowKind::Query => Role::Accent,
            ChromeRowKind::Mode => Role::TextMuted,
            ChromeRowKind::Notice => Role::Warning,
        }));
        for dx in 0..u16::try_from(prefix_cols).unwrap_or(0) {
            let x = area.x.saturating_add(dx);
            if x >= area.right() {
                break;
            }
            let cell = &mut buffer[(x, area.y)];
            let ground = cell.style().bg;
            let mut style = prefix_tone;
            if style.bg.is_none()
                && let Some(bg) = ground
            {
                style = style.bg(bg);
            }
            cell.set_style(style);
        }

        if self.caret {
            let caret_col = display_cols(&line).min(cols).saturating_sub(1);
            let x = area
                .x
                .saturating_add(u16::try_from(caret_col).unwrap_or(0))
                .min(area.right().saturating_sub(1));
            let cell = &mut buffer[(x, area.y)];
            // The caret over an existing symbol is the explicit reversal
            // pair, never a modifier that re-swaps the cell's own colours.
            let style = system.reversed();
            cell.set_style(style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_query_row_is_a_well_with_one_live_cell() {
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 20, 1);
        let mut buffer = Buffer::empty(area);
        ChromeRow::query("main", &system).paint(area, &mut buffer);

        let row: String = (0..area.width).map(|x| buffer[(x, 0)].symbol()).collect();
        assert!(row.starts_with("/main_"), "{row:?}");
        let sunken = system.style(Role::Sunken).bg;
        assert_eq!(
            buffer[(2, 0)].style().bg,
            sunken,
            "the query sits in a well"
        );
        let accent = system.style(Role::Accent).fg;
        let live = (0..area.width)
            .filter(|x| {
                let cell = &buffer[(*x, 0)];
                !cell.symbol().trim().is_empty() && Some(cell.fg) == accent
            })
            .count();
        assert_eq!(live, 1, "the slash, and nothing else in the accent");
    }

    #[test]
    fn a_mode_row_states_its_mode_without_shouting() {
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 24, 1);
        let mut buffer = Buffer::empty(area);
        ChromeRow::mode("rename>", "notes.md", &system)
            .caret(true)
            .paint(area, &mut buffer);
        let row: String = (0..area.width).map(|x| buffer[(x, 0)].symbol()).collect();
        assert!(row.starts_with("rename> notes.md_"), "{row:?}");
        assert_eq!(
            buffer[(0, 0)].style().fg,
            system.style(Role::TextMuted).fg,
            "a mode is metadata, not an alarm"
        );
        assert_eq!(
            buffer[(8, 0)].style().fg,
            system.style(Role::Text).fg,
            "the name being typed reads as text"
        );
    }

    #[test]
    fn a_notice_row_keeps_its_severity_on_the_glyph() {
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 30, 1);
        let mut buffer = Buffer::empty(area);
        ChromeRow::notice("!", "3 files are read-only", &system).paint(area, &mut buffer);
        let warning = system.style(Role::Warning).fg;
        let warned = (0..area.width)
            .filter(|x| {
                let cell = &buffer[(*x, 0)];
                !cell.symbol().trim().is_empty() && Some(cell.fg) == warning
            })
            .count();
        assert_eq!(warned, 1, "severity belongs to the glyph");
    }

    #[test]
    fn unicode_body_and_prefix_use_display_columns() {
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 8, 1);
        let mut buffer = Buffer::empty(area);

        ChromeRow::mode("界", "🙂e\u{301}", &system)
            .caret(true)
            .paint(area, &mut buffer);

        let line = ChromeRow::mode("界", "🙂e\u{301}", &system)
            .caret(true)
            .text()
            + "_";
        let caret_x = u16::try_from(display_cols(&line).saturating_sub(1)).unwrap();
        assert_eq!(buffer[(caret_x, 0)].symbol(), "_");
        // The caret over an existing symbol is the explicit reversal pair
        // (canvas on body text), not a stacked modifier.
        let theme = system.junie_theme();
        assert_eq!(buffer[(caret_x, 0)].fg, theme.canvas);
        assert_eq!(buffer[(caret_x, 0)].bg, theme.text_primary);
        assert!(
            !buffer[(caret_x, 0)]
                .modifier
                .contains(ratatui_core::style::Modifier::REVERSED)
        );
    }
}
