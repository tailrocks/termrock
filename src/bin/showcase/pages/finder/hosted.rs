//! The Finder's Hosted painter: every pane of the shared `SearchDetail` drawn
//! by the page through `SearchDetail::render_hosted` and `SearchPaint`, the
//! way an app with its own look composes the widget.
//!
//! Ownership: `SearchDetail` keeps layout, scroll, selection, focus and hit
//! regions; this module only draws inside the rectangles it is handed and
//! reports the cells it placed. Ranking, rows and actions stay in
//! `finder.rs`. The look is deliberately plainer than the standard painter, so
//! the two are easy to tell apart on the page.

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Modifier;

use super::{ACTIONS, Results, SCOPE_LINK, SCOPES, risk_tone};
use termrock::theme::{CursorStyle, Tone};
use termrock::ui::ctx::{RenderCtx, VisualState, fill};
use termrock::ui::text::{fit, truncate_middle, width};
use termrock::widgets::chips::overflow_run;
use termrock::widgets::empty::{self, EmptyState};
use termrock::widgets::list_detail::DetailPane;
use termrock::widgets::page_header::Heading;
use termrock::widgets::panel::Panel;
use termrock::widgets::scrollbar;
use termrock::widgets::search_detail::{QueryView, ResultRowKind, RowPaint, SearchPaint};

pub(super) struct Hosted {
    pub scope: usize,
    /// The selected action, for the preview and the summary line.
    pub selected: Option<usize>,
    row_w: u16,
}

impl Hosted {
    pub(super) fn new(scope: usize, selected: Option<usize>) -> Self {
        Self {
            scope,
            selected,
            row_w: 0,
        }
    }
}

impl SearchPaint<Results> for Hosted {
    fn query(&mut self, area: Rect, query: QueryView<'_>, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let focused = query.focused;
        let state = VisualState {
            focused,
            editing: focused,
            ..Default::default()
        };
        let field = t.field_style(state);
        fill(buf, area, field);
        let bg = field.bg.unwrap_or(t.field);
        let gutter = VisualState {
            focused,
            ..Default::default()
        };
        buf.set_string(
            area.x,
            area.y,
            t.gutter_symbol(gutter),
            t.gutter(gutter, bg, false),
        );

        let readout = format!("scope ‹ {} ›", SCOPES[self.scope].to_lowercase());
        let readout_w = width(&readout) as u16 + 2;
        let readout_x = area.right().saturating_sub(readout_w);
        let mut link = ctx.title_style(SCOPE_LINK, bg);
        let lifted = ctx.interaction.hovered(SCOPE_LINK) || ctx.interaction.pressed(SCOPE_LINK);
        if self.scope == 0 && !lifted {
            link = link.fg(t.legible(t.text_muted, link.bg.unwrap_or(bg)));
        }
        buf.set_string(readout_x, area.y, format!(" {readout} "), link);
        ctx.clickable(SCOPE_LINK, Rect::new(readout_x, area.y, readout_w, 1));

        let text_w = readout_x.saturating_sub(area.x + 3) as usize;
        if query.text.is_empty() {
            buf.set_string(
                area.x + 2,
                area.y,
                fit(query.placeholder, text_w),
                field.fg(t.text_muted),
            );
        } else {
            let shown = if width(query.text) > text_w {
                truncate_middle(query.text, text_w)
            } else {
                query.text.to_owned()
            };
            let everything = query.selection == Some(0..query.text.len());
            let style = if everything {
                field.fg(t.canvas).bg(t.accent)
            } else if focused {
                field
                    .add_modifier(Modifier::UNDERLINED)
                    .underline_color(t.text_secondary)
            } else {
                field
            };
            buf.set_string(area.x + 2, area.y, shown, style);
        }
        if focused {
            let caret = area.x + 2 + width(query.text).min(text_w) as u16;
            ctx.set_cursor(Position::new(caret, area.y));
        }
    }

    fn rows_begin(&mut self, _area: Rect, row_w: u16) {
        self.row_w = row_w;
    }

