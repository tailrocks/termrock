// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Label / value facts. Labels are muted and right-padded to a shared width.
//! Pad cells between label and value are left unpainted so a dialog fill's
//! leftover fg (dimmed-page faint) stays visible.

use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
};

use crate::style::{JunieTheme, Tone};
use crate::text::{display_cols, truncate_cols, wrap_display_cols};

/// One facts row: muted label, toned value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prop {
    /// Caption painted muted at the row origin.
    pub label: String,
    /// Value painted at `max(label widths) + 2`.
    pub value: String,
    /// Value color.
    pub tone: Tone,
    /// Wrap the value instead of truncating.
    pub wrap: bool,
}

impl Prop {
    /// Label plus value at [`Tone::Normal`].
    #[must_use]
    pub fn new(label: &str, value: impl Into<String>) -> Self {
        Self {
            label: label.to_owned(),
            value: value.into(),
            tone: Tone::Normal,
            wrap: false,
        }
    }

    /// Value color.
    #[must_use]
    pub const fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// Wrap the value across following rows.
    #[must_use]
    pub const fn wrap(mut self) -> Self {
        self.wrap = true;
        self
    }
}

/// Paints each label at `area.x` and each value at `area.x + max_label + 2`.
/// Gap cells are not written. Returns rows used.
pub fn render(area: Rect, buf: &mut Buffer, t: &JunieTheme, props: &[Prop], bg: Color) -> u16 {
    let area = area.intersection(*buf.area());
    if area.is_empty() {
        return 0;
    }
    let label_w = props
        .iter()
        .map(|p| display_cols(&p.label) as u16)
        .max()
        .unwrap_or(0)
        .saturating_add(2);
    let mut y = area.y;
    for p in props {
        if y >= area.bottom() {
            break;
        }
        let lw = display_cols(&p.label).min(usize::from(area.width));
        if lw > 0 {
            buf.set_stringn(area.x, y, &p.label, lw, t.muted().bg(bg));
        }
        let vw = usize::from(area.width.saturating_sub(label_w));
        let style = Style::new().fg(t.tone(p.tone)).bg(bg);
        let vx = area.x.saturating_add(label_w);
        if vx >= area.right() {
            y = y.saturating_add(1);
            continue;
        }
        if p.wrap {
            for line in wrap_display_cols(&p.value, vw.max(4)) {
                if y >= area.bottom() {
                    break;
                }
                let shown = truncate_cols(&line, vw, "…");
                buf.set_stringn(
                    vx,
                    y,
                    shown.as_ref(),
                    display_cols(shown.as_ref()).min(usize::from(area.right().saturating_sub(vx))),
                    style,
                );
                y = y.saturating_add(1);
            }
        } else {
            let shown = truncate_cols(&p.value, vw, "…");
            buf.set_stringn(
                vx,
                y,
                shown.as_ref(),
                display_cols(shown.as_ref()).min(usize::from(area.right().saturating_sub(vx))),
                style,
            );
            y = y.saturating_add(1);
        }
    }
    y.saturating_sub(area.y)
}

#[cfg(test)]
mod tests {
    use ratatui_core::style::Color;

    use super::*;
    use crate::style::DesignSystem;

    #[test]
    fn gap_between_label_and_value_stays_unpainted() {
        let system = DesignSystem::junie();
        let t = system.junie_theme();
        let bg = t.surface_elevated;
        let area = Rect::new(0, 0, 24, 1);
        let mut buf = Buffer::empty(area);
        let faint = Color::Rgb(77, 77, 77);
        for x in 0..24 {
            buf[(x, 0)]
                .set_char(' ')
                .set_style(Style::new().fg(faint).bg(bg));
        }
        let used = render(area, &mut buf, &t, &[Prop::new("Statements", "2")], bg);
        assert_eq!(used, 1);
        assert_eq!(buf[(0, 0)].symbol(), "S");
        assert_eq!(buf[(10, 0)].symbol(), " ");
        assert_eq!(buf[(10, 0)].fg, faint, "pad after label must keep fill fg");
        assert_eq!(buf[(11, 0)].fg, faint);
        assert_eq!(buf[(12, 0)].symbol(), "2");
        assert_eq!(buf[(12, 0)].fg, t.text_primary);
    }
}
