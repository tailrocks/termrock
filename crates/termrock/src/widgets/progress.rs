use ratatui_core::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::{
    style::{Role, Theme},
    text::display_cols,
};

/// Default one-cell braille animation frames for indeterminate progress.
pub const DEFAULT_PROGRESS_FRAMES: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];

#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
/// Determinate and caller-ticked indeterminate progress modes.
pub enum ProgressKind {
    /// A progress bar with a known completion fraction.
    Determinate {
        /// Completed fraction; rendering clamps finite values to `0.0..=1.0`.
        fraction: f64,
    },
    /// A caller-ticked progress indicator with no known completion fraction.
    Indeterminate {
        /// Caller-owned deterministic animation tick.
        tick: u64,
    },
}

#[derive(Debug, Clone, Copy)]
/// A one-row progress indicator with an optional label.
pub struct Progress<'a> {
    kind: ProgressKind,
    label: Option<&'a str>,
    frames: &'a [&'a str],
    theme: &'a Theme,
}

impl<'a> Progress<'a> {
    #[must_use]
    /// Creates an unlabeled progress indicator in the supplied mode.
    pub const fn new(kind: ProgressKind, theme: &'a Theme) -> Self {
        Self {
            kind,
            label: None,
            frames: &DEFAULT_PROGRESS_FRAMES,
            theme,
        }
    }

    #[must_use]
    /// Sets the optional visible label.
    pub const fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    /// Overrides indeterminate animation frames.
    ///
    /// Frames may use any terminal-width-safe Unicode string. An empty slice
    /// intentionally paints neither a frame nor a label.
    #[must_use]
    pub const fn frames(mut self, frames: &'a [&'a str]) -> Self {
        self.frames = frames;
        self
    }
}

impl Widget for &Progress<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        buffer.set_style(area, self.theme.style(Role::TextMuted));
        match self.kind {
            ProgressKind::Determinate { fraction } => {
                render_determinate(area, buffer, self.label, fraction, self.theme);
            }
            ProgressKind::Indeterminate { tick } => {
                render_indeterminate(area, buffer, self.label, tick, self.frames, self.theme);
            }
        }
    }
}

impl Widget for Progress<'_> {
    #[expect(
        clippy::needless_borrows_for_generic_args,
        reason = "explicitly delegate the owned contract to the borrowed renderer"
    )]
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

