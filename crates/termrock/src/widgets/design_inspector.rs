// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Studio-oriented design inspector (lookbook/debug). Not a production shell.

use ratatui_core::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::{
    style::{ColorCapability, DesignSystem, Role, Theme},
    text::take_display_cols,
};

/// Read-only inspector snapshot for one frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesignInspectorFrame<'a> {
    /// Focused element id display.
    pub focused: Option<&'a str>,
    /// Active layer id display.
    pub layer: Option<&'a str>,
    /// Capability ladder.
    pub capability: ColorCapability,
    /// Density label.
    pub density: &'a str,
}

/// Paints a compact inspector strip.
#[derive(Debug, Clone)]
pub struct DesignInspector<'a> {
    frame: DesignInspectorFrame<'a>,
    theme: &'a Theme,
}

impl<'a> DesignInspector<'a> {
    /// Creates an inspector for the given snapshot.
    #[must_use]
    pub const fn new(frame: DesignInspectorFrame<'a>, theme: &'a Theme) -> Self {
        Self { frame, theme }
    }

    /// Convenience from a design system.
    #[must_use]
    pub fn from_system(system: &'a DesignSystem, frame: DesignInspectorFrame<'a>) -> Self {
        Self {
            frame,
            theme: system.theme(),
        }
    }
}

impl Widget for &DesignInspector<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let focus = self.frame.focused.unwrap_or("—");
        let layer = self.frame.layer.unwrap_or("root");
        let line = format!(
            "focus:{focus} layer:{layer} dens:{} cap:{:?}",
            self.frame.density, self.frame.capability
        );
        let clipped = take_display_cols(&line, usize::from(area.width));
        buffer.set_stringn(
            area.x,
            area.y,
            &clipped,
            usize::from(area.width),
            self.theme.style(Role::TextMuted),
        );
    }
}

impl Widget for DesignInspector<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        Widget::render(&self, area, buffer);
    }
}
