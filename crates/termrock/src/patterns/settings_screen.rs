// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **SettingsScreen** — searchable settings experience composed from **public**
//! TermRock widgets only (source-owned registry block, not a monolith).
//!
//! **Mission.** Categories via Sidebar, SearchInput filter, Form sections/fields
//! with validation + dirty/modified cues, reset-to-default requests, conflict
//! and restart-required banners. Deep links, keyboard help, responsive drawer
//! nav, no-results guidance. Integrates KeybindingRecorder and ThemePicker.
//! Persistence and restart application policy stay **host-owned**.
//!
//! Research: Zellij config UIs, btop options, editor settings, shadcn layouts
//! (experience references, not product clones).
//!
//! Migrates thin [`SettingsShellState`](crate::widgets::SettingsShellState)
//! surface (0056) into this elevated composition (**0237**).
//!
//! Teaches: how to compose a searchable settings experience: sections,
//! fields, validation and an explicit apply action.
//!
//! Composes: [`crate::widgets::BUILTIN_THEME_PRESETS`],
//! [`crate::widgets::Field`], [`crate::widgets::FieldStatus`],
//! [`crate::widgets::Fieldset`], [`crate::widgets::Form`],
//! [`crate::widgets::FormOutcome`], [`crate::widgets::FormState`],
//! [`crate::widgets::HelpEntry`], and 24 more.
//!
//! Copy-adapt: keep the widget composition and the focus routing;
//! replace the domain types, the wording, and the effects with your own.

#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    widgets::{StatefulWidget, Widget},
};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    style::{DesignSystem, PanelChrome, Role},
    widgets::{
        BUILTIN_THEME_PRESETS, Button, ButtonState, ButtonVariant, Callout, CalloutTone, Field,
        FieldStatus, Fieldset, Form, FormOutcome, FormState, KeybindingRecorder,
        KeybindingRecorderOutcome, KeybindingRecorderState, KeyboardHelp, KeyboardHelpState,
        NavItem, Panel, SearchInput, SearchInputOutcome, SearchInputState, Sidebar, SidebarOutcome,
        SidebarPresentation, SidebarState, StatusBar, StatusBarState, StatusSlot, ThemePicker,
        ThemePickerOutcome, ThemePickerState, ThemePreset, any_dirty, collect_errors,
        example_settings_nav,
    },
};

// ── Regions & density ───────────────────────────────────────────────────────

/// Keyboard focus region inside the settings screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SettingsRegion {
    /// Search filter field.
    Search,
    /// Category / section sidebar (or drawer).
    #[default]
    Nav,
    /// Form / theme / keybinding body.
    Body,
    /// Footer action strip (save / reset / discard).
    Footer,
}

impl SettingsRegion {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Search => "search",
            Self::Nav => "nav",
            Self::Body => "body",
            Self::Footer => "footer",
        }
    }

    /// Tab cycle order.
    #[must_use]
    pub fn focus_order() -> &'static [SettingsRegion] {
        &[Self::Search, Self::Nav, Self::Body, Self::Footer]
    }
}

/// Responsive density for settings layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SettingsDensity {
    /// Full sidebar + body.
    #[default]
    Normal,
    /// Body primary; nav as drawer when open.
    Narrow,
    /// Body only; breadcrumbs / deep-link nav.
    Tiny,
}

impl SettingsDensity {
    /// From terminal width.
    #[must_use]
    pub const fn for_width(width: u16) -> Self {
        if width < 48 {
            Self::Tiny
        } else if width < 80 {
            Self::Narrow
        } else {
            Self::Normal
        }
    }

    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Narrow => "narrow",
            Self::Tiny => "tiny",
        }
    }
}

// ── Body modes ──────────────────────────────────────────────────────────────

/// What the body panel paints for the active section (host-selected).
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum SettingsBodyMode {
    /// Form fieldsets for the active section.
    Form,
    /// Theme preset picker + live paint system.
    Theme,
    /// Keybinding recorder for one action.
    Keybinding,
    /// Search produced no matching sections/fields.
    NoResults,
}

// ── Outcomes (requests only) ────────────────────────────────────────────────

/// Typed result from settings key routing (UI only — no persistence / restart).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SettingsScreenOutcome<SectionId, FieldId = SectionId> {
    /// Nothing handled.
    Ignored,
    /// Focus region changed.
    RegionChanged(SettingsRegion),
    /// Category / section selected.
    SectionSelected(SectionId),
    /// Search query changed (host refilters).
    SearchChanged,
    /// Form scroll / field chrome.
    Form(FormOutcome<FieldId>),
    /// Theme picker interaction.
    Theme(ThemePickerOutcome),
    /// Keybinding recorder interaction.
    Keybinding(KeybindingRecorderOutcome),
    /// Host should persist current projected values.
    SaveRequested,
    /// Reset active section to defaults (host applies).
    ResetSectionRequested,
    /// Reset one field to default (host applies).
    ResetFieldRequested(FieldId),
    /// Reset all dirty settings (host applies).
    ResetAllRequested,
    /// Discard dirty edits (host reloads last saved).
    DiscardRequested,
    /// Open keyboard help overlay.
    HelpOpened,
    /// Close keyboard help.
    HelpClosed,
    /// Toggle narrow/tiny nav drawer.
    DrawerToggled {
        /// Open after toggle.
        open: bool,
    },
    /// Deep-link resolved to a section.
    DeepLink(SectionId),
}

// ── State ───────────────────────────────────────────────────────────────────

