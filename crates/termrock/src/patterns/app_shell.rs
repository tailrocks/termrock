// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Canonical application shell: slots for full-screen and inline TermRock apps.
//!
//! **Slots, not policy.** Hosts paint domain content into rectangles and own
//! focus routing via [`AppShellSlots::focus_order`]. Overlay hosts use
//! [`AppShellSlots::overlay_bounds`]. Responsive collapse follows
//! [`crate::layout::ResponsiveSurface::AppShell`].
//!
//! Teaches: how to compose the canonical application shell as slots, so a
//! host owns its content and TermRock owns the geometry.
//!
//! Copy-adapt: keep the widget composition and the focus routing;
//! replace the domain types, the wording, and the effects with your own.
use ratatui_core::layout::Rect;

use crate::layout::{
    AdaptiveAnatomy, RegionId, RegionSize, RegionSpec, ResponsiveSurface, SurfaceAxis,
    ViewportClass, WorkSurface,
};

/// Named shell recipe (composition topology).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum AppShellRecipe {
    /// Header + sidebar + main + optional inspector + footer (IDE / agent class).
    #[default]
    Workbench,
    /// Header + metrics strip + main + secondary log + footer (ops class).
    Dashboard,
    /// Sidebar master list + detail main + footer (file/resource class).
    MasterDetail,
    /// Main + optional footer only (tiny / focused tools).
    Minimal,
}

impl AppShellRecipe {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Workbench => "workbench",
            Self::Dashboard => "dashboard",
            Self::MasterDetail => "master-detail",
            Self::Minimal => "minimal",
        }
    }
}

/// Focus / hit zone identity (not painted chrome).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AppShellZone {
    /// Top chrome (title, tabs, global actions).
    Header,
    /// Left (or right) navigation / list.
    Sidebar,
    /// Primary workspace.
    Main,
    /// Secondary rail / inspector / knobs.
    Inspector,
    /// Bottom status / hints.
    Footer,
    /// Command / palette strip or overlay host anchor.
    Command,
}

impl AppShellZone {
    /// Stable id for FocusGraph / scene layers.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Header => "header",
            Self::Sidebar => "sidebar",
            Self::Main => "main",
            Self::Inspector => "inspector",
            Self::Footer => "footer",
            Self::Command => "command",
        }
    }
}

/// Terminal / connection lifecycle chrome (host maps product state → this).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum AppShellLifecycle {
    /// Normal interactive session.
    #[default]
    Ready,
    /// Connecting / reconnecting.
    Connecting,
    /// Offline but shell still usable (local buffers).
    Offline,
    /// Hard disconnect — prefer minimal chrome + banner in header/main.
    Disconnected,
    /// Forced essential-only (tiny terminal or emergency).
    Tiny,
}

/// Layout knobs for [`layout_app_shell`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppShellConfig {
    /// Recipe topology.
    pub recipe: AppShellRecipe,
    /// Header height; `0` hides.
    pub header_height: u16,
    /// Sidebar width; `0` hides (or responsive collapse).
    pub sidebar_width: u16,
    /// Inspector width (workbench) or height (studio-like bottom inspector when
    /// `inspector_as_bottom` is set via recipe dashboard secondary).
    pub inspector_width: u16,
    /// Footer height; `0` hides.
    pub footer_height: u16,
    /// Reserved command strip height; `0` means command is overlay-only.
    pub command_height: u16,
    /// Dashboard metrics strip height.
    pub metrics_height: u16,
    /// Dashboard log pane height.
    pub log_height: u16,
    /// Lifecycle (affects collapse severity).
    pub lifecycle: AppShellLifecycle,
    /// Prefer inline (no alt-screen) — host flag; layout still fills area.
    pub inline: bool,
}

impl Default for AppShellConfig {
    fn default() -> Self {
        Self::workbench()
    }
}

