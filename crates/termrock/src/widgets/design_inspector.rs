// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Studio-oriented design inspector (lookbook/debug). Not a production shell.
//!
//! Multi-panel studio shell: focus/layers, tokens, capabilities, recipes.

use ratatui_core::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::{
    style::DesignSystem,

    style::{
        ColorCapability,
        Role,
        RolePalette,
    },
    text::take_display_cols,
};

/// Which inspector panel is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum InspectorPanel {
    /// Focus + layer strip.
    #[default]
    Focus,
    /// Scene layers list.
    Layers,
    /// Token / density / capability.
    Tokens,
    /// Recipe summary.
    Recipes,
}

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
    /// Optional layer stack labels (top last).
    pub layers: &'a [&'a str],
    /// Optional recipe names visible this frame.
    pub recipes: &'a [&'a str],
    /// Selection chrome label.
    pub selection_chrome: &'a str,
}

impl Default for DesignInspectorFrame<'_> {
    fn default() -> Self {
        Self {
            focused: None,
            layer: None,
            capability: ColorCapability::Truecolor,
            density: "compact",
            layers: &[],
            recipes: &[],
            selection_chrome: "gutter",
        }
    }
}

/// Paints a studio inspector (multi-line when height > 1).
#[derive(Debug, Clone)]
pub struct DesignInspector<'a> {
    frame: DesignInspectorFrame<'a>,
    system: &'a DesignSystem,
    panel: InspectorPanel,
}

impl<'a> DesignInspector<'a> {
    /// Creates an inspector for the given snapshot.
    #[must_use]
    pub const fn new(frame: DesignInspectorFrame<'a>, system: &'a DesignSystem) -> Self {
        Self {
            frame,
            system,
            panel: InspectorPanel::Focus,
        }
    }

    /// Selects which panel content to emphasize.
    #[must_use]
    pub const fn panel(mut self, panel: InspectorPanel) -> Self {
        self.panel = panel;
        self
    }

    /// Convenience from a design system.
    #[must_use]
    pub fn from_system(system: &'a DesignSystem, frame: DesignInspectorFrame<'a>) -> Self {
        Self {
            frame,
            system,
            panel: InspectorPanel::Focus,
        }
    }
}

impl Widget for &DesignInspector<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let style = self.system.style(Role::TextMuted);
        let strong = self.system.style(Role::TextStrong);

        // Tab strip on row 0 when height >= 2.
        let body_y = if area.height >= 2 {
            let tabs = " F:focus L:layers T:tokens R:recipes ";
            let clipped = take_display_cols(tabs, usize::from(area.width));
            buffer.set_stringn(area.x, area.y, &clipped, usize::from(area.width), strong);
            area.y.saturating_add(1)
        } else {
            area.y
        };
        let body_h = area.bottom().saturating_sub(body_y);
        if body_h == 0 {
            return;
        }

        let lines: Vec<String> = match self.panel {
            InspectorPanel::Focus => {
                let focus = self.frame.focused.unwrap_or("—");
                let layer = self.frame.layer.unwrap_or("root");
                vec![format!(
                    "focus:{focus} layer:{layer} dens:{} cap:{:?} sel:{}",
                    self.frame.density, self.frame.capability, self.frame.selection_chrome
                )]
            }
            InspectorPanel::Layers => {
                if self.frame.layers.is_empty() {
                    vec![format!("layers: {}", self.frame.layer.unwrap_or("root"))]
                } else {
                    self.frame
                        .layers
                        .iter()
                        .enumerate()
                        .map(|(i, id)| format!("{i}:{id}"))
                        .collect()
                }
            }
            InspectorPanel::Tokens => vec![format!(
                "density:{} capability:{:?} chrome:{}",
                self.frame.density, self.frame.capability, self.frame.selection_chrome
            )],
            InspectorPanel::Recipes => {
                if self.frame.recipes.is_empty() {
                    vec!["recipes: list_row panel".into()]
                } else {
                    self.frame
                        .recipes
                        .iter()
                        .map(|r| (*r).to_string())
                        .collect()
                }
            }
        };

        for (i, line) in lines.into_iter().take(usize::from(body_h)).enumerate() {
            let y = body_y.saturating_add(u16::try_from(i).unwrap_or(u16::MAX));
            let clipped = take_display_cols(&line, usize::from(area.width));
            buffer.set_stringn(area.x, y, &clipped, usize::from(area.width), style);
        }
    }
}

impl Widget for DesignInspector<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        Widget::render(&self, area, buffer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::RolePalette;

    #[test]
    fn studio_shell_paints_tab_and_layers() {
        let theme = RolePalette::default();
        let system = crate::style::DesignSystem::from_palette(theme.clone());
        let layers = ["root", "approval"];
        let frame = DesignInspectorFrame {
            focused: Some("prompt"),
            layer: Some("approval"),
            capability: ColorCapability::Ansi16,
            density: "comfortable",
            layers: &layers,
            recipes: &["list_row", "panel"],
            selection_chrome: "gutter",
        };
        let area = Rect::new(0, 0, 40, 4);
        let mut buffer = Buffer::empty(area);
        Widget::render(
            DesignInspector::new(frame, &system).panel(InspectorPanel::Layers),
            area,
            &mut buffer,
        );
        let text: String = buffer
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(text.contains("layers") || text.contains("root") || text.contains("F:focus"));
        assert!(text.contains("approval") || text.contains("0:root"));
    }
}