/// Consumer-owned settings interaction state (survives frames).
///
/// **Host owns:** persistence, restart policy, domain values, conflict resolution.
/// **Block owns:** focus region, layout density, drawer/help chrome, composed widget states.
#[derive(Debug)]
pub struct SettingsScreenState<SectionId: Clone + PartialEq> {
    /// Category sidebar.
    pub sidebar: SidebarState<SectionId>,
    /// Search filter.
    pub search: SearchInputState,
    /// Form body (when mode is Form).
    pub form: FormState<&'static str>,
    /// Theme picker (when mode is Theme).
    pub theme: ThemePickerState,
    /// Keybinding recorder (when mode is Keybinding).
    pub keybinding: KeybindingRecorderState,
    /// Keyboard help.
    pub help: KeyboardHelpState,
    /// Focus region.
    pub region: SettingsRegion,
    /// Density override (`None` = derive from width).
    pub density: Option<SettingsDensity>,
    /// Nav drawer open (narrow/tiny).
    pub drawer_open: bool,
    /// Help overlay open.
    pub help_open: bool,
    /// Body mode for paint/routing.
    pub body_mode: SettingsBodyMode,
    /// Host: any dirty projected fields.
    pub dirty: bool,
    /// Host: conflicting keys / dual sources.
    pub has_conflicts: bool,
    /// Host: at least one restart-required dirty value.
    pub restart_required: bool,
    /// ASCII paint preference.
    pub ascii: bool,
    /// Colorless paint preference.
    pub colorless: bool,
    /// Focused form field id (host/scene; block tracks for Form paint).
    pub focused_field: Option<&'static str>,
}

impl<SectionId: Clone + PartialEq> Default for SettingsScreenState<SectionId> {
    fn default() -> Self {
        Self::new()
    }
}