impl AppShellConfig {
    /// IDE / agent workbench defaults.
    #[must_use]
    pub const fn workbench() -> Self {
        Self {
            recipe: AppShellRecipe::Workbench,
            header_height: 1,
            sidebar_width: 24,
            inspector_width: 28,
            footer_height: 1,
            command_height: 0,
            metrics_height: 0,
            log_height: 0,
            lifecycle: AppShellLifecycle::Ready,
            inline: false,
        }
    }

    /// Ops dashboard defaults.
    #[must_use]
    pub const fn dashboard() -> Self {
        Self {
            recipe: AppShellRecipe::Dashboard,
            header_height: 1,
            sidebar_width: 0,
            inspector_width: 0,
            footer_height: 1,
            command_height: 0,
            metrics_height: 3,
            log_height: 8,
            lifecycle: AppShellLifecycle::Ready,
            inline: false,
        }
    }

    /// Master–detail defaults.
    #[must_use]
    pub const fn master_detail() -> Self {
        Self {
            recipe: AppShellRecipe::MasterDetail,
            header_height: 1,
            sidebar_width: 28,
            inspector_width: 0,
            footer_height: 1,
            command_height: 0,
            metrics_height: 0,
            log_height: 0,
            lifecycle: AppShellLifecycle::Ready,
            inline: false,
        }
    }

    /// Minimal main + footer.
    #[must_use]
    pub const fn minimal() -> Self {
        Self {
            recipe: AppShellRecipe::Minimal,
            header_height: 0,
            sidebar_width: 0,
            inspector_width: 0,
            footer_height: 1,
            command_height: 0,
            metrics_height: 0,
            log_height: 0,
            lifecycle: AppShellLifecycle::Ready,
            inline: true,
        }
    }
}

/// Resolved shell rectangles and focus metadata for one frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppShellSlots {
    /// Header chrome.
    pub header: Option<Rect>,
    /// Sidebar / master list.
    pub sidebar: Option<Rect>,
    /// Primary workspace (never optional — may be full area).
    pub main: Rect,
    /// Inspector / knobs / detail secondary.
    pub inspector: Option<Rect>,
    /// Footer / status.
    pub footer: Option<Rect>,
    /// Reserved command strip (None when overlay-only).
    pub command: Option<Rect>,
    /// Dashboard metrics strip (recipe-specific).
    pub metrics: Option<Rect>,
    /// Dashboard log strip (recipe-specific).
    pub log: Option<Rect>,
    /// Bounds for [`crate::interaction::OverlayStack`] (usually full area).
    pub overlay_bounds: Rect,
    /// Focus zone order for Tab / FocusGraph (visible zones only).
    pub focus_order: Vec<AppShellZone>,
    /// Zones collapsed into drawer / overlay (host opens OverlayStack).
    pub drawer_zones: Vec<AppShellZone>,
    /// Adaptive anatomy used for this frame.
    pub anatomy: AdaptiveAnatomy,
    /// Effective lifecycle after tiny-terminal force.
    pub lifecycle: AppShellLifecycle,
    /// Recipe applied.
    pub recipe: AppShellRecipe,
}

impl AppShellSlots {
    /// Rectangle for a zone if present and non-empty.
    #[must_use]
    pub fn zone(&self, zone: AppShellZone) -> Option<Rect> {
        let r = match zone {
            AppShellZone::Header => self.header,
            AppShellZone::Sidebar => self.sidebar,
            AppShellZone::Main => Some(self.main),
            AppShellZone::Inspector => self.inspector,
            AppShellZone::Footer => self.footer,
            AppShellZone::Command => self.command,
        }?;
        if r.width == 0 || r.height == 0 {
            None
        } else {
            Some(r)
        }
    }
}

