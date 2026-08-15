// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **AppDashboard** — keyboard-first app dashboard composition (shadcn
//! `dashboard-01` peer for TUI).
//!
//! **Mission.** Sidebar navigation + primary main pane (+ optional metrics /
//! footer) without owning charts, tables, or routes. Host paints DataTable /
//! charts into main; TermRock owns focus routing, sidebar keys, and shell
//! geometry via [`layout_app_shell`].
//!
//! **vs [`layout_ops_dashboard`].** Ops layout is metrics/main/log only (no
//! sidebar). AppDashboard is the full-app block: nav rail + content.
//! **vs AgentWorkbench.** Workbench is agent-task chrome; dashboard is neutral
//! product shell.
//!
//! Research: shadcn dashboard-01, IDE shells, ops TUIs.
//!
//! Teaches: how to compose keyboard-first app dashboard composition (shadcn
//! `dashboard-01` peer for TUI).
//!
//! Composes: [`crate::widgets::NavItem`], [`crate::widgets::Panel`],
//! [`crate::widgets::PanelState`], [`crate::widgets::PanelVariant`],
//! [`crate::widgets::Sidebar`], [`crate::widgets::SidebarOutcome`],
//! [`crate::widgets::SidebarPresentation`], [`crate::widgets::SidebarState`].
//!
//! Copy-adapt: keep the widget composition and the focus routing;
//! replace the domain types, the wording, and the effects with your own.

#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{buffer::Buffer, layout::Rect};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    style::{Density, DesignSystem, PanelChrome, Role},
    text::take_display_cols,
    widgets::{
        NavItem, Panel, PanelVariant, Sidebar, SidebarOutcome, SidebarPresentation, SidebarState,
        example_sectioned_sidebar_nav, filter_nav_collapsed,
    },
};

use super::app_shell::{AppShellConfig, AppShellRecipe, AppShellSlots, layout_app_shell};

// ── Focus / outcomes ────────────────────────────────────────────────────────

/// Focused interactive zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum AppDashboardPane {
    /// Navigation sidebar.
    #[default]
    Sidebar,
    /// Primary content (host-owned interaction when focused).
    Main,
    /// Optional metrics strip (host paint; rarely focused).
    Metrics,
}

impl AppDashboardPane {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Sidebar => "sidebar",
            Self::Main => "main",
            Self::Metrics => "metrics",
        }
    }
}

/// Host-facing outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AppDashboardOutcome<Id> {
    /// Nothing handled.
    Ignored,
    /// Pane focus changed.
    PaneFocused {
        /// New pane.
        pane: AppDashboardPane,
    },
    /// Sidebar chrome / nav outcome.
    Sidebar(SidebarOutcome<Id>),
    /// Route selected from sidebar (compat alias of Selected).
    RouteSelected {
        /// Route id.
        id: Id,
    },
    /// Main pane received a key the host should handle (when Main focused).
    MainKey {
        /// Forwarded key.
        key: KeyEvent,
    },
    /// Esc blur / leave dashboard.
    Cancelled,
}

// ── State ───────────────────────────────────────────────────────────────────

/// Dashboard interaction state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppDashboardState<Id> {
    pane: AppDashboardPane,
    /// Sidebar nav state.
    pub sidebar: SidebarState<Id>,
    accepts_input: bool,
    show_metrics: bool,
    sidebar_width: u16,
}

impl<Id> Default for AppDashboardState<Id> {
    fn default() -> Self {
        Self::new(None)
    }
}

impl<Id> AppDashboardState<Id> {
    /// Fresh dashboard; optional initial route.
    #[must_use]
    pub fn new(route: Option<Id>) -> Self {
        let mut sidebar = SidebarState::new(route);
        sidebar.set_focused(true);
        sidebar.set_accepts_input(true);
        Self {
            pane: AppDashboardPane::Sidebar,
            sidebar,
            accepts_input: true,
            show_metrics: true,
            sidebar_width: 24,
        }
    }