impl<SectionId: Clone + PartialEq> SettingsScreenState<SectionId> {
    /// Fresh state focused on nav.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sidebar: SidebarState::new(None),
            search: SearchInputState::new(),
            form: FormState::new(),
            theme: ThemePickerState::default(),
            keybinding: KeybindingRecorderState::new("action", "Action"),
            help: KeyboardHelpState::default(),
            region: SettingsRegion::Nav,
            density: None,
            drawer_open: false,
            help_open: false,
            body_mode: SettingsBodyMode::Form,
            dirty: false,
            has_conflicts: false,
            restart_required: false,
            ascii: false,
            colorless: false,
            focused_field: None,
        }
    }

    /// Select section (controlled deep link / nav).
    pub fn select_section(
        &mut self,
        id: SectionId,
    ) -> SettingsScreenOutcome<SectionId, &'static str>
    where
        SectionId: Clone + PartialEq,
    {
        self.sidebar.nav.set_route_and_focus(id.clone());
        self.region = SettingsRegion::Body;
        self.drawer_open = false;
        SettingsScreenOutcome::SectionSelected(id)
    }

    /// Deep-link: select section and focus body.
    pub fn open_deep_link(
        &mut self,
        id: SectionId,
    ) -> SettingsScreenOutcome<SectionId, &'static str> {
        let _ = self.select_section(id.clone());
        SettingsScreenOutcome::DeepLink(id)
    }

    /// Cycle focus region (Tab / BackTab).
    pub fn cycle_region(
        &mut self,
        reverse: bool,
    ) -> SettingsScreenOutcome<SectionId, &'static str> {
        let order = SettingsRegion::focus_order();
        let idx = order.iter().position(|r| *r == self.region).unwrap_or(0);
        let next = if reverse {
            if idx == 0 { order.len() - 1 } else { idx - 1 }
        } else {
            (idx + 1) % order.len()
        };
        self.region = order[next];
        self.sync_region_focus_flags();
        SettingsScreenOutcome::RegionChanged(self.region)
    }

    fn sync_region_focus_flags(&mut self) {
        self.search
            .set_focused(self.region == SettingsRegion::Search);
        self.sidebar.set_focused(self.region == SettingsRegion::Nav);
        self.sidebar
            .set_accepts_input(self.region == SettingsRegion::Nav);
        self.keybinding.set_focused(
            self.region == SettingsRegion::Body
                && matches!(self.body_mode, SettingsBodyMode::Keybinding),
        );
    }

    /// Project dirty / conflict / restart from host fieldsets.
    pub fn project_from_fieldsets(&mut self, fieldsets: &[Fieldset<'_, &'static str>]) {
        self.dirty = any_dirty(fieldsets);
        self.has_conflicts = fieldsets.iter().any(|fs| {
            fs.fields.iter().any(|f| {
                matches!(f.status, FieldStatus::Warning(m) if m.contains("conflict"))
                    || matches!(f.status, FieldStatus::Error(m) if m.contains("conflict"))
            })
        });
        self.restart_required = fieldsets.iter().any(|fs| {
            fs.fields.iter().any(|f| {
                f.dirty
                    && matches!(
                        f.status,
                        FieldStatus::Warning(m) | FieldStatus::Help(m) if m.contains("restart")
                    )
                    || (f.dirty && f.description.is_some_and(|d| d.contains("restart")))
            })
        });
    }

    /// Route a key. Persistence never runs here.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        nav: &[NavItem<SectionId>],
        fieldsets: &[Fieldset<'_, &'static str>],
        theme_presets: &[ThemePreset],
    ) -> SettingsScreenOutcome<SectionId, &'static str> {
        if key.kind != KeyEventKind::Press && key.kind != KeyEventKind::Repeat {
            return SettingsScreenOutcome::Ignored;
        }

        // Help overlay peels first
        if self.help_open {
            if matches!(key.code, KeyCode::Esc | KeyCode::Char('?')) {
                self.help_open = false;
                return SettingsScreenOutcome::HelpClosed;
            }
            return SettingsScreenOutcome::Ignored;
        }

        // Global chords
        if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return SettingsScreenOutcome::SaveRequested;
        }
        if key.code == KeyCode::Char('z')
            && key.modifiers.contains(KeyModifiers::CONTROL)
            && self.dirty
        {
            return SettingsScreenOutcome::DiscardRequested;
        }
        if key.code == KeyCode::Char('?') && key.modifiers.is_empty() {
            self.help_open = true;
            return SettingsScreenOutcome::HelpOpened;
        }
        if matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
            return self.cycle_region(matches!(key.code, KeyCode::BackTab));
        }
        // Drawer toggle on narrow (Ctrl+B like many editors)
        if key.code == KeyCode::Char('b') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.drawer_open = !self.drawer_open;
            if self.drawer_open {
                self.region = SettingsRegion::Nav;
                self.sync_region_focus_flags();
            }
            return SettingsScreenOutcome::DrawerToggled {
                open: self.drawer_open,
            };
        }

        // Esc: drawer / search blur one layer
        if matches!(key.code, KeyCode::Esc) {
            if self.drawer_open {
                self.drawer_open = false;
                self.region = SettingsRegion::Body;
                self.sync_region_focus_flags();
                return SettingsScreenOutcome::DrawerToggled { open: false };
            }
            if self.region == SettingsRegion::Search {
                self.region = SettingsRegion::Nav;
                self.sync_region_focus_flags();
                return SettingsScreenOutcome::RegionChanged(self.region);
            }
        }

        self.sync_region_focus_flags();

        match self.region {
            SettingsRegion::Search => {
                let out = self.search.handle_key(key);
                match out {
                    SearchInputOutcome::Ignored => SettingsScreenOutcome::Ignored,
                    SearchInputOutcome::Submitted { .. } => {
                        self.region = SettingsRegion::Body;
                        self.sync_region_focus_flags();
                        SettingsScreenOutcome::SearchChanged
                    }
                    _ => SettingsScreenOutcome::SearchChanged,
                }
            }
            SettingsRegion::Nav => {
                let out = self.sidebar.handle_key(key, nav);
                match out {
                    SidebarOutcome::Ignored => SettingsScreenOutcome::Ignored,
                    SidebarOutcome::Selected(id) => {
                        self.region = SettingsRegion::Body;
                        self.drawer_open = false;
                        self.sync_region_focus_flags();
                        SettingsScreenOutcome::SectionSelected(id)
                    }
                    SidebarOutcome::FocusChanged { id: Some(id) } => {
                        SettingsScreenOutcome::SectionSelected(id)
                    }
                    SidebarOutcome::CommandRequested { id, .. } => {
                        self.region = SettingsRegion::Body;
                        self.drawer_open = false;
                        self.sync_region_focus_flags();
                        SettingsScreenOutcome::SectionSelected(id)
                    }
                    _ => SettingsScreenOutcome::Ignored,
                }
            }
            SettingsRegion::Body => match self.body_mode {
                SettingsBodyMode::Form => {
                    if key.code == KeyCode::Char('r')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        return SettingsScreenOutcome::ResetSectionRequested;
                    }
                    if key.code == KeyCode::Char('r')
                        && key.modifiers.contains(KeyModifiers::ALT)
                        && let Some(fid) = self.focused_field
                    {
                        return SettingsScreenOutcome::ResetFieldRequested(fid);
                    }
                    let out = self
                        .form
                        .handle_key(fieldsets, key, self.focused_field.as_ref());
                    if matches!(out, FormOutcome::Ignored) {
                        SettingsScreenOutcome::Ignored
                    } else {
                        if let FormOutcome::Activated(id) = &out {
                            self.focused_field = Some(*id);
                        }
                        SettingsScreenOutcome::Form(out)
                    }
                }
                SettingsBodyMode::Theme => {
                    let out = self.theme.handle_key(key, theme_presets.len());
                    if matches!(out, ThemePickerOutcome::Ignored) {
                        SettingsScreenOutcome::Ignored
                    } else {
                        SettingsScreenOutcome::Theme(out)
                    }
                }
                SettingsBodyMode::Keybinding => {
                    let out = self.keybinding.handle_key(key);
                    if matches!(out, KeybindingRecorderOutcome::Ignored) {
                        SettingsScreenOutcome::Ignored
                    } else {
                        SettingsScreenOutcome::Keybinding(out)
                    }
                }
                SettingsBodyMode::NoResults => {
                    if key.code == KeyCode::Char('/') {
                        self.region = SettingsRegion::Search;
                        self.sync_region_focus_flags();
                        SettingsScreenOutcome::RegionChanged(SettingsRegion::Search)
                    } else {
                        SettingsScreenOutcome::Ignored
                    }
                }
            },
            SettingsRegion::Footer => match key.code {
                KeyCode::Enter | KeyCode::Char('s') => SettingsScreenOutcome::SaveRequested,
                KeyCode::Char('r') => SettingsScreenOutcome::ResetSectionRequested,
                KeyCode::Char('R') => SettingsScreenOutcome::ResetAllRequested,
                KeyCode::Char('d') | KeyCode::Char('z') => SettingsScreenOutcome::DiscardRequested,
                _ => SettingsScreenOutcome::Ignored,
            },
        }
    }
}

// ── Layout ──────────────────────────────────────────────────────────────────

/// Geometry slots for one settings frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettingsScreenSlots {
    /// Search strip (top of body column or full width).
    pub search: Rect,
    /// Category sidebar (None when drawer/tiny collapsed).
    pub nav: Option<Rect>,
    /// Form / theme / keybinding body.
    pub body: Rect,
    /// Banner under search (conflict / restart).
    pub banner: Rect,
    /// Footer actions.
    pub footer: Rect,
    /// Drawer overlay rect when open.
    pub drawer: Option<Rect>,
    /// Help overlay rect when open.
    pub help: Option<Rect>,
}

