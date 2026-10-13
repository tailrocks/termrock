// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Studio-oriented design inspector (lookbook/debug). Not a production shell.
//!
//! Multi-panel studio shell: focus/layers, tokens, capabilities, recipes.

#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::{
    style::DesignSystem,
    style::{ColorCapability, Role, RolePalette},
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
    /// Frame-local semantic tree summary.
    Semantics,
    /// Focus graph / Focus Lens summary.
    FocusGraph,
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
    /// Optional semantic-tree summary lines (from [`crate::interaction::SemanticSnapshot`]).
    pub semantics: &'a [&'a str],
    /// Optional FocusGraph / Focus Lens summary lines.
    pub focus_graph: &'a [&'a str],
}

impl DesignInspectorFrame<'_> {
    /// Reads the chrome facts off the system that is actually painting.
    ///
    /// The inspector used to hardcode `gutter`, `compact` and `Truecolor`, so
    /// it reported the same three answers whatever the host had configured —
    /// an inspector that cannot be trusted is worse than none (plans/011
    /// Step 4).
    #[must_use]
    pub fn from_system(system: &DesignSystem) -> Self {
        Self {
            capability: system.capability,
            density: system.density.id(),
            selection_chrome: system.selection.id(),
            ..Self::default()
        }
    }
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
            semantics: &[],
            focus_graph: &[],
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
            let tabs = " F:focus L:layers T:tokens R:recipes S:sem G:graph ";
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
            InspectorPanel::Semantics => {
                if self.frame.semantics.is_empty() {
                    vec![format!(
                        "semantics: {} nodes (register SemanticScene)",
                        self.frame.semantics.len()
                    )]
                } else {
                    self.frame
                        .semantics
                        .iter()
                        .map(|line| (*line).to_string())
                        .collect()
                }
            }
            InspectorPanel::FocusGraph => {
                if self.frame.focus_graph.is_empty() {
                    vec!["focus_graph: (register FocusGraph)".into()]
                } else {
                    self.frame
                        .focus_graph
                        .iter()
                        .map(|line| (*line).to_string())
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
            semantics: &["list@list [f] Files", "row0@list_item [fs] a.rs"],
            focus_graph: &["focus:list trap:—"],
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

    #[test]
    fn studio_semantics_panel_paints_snapshot_lines() {
        let system = crate::style::DesignSystem::phosphor();
        let lines = ["list@list [f] Files", "row0@list_item [fs] a.rs"];
        let frame = DesignInspectorFrame {
            semantics: &lines,
            ..DesignInspectorFrame::default()
        };
        let area = Rect::new(0, 0, 48, 4);
        let mut buffer = Buffer::empty(area);
        Widget::render(
            DesignInspector::new(frame, &system).panel(InspectorPanel::Semantics),
            area,
            &mut buffer,
        );
        let text: String = buffer
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(text.contains("list") || text.contains("sem") || text.contains("S:sem"));
        assert!(text.contains("row0") || text.contains("Files"));
    }
}