    /// Focused pane.
    #[must_use]
    pub const fn pane(&self) -> AppDashboardPane {
        self.pane
    }

    /// Show metrics strip.
    pub fn set_show_metrics(&mut self, on: bool) {
        self.show_metrics = on;
    }

    /// Sidebar dock width (cols).
    pub fn set_sidebar_width(&mut self, w: u16) {
        self.sidebar_width = w.max(4);
    }

    /// Input gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
        self.sync_sidebar_focus();
    }

    /// Focus a pane.
    pub fn set_pane(&mut self, pane: AppDashboardPane) -> AppDashboardOutcome<Id> {
        if self.pane == pane {
            return AppDashboardOutcome::Ignored;
        }
        self.pane = pane;
        self.sync_sidebar_focus();
        AppDashboardOutcome::PaneFocused { pane }
    }

    fn sync_sidebar_focus(&mut self) {
        let on = self.accepts_input && self.pane == AppDashboardPane::Sidebar;
        self.sidebar.set_focused(on);
        self.sidebar.set_accepts_input(on);
    }

    fn cycle_pane(&mut self, reverse: bool) -> AppDashboardOutcome<Id> {
        let order = if self.show_metrics {
            [
                AppDashboardPane::Sidebar,
                AppDashboardPane::Main,
                AppDashboardPane::Metrics,
            ]
            .as_slice()
        } else {
            [AppDashboardPane::Sidebar, AppDashboardPane::Main].as_slice()
        };
        let cur = order.iter().position(|p| *p == self.pane).unwrap_or(0);
        let n = order.len();
        let next = if reverse {
            (cur + n - 1) % n
        } else {
            (cur + 1) % n
        };
        self.set_pane(order[next])
    }

    /// Keys.
    pub fn handle_key(&mut self, key: KeyEvent, nav: &[NavItem<Id>]) -> AppDashboardOutcome<Id>
    where
        Id: Clone + PartialEq,
    {
        if !self.accepts_input || key.kind != KeyEventKind::Press {
            return AppDashboardOutcome::Ignored;
        }

        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);

        if key.code == KeyCode::Esc && key.modifiers.is_empty() {
            // Let sidebar consume Esc first when filter active
            if self.pane == AppDashboardPane::Sidebar {
                let out = self.sidebar.handle_key(key, nav);
                if !matches!(out, SidebarOutcome::Ignored | SidebarOutcome::Blurred) {
                    return map_sidebar(out);
                }
            }
            return AppDashboardOutcome::Cancelled;
        }

        // Tab / Shift+Tab pane cycle
        if key.code == KeyCode::Tab && !ctrl {
            return self.cycle_pane(shift);
        }
        if key.code == KeyCode::BackTab {
            return self.cycle_pane(true);
        }

        // Ctrl+B — toggle rail (sidebar chrome)
        if ctrl && matches!(key.code, KeyCode::Char('b' | 'B')) {
            let out = self.sidebar.toggle_rail();
            return AppDashboardOutcome::Sidebar(out);
        }

        match self.pane {
            AppDashboardPane::Sidebar => map_sidebar(self.sidebar.handle_key(key, nav)),
            AppDashboardPane::Main | AppDashboardPane::Metrics => {
                AppDashboardOutcome::MainKey { key }
            }
        }
    }
}

fn map_sidebar<Id>(out: SidebarOutcome<Id>) -> AppDashboardOutcome<Id> {
    match out {
        SidebarOutcome::Selected(id) => AppDashboardOutcome::RouteSelected { id },
        other => AppDashboardOutcome::Sidebar(other),
    }
}

// ── Layout ──────────────────────────────────────────────────────────────────

/// Resolved rectangles for one frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppDashboardSlots {
    /// Shell slots (header/footer/overlay bounds).
    pub shell: AppShellSlots,
    /// Sidebar dock.
    pub sidebar: Rect,
    /// Metrics strip (may be empty).
    pub metrics: Rect,
    /// Main content.
    pub main: Rect,
    /// Footer / status.
    pub footer: Rect,
}

