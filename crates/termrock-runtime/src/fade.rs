use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};

use crate::core::scroll::ScrollState;
use crate::ui::ctx::RenderCtx;

pub const OUTER_KEEP: f32 = 0.55;

pub const INNER_KEEP: f32 = 0.8;

pub const DEEP_FROM: u16 = 12;

pub const MIN_ROWS: u16 = 4;

pub fn scroll_edges(buf: &mut Buffer, ctx: &RenderCtx, area: Rect, scroll: &ScrollState) {
    scroll_edges_except(buf, ctx, area, scroll, &[]);
}

pub fn scroll_edges_except(
    buf: &mut Buffer,
    ctx: &RenderCtx,
    area: Rect,
    scroll: &ScrollState,
    keep: &[u16],
) {
    let area = area.intersection(*buf.area());
    if area.is_empty() || area.height < MIN_ROWS {
        return;
    }
    let up = scroll.offset > 0;
    let down = scroll.viewport_len > 0 && scroll.offset + scroll.viewport_len < scroll.content_len;
    if !up && !down {
        return;
    }
    let depth: u16 = if area.height >= DEEP_FROM { 2 } else { 1 };
    let container = majority_bg(buf, area);
    let cursor_row = ctx.cursor.filter(|p| area.contains(*p)).map(|p| p.y);
    let mut rows: Vec<(u16, f32)> = vec![];
    if up {
        rows.push((area.y, OUTER_KEEP));
        if depth == 2 {
            rows.push((area.y + 1, INNER_KEEP));
        }
    }
    if down {
        rows.push((area.bottom() - 1, OUTER_KEEP));
        if depth == 2 {
            rows.push((area.bottom() - 2, INNER_KEEP));
        }
    }
    for (y, strength) in rows {
        if Some(y) == cursor_row || keep.contains(&y) {
            continue;
        }
        fade_row(buf, area, y, strength, container);
    }
}

fn majority_bg(buf: &Buffer, area: Rect) -> Color {
    let mut counts: Vec<(Color, usize)> = vec![];
    for pos in area.positions() {
        let bg = buf[pos].bg;
        match counts.iter_mut().find(|(c, _)| *c == bg) {
            Some((_, n)) => *n += 1,
            None => counts.push((bg, 1)),
        }
    }
    counts
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .map(|(c, _)| c)
        .unwrap_or(Color::Reset)
}

fn fade_row(buf: &mut Buffer, area: Rect, y: u16, keep: f32, container: Color) {
    let outer = keep <= OUTER_KEEP;
    for x in area.x..area.right() {
        let Some(cell) = buf.cell_mut((x, y)) else {
            continue;
        };
        if cell.bg != container || cell.modifier.contains(Modifier::REVERSED) {
            continue;
        }
        match (cell.fg, container) {
            (Color::Rgb(fr, fg, fb), Color::Rgb(br, bgg, bb)) => {
                cell.fg = Color::Rgb(mix(fr, br, keep), mix(fg, bgg, keep), mix(fb, bb, keep));
            }

            _ if outer => {
                cell.modifier |= Modifier::DIM;
            }
            _ => {}
        }
    }
}

fn mix(fg: u8, bg: u8, keep: f32) -> u8 {
    (bg as f32 + (fg as f32 - bg as f32) * keep)
        .round()
        .clamp(0.0, 255.0) as u8
}
