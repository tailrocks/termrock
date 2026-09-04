// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Lookbook / Studio shell geometry: preview + multi-panel design inspector.
//!
//! Product-neutral layout only. Story content and knobs stay caller-owned.
//! Built on AppShell Workbench (knobs = inspector rail) with a bottom
//! inspector band subdivided from main.
//!
//! Teaches: how to compose the lookbook shell's geometry — a preview pane and
//! a multi-panel design inspector.
//!
//! Copy-adapt: keep the widget composition and the focus routing;
//! replace the domain types, the wording, and the effects with your own.
use ratatui_core::layout::Rect;

use crate::layout::{RegionId, RegionSize, SurfaceAxis, WorkSurface};
use ratatui_core::{buffer::Buffer, widgets::StatefulWidget, widgets::Widget};

use crate::style::{DesignSystem, PanelChrome, Role};
use crate::widgets::{
    DesignInspector, DesignInspectorFrame, InspectorPanel, Panel, StatusBar, StatusBarState,
    StatusSlot,
};

use super::app_shell::{AppShellConfig, AppShellRecipe, layout_app_shell};

/// Studio shell regions for one frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudioShellSlots {
    /// Main component preview.
    pub preview: Rect,
    /// Design inspector (focus/layers/tokens/recipes).
    pub inspector: Rect,
    /// Optional knobs column (None when width too narrow).
    pub knobs: Option<Rect>,
    /// Hint / status strip.
    pub status: Rect,
}

/// Layout knobs for [`layout_studio_shell`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StudioShellLayout {
    /// Inspector height (bottom band).
    pub inspector_height: u16,
    /// Knobs width (right rail); 0 hides knobs.
    pub knobs_width: u16,
    /// Status height.
    pub status_height: u16,
}

impl Default for StudioShellLayout {
    fn default() -> Self {
        Self {
            inspector_height: 4,
            knobs_width: 24,
            status_height: 1,
        }
    }
}

/// Resolves studio shell rectangles (preview + inspector + optional knobs).
#[must_use]
pub fn layout_studio_shell(area: Rect, config: StudioShellLayout) -> StudioShellSlots {
    let shell = layout_app_shell(
        area,
        AppShellConfig {
            recipe: AppShellRecipe::Workbench,
            header_height: 0,
            sidebar_width: 0,
            inspector_width: config.knobs_width,
            footer_height: config.status_height.max(1),
            command_height: 0,
            metrics_height: 0,
            log_height: 0,
            lifecycle: Default::default(),
            inline: false,
        },
    );

    let status = shell.footer.unwrap_or(Rect {
        x: area.x,
        y: area.y.saturating_add(area.height.saturating_sub(1)),
        width: area.width,
        height: 1.min(area.height),
    });
    let knobs = shell.inspector;
    let body = shell.main;

    let inspector_h = config
        .inspector_height
        .min(body.height.saturating_sub(3))
        .max(1);
    let rows = WorkSurface::new()
        .axis(SurfaceAxis::Vertical)
        .regions([
            crate::layout::RegionSpec {
                id: RegionId::from_static("preview"),
                size: RegionSize::Weight(1),
            },
            crate::layout::RegionSpec {
                id: RegionId::from_static("inspector"),
                size: RegionSize::Fixed(inspector_h),
            },
        ])
        .layout(body);

    StudioShellSlots {
        preview: rows[0].area,
        inspector: rows[1].area,
        knobs,
        status,
    }
}

// ── Reference paint ─────────────────────────────────────────────────────────

/// Host-owned content for one studio shell frame.
#[derive(Debug, Clone)]
pub struct StudioShellView<'a> {
    /// Title for the preview pane.
    pub preview_title: &'a str,
    /// Inspector snapshot.
    pub frame: DesignInspectorFrame<'a>,
    /// Which inspector panel is emphasized.
    pub panel: InspectorPanel,
    /// Knob rows (`label`, `value`) for the optional knobs column.
    pub knobs: &'a [(&'a str, &'a str)],
    /// Footer hints.
    pub hints: &'a [StatusSlot<'a, &'a str>],
}

/// Paints a reference studio shell over [`layout_studio_shell`]'s slots.
///
/// The preview pane is a [`Panel`] the host paints a story into (the returned
/// slots carry its inner rect), the inspector is [`DesignInspector`], and the
/// footer is a [`StatusBar`].
pub fn render_studio_shell(
    area: Rect,
    buffer: &mut Buffer,
    system: &DesignSystem,
    config: StudioShellLayout,
    view: StudioShellView<'_>,
    preview_focused: bool,
) -> StudioShellSlots {
    let slots = layout_studio_shell(area, config);

    if slots.preview.height > 0 {
        let inner = Panel::new(system)
            .title(view.preview_title)
            .emphasis(PanelChrome::for_focus(preview_focused))
            .paint(slots.preview, buffer, None);
        let _ = inner;
    }

    if slots.inspector.height > 0 {
        let inspector = DesignInspector::new(view.frame, system).panel(view.panel);
        Widget::render(&inspector, slots.inspector, buffer);
    }

    if let Some(knobs) = slots.knobs
        && knobs.height > 0
    {
        let body = Panel::new(system).title("Knobs").paint(knobs, buffer, None);
        for (i, (label, value)) in view.knobs.iter().take(usize::from(body.height)).enumerate() {
            let row = Rect::new(
                body.x,
                body.y.saturating_add(u16::try_from(i).unwrap_or(0)),
                body.width,
                1,
            );
            let separator = system.kv_separator().text();
            system.paint_row(
                buffer,
                row,
                &format!("{label}{separator}{value}"),
                system.style(Role::TextMuted),
            );
        }
    }

    if slots.status.height > 0 {
        let mut status = StatusBarState::new();
        StatusBar::new(view.hints, &[], system).render(slots.status, buffer, &mut status);
    }

    slots
}

#[cfg(test)]
mod tests {

    #[test]
    fn reference_paint_fills_preview_inspector_and_status() {
        use crate::style::DesignSystem;
        use crate::widgets::{DesignInspectorFrame, InspectorPanel, StatusSlot};
        use ratatui_core::buffer::Buffer;

        let system = DesignSystem::default();
        let hints = [StatusSlot::new("tab", "tab panel")];
        let knobs = [("density", "compact"), ("theme", "junie")];
        let view = StudioShellView {
            preview_title: "Button",
            frame: DesignInspectorFrame::default(),
            panel: InspectorPanel::Focus,
            knobs: &knobs,
            hints: &hints,
        };
        let area = Rect::new(0, 0, 100, 24);
        let mut buffer = Buffer::empty(area);
        let slots = render_studio_shell(
            area,
            &mut buffer,
            &system,
            StudioShellLayout::default(),
            view,
            true,
        );

        let painted = |rect: Rect| {
            (rect.x..rect.right()).any(|x| {
                (rect.y..rect.bottom()).any(|y| !buffer[(x, y)].symbol().trim().is_empty())
            })
        };
        assert!(painted(slots.preview), "preview painted nothing");
        assert!(painted(slots.inspector), "inspector painted nothing");
        assert!(painted(slots.status), "status painted nothing");
    }
    use super::*;

    #[test]
    fn studio_shell_hides_knobs_when_narrow() {
        let wide = layout_studio_shell(Rect::new(0, 0, 120, 40), StudioShellLayout::default());
        assert!(wide.knobs.is_some());
        assert!(wide.inspector.height >= 1);
        let narrow = layout_studio_shell(Rect::new(0, 0, 40, 20), StudioShellLayout::default());
        assert!(narrow.knobs.is_none());
        assert!(narrow.preview.width > 0);
    }
}