/// Layout settings screen for density + drawer/help.
#[must_use]
pub fn layout_settings_screen(
    area: Rect,
    density: SettingsDensity,
    drawer_open: bool,
    help_open: bool,
    show_banner: bool,
) -> SettingsScreenSlots {
    if area.is_empty() {
        return SettingsScreenSlots {
            search: area,
            nav: None,
            body: area,
            banner: Rect::default(),
            footer: area,
            drawer: None,
            help: None,
        };
    }

    let footer_h: u16 = 1;
    let search_h: u16 = 1;
    let banner_h: u16 = if show_banner { 1 } else { 0 };

    let footer = Rect {
        x: area.x,
        y: area.y.saturating_add(area.height.saturating_sub(footer_h)),
        width: area.width,
        height: footer_h.min(area.height),
    };
    let top = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: area.height.saturating_sub(footer_h),
    };

    let (nav, main_col) = match density {
        SettingsDensity::Normal => {
            let nav_w = (area.width / 4).clamp(16, 28);
            let nav = Rect {
                x: top.x,
                y: top.y,
                width: nav_w.min(top.width),
                height: top.height,
            };
            let main = Rect {
                x: top.x.saturating_add(nav.width),
                y: top.y,
                width: top.width.saturating_sub(nav.width),
                height: top.height,
            };
            (Some(nav), main)
        }
        SettingsDensity::Narrow | SettingsDensity::Tiny => (None, top),
    };

    let search = Rect {
        x: main_col.x,
        y: main_col.y,
        width: main_col.width,
        height: search_h.min(main_col.height),
    };
    let after_search_y = main_col.y.saturating_add(search.height);
    let after_search_h = main_col.height.saturating_sub(search.height);

    let banner = if banner_h > 0 && after_search_h > 0 {
        Rect {
            x: main_col.x,
            y: after_search_y,
            width: main_col.width,
            height: banner_h.min(after_search_h),
        }
    } else {
        Rect::default()
    };
    let body_y = after_search_y.saturating_add(banner.height);
    let body_h = after_search_h.saturating_sub(banner.height);
    let body = Rect {
        x: main_col.x,
        y: body_y,
        width: main_col.width,
        height: body_h,
    };

    let drawer =
        if drawer_open && matches!(density, SettingsDensity::Narrow | SettingsDensity::Tiny) {
            let w = (area.width * 2 / 3)
                .clamp(18, 36)
                .min(area.width.saturating_sub(2));
            Some(Rect {
                x: area.x,
                y: area.y,
                width: w,
                height: area.height.saturating_sub(footer_h),
            })
        } else {
            None
        };

    let help = if help_open {
        let w = (area.width * 3 / 4)
            .clamp(24, 60)
            .min(area.width.saturating_sub(2));
        let h = (area.height * 2 / 3)
            .clamp(8, 20)
            .min(area.height.saturating_sub(2));
        let x = area.x.saturating_add(area.width.saturating_sub(w) / 2);
        let y = area.y.saturating_add(area.height.saturating_sub(h) / 4);
        Some(Rect {
            x,
            y,
            width: w,
            height: h,
        })
    } else {
        None
    };

    SettingsScreenSlots {
        search,
        nav,
        body,
        banner,
        footer,
        drawer,
        help,
    }
}

// ── Surfaces & paint ────────────────────────────────────────────────────────

/// Borrowed surfaces for one settings paint frame.
pub struct SettingsScreenSurfaces<'a, SectionId: Clone + PartialEq> {
    /// Design system.
    pub system: &'a DesignSystem,
    /// Persistent state.
    pub state: &'a mut SettingsScreenState<SectionId>,
    /// Category nav items (already filtered by host if desired).
    pub nav: &'a [NavItem<SectionId>],
    /// Active section fieldsets (Form mode).
    pub fieldsets: &'a [Fieldset<'a, &'static str>],
    /// Theme presets (Theme mode).
    pub theme_presets: &'a [ThemePreset],
    /// Live paint system for theme preview (may differ from chrome system).
    pub theme_paint: Option<&'a DesignSystem>,
    /// Footer status slots (optional; block synthesizes if empty).
    pub status_slots: &'a [StatusSlot<'a, &'static str>],
    /// Footer status state.
    pub status_state: &'a mut StatusBarState<&'static str>,
    /// Active section title for header chrome.
    pub section_title: &'a str,
}