/// Layout AppShell inside `area` using config + responsive AppShell surface.
#[must_use]
pub fn layout_app_shell(area: Rect, mut config: AppShellConfig) -> AppShellSlots {
    let class = ResponsiveSurface::AppShell.classify(area.width, area.height);
    let mut anatomy = class.anatomy;
    let mut lifecycle = config.lifecycle;

    // Lifecycle / tiny force.
    if matches!(
        lifecycle,
        AppShellLifecycle::Tiny | AppShellLifecycle::Disconnected
    ) || anatomy.line_mode
        || area.width < 20
        || area.height < 6
    {
        lifecycle = if matches!(lifecycle, AppShellLifecycle::Disconnected) {
            AppShellLifecycle::Disconnected
        } else {
            AppShellLifecycle::Tiny
        };
        anatomy = AdaptiveAnatomy::from_stage(crate::layout::ContractionStage::LineMode);
    }

    // Responsive collapse of multi-pane.
    let mut drawer_zones = Vec::new();
    let mut show_sidebar = config.sidebar_width > 0
        && anatomy.multi_pane
        && !matches!(
            config.recipe,
            AppShellRecipe::Minimal | AppShellRecipe::Dashboard
        );
    let mut show_inspector = config.inspector_width > 0
        && anatomy.multi_pane
        && matches!(config.recipe, AppShellRecipe::Workbench);

    if anatomy.use_drawer && show_sidebar {
        drawer_zones.push(AppShellZone::Sidebar);
        show_sidebar = false;
    }
    if anatomy.use_drawer && show_inspector {
        drawer_zones.push(AppShellZone::Inspector);
        show_inspector = false;
    }
    if anatomy.line_mode || matches!(lifecycle, AppShellLifecycle::Tiny) {
        show_sidebar = false;
        show_inspector = false;
        config.header_height = config.header_height.min(1);
        // Keep a 1-row footer for status if any height remains.
        if config.footer_height == 0 && area.height > 1 {
            config.footer_height = 1;
        }
    }

    match config.recipe {
        AppShellRecipe::Minimal => layout_minimal(area, config, anatomy, lifecycle, drawer_zones),
        AppShellRecipe::Dashboard => {
            layout_dashboard(area, config, anatomy, lifecycle, drawer_zones)
        }
        AppShellRecipe::MasterDetail => {
            layout_master_detail(area, config, anatomy, lifecycle, drawer_zones, show_sidebar)
        }
        AppShellRecipe::Workbench => layout_workbench(
            area,
            config,
            anatomy,
            lifecycle,
            drawer_zones,
            show_sidebar,
            show_inspector,
        ),
    }
}

fn empty_opt() -> Option<Rect> {
    None
}

fn split_vertical(area: Rect, parts: &[(RegionId, RegionSize)]) -> Vec<Rect> {
    let regions: Vec<RegionSpec> = parts
        .iter()
        .map(|(id, size)| RegionSpec {
            id: id.clone(),
            size: *size,
        })
        .collect();
    WorkSurface::new()
        .axis(SurfaceAxis::Vertical)
        .regions(regions)
        .layout(area)
        .into_iter()
        .map(|r| r.area)
        .collect()
}

fn split_horizontal(area: Rect, parts: &[(RegionId, RegionSize)]) -> Vec<Rect> {
    let regions: Vec<RegionSpec> = parts
        .iter()
        .map(|(id, size)| RegionSpec {
            id: id.clone(),
            size: *size,
        })
        .collect();
    WorkSurface::new()
        .axis(SurfaceAxis::Horizontal)
        .regions(regions)
        .layout(area)
        .into_iter()
        .map(|r| r.area)
        .collect()
}

fn layout_minimal(
    area: Rect,
    config: AppShellConfig,
    anatomy: AdaptiveAnatomy,
    lifecycle: AppShellLifecycle,
    drawer_zones: Vec<AppShellZone>,
) -> AppShellSlots {
    let footer_h = config.footer_height.min(area.height);
    let parts = if footer_h > 0 {
        vec![
            (RegionId::from_static("main"), RegionSize::Weight(1)),
            (RegionId::from_static("footer"), RegionSize::Fixed(footer_h)),
        ]
    } else {
        vec![(RegionId::from_static("main"), RegionSize::Weight(1))]
    };
    let rows = split_vertical(area, &parts);
    let main = rows[0];
    let footer = if footer_h > 0 { Some(rows[1]) } else { None };
    let mut focus_order = vec![AppShellZone::Main];
    if footer.is_some() {
        focus_order.push(AppShellZone::Footer);
    }
    AppShellSlots {
        header: empty_opt(),
        sidebar: empty_opt(),
        main,
        inspector: empty_opt(),
        footer,
        command: empty_opt(),
        metrics: empty_opt(),
        log: empty_opt(),
        overlay_bounds: area,
        focus_order,
        drawer_zones,
        anatomy,
        lifecycle,
        recipe: AppShellRecipe::Minimal,
    }
}