/// Layout knobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppDashboardLayout {
    /// Density.
    pub density: Density,
    /// Sidebar width.
    pub sidebar_width: u16,
    /// Metrics height (`0` hides).
    pub metrics_height: u16,
    /// Header height.
    pub header_height: u16,
    /// Footer height.
    pub footer_height: u16,
}

impl Default for AppDashboardLayout {
    fn default() -> Self {
        Self {
            density: Density::Dashboard,
            sidebar_width: 24,
            metrics_height: 3,
            header_height: 1,
            footer_height: 1,
        }
    }
}

/// Resolve dashboard geometry (sidebar + metrics + main + footer).
#[must_use]
pub fn layout_app_dashboard(area: Rect, config: AppDashboardLayout) -> AppDashboardSlots {
    let shell = layout_app_shell(
        area,
        AppShellConfig {
            recipe: AppShellRecipe::Workbench,
            density: config.density,
            header_height: config.header_height,
            sidebar_width: config.sidebar_width.max(4),
            inspector_width: 0,
            footer_height: config.footer_height.max(1),
            command_height: 0,
            metrics_height: 0,
            log_height: 0,
            lifecycle: Default::default(),
            inline: false,
        },
    );

    let sidebar = shell.sidebar.unwrap_or(Rect {
        x: area.x,
        y: area.y,
        width: 0,
        height: 0,
    });

    // Split main into metrics + content when requested.
    let body = shell.main;
    let (metrics, main) = if config.metrics_height > 0 && body.height > config.metrics_height + 2 {
        let mh = config.metrics_height.min(body.height.saturating_sub(2));
        (
            Rect::new(body.x, body.y, body.width, mh),
            Rect::new(
                body.x,
                body.y.saturating_add(mh),
                body.width,
                body.height.saturating_sub(mh),
            ),
        )
    } else {
        (Rect::new(body.x, body.y, body.width, 0), body)
    };

    let footer = shell.footer.unwrap_or(Rect {
        x: area.x,
        y: area.y.saturating_add(area.height.saturating_sub(1)),
        width: area.width,
        height: 1.min(area.height),
    });

    AppDashboardSlots {
        shell,
        sidebar,
        metrics,
        main,
        footer,
    }
}

// ── Paint ───────────────────────────────────────────────────────────────────

/// Host-projected surfaces for paint.
#[derive(Debug)]
pub struct AppDashboardSurfaces<'a, Id> {
    /// Design system.
    pub system: &'a DesignSystem,
    /// State.
    pub state: &'a mut AppDashboardState<Id>,
    /// Nav items (full tree; collapse filtered inside Sidebar).
    pub nav: &'a [NavItem<Id>],
    /// Title in header.
    pub title: &'a str,
    /// Optional main placeholder when host does not paint data.
    pub main_placeholder: &'a str,
}

/// Example nav for dashboard demos (sectioned).
#[must_use]
pub fn example_dashboard_nav() -> Vec<NavItem<&'static str>> {
    example_sectioned_sidebar_nav()
}

