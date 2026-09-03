//! Product-neutral dialog shell and body helpers.
use ratatui_core::{layout::Rect, terminal::Frame, widgets::Widget};
use ratatui_widgets::clear::Clear;

use crate::widgets::{Panel, PanelChrome, PanelVariant};

/// Minimal dialog shell: clear area, paint bordered block, return inner area.
#[must_use]
pub fn paint_dialog_shell(
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

#[cfg(test)]
mod tests {
    use ratatui_core::{backend::TestBackend, terminal::Terminal};

    use super::*;
    use crate::style::DesignSystem;

    #[test]
    fn dialog_shell_uses_caller_theme_for_each_border_mode() {
        let system = DesignSystem::junie();
        let junie = system.junie_theme();
        for (emphasis, expected) in [
            (PanelChrome::Normal, junie.border_subtle),
            (PanelChrome::Focused, junie.border_strong),
            (PanelChrome::Danger, junie.error),
        ] {
            let mut terminal = Terminal::new(TestBackend::new(12, 4)).unwrap();
            terminal
                .draw(|frame| {
                    let _ =
                        paint_dialog_shell(frame, frame.area(), Some("Test"), emphasis, &system);
                })
                .unwrap();
            assert_eq!(terminal.backend().buffer()[(0, 0)].fg, expected);
        }
    }
}