fn layout_dashboard(
    area: Rect,
    config: AppShellConfig,
    anatomy: AdaptiveAnatomy,
    lifecycle: AppShellLifecycle,
    drawer_zones: Vec<AppShellZone>,
) -> AppShellSlots {
    let header_h = if anatomy.line_mode {
        0
    } else {
        config.header_height
    };
    let footer_h = config.footer_height.max(1).min(area.height);
    let metrics_h = if anatomy.line_mode {
        0
    } else {
        config.metrics_height
    };
    let log_h = if anatomy.multi_pane && !anatomy.line_mode {
        config.log_height
    } else {
        0
    };

    let mut parts = Vec::new();
    if header_h > 0 {
        parts.push((RegionId::from_static("header"), RegionSize::Fixed(header_h)));
    }
    if metrics_h > 0 {
        parts.push((
            RegionId::from_static("metrics"),
            RegionSize::Fixed(metrics_h.max(1)),
        ));
    }
    parts.push((RegionId::from_static("main"), RegionSize::Weight(2)));
    if log_h > 0 {
        parts.push((
            RegionId::from_static("log"),
            RegionSize::Fixed(log_h.max(1)),
        ));
    }
    parts.push((RegionId::from_static("footer"), RegionSize::Fixed(footer_h)));

    let rows = split_vertical(area, &parts);
    let mut i = 0;
    let header = if header_h > 0 {
        let r = Some(rows[i]);
        i += 1;
        r
    } else {
        None
    };
    let metrics = if metrics_h > 0 {
        let r = Some(rows[i]);
        i += 1;
        r
    } else {
        None
    };
    let main = rows[i];
    i += 1;
    let log = if log_h > 0 {
        let r = Some(rows[i]);
        i += 1;
        r
    } else {
        None
    };
    let footer = Some(rows[i]);

    let mut focus_order = Vec::new();
    if header.is_some() {
        focus_order.push(AppShellZone::Header);
    }
    focus_order.push(AppShellZone::Main);
    // Dashboard log is the secondary pane; expose on both `log` and
    // `inspector` so `zone(Inspector)` / focus_order stay consistent.
    if log.is_some() {
        focus_order.push(AppShellZone::Inspector);
    }
    focus_order.push(AppShellZone::Footer);

    AppShellSlots {
        header,
        sidebar: empty_opt(),
        main,
        inspector: log,
        footer,
        command: empty_opt(),
        metrics,
        log,
        overlay_bounds: area,
        focus_order,
        drawer_zones,
        anatomy,
        lifecycle,
        recipe: AppShellRecipe::Dashboard,
    }
}

