// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/showcase/pages/overview.rs (MIT).

//! Tokens and principles. Nothing interactive: this page is the reference.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use crate::ctx::RenderCtx;
use crate::draw::fill;
use crate::id::WidgetId;
use crate::layout;
use crate::outcome::Route;
use crate::page::{Hint, Page, PageCtx, PageEvent};
use crate::text;
use termrock::style::JunieTheme;

pub struct OverviewPage;

impl OverviewPage {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

fn swatches(t: &JunieTheme) -> Vec<(&'static str, Color, &'static str)> {
    vec![
        ("canvas", t.canvas, "#000000"),
        ("surface", t.surface, "#111111"),
        ("surface.elevated", t.surface_elevated, "#18181b"),
        ("surface.overlay", t.surface_overlay, "#27272a"),
        ("field", t.field, "#1e1e22"),
        ("popover", t.popover, "#3f3f46"),
        ("border.subtle", t.border_subtle, "white 15%"),
        ("border.strong", t.border_strong, "white 30%"),
        ("text.primary", t.text_primary, "#ffffff"),
        ("text.secondary", t.text_secondary, "white 70%"),
        ("text.muted", t.text_muted, "white 50%"),
        ("text.faint", t.text_faint, "white 30%"),
        ("accent", t.accent, "#48e054"),
        ("accent.hover", t.accent_hover, "#3ab343"),
        ("accent.pressed", t.accent_pressed, "#2b8632"),
        ("accent.bg", t.accent_bg, "green 20%"),
        ("error", t.error, "#e44545"),
        ("warning", t.warning, "#f59e09"),
        // Source overview lists `info`; JunieTheme does not expose the dormant token.
        ("info", Color::Rgb(0x87, 0x87, 0xff), "#8787ff"),
    ]
}

const PRINCIPLES: &[(&str, &str)] = &[
    (
        "One hue",
        "Green means focus, primary action or selection. Everything else is achromatic.",
    ),
    (
        "Alpha ladder",
        "Text and borders step down in white opacity, never in arbitrary grays.",
    ),
    (
        "State is geometry",
        "Hover lifts the surface, focus adds a bar, selection adds a marker, editing shows the cursor.",
    ),
    (
        "Three planes",
        "Canvas, surface, elevated. Depth comes from lightness, not borders.",
    ),
    (
        "Quiet chrome",
        "Bold is reserved for the focused control. No box around a thing unless the box carries meaning.",
    ),
];

impl Page for OverviewPage {
    fn title(&self) -> &'static str {
        "Overview"
    }
    fn blurb(&self) -> &'static str {
        "Tokens and principles behind every component"
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let t = ctx.theme;
        let (left, right) = layout::columns(area, 46, 2);

        let all = swatches(t);
        let left = Rect::new(
            left.x,
            left.y,
            left.width,
            left.height.min(all.len() as u16 + 3),
        );
        let (inner, bg) = layout::card(left, buf, t, Some("Tokens"), None, false);
        let two_col = (inner.height as usize) < all.len() && inner.width >= 44;
        let per_col = if two_col {
            all.len().div_ceil(2)
        } else {
            all.len()
        };
        let col_w = if two_col {
            inner.width / 2
        } else {
            inner.width
        };
        for (i, (name, color, note)) in all.iter().enumerate() {
            let col = (i / per_col) as u16;
            let y = inner.y + (i % per_col) as u16;
            if y >= inner.bottom() {
                continue;
            }
            let x = inner.x + col * col_w;
            fill(buf, Rect::new(x, y, 4, 1), Style::new().bg(*color));
            buf.set_string(x + 4, y, "▏", t.faint().bg(bg));
            buf.set_string(x + 6, y, name, t.primary().bg(bg));
            let nw = text::width(note) as u16;
            if col_w > 30 {
                buf.set_string(x + col_w.saturating_sub(nw + 1), y, note, t.muted().bg(bg));
            }
        }

        let inner_w = right.width.saturating_sub(4) as usize;
        let wrapped: Vec<(&str, Vec<String>)> = PRINCIPLES
            .iter()
            .map(|(title, body)| (*title, text::wrap(body, inner_w)))
            .collect();
        let needed: u16 = wrapped.iter().map(|(_, l)| l.len() as u16 + 2).sum::<u16>() + 2;
        let rows = layout::rows(right, &[needed.min(right.height.saturating_sub(10)), 1, 0]);
        let (inner, bg) = layout::card(rows[0], buf, t, Some("Principles"), None, false);
        let mut y = inner.y;
        for (title, lines) in &wrapped {
            if y + 1 >= inner.bottom() {
                break;
            }
            buf.set_string(
                inner.x,
                y,
                title,
                t.primary().bg(bg).add_modifier(Modifier::BOLD),
            );
            y += 1;
            for l in lines {
                if y >= inner.bottom() {
                    break;
                }
                buf.set_string(inner.x, y, l, t.secondary().bg(bg));
                y += 1;
            }
            y += 1;
        }

        let legend_area = Rect::new(rows[2].x, rows[2].y, rows[2].width, rows[2].height.min(10));
        let (inner, bg) = layout::card(legend_area, buf, t, Some("State language"), None, false);
        let legend: [(&str, &str, ratatui::style::Style); 7] = [
            ("▎", "focus", t.accent_fg()),
            ("░", "hover lifts the surface", t.secondary()),
            ("›", "current / chosen", t.accent_fg()),
            ("✓", "checked", t.accent_fg()),
            ("!", "error", t.error_fg()),
            ("▁", "editing: cursor + underline", t.primary()),
            ("○", "disabled: faint, no hover", t.faint()),
        ];
        for (i, (g, body, st)) in legend.iter().enumerate() {
            let y = inner.y + i as u16;
            if y >= inner.bottom() {
                break;
            }
            buf.set_string(inner.x, y, g, st.bg(bg));
            buf.set_string(inner.x + 3, y, body, t.secondary().bg(bg));
        }
    }

    fn handle(&mut self, _ev: &PageEvent, _cx: &mut PageCtx<'_>) -> Route {
        Route::Ignored
    }

    fn hints(&self, _focus: Option<WidgetId>) -> Vec<Hint> {
        vec![("[ ]", "Pages"), ("i", "Inspector")]
    }
}
