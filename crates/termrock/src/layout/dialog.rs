//! Product-neutral dialog shell and body helpers.

use ratatui_core::{
    layout::{Constraint, Direction, Layout, Rect},
    terminal::Frame,
    text::{Line, Span},
    widgets::Widget,
};
use ratatui_widgets::{
    block::Block, borders::Borders, clear::Clear, paragraph::Paragraph,
};

use crate::{
    scroll::{DialogScroll, effective_offset, line_width},
    style::{Role, Theme},
    widgets::{Panel, PanelEmphasis},
};

/// Available dialog border emphasis choices for shells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DialogBorder {
    /// Focused phosphor border for the active dialog.
    Default,
    /// Danger-accent border for destructive confirmations.
    Danger,
}

/// Minimal dialog shell: clear area, paint bordered block, return inner area.
#[must_use]
pub fn render_dialog_shell(
    frame: &mut Frame<'_>,
    area: Rect,
    title: Option<&str>,
    border: DialogBorder,
) -> Rect {
    Clear.render(area, frame.buffer_mut());

    let theme = Theme::default();
    let block = match border {
        DialogBorder::Default => {
            let mut panel = Panel::new(&theme).emphasis(PanelEmphasis::Focused);
            if let Some(t) = title {
                panel = panel.title(t);
            }
            panel.block()
        }
        DialogBorder::Danger => {
            let mut block = Block::default()
                .borders(Borders::ALL)
                .border_style(theme.style(Role::Danger));
            if let Some(t) = title {
                block = block.title(Span::styled(
                    format!(" {} ", t.trim()),
                    theme.style(Role::Danger),
                ));
            }
            block
        }
    };

    let inner = block.inner(area);
    frame.render_widget(block, area);
    inner
}

/// Split `inner` into the canonical five-slot dialog layout.
#[must_use]
pub fn dialog_inner_chunks(inner: Rect, content_rows: Option<u16>) -> [Rect; 5] {
    let content = content_rows.map_or(Constraint::Min(1), Constraint::Length);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            content,
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);
    [chunks[0], chunks[1], chunks[2], chunks[3], chunks[4]]
}

/// Minimum inner height for the canonical dialog layout with `content_rows`.
#[must_use]
pub const fn dialog_inner_height(content_rows: u16) -> u16 {
    1u16.saturating_add(content_rows)
        .saturating_add(1)
        .saturating_add(1)
        .saturating_add(1)
}

/// Render a dialog body with both-axis scroll and border scrollbars.
pub fn render_scrollable_dialog_body(
    frame: &mut Frame<'_>,
    block_area: Rect,
    content_area: Rect,
    lines: &[Line<'_>],
    scroll: &mut DialogScroll,
) -> (usize, usize) {
    let content_width = lines.iter().map(line_width).max().unwrap_or(0);
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
        .style(Theme::default().style(Role::Text))
        .render(content_area, frame.buffer_mut());
    scroll.render_scrollbars(frame, block_area, content_height, content_width);
    (content_width, content_height)
}