fn render_determinate(
    area: Rect,
    buffer: &mut Buffer,
    label: Option<&str>,
    fraction: f64,
    theme: &Theme,
) {
    let fraction = if fraction.is_finite() {
        fraction.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let percentage = format!("{:>3}%", (fraction * 100.0).round() as u8);
    let percentage_width = u16::try_from(display_cols(&percentage))
        .unwrap_or(u16::MAX)
        .min(area.width);
    let percentage_x = area.right().saturating_sub(percentage_width);
    buffer.set_stringn(
        percentage_x,
        area.y,
        &percentage,
        usize::from(percentage_width),
        theme.style(Role::Text),
    );

    let mut track_x = area.x;
    if let Some(label) = label {
        let available = percentage_x.saturating_sub(area.x);
        let label_width = u16::try_from(display_cols(label))
            .unwrap_or(u16::MAX)
            .min(available.saturating_sub(1));
        buffer.set_stringn(
            area.x,
            area.y,
            label,
            usize::from(label_width),
            theme.style(Role::TextMuted),
        );
        track_x = area.x.saturating_add(label_width);
        if track_x < percentage_x {
            track_x = track_x.saturating_add(1);
        }
    }
    let track_width = percentage_x.saturating_sub(track_x).saturating_sub(1);
    // Positive fractions round to the nearest cell with ties toward the
    // completed side, matching percentage rounding.
    let filled = ((f64::from(track_width) * fraction).round() as u16).min(track_width);
    for column in 0..track_width {
        buffer.set_string(
            track_x.saturating_add(column),
            area.y,
            if column < filled { "█" } else { "░" },
            theme.style(if column < filled {
                Role::Accent
            } else {
                Role::TextMuted
            }),
        );
    }
}

fn render_indeterminate(
    area: Rect,
    buffer: &mut Buffer,
    label: Option<&str>,
    tick: u64,
    frames: &[&str],
    theme: &Theme,
) {
    if frames.is_empty() {
        return;
    }
    let frame_count = u64::try_from(frames.len()).unwrap_or(u64::MAX);
    let frame_index = usize::try_from(tick % frame_count).unwrap_or(0);
    let glyph = frames[frame_index];
    let glyph_width = u16::try_from(display_cols(glyph))
        .unwrap_or(u16::MAX)
        .min(area.width);
    buffer.set_stringn(
        area.x,
        area.y,
        glyph,
        usize::from(glyph_width),
        theme.style(Role::Accent),
    );
    if let Some(label) = label
        && glyph_width < area.width
    {
        let label_x = area.x.saturating_add(glyph_width).saturating_add(1);
        let label_width = area.right().saturating_sub(label_x);
        buffer.set_stringn(
            label_x,
            area.y,
            label,
            usize::from(label_width),
            theme.style(Role::TextMuted),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered(buffer: &Buffer) -> String {
        buffer.content().iter().map(|cell| cell.symbol()).collect()
    }

    #[test]
    fn determinate_progress_clamps_and_keeps_percentage_non_color_cue() {
        let theme = Theme::default();
        let area = Rect::new(2, 1, 18, 1);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 22, 3));
        (&Progress::new(ProgressKind::Determinate { fraction: 1.5 }, &theme).label("Index"))
            .render(area, &mut buffer);

        let row = rendered(&buffer);
        assert!(row.contains("Index"));
        assert!(row.contains("100%"));
        assert!(row.contains('█'));
    }

    #[test]
    fn indeterminate_tick_is_deterministic_and_tiny_areas_are_safe() {
        let theme = Theme::default();
        let area = Rect::new(0, 0, 8, 1);
        let mut first = Buffer::empty(area);
        let mut second = Buffer::empty(area);
        let progress = Progress::new(ProgressKind::Indeterminate { tick: 3 }, &theme).label("Load");
        (&progress).render(area, &mut first);
        (&progress).render(area, &mut second);

        assert_eq!(first, second);
        assert_eq!(first[(0, 0)].symbol(), "⠸");
        (&progress).render(Rect::new(0, 0, 0, 0), &mut first);
    }

    fn determinate(fraction: f64, width: u16) -> Buffer {
        let theme = Theme::default();
        let area = Rect::new(0, 0, width, 1);
        let mut buffer = Buffer::empty(area);
        (&Progress::new(ProgressKind::Determinate { fraction }, &theme)).render(area, &mut buffer);
        buffer
    }

    #[test]
    fn zero_fraction_renders_all_empty_glyphs() {
        let buffer = determinate(0.0, 9);
        assert!((0..4).all(|x| buffer[(x, 0)].symbol() == "░"));
    }

    #[test]
    fn half_fraction_splits_cells_exactly() {
        let buffer = determinate(0.5, 9);
        assert_eq!(buffer[(0, 0)].symbol(), "█");
        assert_eq!(buffer[(1, 0)].symbol(), "█");
        assert_eq!(buffer[(2, 0)].symbol(), "░");
        assert_eq!(buffer[(3, 0)].symbol(), "░");
    }

    #[test]
    fn full_fraction_renders_all_filled_glyphs() {
        let buffer = determinate(1.0, 9);
        assert!((0..4).all(|x| buffer[(x, 0)].symbol() == "█"));
    }

    #[test]
    fn nan_and_infinite_clamp_to_zero() {
        for fraction in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let buffer = determinate(fraction, 9);
            assert!((0..4).all(|x| buffer[(x, 0)].symbol() == "░"));
            assert!(rendered(&buffer).contains("0%"));
        }
    }

    #[test]
    fn width_zero_and_one_do_not_panic() {
        let theme = Theme::default();
        let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
        let progress = Progress::new(ProgressKind::Determinate { fraction: 0.5 }, &theme);
        (&progress).render(Rect::new(0, 0, 0, 0), &mut buffer);
        (&progress).render(Rect::new(0, 0, 1, 1), &mut buffer);
    }

    #[test]
    fn filled_and_empty_zones_differ_by_glyph() {
        let buffer = determinate(0.5, 9);
        assert_ne!(buffer[(0, 0)].symbol(), buffer[(3, 0)].symbol());
    }

    #[test]
    fn wide_char_label_truncates_on_grapheme_boundary() {
        let theme = Theme::default();
        let area = Rect::new(0, 0, 10, 1);
        let mut buffer = Buffer::empty(area);
        (&Progress::new(ProgressKind::Determinate { fraction: 0.5 }, &theme).label("東京🪨"))
            .render(area, &mut buffer);
        assert_eq!(buffer[(0, 0)].symbol(), "東");
        assert_eq!(buffer[(2, 0)].symbol(), "京");
        assert!(!rendered(&buffer).contains('🪨'));
    }

    #[test]
    fn custom_frames_cycle_and_wrap() {
        let theme = Theme::default();
        let frames = ["A", "B"];
        for (tick, expected) in [(0, "A"), (1, "B"), (2, "A")] {
            let area = Rect::new(0, 0, 3, 1);
            let mut buffer = Buffer::empty(area);
            (&Progress::new(ProgressKind::Indeterminate { tick }, &theme).frames(&frames))
                .render(area, &mut buffer);
            assert_eq!(buffer[(0, 0)].symbol(), expected);
        }
    }

    #[test]
    fn empty_frames_render_nothing() {
        let theme = Theme::default();
        let area = Rect::new(0, 0, 8, 1);
        let mut buffer = Buffer::empty(area);
        (&Progress::new(ProgressKind::Indeterminate { tick: 3 }, &theme)
            .frames(&[])
            .label("hidden"))
            .render(area, &mut buffer);
        assert!(rendered(&buffer).trim().is_empty());
    }
}
