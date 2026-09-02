//! Product-neutral dialog shell and body helpers.
#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{layout::Rect, terminal::Frame, text::Line, widgets::Widget};
use ratatui_widgets::{clear::Clear, paragraph::Paragraph};

use crate::{
    scroll::{DialogScroll, effective_offset},
    style::{DesignSystem, Role, RolePalette},
    widgets::{Panel, PanelChrome, PanelVariant},
};

/// Minimal dialog shell: clear area, paint bordered block, return inner area.
#[must_use]
pub fn render_dialog_shell(
    frame: &mut Frame<'_>,
    area: Rect,
    title: Option<&str>,
    emphasis: PanelChrome,
    system: &crate::style::DesignSystem,
) -> Rect {
    Clear.render(area, frame.buffer_mut());

    let mut panel = Panel::new(system)
        .variant(PanelVariant::Bordered)
        .emphasis(emphasis);
    if let Some(title) = title {
        panel = panel.title(title);
    }
    let inner = panel.inner(area);
    panel.paint(area, frame.buffer_mut(), None);
    inner
}

/// Render a dialog body with both-axis scroll and border scrollbars.
pub fn render_scrollable_dialog_body(
    frame: &mut Frame<'_>,
    block_area: Rect,
    content_area: Rect,
    lines: &[Line<'_>],
    scroll: &mut DialogScroll,
    system: &crate::style::DesignSystem,
) -> (usize, usize) {
    let content_width = lines.iter().map(Line::width).max().unwrap_or(0);
    let content_height = lines.len();
    let vp_w = usize::from(content_area.width);
    let vp_h = usize::from(content_area.height);
    let eff_x = effective_offset(content_width, vp_w, scroll.scroll_x);
    let eff_y = effective_offset(content_height, vp_h, scroll.scroll_y);
    scroll.scroll_x = eff_x;
    scroll.scroll_y = eff_y;

    let start = usize::from(eff_y).min(content_height);
    let visible = lines[start..]
        .iter()
        .take(vp_h)
        .cloned()
        .collect::<Vec<_>>();
    Paragraph::new(visible)
        .scroll((0, eff_x))
        .style(system.style(Role::Text))
        .render(content_area, frame.buffer_mut());
    scroll.render_scrollbars(frame, block_area, content_height, content_width, system);
    (content_width, content_height)
}

#[cfg(test)]
mod tests {
    use ratatui_core::{
        backend::TestBackend,
        style::{Color, Style},
        terminal::Terminal,
    };

    use super::*;

    #[test]
    fn dialog_shell_uses_caller_theme_for_each_border_mode() {
        let theme = RolePalette::default()
            .with_role(Role::Border, Style::new().fg(Color::Blue))
            .with_role(Role::BorderFocused, Style::new().fg(Color::Green))
            .with_role(Role::Danger, Style::new().fg(Color::Red));

        for (emphasis, expected) in [
            (PanelChrome::Normal, Color::Blue),
            (PanelChrome::Focused, Color::Green),
            (PanelChrome::Danger, Color::Red),
        ] {
            let mut terminal = Terminal::new(TestBackend::new(12, 4)).unwrap();
            terminal
                .draw(|frame| {
                    let _ = render_dialog_shell(
                        frame,
                        frame.area(),
                        Some("Test"),
                        emphasis,
                        &DesignSystem::from_palette(theme.clone()),
                    );
                })
                .unwrap();
            assert_eq!(terminal.backend().buffer()[(0, 0)].fg, expected);
        }
    }
}
