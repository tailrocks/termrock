// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Pure, frame-driven motion and cell-style helpers.
//!
//! Callers supply deterministic ticks. Reduced/off motion always resolves to
//! a static fully-visible result; no helper reads wall time.

use ratatui_core::style::{Color, Style};

use super::Motion;

#[must_use]
/// Temporal sine-squared pulse for a deterministic tick and period.
pub fn pulse_brightness(tick: u64, period: u64) -> f32 {
    if period == 0 {
        return 1.0;
    }
    let phase = (tick % period) as f32 / period as f32;
    (std::f32::consts::PI * phase).sin().powi(2)
}

#[must_use]
/// Spatial sine-squared wave flowing along a row axis.
pub fn wave_brightness(tick: u64, row: u16, wave_rows: u16, speed: f32) -> f32 {
    if wave_rows == 0 {
        return 1.0;
    }
    let phase = (f32::from(row) - tick as f32 * speed) / f32::from(wave_rows);
    (std::f32::consts::PI * phase).sin().powi(2).clamp(0.0, 1.0)
}

#[must_use]
/// Clamped cubic smoothstep over `0..=1`.
pub fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[must_use]
/// Symmetric smooth edge alpha for a bounded row of cells.
pub fn edge_fade(col: u16, width: u16, fade_cols: u16) -> f32 {
    if width == 0 || fade_cols == 0 {
        return 1.0;
    }
    let left = f32::from(col.saturating_add(1)) / f32::from(fade_cols);
    let right = f32::from(width.saturating_sub(col)) / f32::from(fade_cols);
    smoothstep(left.min(right))
}

#[must_use]
/// Blend RGB `from` toward RGB `to`; preserve unsupported color forms.
pub fn blend_toward(from: Color, to: Color, t: f32) -> Color {
    match (from, to) {
        (Color::Rgb(fr, fg, fb), Color::Rgb(tr, tg, tb)) => {
            let t = t.clamp(0.0, 1.0);
            let lerp =
                |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round() as u8;
            Color::Rgb(lerp(fr, tr), lerp(fg, tg), lerp(fb, tb))
        }
        (other, _) => other,
    }
}

#[must_use]
/// Blend every color channel in a style toward an explicit canvas color.
pub fn fade_style(mut style: Style, alpha: f32, canvas: Color) -> Style {
    let toward = 1.0 - alpha.clamp(0.0, 1.0);
    if let Some(color) = style.fg {
        style.fg = Some(blend_toward(color, canvas, toward));
    }
    if let Some(color) = style.bg {
        style.bg = Some(blend_toward(color, canvas, toward));
    }
    if let Some(color) = style.underline_color {
        style.underline_color = Some(blend_toward(color, canvas, toward));
    }
    style
}

#[must_use]
/// Resolve animated alpha to a static visible fallback under reduced motion.
pub fn effective_alpha(motion: Motion, animated: f32) -> f32 {
    if matches!(motion, Motion::Full) {
        animated.clamp(0.0, 1.0)
    } else {
        1.0
    }
}

#[must_use]
/// Merge adjacent equal-style cells into minimal text runs.
pub fn coalesce_cells(cells: impl IntoIterator<Item = (char, Style)>) -> Vec<(String, Style)> {
    let mut runs: Vec<(String, Style)> = Vec::new();
    for (cell, style) in cells {
        if let Some((text, previous)) = runs.last_mut()
            && *previous == style
        {
            text.push(cell);
        } else {
            runs.push((cell.to_string(), style));
        }
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motion_math_is_bounded_and_symmetric() {
        assert!(pulse_brightness(0, 8).abs() < 0.001);
        assert!((pulse_brightness(4, 8) - 1.0).abs() < 0.001);
        assert!((smoothstep(0.5) - 0.5).abs() < 0.001);
        assert_eq!(edge_fade(0, 10, 3), edge_fade(9, 10, 3));
        assert!((wave_brightness(3, 2, 8, 1.0) - wave_brightness(3, 10, 8, 1.0)).abs() < 0.001);
    }

    #[test]
    fn reduced_motion_is_static() {
        assert_eq!(effective_alpha(Motion::Reduced, 0.2), 1.0);
        assert_eq!(effective_alpha(Motion::Off, 0.0), 1.0);
    }

    #[test]
    fn coalesces_equal_style_runs() {
        let a = Style::new().fg(Color::Red);
        let b = Style::new().fg(Color::Blue);
        let cells = (0..1000).map(|i| ('x', if i < 300 || i >= 700 { a } else { b }));
        assert_eq!(coalesce_cells(cells).len(), 3);
    }
}
