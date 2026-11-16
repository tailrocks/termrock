//! The List & detail page's host-owned painters: the `rows` painter draws only
//! the master rows (`ListDetail::render_source_selected_with`), the `hosted`
//! painter draws every pane (`ListDetail::render_hosted` and `PaneHost`).
//!
//! Ownership: `ListDetail` keeps layout, master scroll, row hit regions, the
//! detail scroll and the drawer; this module only draws inside the rectangles
//! it is handed. The selection lives in the page, by job key.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use super::{Job, JobRows, LIST};
use junie_tui::core::id::{Part, PartRef};
use junie_tui::theme::{CursorStyle, Tone};
use junie_tui::ui::ctx::{RenderCtx, fill};
use junie_tui::ui::text::{fit, width, wrap};
use junie_tui::widgets::empty::{self, EmptyState};
use junie_tui::widgets::list_detail::{DetailPane, ListDetailSource, MasterRow, PaneHost};
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::scrollbar;

/// One master row in the plain look: the job, then its state in brackets.
pub(super) fn paint_row(rows: &JobRows, row: MasterRow, buf: &mut Buffer, ctx: &mut RenderCtx) {
    let t = ctx.theme;
    let data = rows.row(row.index);
    let rect = row.rect;
    let hit = LIST.part(PartRef::item(Part::ROW, row.key));
    let mut state = ctx.state(hit);
    state.focused = row.focused && row.selected;
    state.selected = row.selected;
    let cursor = t.cursor_row(state, t.canvas, CursorStyle::Fill);
    fill(buf, rect, cursor.style);
    buf.set_string(rect.x, rect.y, cursor.gutter, cursor.gutter_style);
    let tag = format!("[{}]", data.meta);
    let tag_w = width(&tag) as u16;
    let ink = if data.faint {
        t.text_muted
    } else {
        t.text_primary
    };
    let label_w = rect.width.saturating_sub(tag_w + 4) as usize;
    buf.set_string(
        rect.x + 2,
        rect.y,
        fit(data.label, label_w),
        cursor.style.fg(t.row_ink(&cursor, ink)),
    );
    if rect.width > tag_w + 2 {
        buf.set_string(
            rect.right().saturating_sub(tag_w + 1),
            rect.y,
            tag,
            cursor.style.fg(t.row_ink(&cursor, t.tone(data.meta_tone))),
        );
    }
}

/// The detail card's lines: label, text and tone, with long text wrapped so
/// the card has something to scroll.
fn card_lines(job: &Job, text_w: usize) -> Vec<(&'static str, String, Tone)> {
    let mut out = Vec::new();
    let mut push = |label: &'static str, text: &str, tone: Tone| {
        for (i, line) in wrap(text, text_w.max(8)).into_iter().enumerate() {
            out.push((if i == 0 { label } else { "" }, line, tone));
        }
    };
    push("State", job.word, job.tone);
    push("About", job.summary, Tone::Normal);
    if let Some(problem) = job.problem {
        push("Issue", problem, Tone::Warning);
    }
    if job.bindings.is_empty() {
        push("Binds", "none", Tone::Muted);
    }
    for (kind, name, how) in job.bindings {
        push("Binds", &format!("{kind} {name} · {how}"), Tone::Normal);
    }
    out
}

/// Paints every pane of the jobs list-detail.
pub(super) struct JobsHost<'a> {
    pub rows: &'a JobRows,
    /// The selected job, for the card and the summary line.
    pub selected: Option<&'static Job>,
}

impl<'a> JobsHost<'a> {
    pub(super) fn new(rows: &'a JobRows, selected: Option<&'static Job>) -> Self {
        Self { rows, selected }
    }
}

impl PaneHost for JobsHost<'_> {
    fn row(&mut self, row: MasterRow, buf: &mut Buffer, ctx: &mut RenderCtx) {
        paint_row(self.rows, row, buf, ctx);
    }

    fn empty(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        empty::render(
            area,
            buf,
            ctx.theme,
            &EmptyState::new("No jobs yet").hint("Jobs you add are listed here."),
            ctx.theme.canvas,
        );
    }

    fn detail(&mut self, pane: DetailPane<'_>, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let (area, id) = (pane.area, pane.id);
        let focused = ctx.interaction.focused(id);
        let Some(job) = self.selected else {
            return;
        };
        let title = if pane.drawer {
            format!("{} · drawer", job.name)
        } else {
            job.name.to_owned()
        };
        let panel = Panel::card(Some(&title)).focused(focused).meta(job.word);
        let inner = panel.render(area, buf, t);
        ctx.control(id, area, false);
        ctx.scrollable(id, inner);
        if inner.is_empty() {
            return;
        }
        let text_w = usize::from(inner.width.saturating_sub(8 + 1));
        let lines = card_lines(job, text_w);
        pane.scroll.set_content(lines.len());
        pane.scroll.set_viewport(usize::from(inner.height));
        let bg = panel.bg(t);
        for (k, i) in pane.scroll.visible_range().enumerate() {
            let y = inner.y + k as u16;
            let (label, text, tone) = &lines[i];
            buf.set_string(inner.x, y, label, t.muted().bg(bg));
            buf.set_string(
                inner.x + 8,
                y,
                fit(text, text_w),
                Style::new().fg(t.tone(*tone)).bg(bg),
            );
        }
        if pane.scroll.overflows() {
            scrollbar::render_with_fade(inner, buf, ctx, id, pane.scroll, focused);
        }
    }

    fn summary(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let Some(job) = self.selected else {
            return;
        };
        let line = format!("→ {} · {}", job.name, job.word);
        buf.set_string(
            area.x + 1,
            area.y,
            fit(&line, usize::from(area.width.saturating_sub(2))),
            ctx.theme.faint().bg(ctx.theme.canvas),
        );
    }
}