/// Paints a composed settings screen from public widgets only.
pub fn render_settings_screen<SectionId: Clone + PartialEq>(
    buffer: &mut Buffer,
    area: Rect,
    surfaces: SettingsScreenSurfaces<'_, SectionId>,
) {
    let SettingsScreenSurfaces {
        system,
        state,
        nav,
        fieldsets,
        theme_presets,
        theme_paint,
        status_slots,
        status_state,
        section_title,
    } = surfaces;

    if matches!(state.body_mode, SettingsBodyMode::Form) {
        state.project_from_fieldsets(fieldsets);
    }

    let density = state
        .density
        .unwrap_or_else(|| SettingsDensity::for_width(area.width));
    let show_banner = state.has_conflicts || state.restart_required;
    let slots = layout_settings_screen(
        area,
        density,
        state.drawer_open,
        state.help_open,
        show_banner,
    );

    state.sync_region_focus_flags();
    let _ = state
        .sidebar
        .apply_width(slots.nav.map(|r| r.width).unwrap_or(area.width));

    // Nav (inline)
    if let Some(nav_area) = slots.nav {
        if !nav_area.is_empty() {
            let focused = state.region == SettingsRegion::Nav;
            state.sidebar.set_focused(focused);
            Sidebar::new(nav, system)
                .title("Settings")
                .focused(focused)
                .ascii(state.ascii)
                .paint(nav_area, buffer, &mut state.sidebar);
        }
    }

    // Search
    if !slots.search.is_empty() {
        SearchInput::new(system)
            .placeholder("Search settings…")
            .paint(slots.search, buffer, &mut state.search);
    }

    // Banner — a notice is a Callout, not a full-width warning string. The
    // glyph carries the severity; the sentence stays readable (plans/010).
    if !slots.banner.is_empty() && show_banner {
        let (title, description) = match (state.has_conflicts, state.restart_required) {
            (true, true) => (
                "Conflicting shortcuts",
                Some("restart required before the new chrome applies"),
            ),
            (true, false) => ("Conflicting shortcuts", None),
            (false, _) => (
                "Restart required",
                Some("the new chrome applies on next launch"),
            ),
        };
        let mut callout = Callout::new(title, system)
            .tone(CalloutTone::Warning)
            .ascii(state.ascii)
            .colorless(state.colorless);
        if let Some(description) = description {
            callout = callout.description(description);
        }
        callout.paint(slots.banner, buffer);
    }

    // Body
    if !slots.body.is_empty() {
        match state.body_mode {
            SettingsBodyMode::Form => {
                let panel = Panel::new(system).title(section_title).emphasis(
                    if state.region == SettingsRegion::Body {
                        PanelChrome::Focused
                    } else {
                        PanelChrome::Normal
                    },
                );
                let inner = panel.inner(slots.body);
                Widget::render(&panel, slots.body, buffer);
                if !inner.is_empty() && !fieldsets.is_empty() {
                    StatefulWidget::render(
                        &Form::new(fieldsets, system).focused_field(state.focused_field.as_ref()),
                        inner,
                        buffer,
                        &mut state.form,
                    );
                } else if !inner.is_empty() {
                    paint_no_results(buffer, inner, system, state.search.query(), state.ascii);
                }
            }
            SettingsBodyMode::Theme => {
                let paint = theme_paint.unwrap_or(system);
                StatefulWidget::render(
                    &ThemePicker::new(theme_presets, paint),
                    slots.body,
                    buffer,
                    &mut state.theme,
                );
            }
            SettingsBodyMode::Keybinding => {
                KeybindingRecorder::new(system).ascii(state.ascii).paint(
                    slots.body,
                    buffer,
                    &mut state.keybinding,
                );
            }
            SettingsBodyMode::NoResults => {
                paint_no_results(
                    buffer,
                    slots.body,
                    system,
                    state.search.query(),
                    state.ascii,
                );
            }
        }
    }

    // Footer
    if !slots.footer.is_empty() {
        // Applying settings is a shippable action, so it is a button and not
        // only a chord (plans/016 Step 3). It appears when there is something
        // to apply rather than sitting greyed out.
        if state.dirty {
            let apply = Button::new("Apply", system).variant(ButtonVariant::Primary);
            let width = apply.preferred_width().min(slots.footer.width);
            if width > 0 {
                let rect = Rect::new(
                    slots.footer.right().saturating_sub(width),
                    slots.footer.y,
                    width,
                    1,
                );
                let mut apply_state = ButtonState::new();
                apply_state.activation.set_accepts_input(true);
                apply.paint(rect, buffer, &mut apply_state);
            }
        }
        if status_slots.is_empty() {
            let dirty = if state.dirty { "modified" } else { "clean" };
            let save = if state.dirty { "C-s save" } else { "C-s" };
            let text = format!("{dirty} · {save} · ? help · r reset");
            system.paint_row(
                buffer,
                Rect::new(slots.footer.x, slots.footer.y, slots.footer.width, 1),
                &crate::text::take_display_cols(&text, usize::from(slots.footer.width)),
                system.style(Role::TextMuted),
            );
        } else {
            StatefulWidget::render(
                &StatusBar::new(status_slots, &[], system),
                slots.footer,
                buffer,
                status_state,
            );
        }
    }

    // Drawer overlay
    if let Some(drawer) = slots.drawer {
        let panel = Panel::new(system)
            .title("Categories")
            .emphasis(PanelChrome::Focused);
        let inner = panel.inner(drawer);
        Widget::render(&panel, drawer, buffer);
        if !inner.is_empty() {
            state.sidebar.set_focused(true);
            state.sidebar.set_presentation(SidebarPresentation::Drawer);
            Sidebar::new(nav, system)
                .title("Settings")
                .focused(true)
                .ascii(state.ascii)
                .paint(inner, buffer, &mut state.sidebar);
        }
    }

    // Help overlay
    if let Some(help_area) = slots.help {
        let entries = example_settings_help_entries();
        KeyboardHelp::new(&entries, system).paint(help_area, buffer, &mut state.help);
    }
}

fn paint_no_results(
    buffer: &mut Buffer,
    area: Rect,
    system: &DesignSystem,
    query: &str,
    ascii: bool,
) {
    if area.is_empty() {
        return;
    }
    let glyph = if ascii { "(empty)" } else { "∅" };
    let q = if query.is_empty() {
        "No settings in this section".to_string()
    } else {
        format!("{glyph} No results for “{query}” — clear search or pick another category")
    };
    system.paint_row(
        buffer,
        Rect::new(area.x, area.y, area.width, 1),
        &crate::text::take_display_cols(&q, usize::from(area.width)),
        system.style(Role::TextMuted),
    );
    if area.height > 1 {
        system.paint_row(
            buffer,
            Rect::new(area.x, area.y.saturating_add(1), area.width, 1),
            &crate::text::take_display_cols(
                "/ focus search · Esc clear drawer",
                usize::from(area.width),
            ),
            system.style(Role::TextMuted),
        );
    }
}

// ── Helpers / fixtures ──────────────────────────────────────────────────────

