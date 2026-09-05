// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Modal backdrop paint adapter.
//!
//! Z-order, nesting, focus restoration, and outside/Escape dismissal live only
//! on [`crate::interaction::OverlayStack`].
use ratatui_core::layout::Rect;
use ratatui_core::terminal::Frame;

use super::BackdropPolicy;
use crate::style::DesignSystem;
use crate::widgets::Backdrop;

/// Render the stack-selected backdrop across the whole overlay layer.
///
/// Call this with the layer rect (usually the frame area) before drawing the
/// modal, whenever the overlay stack asks for
/// [`BackdropPolicy`]: the widget itself only ever receives its own rect, so
/// the backdrop belongs to the host that owns the layer.
pub fn render_backdrop(
    frame: &mut Frame<'_>,
    full_area: Rect,
    system: &DesignSystem,
    policy: BackdropPolicy,
) {
    frame.render_widget(Backdrop::new(system).policy(policy), full_area);
}
