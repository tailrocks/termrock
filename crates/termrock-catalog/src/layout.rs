// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/showcase/pages/mod.rs layout + widgets/panel.rs (MIT).

//! Shared page layout grammar (source captions, rows, columns, cards).

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use termrock::style::JunieTheme;

use crate::draw::fill;
use crate::text;

/// Small muted label above a control.
pub fn caption(x: u16, y: u16, buf: &mut Buffer, t: &JunieTheme, text: &str, bg: Color) {
    buf.set_string(x, y, text, t.muted().bg(bg));
}

/// Split vertically into rows with fixed heights; the last takes the rest.
#[must_use]
pub fn rows(area: Rect, heights: &[u16]) -> Vec<Rect> {
    let mut y = area.y;
    let mut out = Vec::new();
    for (i, &h) in heights.iter().enumerate() {
        let last = i == heights.len() - 1;
        let h = if last {
            area.bottom().saturating_sub(y)
        } else {
            h.min(area.bottom().saturating_sub(y))
        };
        out.push(Rect::new(area.x, y, area.width, h));
        y = y.saturating_add(h);
    }
    out
}

/// Pack controls left-to-right (source `button::row_layout`).
#[must_use]
pub fn row_layout(area: Rect, widths: &[u16], gap: u16) -> Vec<Rect> {
    let mut x = area.x;
    let mut out = Vec::new();
    for &w in widths {
        let w = w.min(area.right().saturating_sub(x));
        out.push(Rect::new(x, area.y, w, area.height.min(1)));
        x = x.saturating_add(w).saturating_add(gap);
    }
    out
}

/// Right-aligned row (source dialog action bars).
#[must_use]
pub fn row_layout_right(area: Rect, widths: &[u16], gap: u16) -> Vec<Rect> {
    let total: u16 = widths.iter().sum::<u16>() + gap * widths.len().saturating_sub(1) as u16;
    let x = area.right().saturating_sub(total).max(area.x);
    row_layout(
        Rect::new(x, area.y, area.right().saturating_sub(x), area.height),
        widths,
        gap,
    )
}

/// Source `scrollbar::position_label`: empty until a previous paint set a
/// non-zero viewport (`ScrollState::overflows` requires `viewport_len > 0`).
#[must_use]
pub fn overflow_label(offset: usize, viewport: usize, total: usize) -> String {
    if viewport == 0 || total <= viewport {
        return String::new();
    }
    let start = offset.saturating_add(1);
    let end = offset.saturating_add(viewport).min(total);
    format!("{start}–{end} of {total}")
}

/// Two columns with a gap; if too narrow, stack vertically.
#[must_use]
pub fn columns(area: Rect, left_w: u16, gap: u16) -> (Rect, Rect) {
    if area.width < left_w + gap + 20 {
        let h = area.height / 2;
        return (
            Rect::new(area.x, area.y, area.width, h),
            Rect::new(area.x, area.y + h, area.width, area.height - h),
        );
    }
    (
        Rect::new(area.x, area.y, left_w, area.height),
        Rect::new(
            area.x + left_w + gap,
            area.y,
            area.width - left_w - gap,
            area.height,
        ),
    )
}

/// Source card chrome: filled surface, no border, title row, inset 2×1.
///
/// This is page grammar from the source showcase, painted through
/// [`JunieTheme`] (public TermRock tokens). Not a second Panel widget.
pub fn card(
    area: Rect,
    buf: &mut Buffer,
    t: &JunieTheme,
    title: Option<&str>,
    meta: Option<&str>,
    focused: bool,
) -> (Rect, Color) {
    let area = area.intersection(*buf.area());
    let bg = t.surface;
    if area.is_empty() {
        return (area, bg);
    }
    fill(buf, area, Style::new().bg(bg));
    let inner = Rect::new(
        area.x.saturating_add(2),
        area.y.saturating_add(1),
        area.width.saturating_sub(4),
        area.height.saturating_sub(2),
    );
    if focused && title.is_some() {
        buf.set_string(area.x + 1, area.y, "▎", Style::new().fg(t.focus).bg(bg));
    }
    title_row(
        area.x + 2,
        area.y,
        area.width.saturating_sub(4),
        buf,
        t,
        bg,
        title,
        meta,
        focused,
    );
    let body = if title.is_some() {
        Rect::new(
            inner.x,
            inner.y.saturating_add(1),
            inner.width,
            inner.height.saturating_sub(1),
        )
    } else {
        inner
    };
    (body, bg)
}

/// Source framed pane: rounded border on canvas.
pub fn framed(
    area: Rect,
    buf: &mut Buffer,
    t: &JunieTheme,
    title: Option<&str>,
    focused: bool,
) -> (Rect, Color) {
    use ratatui::widgets::{Block, BorderType, Borders, Widget};
    let area = area.intersection(*buf.area());
    let bg = t.canvas;
    if area.is_empty() {
        return (area, bg);
    }
    fill(buf, area, Style::new().bg(bg));
    let block = Block::new()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(t.border(focused).bg(bg));
    block.render(area, buf);
    if area.width > 4 {
        let title_s = title.map(|s| format!(" {s} "));
        title_row(
            area.x + 2,
            area.y,
            area.width.saturating_sub(4),
            buf,
            t,
            bg,
            title_s.as_deref(),
            None,
            focused,
        );
        // junie `.meta("")` still paints `"  "` faint before `─╮`.
        buf.set_stringn(
            area.right().saturating_sub(4),
            area.y,
            "  ",
            2,
            t.faint().bg(bg),
        );
    }
    let inner = Rect::new(
        area.x.saturating_add(3),
        area.y.saturating_add(1),
        area.width.saturating_sub(5),
        area.height.saturating_sub(2),
    );
    (inner, bg)
}

fn title_row(
    x: u16,
    y: u16,
    w: u16,
    buf: &mut Buffer,
    t: &JunieTheme,
    bg: Color,
    title: Option<&str>,
    meta: Option<&str>,
    focused: bool,
) {
    if w == 0 {
        return;
    }
    let mut cx = x;
    if let Some(title) = title {
        let style = if focused {
            t.title().bg(bg)
        } else {
            t.secondary().bg(bg)
        };
        let title = text::truncate(title, w as usize);
        buf.set_string(cx, y, &title, style);
        cx += text::width(&title) as u16;
    }
    if let Some(meta) = meta {
        let mw = text::width(meta) as u16;
        if x + w > cx + mw + 1 {
            buf.set_string(x + w - mw, y, meta, t.faint().bg(bg));
        }
    }
}