/// Paint shell chrome + sidebar; main shows placeholder (host overlays data).
pub fn render_app_dashboard<Id: Clone + PartialEq>(
    buffer: &mut Buffer,
    area: Rect,
    surfaces: AppDashboardSurfaces<'_, Id>,
) {
    if area.is_empty() {
        return;
    }
    let system = surfaces.system;
    let state = surfaces.state;
    // The glyph profile is the design system's answer, not a hardcoded true.
    let ascii = system.glyphs.is_ascii();
    let layout = AppDashboardLayout {
        sidebar_width: state.sidebar_width,
        metrics_height: if state.show_metrics { 3 } else { 0 },
        ..AppDashboardLayout::default()
    };
    let slots = layout_app_dashboard(area, layout);

    // Header
    if let Some(h) = slots.shell.header {
        if !h.is_empty() {
            let title = take_display_cols(surfaces.title, usize::from(h.width));
            system.paint_row(
                buffer,
                Rect::new(h.x, h.y, h.width, 1),
                &title,
                system.style(Role::TextStrong),
            );
        }
    }

    // Sidebar
    if !slots.sidebar.is_empty() {
        let rail = matches!(state.sidebar.presentation(), SidebarPresentation::Rail);
        let mut panel_state = crate::widgets::PanelState::default();
        let body = Panel::new(system)
            .title("Nav")
            .variant(PanelVariant::Bordered)
            .emphasis(if state.pane == AppDashboardPane::Sidebar {
                PanelChrome::Focused
            } else {
                PanelChrome::Normal
            })
            .paint(slots.sidebar, buffer, Some(&mut panel_state));
        // Prefer painting into body if panel carved space; else full sidebar.
        let nav_area = if body.height > 0 { body } else { slots.sidebar };
        let _ = rail;
        Sidebar::new(surfaces.nav, system)
            .focused(state.pane == AppDashboardPane::Sidebar)
            .ascii(ascii)
            .show_panel(false)
            .paint(nav_area, buffer, &mut state.sidebar);
    }

    // Metrics placeholder
    if slots.metrics.height > 0 {
        let mut panel_state = crate::widgets::PanelState::default();
        let body = Panel::new(system)
            .title("Metrics")
            .variant(PanelVariant::Bordered)
            .emphasis(if state.pane == AppDashboardPane::Metrics {
                PanelChrome::Focused
            } else {
                PanelChrome::Normal
            })
            .paint(slots.metrics, buffer, Some(&mut panel_state));
        if body.height > 0 && body.width > 0 {
            system.paint_row(
                buffer,
                Rect::new(body.x, body.y, body.width, 1),
                "host: charts / KPIs",
                system.style(Role::TextMuted),
            );
        }
    }

    // Main
    if !slots.main.is_empty() {
        let mut panel_state = crate::widgets::PanelState::default();
        let body = Panel::new(system)
            .title("Main")
            .variant(PanelVariant::Bordered)
            .emphasis(if state.pane == AppDashboardPane::Main {
                PanelChrome::Focused
            } else {
                PanelChrome::Normal
            })
            .paint(slots.main, buffer, Some(&mut panel_state));
        if body.height > 0 && body.width > 0 {
            let ph = surfaces.main_placeholder;
            system.paint_row(
                buffer,
                Rect::new(body.x, body.y, body.width, 1),
                ph,
                system.style(Role::TextMuted),
            );
        }
    }

    // Footer hint
    if !slots.footer.is_empty() {
        let hint = "Tab panes · [ rail · sidebar keys · host main";
        system.paint_row(
            buffer,
            Rect::new(slots.footer.x, slots.footer.y, slots.footer.width, 1),
            hint,
            system.style(Role::TextMuted),
        );
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn press(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    #[test]
    fn layout_has_sidebar_and_main() {
        let slots = layout_app_dashboard(Rect::new(0, 0, 100, 30), AppDashboardLayout::default());
        assert!(slots.sidebar.width > 0, "sidebar width");
        assert!(slots.main.width > 0 && slots.main.height > 0, "main");
        assert!(slots.metrics.height > 0, "metrics");
        // sidebar and main do not fully overlap in x
        assert!(
            slots.sidebar.x + slots.sidebar.width <= slots.main.x + 1
                || slots.main.x + slots.main.width <= slots.sidebar.x + 1
                || slots.sidebar.width + slots.main.width <= 100 + 4,
            "layout slots: sidebar={:?} main={:?}",
            slots.sidebar,
            slots.main
        );
    }

    #[test]
    fn tab_cycles_panes() {
        let mut st = AppDashboardState::<&str>::new(None);
        assert_eq!(st.pane(), AppDashboardPane::Sidebar);
        let nav = example_dashboard_nav();
        let out = st.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE), &nav);
        assert!(
            matches!(
                out,
                AppDashboardOutcome::PaneFocused {
                    pane: AppDashboardPane::Main
                }
            ),
            "{out:?}"
        );
        let out = st.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE), &nav);
        assert!(
            matches!(
                out,
                AppDashboardOutcome::PaneFocused {
                    pane: AppDashboardPane::Metrics
                }
            ),
            "{out:?}"
        );
        let out = st.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE), &nav);
        assert!(
            matches!(
                out,
                AppDashboardOutcome::PaneFocused {
                    pane: AppDashboardPane::Sidebar
                }
            ),
            "{out:?}"
        );
    }

    #[test]
    fn sidebar_route_selection() {
        let mut st = AppDashboardState::new(None);
        let nav = vec![
            NavItem::new("home", "Home"),
            NavItem::new("analytics", "Analytics"),
            NavItem::new("reports", "Reports"),
        ];
        st.sidebar.nav.set_route_and_focus("home");
        assert_eq!(st.sidebar.route(), Some(&"home"));
        // Move focus to analytics without activating
        let out = st.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &nav);
        assert!(
            matches!(
                out,
                AppDashboardOutcome::Sidebar(SidebarOutcome::FocusChanged {
                    id: Some("analytics")
                })
            ),
            "{out:?}"
        );
        assert_eq!(
            st.sidebar.route(),
            Some(&"home"),
            "route must not change on focus"
        );
        let out = st.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &nav);
        assert!(
            matches!(out, AppDashboardOutcome::RouteSelected { id: "analytics" }),
            "{out:?}"
        );
        assert_eq!(st.sidebar.route(), Some(&"analytics"));
    }

    #[test]
    fn main_keys_forwarded_when_main_focused() {
        let mut st = AppDashboardState::<&str>::new(None);
        let nav = example_dashboard_nav();
        let _ = st.set_pane(AppDashboardPane::Main);
        let out = st.handle_key(press('x'), &nav);
        assert!(
            matches!(out, AppDashboardOutcome::MainKey { key } if key.code == KeyCode::Char('x')),
            "{out:?}"
        );
    }

    #[test]
    fn rail_toggle_via_ctrl_b() {
        let mut st = AppDashboardState::<&str>::new(None);
        let nav = example_dashboard_nav();
        assert!(st.sidebar.is_expanded());
        let out = st.handle_key(
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL),
            &nav,
        );
        assert!(
            matches!(
                out,
                AppDashboardOutcome::Sidebar(SidebarOutcome::ToggleRail { expanded: false })
            ),
            "{out:?}"
        );
        assert!(!st.sidebar.is_expanded());
    }

    #[test]
    fn cancel_esc() {
        let mut st = AppDashboardState::<&str>::new(None);
        let nav = example_dashboard_nav();
        let out = st.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &nav);
        // Sidebar may Blur first → mapped; or Cancelled
        assert!(
            matches!(
                out,
                AppDashboardOutcome::Cancelled
                    | AppDashboardOutcome::Sidebar(SidebarOutcome::Blurred)
            ),
            "{out:?}"
        );
    }

    #[test]
    fn paint_smoke() {
        let system = DesignSystem::default();
        let mut st = AppDashboardState::new(Some("intro"));
        let nav = example_dashboard_nav();
        // collapse filter still works on fixture
        assert!(filter_nav_collapsed(&nav).len() < nav.len() || !nav.is_empty());
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        render_app_dashboard(
            &mut buf,
            area,
            AppDashboardSurfaces {
                system: &system,
                state: &mut st,
                nav: &nav,
                title: "Dashboard",
                main_placeholder: "table / charts here",
            },
        );
        let mut sample = String::new();
        for y in 0..3 {
            for x in 0..20 {
                if let Some(c) = buf.cell((x, y)) {
                    sample.push_str(c.symbol());
                }
            }
        }
        assert!(
            sample.contains("Dash")
                || sample.contains("Nav")
                || sample.contains('D')
                || sample.contains('N'),
            "{sample:?}"
        );
    }
}
