// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Shared catalog/TablePro chrome helpers over public TermRock widgets.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use termrock::widgets::{Button, ButtonState, ButtonVariant};

use termrock::runtime::FrameTick;

use crate::ctx::RenderCtx;
use crate::id::WidgetId;
use crate::text;

/// Catalog animation tick: 80ms frames from Interaction.tick.
#[must_use]
pub fn tick_frame(tick: u64) -> FrameTick {
    FrameTick::manual(
        termrock::runtime::Instant::now(),
        std::time::Duration::from_millis(tick.saturating_mul(80)),
        std::time::Duration::from_millis(80),
    )
}

/// Junie button width: label plus one cell of padding on each side.
#[must_use]
pub fn button_width(label: &str) -> u16 {
    (text::width(label) + 2) as u16
}

/// Paint a TermRock button and register it as a catalog control.
pub fn button(
    label: &str,
    variant: ButtonVariant,
    id: WidgetId,
    area: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx<'_>,
    state: &mut ButtonState,
    disabled: bool,
    bg: Color,
) {
    state.focused = ctx.interaction.focused(id);
    state.hovered = ctx.interaction.hovered(id);
    state.activation.set_enabled(!disabled);
    state.activation.set_accepts_input(!disabled);
    let _ = Button::new(label, ctx.system)
        .variant(variant)
        .container(bg)
        .paint(area, buf, state);
    ctx.control(id, area, disabled);
}