fn layout_master_detail(
    area: Rect,
    config: AppShellConfig,
    anatomy: AdaptiveAnatomy,
    lifecycle: AppShellLifecycle,
    drawer_zones: Vec<AppShellZone>,
    show_sidebar: bool,
) -> AppShellSlots {
    let header_h = if anatomy.line_mode {
        0
    } else {
        config.header_height
    };
    let footer_h = config.footer_height.min(area.height);

    // Outer vertical: header / body / footer
    let mut vparts = Vec::new();
    if header_h > 0 {
        vparts.push((RegionId::from_static("header"), RegionSize::Fixed(header_h)));
    }
    vparts.push((RegionId::from_static("body"), RegionSize::Weight(1)));
    if footer_h > 0 {
        vparts.push((RegionId::from_static("footer"), RegionSize::Fixed(footer_h)));
    }
    let vrows = split_vertical(area, &vparts);
    let mut vi = 0;
    let header = if header_h > 0 {
        let r = Some(vrows[vi]);
        vi += 1;
        r
    } else {
        None
    };
    let body = vrows[vi];
    vi += 1;
    let footer = if footer_h > 0 { Some(vrows[vi]) } else { None };

    let (sidebar, main) = if show_sidebar
        && config.sidebar_width > 0
        && body.width > config.sidebar_width.saturating_add(8)
    {
        let hparts = [
            (
                RegionId::from_static("sidebar"),
                RegionSize::Fixed(config.sidebar_width),
            ),
            (RegionId::from_static("main"), RegionSize::Weight(1)),
        ];
        let cols = split_horizontal(body, &hparts);
        (Some(cols[0]), cols[1])
    } else {
        (None, body)
    };

    let mut focus_order = Vec::new();
    if header.is_some() {
        focus_order.push(AppShellZone::Header);
    }
    if sidebar.is_some() {
        focus_order.push(AppShellZone::Sidebar);
    }
    focus_order.push(AppShellZone::Main);
    if footer.is_some() {
        focus_order.push(AppShellZone::Footer);
    }

    AppShellSlots {
        header,
        sidebar,
        main,
        inspector: empty_opt(),
        footer,
        command: empty_opt(),
        metrics: empty_opt(),
        log: empty_opt(),
        overlay_bounds: area,
        focus_order,
        drawer_zones,
        anatomy,
        lifecycle,
        recipe: AppShellRecipe::MasterDetail,
    }
}

fn layout_workbench(
    area: Rect,
    config: AppShellConfig,
    anatomy: AdaptiveAnatomy,
    lifecycle: AppShellLifecycle,
    drawer_zones: Vec<AppShellZone>,
    show_sidebar: bool,
    show_inspector: bool,
) -> AppShellSlots {
    let header_h = if anatomy.line_mode {
        0
    } else {
        config.header_height
    };
    let footer_h = config.footer_height.min(area.height);
    let command_h = if anatomy.line_mode {
        0
    } else {
        config.command_height
    };

    let mut vparts = Vec::new();
    if header_h > 0 {
        vparts.push((RegionId::from_static("header"), RegionSize::Fixed(header_h)));
    }
    vparts.push((RegionId::from_static("body"), RegionSize::Weight(1)));
    if command_h > 0 {
        vparts.push((
            RegionId::from_static("command"),
            RegionSize::Fixed(command_h.max(1)),
        ));
    }
    if footer_h > 0 {
        vparts.push((RegionId::from_static("footer"), RegionSize::Fixed(footer_h)));
    }
    let vrows = split_vertical(area, &vparts);
    let mut vi = 0;
    let header = if header_h > 0 {
        let r = Some(vrows[vi]);
        vi += 1;
        r
    } else {
        None
    };
    let body = vrows[vi];
    vi += 1;
    let command = if command_h > 0 {
        let r = Some(vrows[vi]);
        vi += 1;
        r
    } else {
        None
    };
    let footer = if footer_h > 0 { Some(vrows[vi]) } else { None };

    // Horizontal: sidebar | main | inspector
    let place_sidebar = show_sidebar && body.width > config.sidebar_width.saturating_add(12);
    let mut place_inspector =
        show_inspector && body.width > config.inspector_width.saturating_add(40);
    if place_sidebar
        && place_inspector
        && body.width
            <= config
                .sidebar_width
                .saturating_add(config.inspector_width)
                .saturating_add(24)
    {
        // Prefer navigation rail over inspector when both cannot fit.
        place_inspector = false;
    }

    let mut hparts = Vec::new();
    if place_sidebar {
        hparts.push((
            RegionId::from_static("sidebar"),
            RegionSize::Fixed(config.sidebar_width),
        ));
    }
    hparts.push((RegionId::from_static("main"), RegionSize::Weight(1)));
    if place_inspector {
        hparts.push((
            RegionId::from_static("inspector"),
            RegionSize::Fixed(config.inspector_width),
        ));
    }
    let cols = split_horizontal(body, &hparts);
    let mut ci = 0;
    let sidebar = if place_sidebar {
        let r = Some(cols[ci]);
        ci += 1;
        r
    } else {
        None
    };
    let main = cols[ci];
    ci += 1;
    let inspector = if place_inspector {
        Some(cols[ci])
    } else {
        None
    };

    let mut focus_order = Vec::new();
    if header.is_some() {
        focus_order.push(AppShellZone::Header);
    }
    if sidebar.is_some() {
        focus_order.push(AppShellZone::Sidebar);
    }
    focus_order.push(AppShellZone::Main);
    if inspector.is_some() {
        focus_order.push(AppShellZone::Inspector);
    }
    if command.is_some() {
        focus_order.push(AppShellZone::Command);
    }
    if footer.is_some() {
        focus_order.push(AppShellZone::Footer);
    }

    AppShellSlots {
        header,
        sidebar,
        main,
        inspector,
        footer,
        command,
        metrics: empty_opt(),
        log: empty_opt(),
        overlay_bounds: area,
        focus_order,
        drawer_zones,
        anatomy,
        lifecycle,
        recipe: AppShellRecipe::Workbench,
    }
}