    fn row(&mut self, mut row: RowPaint<'_, Results>, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let bg = t.canvas;
        let rect = row.rect;
        let y = rect.y;
        let data = &row.source.rows[row.row];
        match row.kind {
            ResultRowKind::Heading => Heading::new(data.label).render(
                Rect::new(rect.x + 2, y, rect.width.saturating_sub(2), 1),
                buf,
                t,
                bg,
            ),
            ResultRowKind::Spacer => {}
            ResultRowKind::Item => {
                let mut state = ctx.state(row.hit);
                state.focused = row.focused && row.selected;
                state.selected = row.selected;
                let cursor = t.cursor_row(state, bg, CursorStyle::Fill);
                fill(buf, rect, cursor.style);
                buf.set_string(rect.x, y, cursor.gutter, cursor.gutter_style);
                let ink = if row.stale {
                    t.text_muted
                } else {
                    t.text_primary
                };
                let label_w = 22.min(self.row_w.saturating_sub(12));
                buf.set_string(
                    rect.x + 2,
                    y,
                    fit(data.label, usize::from(label_w)),
                    cursor.style.fg(t.row_ink(&cursor, ink)),
                );
                let note_x = rect.x + 3 + label_w;
                let meta_w = width(data.meta) as u16;
                let note_w = rect
                    .right()
                    .saturating_sub(note_x + meta_w + 2)
                    .min(rect.width);
                buf.set_string(
                    note_x,
                    y,
                    fit(&data.detail, usize::from(note_w)),
                    cursor.style.fg(t.row_ink(&cursor, t.text_muted)),
                );
                if meta_w > 0 {
                    buf.set_string(
                        rect.right().saturating_sub(meta_w + 1),
                        y,
                        data.meta,
                        cursor.style.fg(t.row_ink(&cursor, t.tone(data.meta_tone))),
                    );
                }
            }
            ResultRowKind::Cells => {
                let mut state = ctx.state(row.hit);
                state.focused = row.focused && row.selected;
                let cursor = t.cursor_row(state, bg, CursorStyle::Fill);
                fill(buf, rect, cursor.style);
                buf.set_string(rect.x, y, cursor.gutter, cursor.gutter_style);
                buf.set_string(
                    rect.x + 2,
                    y,
                    fit(data.label, 7),
                    cursor.style.fg(t.row_ink(&cursor, t.text_muted)),
                );
                let mut x = rect.x + 10;
                let widths: Vec<u16> = data
                    .actions
                    .iter()
                    .map(|&a| width(ACTIONS[a].name) as u16 + 2)
                    .collect();
                let (shown, more) = overflow_run(&widths, rect.right().saturating_sub(x), 1);
                for (cell, &action) in data.actions.iter().enumerate().take(shown) {
                    let hit = row.cell_hit(cell);
                    let mut style = ctx
                        .title_style(hit, cursor.style.bg.unwrap_or(bg))
                        .fg(t.text_primary);
                    if row.selected_cell == Some(cell) {
                        style = style.add_modifier(Modifier::BOLD);
                        buf.set_string(
                            x,
                            y,
                            "›",
                            cursor.style.fg(t.row_ink(&cursor, t.text_secondary)),
                        );
                    }
                    buf.set_string(x + 1, y, ACTIONS[action].name, style);
                    row.place_cell(cell, Rect::new(x, y, widths[cell], 1));
                    x += widths[cell] + 1;
                }
                if let Some(marker) = more {
                    buf.set_string(x, y, marker, cursor.style.fg(t.text_muted));
                }
            }
        }
    }

    fn empty(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        empty::render(
            area,
            buf,
            ctx.theme,
            &EmptyState::new("No matches").hint("Widen the scope with Alt+Up"),
            ctx.theme.canvas,
        );
    }

    fn detail(&mut self, pane: DetailPane<'_>, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let (area, id) = (pane.area, pane.id);
        let focused = ctx.interaction.focused(id);
        let Some(action) = self.selected.map(|i| &ACTIONS[i]) else {
            let card = Rect::new(area.x, area.y, area.width, 5.min(area.height));
            let panel = Panel::card(Some("Preview")).focused(focused);
            let inner = panel.render(card, buf, t);
            empty::render(
                inner,
                buf,
                t,
                &EmptyState::new("Nothing selected"),
                panel.bg(t),
            );
            ctx.control(id, card, false);
            return;
        };
        let meta = format!("{} · {}", action.group, SCOPES[action.scope].to_lowercase());
        let panel = Panel::card(Some(action.name)).focused(focused).meta(&meta);
        let rows: [(&str, &str, Tone); 7] = [
            ("Runs", action.runs, Tone::Normal),
            ("In", "~/work/holla", Tone::Normal),
            ("Host", "mbp · local", Tone::Normal),
            ("State", action.note, Tone::Normal),
            ("Risk", action.risk, risk_tone(action.risk)),
            (
                "Gate",
                if action.risk == "destructive" {
                    "asks first"
                } else {
                    "runs directly"
                },
                Tone::Normal,
            ),
            ("Data", "live · 2 s ago", Tone::Muted),
        ];
        let card = Rect::new(
            area.x,
            area.y,
            area.width,
            (rows.len() as u16 + 3).min(area.height),
        );
        let inner = panel.render(card, buf, t);
        ctx.control(id, card, false);
        ctx.scrollable(id, inner);
        if inner.is_empty() {
            return;
        }
        pane.scroll.set_content(rows.len());
        pane.scroll.set_viewport(usize::from(inner.height));
        let bg = panel.bg(t);
        let value_x = inner.x + 7;
        let value_w = inner.width.saturating_sub(7 + 1);
        for (k, i) in pane.scroll.visible_range().enumerate() {
            let y = inner.y + k as u16;
            let (label, value, tone) = rows[i];
            buf.set_string(inner.x, y, label, t.muted().bg(bg));
            buf.set_string(
                value_x,
                y,
                fit(value, usize::from(value_w)),
                ratatui::style::Style::new().fg(t.tone(tone)).bg(bg),
            );
        }
        if pane.scroll.overflows() {
            scrollbar::render_with_fade(inner, buf, ctx, id, pane.scroll, focused);
        }
    }

    fn summary(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let Some(action) = self.selected.map(|i| &ACTIONS[i]) else {
            return;
        };
        let line = format!(
            "→ {} · {} · {}",
            action.runs,
            SCOPES[action.scope].to_lowercase(),
            action.risk
        );
        buf.set_string(
            area.x + 2,
            area.y,
            fit(&line, usize::from(area.width.saturating_sub(2))),
            ctx.theme.muted(),
        );
    }

    fn is_lead(&self, source: &Results, row: usize) -> bool {
        source.rows[row].kind == ResultRowKind::Heading
    }
}