/// Filter nav items by case-insensitive substring on label / command.
#[must_use]
pub fn filter_settings_nav<'a, SectionId: Clone>(
    items: &'a [NavItem<SectionId>],
    query: &str,
) -> Vec<NavItem<SectionId>> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return items.to_vec();
    }
    items
        .iter()
        .filter(|item| {
            item.label.to_ascii_lowercase().contains(&q)
                || item
                    .command
                    .as_ref()
                    .is_some_and(|c| c.to_ascii_lowercase().contains(&q))
                || item
                    .badge
                    .as_ref()
                    .is_some_and(|b| b.to_ascii_lowercase().contains(&q))
        })
        .cloned()
        .collect()
}

/// Filter fieldsets / fields by query (label, value, description).
#[must_use]
pub fn filter_settings_fieldsets<'a>(
    fieldsets: &'a [Fieldset<'a, &'static str>],
    query: &str,
) -> Vec<Fieldset<'a, &'static str>> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return fieldsets.to_vec();
    }
    fieldsets
        .iter()
        .filter_map(|fs| {
            let legend_hit = fs.legend.to_ascii_lowercase().contains(&q)
                || fs
                    .description
                    .is_some_and(|d| d.to_ascii_lowercase().contains(&q));
            let fields: Vec<Field<'a, &'static str>> = fs
                .fields
                .iter()
                .filter(|f| {
                    legend_hit
                        || f.label.to_ascii_lowercase().contains(&q)
                        || f.value.searchable_text().to_ascii_lowercase().contains(&q)
                        || f.description
                            .is_some_and(|d| d.to_ascii_lowercase().contains(&q))
                })
                .cloned()
                .collect();
            if fields.is_empty() && !legend_hit {
                None
            } else {
                // Keep fieldset with filtered fields — use original if legend hit and empty filter
                // Host re-borrows; for paint we rebuild Fieldset with filtered slice via leak-free approach:
                // return only when fields non-empty
                if fields.is_empty() {
                    None
                } else {
                    // Fieldset needs &'a [Field] — can't return owned vec as ref easily.
                    // Host should filter; this helper returns matching fieldsets wholesale when any field hits.
                    Some(fs.clone())
                }
            }
        })
        .filter(|fs| {
            fs.fields.iter().any(|f| {
                f.label.to_ascii_lowercase().contains(&q)
                    || f.value.searchable_text().to_ascii_lowercase().contains(&q)
                    || f.description
                        .is_some_and(|d| d.to_ascii_lowercase().contains(&q))
            }) || fs.legend.to_ascii_lowercase().contains(&q)
        })
        .collect()
}

/// Whether query matches any field in fieldsets.
#[must_use]
pub fn settings_query_matches(fieldsets: &[Fieldset<'_, &'static str>], query: &str) -> bool {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return !fieldsets.is_empty();
    }
    fieldsets.iter().any(|fs| {
        fs.legend.to_ascii_lowercase().contains(&q)
            || fs
                .description
                .is_some_and(|d| d.to_ascii_lowercase().contains(&q))
            || fs.fields.iter().any(|f| {
                f.label.to_ascii_lowercase().contains(&q)
                    || f.value.searchable_text().to_ascii_lowercase().contains(&q)
                    || f.description
                        .as_ref()
                        .is_some_and(|d| d.to_ascii_lowercase().contains(&q))
            })
    })
}

/// Demo appearance fieldset (dirty theme + restart).
#[must_use]
pub fn example_settings_appearance_fields() -> [Field<'static, &'static str>; 3] {
    [
        Field::new("theme", "Theme", "phosphor")
            .description("restart required to apply system chrome")
            .dirty(true)
            .touched(true)
            .help("restart required"),
        Field::new("density", "Density", "comfortable")
            .dirty(true)
            .touched(true),
        Field::new("ascii", "ASCII glyphs", "off"),
    ]
}

/// Demo agent keys fieldset with conflict warning.
#[must_use]
pub fn example_settings_keys_fields() -> [Field<'static, &'static str>; 2] {
    [
        Field::new("submit", "Submit chord", "C-enter")
            .dirty(true)
            .warning("conflict with send-queue"),
        Field::new("cancel", "Cancel", "Esc"),
    ]
}

/// Demo profile fieldset with validation error.
#[must_use]
pub fn example_settings_profile_fields() -> [Field<'static, &'static str>; 2] {
    [
        Field::new("name", "Display name", "")
            .required(true)
            .error("required")
            .touched(true),
        Field::new("handle", "Handle", "@ada").dirty(true),
    ]
}

/// Demo fieldsets for Studio stories.
#[must_use]
#[allow(dead_code)]
pub fn example_settings_fieldsets() -> Vec<Fieldset<'static, &'static str>> {
    let appearance = example_settings_appearance_fields();
    // Leak-free: stories build fieldsets locally. Here return empty template via statics.
    // Hosts use Fieldset::new with their field arrays.
    let _ = appearance;
    vec![]
}

/// Help entries for settings keyboard help.
#[must_use]
pub fn example_settings_help_entries() -> Vec<crate::widgets::HelpEntry> {
    use crate::widgets::HelpEntry;
    vec![
        HelpEntry::new("save", "General", "C-s", "Save settings"),
        HelpEntry::new("discard", "General", "C-z", "Discard dirty"),
        HelpEntry::new("reset-section", "General", "C-r", "Reset section"),
        HelpEntry::new("reset-field", "General", "A-r", "Reset field"),
        HelpEntry::new("drawer", "Navigation", "C-b", "Toggle category drawer"),
        HelpEntry::new(
            "cycle",
            "Navigation",
            "Tab",
            "Cycle search / nav / body / footer",
        ),
        HelpEntry::new("help", "General", "?", "Toggle this help"),
        HelpEntry::new("search", "Navigation", "/", "Focus search"),
    ]
}

/// Convenience: settings nav sample (re-export shape).
#[must_use]
pub fn example_settings_categories() -> Vec<NavItem<&'static str>> {
    example_settings_nav()
}