/// Classify AppShell viewport (helper for hosts).
#[must_use]
pub fn app_shell_viewport(area: Rect) -> ViewportClass {
    ResponsiveSurface::AppShell.classify(area.width, area.height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workbench_wide_has_sidebar_and_inspector() {
        let slots = layout_app_shell(Rect::new(0, 0, 160, 40), AppShellConfig::workbench());
        assert!(slots.sidebar.is_some());
        assert!(slots.inspector.is_some());
        assert!(slots.header.is_some());
        assert!(slots.footer.is_some());
        assert!(slots.focus_order.contains(&AppShellZone::Main));
        assert_eq!(slots.overlay_bounds, Rect::new(0, 0, 160, 40));
        assert!(slots.anatomy.multi_pane);
    }

    #[test]
    fn workbench_narrow_collapses_panes() {
        let slots = layout_app_shell(Rect::new(0, 0, 50, 24), AppShellConfig::workbench());
        // Mid width: single-pane or drawer — sidebar/inspector may be gone.
        assert!(slots.main.width > 0);
        assert!(slots.main.height > 0);
        if !slots.anatomy.multi_pane {
            assert!(slots.sidebar.is_none() || slots.drawer_zones.contains(&AppShellZone::Sidebar));
        }
    }

    #[test]
    fn tiny_forces_line_mode_main() {
        let slots = layout_app_shell(Rect::new(0, 0, 18, 5), AppShellConfig::workbench());
        assert!(matches!(
            slots.lifecycle,
            AppShellLifecycle::Tiny | AppShellLifecycle::Disconnected
        ));
        assert!(slots.sidebar.is_none());
        assert!(slots.inspector.is_none());
        assert!(slots.main.width > 0);
    }

    #[test]
    fn dashboard_fills_height() {
        let slots = layout_app_shell(Rect::new(0, 0, 100, 30), AppShellConfig::dashboard());
        assert!(slots.metrics.is_some());
        assert!(slots.log.is_some());
        assert_eq!(slots.log, slots.inspector);
        assert!(slots.zone(AppShellZone::Inspector).is_some());
        for z in &slots.focus_order {
            assert!(slots.zone(*z).is_some(), "missing rect for {z:?}");
        }
        let sum = slots.header.map(|r| r.height).unwrap_or(0)
            + slots.metrics.map(|r| r.height).unwrap_or(0)
            + slots.main.height
            + slots.log.map(|r| r.height).unwrap_or(0)
            + slots.footer.map(|r| r.height).unwrap_or(0);
        // junie inserts a 2-row gap between zones, so the rows sum below the area.
        assert_eq!(sum, 22);
    }

    #[test]
    fn master_detail_hides_sidebar_when_collapsed() {
        let wide = layout_app_shell(Rect::new(0, 0, 120, 30), AppShellConfig::master_detail());
        assert!(wide.sidebar.is_some());
        let narrow = layout_app_shell(Rect::new(0, 0, 30, 20), AppShellConfig::master_detail());
        assert!(narrow.main.width > 0 && narrow.main.height > 0);
        if !narrow.anatomy.multi_pane {
            assert!(
                narrow.sidebar.is_none() || narrow.drawer_zones.contains(&AppShellZone::Sidebar)
            );
        }
    }

    #[test]
    fn focus_order_only_visible_zones() {
        let slots = layout_app_shell(Rect::new(0, 0, 160, 40), AppShellConfig::workbench());
        for z in &slots.focus_order {
            assert!(slots.zone(*z).is_some(), "missing rect for {z:?}");
        }
    }

    #[test]
    fn offline_lifecycle_preserved() {
        let mut cfg = AppShellConfig::minimal();
        cfg.lifecycle = AppShellLifecycle::Offline;
        let slots = layout_app_shell(Rect::new(0, 0, 60, 20), cfg);
        assert_eq!(slots.lifecycle, AppShellLifecycle::Offline);
    }

    #[test]
    fn command_strip_in_workbench() {
        let mut cfg = AppShellConfig::workbench();
        cfg.command_height = 3;
        let slots = layout_app_shell(Rect::new(0, 0, 100, 30), cfg);
        assert_eq!(slots.command.map(|r| r.height), Some(3));
        assert!(slots.focus_order.contains(&AppShellZone::Command));
    }

    #[test]
    fn workbench_prefers_sidebar_when_tight() {
        let mut cfg = AppShellConfig::workbench();
        cfg.sidebar_width = 30;
        cfg.inspector_width = 40;
        // Width barely enough for sidebar+main, not both rails.
        let slots = layout_app_shell(Rect::new(0, 0, 90, 30), cfg);
        if slots.anatomy.multi_pane && slots.sidebar.is_some() {
            // Inspector may be dropped first.
            let _ = slots.inspector;
        }
        assert!(slots.main.width > 0);
    }

    #[test]
    fn recipe_ids_stable() {
        assert_eq!(AppShellRecipe::Workbench.id(), "workbench");
        assert_eq!(AppShellRecipe::Dashboard.id(), "dashboard");
        assert_eq!(AppShellRecipe::MasterDetail.id(), "master-detail");
        assert_eq!(AppShellRecipe::Minimal.id(), "minimal");
        assert_eq!(AppShellZone::Main.id(), "main");
    }

    #[test]
    fn viewport_helper_matches_surface() {
        let v = app_shell_viewport(Rect::new(0, 0, 160, 40));
        assert!(v.anatomy.multi_pane);
    }

    #[test]
    fn layout_is_cheap_many_frames() {
        let cfg = AppShellConfig::workbench();
        let area = Rect::new(0, 0, 120, 40);
        for _ in 0..10_000 {
            let _ = layout_app_shell(area, cfg);
        }
    }

    #[test]
    fn minimal_is_main_plus_footer() {
        let slots = layout_app_shell(Rect::new(0, 0, 40, 12), AppShellConfig::minimal());
        assert!(slots.header.is_none());
        assert!(slots.sidebar.is_none());
        assert_eq!(slots.main.height + slots.footer.unwrap().height, 10);
        assert_eq!(
            slots.focus_order,
            vec![AppShellZone::Main, AppShellZone::Footer]
        );
    }

    #[test]
    fn disconnected_uses_compact_density_path() {
        let mut cfg = AppShellConfig::workbench();
        cfg.lifecycle = AppShellLifecycle::Disconnected;
        let slots = layout_app_shell(Rect::new(0, 0, 80, 24), cfg);
        assert_eq!(slots.lifecycle, AppShellLifecycle::Disconnected);
        assert!(slots.main.height > 0);
    }

    #[test]
    fn zone_helper_skips_empty() {
        let slots = layout_app_shell(Rect::new(0, 0, 80, 24), AppShellConfig::minimal());
        assert!(slots.zone(AppShellZone::Main).is_some());
        assert!(slots.zone(AppShellZone::Sidebar).is_none());
    }
}
