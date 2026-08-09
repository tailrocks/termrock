// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Density chart primitives (sparkline, bar series, segmented meter).

use ratatui_core::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::{
    style::{Role, Theme},
    text::take_display_cols,
};

const SPARK_GLYPHS: &[char] = &[' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// One-row sparkline over normalized samples in `0.0..=1.0`.
#[derive(Debug, Clone, Copy)]
pub struct Sparkline<'a> {
    samples: &'a [f64],
    theme: &'a Theme,
}

impl<'a> Sparkline<'a> {
    /// Creates a sparkline from borrowed samples.
    #[must_use]
    pub const fn new(samples: &'a [f64], theme: &'a Theme) -> Self {
        Self { samples, theme }
    }
}

impl Widget for &Sparkline<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() || self.samples.is_empty() {
            return;
        }
        let width = usize::from(area.width);
        for col in 0..width {
            let index = col * self.samples.len() / width.max(1);
            let sample = self.samples.get(index).copied().unwrap_or(0.0);
            let fraction = if sample.is_finite() {
                sample.clamp(0.0, 1.0)
            } else {
                0.0
            };
            let glyph_index = ((fraction * (SPARK_GLYPHS.len() - 1) as f64).round() as usize)
                .min(SPARK_GLYPHS.len() - 1);
            let glyph = SPARK_GLYPHS[glyph_index].to_string();
            buffer.set_stringn(
                area.x.saturating_add(col as u16),
                area.y,
                &glyph,
                1,
                self.theme.style(Role::Accent),
            );
        }
    }
}

impl Widget for Sparkline<'_> {
    #[expect(
        clippy::needless_borrows_for_generic_args,
        reason = "explicitly delegate the owned contract to the borrowed renderer"
    )]
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

/// One named bar in a horizontal bar series.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarDatum<'a> {
    /// Label shown to the left when width allows.
    pub label: &'a str,
    /// Fraction filled (`0.0..=1.0`).
    pub fraction: f64,
}

/// Multi-row horizontal bar chart.
#[derive(Debug, Clone, Copy)]
pub struct BarSeries<'a> {
    bars: &'a [BarDatum<'a>],
    theme: &'a Theme,
}

impl<'a> BarSeries<'a> {
    /// Creates a bar series.
    #[must_use]
    pub const fn new(bars: &'a [BarDatum<'a>], theme: &'a Theme) -> Self {
        Self { bars, theme }
    }
}

impl Widget for &BarSeries<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let rows = usize::from(area.height).min(self.bars.len());
        let label_width = self
            .bars
            .iter()
            .take(rows)
            .map(|bar| unicode_width::UnicodeWidthStr::width(bar.label))
            .max()
            .unwrap_or(0)
            .min(usize::from(area.width) / 3)
            .min(12);
        for (row, bar) in self.bars.iter().take(rows).enumerate() {
            let y = area.y.saturating_add(row as u16);
            let label = take_display_cols(bar.label, label_width);
            if label_width > 0 {
                buffer.set_stringn(
                    area.x,
                    y,
                    &label,
                    label_width,
                    self.theme.style(Role::TextMuted),
                );
            }
            let track_x = area
                .x
                .saturating_add(u16::try_from(label_width).unwrap_or(0));
            let track_w = area
                .width
                .saturating_sub(u16::try_from(label_width).unwrap_or(0));
            if track_w == 0 {
                continue;
            }
            let fraction = if bar.fraction.is_finite() {
                bar.fraction.clamp(0.0, 1.0)
            } else {
                0.0
            };
            let filled = ((f64::from(track_w) * fraction).round() as u16).min(track_w);
            let fill = "█".repeat(usize::from(filled));
            let empty = "░".repeat(usize::from(track_w.saturating_sub(filled)));
            buffer.set_stringn(
                track_x,
                y,
                &fill,
                usize::from(filled),
                self.theme.style(Role::Accent),
            );
            if filled < track_w {
                buffer.set_stringn(
                    track_x.saturating_add(filled),
                    y,
                    &empty,
                    usize::from(track_w.saturating_sub(filled)),
                    self.theme.style(Role::TextDisabled),
                );
            }
        }
    }
}

impl Widget for BarSeries<'_> {
    #[expect(
        clippy::needless_borrows_for_generic_args,
        reason = "explicitly delegate the owned contract to the borrowed renderer"
    )]
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

/// One segment in a segmented meter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeterSegment<'a> {
    /// Segment label for narrow fallback text.
    pub label: &'a str,
    /// Relative weight (normalized across segments).
    pub weight: f64,
    /// Semantic role used for the fill.
    pub role: Role,
}

/// Single-row segmented meter (stacked proportions).
#[derive(Debug, Clone, Copy)]
pub struct SegmentedMeter<'a> {
    segments: &'a [MeterSegment<'a>],
    theme: &'a Theme,
}

impl<'a> SegmentedMeter<'a> {
    /// Creates a segmented meter.
    #[must_use]
    pub const fn new(segments: &'a [MeterSegment<'a>], theme: &'a Theme) -> Self {
        Self { segments, theme }
    }
}

impl Widget for &SegmentedMeter<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() || self.segments.is_empty() {
            return;
        }
        let total: f64 = self
            .segments
            .iter()
            .map(|segment| {
                if segment.weight.is_finite() && segment.weight > 0.0 {
                    segment.weight
                } else {
                    0.0
                }
            })
            .sum::<f64>()
            .max(f64::EPSILON);
        let mut remaining = area.width;
        let mut x = area.x;
        for (index, segment) in self.segments.iter().enumerate() {
            let weight = if segment.weight.is_finite() && segment.weight > 0.0 {
                segment.weight
            } else {
                0.0
            };
            let mut width =
                ((f64::from(area.width) * weight / total).round() as u16).min(remaining);
            if index + 1 == self.segments.len() {
                width = remaining;
            }
            if width == 0 {
                continue;
            }
            let fill = "█".repeat(usize::from(width));
            buffer.set_stringn(
                x,
                area.y,
                &fill,
                usize::from(width),
                self.theme.style(segment.role),
            );
            x = x.saturating_add(width);
            remaining = remaining.saturating_sub(width);
        }
    }
}

impl Widget for SegmentedMeter<'_> {
    #[expect(
        clippy::needless_borrows_for_generic_args,
        reason = "explicitly delegate the owned contract to the borrowed renderer"
    )]
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparkline_uses_block_glyphs() {
        let theme = Theme::default();
        let samples = [0.0, 0.5, 1.0];
        let mut buffer = Buffer::empty(Rect::new(0, 0, 3, 1));
        Sparkline::new(&samples, &theme).render(Rect::new(0, 0, 3, 1), &mut buffer);
        assert_ne!(buffer[(0, 0)].symbol(), buffer[(2, 0)].symbol());
    }

    #[test]
    fn segmented_meter_covers_full_width() {
        let theme = Theme::default();
        let segments = [
            MeterSegment {
                label: "a",
                weight: 1.0,
                role: Role::Success,
            },
            MeterSegment {
                label: "b",
                weight: 1.0,
                role: Role::Danger,
            },
        ];
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 1));
        SegmentedMeter::new(&segments, &theme).render(Rect::new(0, 0, 10, 1), &mut buffer);
        for x in 0..10 {
            assert_eq!(buffer[(x, 0)].symbol(), "█");
        }
    }
}
