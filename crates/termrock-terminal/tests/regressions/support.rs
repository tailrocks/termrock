//! Drawing helpers shared by the `regressions` tests of `termrock-terminal`.

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::widgets::Widget;
use termrock_core::focus::FocusRing;
use termrock_core::hit::HitRegistry;
use termrock_core::scroll::TrackClick;
use termrock_runtime::ctx::{Interaction, RenderCtx};
use termrock_terminal::{TerminalState, TerminalView};
use termrock_theme::Theme;
use termrock_viewport::viewport_ops::ViewportOps;

/// Renders a terminal state into a buffer the size of its grid.
pub(super) fn draw_state(state: &TerminalState, focused: bool) -> Buffer {
    let (rows, cols) = state.size();
    let area = Rect::new(0, 0, cols, rows);
    let mut buf = Buffer::empty(area);
    TerminalView::new(state)
        .focused(focused)
        .render(area, &mut buf);
    buf
}

/// Paints a viewport into a 30x14 buffer and returns the caret position.
pub(super) fn paint_viewport(
    v: &dyn ViewportOps,
    area: Rect,
    theme: &Theme,
    caret_allowed: bool,
    track_click: TrackClick,
) -> (Buffer, Option<Position>) {
    let mut buf = Buffer::empty(Rect::new(0, 0, 30, 14));
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(theme, Interaction::default(), &mut hits, &mut ring);
    ctx.track_click = track_click;
    let cursor = v.draw(area, &mut buf, &mut ctx, theme.canvas, caret_allowed);
    (buf, cursor)
}