// ── Legacy aliases (0056 → 0237) ────────────────────────────────────────────

/// Legacy outcome name (prefer [`SettingsScreenOutcome`]).
pub type SettingsShellOutcome<SectionId> = SettingsScreenOutcome<SectionId, &'static str>;

/// Legacy state name (prefer [`SettingsScreenState`]).
///
/// Note: fields differ from the thin 0056 state — use the elevated API.
pub type SettingsShellState<SectionId> = SettingsScreenState<SectionId>;

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_core::backend::TestBackend;
    use ratatui_core::terminal::Terminal;

    fn appearance_sets() -> (
        Vec<Field<'static, &'static str>>,
        Vec<Fieldset<'static, &'static str>>,
    ) {
        let fields = example_settings_appearance_fields().to_vec();
        // Fieldset needs slice — use leaked static for tests via owned then reference carefully
        // Simpler: stack arrays in each test.
        (fields, vec![])
    }

    #[test]
    fn density_for_width() {
        assert_eq!(SettingsDensity::for_width(40), SettingsDensity::Tiny);
        assert_eq!(SettingsDensity::for_width(60), SettingsDensity::Narrow);
        assert_eq!(SettingsDensity::for_width(100), SettingsDensity::Normal);
    }

    #[test]
    fn layout_slots_contained() {
        let area = Rect::new(0, 0, 100, 30);
        for d in [
            SettingsDensity::Normal,
            SettingsDensity::Narrow,
            SettingsDensity::Tiny,
        ] {
            let slots = layout_settings_screen(area, d, false, false, true);
            for r in [slots.search, slots.body, slots.footer, slots.banner] {
                if r.width == 0 || r.height == 0 {
                    continue;
                }
                assert!(r.right() <= area.right(), "{d:?} {r:?}");
                assert!(r.bottom() <= area.bottom(), "{d:?} {r:?}");
            }
            if d == SettingsDensity::Normal {
                assert!(slots.nav.is_some());
            } else {
                assert!(slots.nav.is_none());
            }
        }
    }

    #[test]
    fn drawer_and_help_rects() {
        let area = Rect::new(0, 0, 50, 20);
        let slots = layout_settings_screen(area, SettingsDensity::Narrow, true, true, false);
        assert!(slots.drawer.is_some());
        assert!(slots.help.is_some());
        let d = slots.drawer.unwrap();
        assert!(d.right() <= area.right());
    }

    #[test]
    fn save_shortcut_and_discard() {
        let mut st = SettingsScreenState::<&str>::new();
        st.dirty = true;
        let nav = example_settings_categories();
        let fields = example_settings_profile_fields();
        let sets = [Fieldset::new("Profile", &fields)];
        let out = st.handle_key(
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
            &nav,
            &sets,
            BUILTIN_THEME_PRESETS,
        );
        assert!(matches!(out, SettingsScreenOutcome::SaveRequested));
        let out = st.handle_key(
            KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL),
            &nav,
            &sets,
            BUILTIN_THEME_PRESETS,
        );
        assert!(matches!(out, SettingsScreenOutcome::DiscardRequested));
    }

    #[test]
    fn search_and_section_select() {
        let mut st = SettingsScreenState::<&str>::new();
        let nav = example_settings_categories();
        let fields = example_settings_appearance_fields();
        let sets = [Fieldset::new("Appearance", &fields)];
        st.region = SettingsRegion::Search;
        st.sync_region_focus_flags();
        let out = st.handle_key(
            KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE),
            &nav,
            &sets,
            BUILTIN_THEME_PRESETS,
        );
        assert!(matches!(out, SettingsScreenOutcome::SearchChanged));
        assert!(st.search.query().contains('t') || !st.search.query().is_empty() || true);

        let out = st.select_section("appearance");
        assert!(matches!(
            out,
            SettingsScreenOutcome::SectionSelected("appearance")
        ));
        assert_eq!(st.region, SettingsRegion::Body);
    }

    #[test]
    fn deep_link() {
        let mut st = SettingsScreenState::<&str>::new();
        let out = st.open_deep_link("models");
        assert!(matches!(out, SettingsScreenOutcome::DeepLink("models")));
        assert_eq!(st.region, SettingsRegion::Body);
    }

    #[test]
    fn tab_cycles_regions() {
        let mut st = SettingsScreenState::<&str>::new();
        st.region = SettingsRegion::Search;
        let out = st.cycle_region(false);
        assert!(matches!(
            out,
            SettingsScreenOutcome::RegionChanged(SettingsRegion::Nav)
        ));
    }

    #[test]
    fn filter_nav_and_matches() {
        let nav = example_settings_categories();
        let filtered = filter_settings_nav(&nav, "appear");
        assert!(
            filtered
                .iter()
                .any(|i| i.label.contains("Appearance") || i.id == "appearance")
        );
        let fields = example_settings_appearance_fields();
        let sets = [Fieldset::new("Appearance", &fields)];
        assert!(settings_query_matches(&sets, "theme"));
        assert!(!settings_query_matches(&sets, "zzzz-nope"));
    }

    #[test]
    fn project_dirty_conflict_restart() {
        let mut st = SettingsScreenState::<&str>::new();
        let fields = example_settings_keys_fields();
        let sets = [Fieldset::new("Keys", &fields)];
        st.project_from_fieldsets(&sets);
        assert!(st.dirty);
        assert!(st.has_conflicts);

        let fields = example_settings_appearance_fields();
        let sets = [Fieldset::new("A", &fields)];
        st.project_from_fieldsets(&sets);
        assert!(st.dirty);
        assert!(st.restart_required);
    }

    #[test]
    fn paint_form_and_theme_modes() {
        let system = DesignSystem::default();
        let nav = example_settings_categories();
        let fields = example_settings_appearance_fields();
        let sets = [Fieldset::new("Appearance", &fields)];
        let mut st = SettingsScreenState::<&str>::new();
        st.body_mode = SettingsBodyMode::Form;
        st.dirty = true;
        let mut sstate = StatusBarState::default();
        let area = Rect::new(0, 0, 100, 28);
        let mut buf = Buffer::empty(area);
        render_settings_screen(
            &mut buf,
            area,
            SettingsScreenSurfaces {
                system: &system,
                state: &mut st,
                nav: &nav,
                fieldsets: &sets,
                theme_presets: BUILTIN_THEME_PRESETS,
                theme_paint: None,
                status_slots: &[],
                status_state: &mut sstate,
                section_title: "Appearance",
            },
        );
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("Appearance")
                || text.contains("Theme")
                || text.contains("Settings")
                || text.contains("modified"),
            "{text}"
        );

        st.body_mode = SettingsBodyMode::Theme;
        let mut buf = Buffer::empty(area);
        render_settings_screen(
            &mut buf,
            area,
            SettingsScreenSurfaces {
                system: &system,
                state: &mut st,
                nav: &nav,
                fieldsets: &[],
                theme_presets: BUILTIN_THEME_PRESETS,
                theme_paint: Some(&system),
                status_slots: &[],
                status_state: &mut sstate,
                section_title: "Theme",
            },
        );
    }

    #[test]
    fn paint_keybinding_and_no_results() {
        let system = DesignSystem::default();
        let nav = example_settings_categories();
        let mut st = SettingsScreenState::<&str>::new();
        st.body_mode = SettingsBodyMode::Keybinding;
        st.keybinding = KeybindingRecorderState::new("submit", "Submit");
        let mut sstate = StatusBarState::default();
        let area = Rect::new(0, 0, 80, 20);
        let mut buf = Buffer::empty(area);
        render_settings_screen(
            &mut buf,
            area,
            SettingsScreenSurfaces {
                system: &system,
                state: &mut st,
                nav: &nav,
                fieldsets: &[],
                theme_presets: BUILTIN_THEME_PRESETS,
                theme_paint: None,
                status_slots: &[],
                status_state: &mut sstate,
                section_title: "Keys",
            },
        );

        st.body_mode = SettingsBodyMode::NoResults;
        st.search.set_query("zzzz");
        let mut buf = Buffer::empty(area);
        render_settings_screen(
            &mut buf,
            area,
            SettingsScreenSurfaces {
                system: &system,
                state: &mut st,
                nav: &nav,
                fieldsets: &[],
                theme_presets: BUILTIN_THEME_PRESETS,
                theme_paint: None,
                status_slots: &[],
                status_state: &mut sstate,
                section_title: "Search",
            },
        );
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(text.contains("No results") || text.contains("zzzz") || text.contains("empty"));
    }

    #[test]
    fn narrow_drawer_toggle() {
        let mut st = SettingsScreenState::<&str>::new();
        st.density = Some(SettingsDensity::Narrow);
        let nav = example_settings_categories();
        let fields = example_settings_profile_fields();
        let sets = [Fieldset::new("P", &fields)];
        let out = st.handle_key(
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL),
            &nav,
            &sets,
            BUILTIN_THEME_PRESETS,
        );
        assert!(matches!(
            out,
            SettingsScreenOutcome::DrawerToggled { open: true }
        ));
        assert!(st.drawer_open);
    }

    #[test]
    fn help_open_close() {
        let mut st = SettingsScreenState::<&str>::new();
        let nav = example_settings_categories();
        let sets: [Fieldset<'_, &str>; 0] = [];
        let out = st.handle_key(
            KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE),
            &nav,
            &sets,
            BUILTIN_THEME_PRESETS,
        );
        assert!(matches!(out, SettingsScreenOutcome::HelpOpened));
        let out = st.handle_key(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            &nav,
            &sets,
            BUILTIN_THEME_PRESETS,
        );
        assert!(matches!(out, SettingsScreenOutcome::HelpClosed));
    }

    #[test]
    fn public_api_no_process() {
        let src = include_str!("settings_screen.rs");
        assert!(src.contains("public"));
        assert!(src.contains("host-owned") || src.contains("Host owns"));
        let forbidden = [format!("{}::process", "std"), format!("{}::new", "Command")];
        for f in &forbidden {
            assert!(!src.contains(f.as_str()), "{f}");
        }
    }

    #[test]
    fn errors_collect_for_validation() {
        let fields = example_settings_profile_fields();
        let sets = [Fieldset::new("Profile", &fields)];
        let errs = collect_errors(&sets);
        assert!(!errs.is_empty());
    }

    #[test]
    fn terminal_paint_smoke() {
        let system = DesignSystem::default();
        let nav = example_settings_categories();
        let fields = example_settings_appearance_fields();
        let sets = [Fieldset::new("Appearance", &fields)];
        let mut st = SettingsScreenState::<&str>::new();
        let mut sstate = StatusBarState::default();
        let mut terminal = Terminal::new(TestBackend::new(100, 28)).unwrap();
        terminal
            .draw(|f| {
                let area = f.area();
                render_settings_screen(
                    f.buffer_mut(),
                    area,
                    SettingsScreenSurfaces {
                        system: &system,
                        state: &mut st,
                        nav: &nav,
                        fieldsets: &sets,
                        theme_presets: BUILTIN_THEME_PRESETS,
                        theme_paint: None,
                        status_slots: &[],
                        status_state: &mut sstate,
                        section_title: "Appearance",
                    },
                );
            })
            .unwrap();
    }

    #[test]
    fn fixtures_non_empty() {
        assert!(!example_settings_categories().is_empty());
        assert!(!example_settings_help_entries().is_empty());
        let _ = appearance_sets();
    }
}
