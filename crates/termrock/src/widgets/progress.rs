use ratatui_core::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::{
    style::{Role, Theme},
    text::display_cols,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProgressKind {
    Determinate { fraction: f64 },
    Indeterminate { tick: u64 },
}

#[derive(Debug, Clone, Copy)]
pub struct Progress<'a> {
    pub kind: ProgressKind,
    pub label: Option<&'a str>,
    pub theme: &'a Theme,
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
                render_indeterminate(area, buffer, self.label, tick, self.theme);
            }
        }
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
    theme: &Theme,
) {
    const SPINNER: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];
    let glyph = SPINNER[tick as usize % SPINNER.len()];
    buffer.set_string(area.x, area.y, glyph, theme.style(Role::Accent));
    if let Some(label) = label
        && area.width > 2
    {
        buffer.set_stringn(
            area.x.saturating_add(2),
            area.y,
            label,
            usize::from(area.width.saturating_sub(2)),
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
        (&Progress {
            kind: ProgressKind::Determinate { fraction: 1.5 },
            label: Some("Index"),
            theme: &theme,
        })
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
        let progress = Progress {
            kind: ProgressKind::Indeterminate { tick: 3 },
            label: Some("Load"),
            theme: &theme,
        };
        (&progress).render(area, &mut first);
        (&progress).render(area, &mut second);

        assert_eq!(first, second);
        assert_eq!(first[(0, 0)].symbol(), "⠸");
        (&progress).render(Rect::new(0, 0, 0, 0), &mut first);
    }
}
