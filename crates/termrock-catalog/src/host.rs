// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Host event JSON shape retained from the prior presentation adapter (Apache-2.0).

//! Persistent catalog sessions for the WASM / browser host.

use std::collections::BTreeSet;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Position;
use serde::{Deserialize, Serialize};
use termrock::input::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use termrock::runtime::FrameTick;
use termrock::style::{ColorCapability, color_to_rgb};

use crate::catalog::{
    CatalogProfile, CatalogScenario, NavEntry, PageId, catalog_scenarios, nav_entries,
    scenario_by_id,
};
use crate::scenarios::Host;
use crate::shell::{App, PageMetadata};
use crate::snapshot::Snapshot;
use crate::tablepro::App as TableProApp;

/// Static catalog metadata available before mounting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DemoDescriptor {
    pub id: String,
    pub title: &'static str,
    pub component: &'static str,
    pub description: &'static str,
    pub cols: u16,
    pub rows: u16,
    pub interactive: bool,
    /// High-level interaction family used by the browser host.
    pub interaction_kind: &'static str,
    pub hints: Vec<&'static str>,
}

/// Host-to-demo event serialized by the WASM adapter.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum DemoEvent {
    Key {
        key: String,
        #[serde(default = "default_key_kind")]
        kind: String,
        #[serde(default)]
        shift: bool,
        #[serde(default)]
        ctrl: bool,
        #[serde(default)]
        alt: bool,
        #[serde(default)]
        meta: bool,
    },
    Pointer {
        kind: String,
        x: u16,
        y: u16,
        #[serde(default = "default_button")]
        button: String,
    },
    Wheel {
        #[serde(default)]
        delta_x: i16,
        #[serde(default)]
        delta_y: i16,
        x: u16,
        y: u16,
    },
    Paste {
        text: String,
    },
    Resize {
        cols: u16,
        rows: u16,
    },
    Focus {
        focused: bool,
    },
    Tick {
        elapsed_ms: u64,
    },
}

fn default_key_kind() -> String {
    "press".into()
}
fn default_button() -> String {
    "left".into()
}

/// Result of one event dispatch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DemoUpdate {
    pub changed: bool,
    pub outcome: Option<String>,
    pub hints: Vec<&'static str>,
    pub interactive: bool,
    /// Whether the current state accepts literal text and paste payloads.
    pub captures_text_input: bool,
    pub next_deadline_ms: Option<u64>,
    pub deadline_kind: Option<&'static str>,
    pub semantic_revision: u64,
}

/// One truecolor cell for the browser host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FrameCell {
    /// Grapheme cluster or terminal cell symbol.
    pub ch: String,
    /// Truecolor foreground RGB.
    pub fg: [u8; 3],
    /// Truecolor background RGB.
    pub bg: [u8; 3],
    /// Bold modifier.
    pub bold: bool,
    /// Dim modifier.
    pub dim: bool,
    /// Underline modifier.
    pub underline: bool,
    /// Reverse-video modifier after terminal color resolution.
    pub reversed: bool,
    /// Italic modifier.
    pub italic: bool,
    /// Strikethrough modifier.
    pub strike: bool,
}

/// Full terminal frame for a catalog page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TerminalFrame {
    pub story_id: String,
    pub title: String,
    pub component: String,
    pub cols: u16,
    pub rows: u16,
    /// Inner story columns. Catalog frames have no outer padding.
    pub story_cols: u16,
    /// Inner story rows. Catalog frames have no outer padding.
    pub story_rows: u16,
    pub cells: Vec<FrameCell>,
    /// Hardware cursor cell, when the mounted page is editing.
    pub cursor: Option<[u16; 2]>,
    /// Whether the hardware cursor is visible.
    pub cursor_visible: bool,
    pub interactive: bool,
    pub theme: String,
}

/// All catalog entries the web host can mount.
///
/// This includes page entries used by the native shell and every canonical
/// representative scenario consumed by the documentation/poster host. Both
/// entry kinds mount through [`CatalogSession`] and the same `App` renderer.
#[must_use]
pub fn catalog() -> Vec<DemoDescriptor> {
    catalog_for_profile(CatalogProfile::TermRock)
}

/// Return the complete catalog namespace visible to one profile.
#[must_use]
pub fn catalog_for_profile(profile: CatalogProfile) -> Vec<DemoDescriptor> {
    let mut app = App::new(profile, ColorCapability::Truecolor);
    let scenarios = catalog_scenarios_for_profile(profile).collect::<Vec<_>>();
    let mut entries = Vec::with_capacity(nav_entries(profile).len() + scenarios.len());
    let mut ids = BTreeSet::new();

    for entry in nav_entries(profile) {
        app.goto(entry.id);
        let descriptor = descriptor(entry, &app.page_metadata());
        assert!(
            ids.insert(descriptor.id.clone()),
            "duplicate catalog entry id {:?}",
            descriptor.id
        );
        entries.push(descriptor);
    }

    for scenario in scenarios {
        app.goto(scenario.page);
        let metadata = app.page_metadata();
        let descriptor = scenario_descriptor(&scenario, &metadata);
        assert!(
            ids.insert(descriptor.id.clone()),
            "duplicate catalog entry id {:?}",
            descriptor.id
        );
        entries.push(descriptor);
    }

    entries
}

fn catalog_scenarios_for_profile(profile: CatalogProfile) -> impl Iterator<Item = CatalogScenario> {
    catalog_scenarios()
        .into_iter()
        .filter(move |scenario| page_is_visible(profile, scenario.page))
}

fn page_is_visible(profile: CatalogProfile, page: PageId) -> bool {
    nav_entries(profile).iter().any(|entry| entry.id == page)
}

fn scenario_descriptor(scenario: &CatalogScenario, metadata: &PageMetadata) -> DemoDescriptor {
    DemoDescriptor {
        id: scenario.id.to_owned(),
        title: scenario.title,
        component: scenario.component,
        description: scenario.description,
        cols: scenario.cols,
        rows: scenario.rows,
        interactive: scenario.interactive,
        interaction_kind: scenario.interaction_kind,
        hints: metadata.hints.iter().map(|(key, _)| *key).collect(),
    }
}

fn descriptor(e: &NavEntry, metadata: &PageMetadata) -> DemoDescriptor {
    DemoDescriptor {
        id: crate::catalog::normalize(e.label),
        title: metadata.title,
        component: e.section,
        description: metadata.description,
        cols: 120,
        rows: 40,
        interactive: metadata.interactive,
        interaction_kind: metadata.interaction_kind,
        hints: metadata.hints.iter().map(|(key, _)| *key).collect(),
    }
}

fn frame_cells(snapshot: &Snapshot) -> Vec<FrameCell> {
    snapshot
        .cells
        .iter()
        .map(|c| {
            let reversed = c.modifier.contains(ratatui::style::Modifier::REVERSED);
            let mut fg = match c.fg {
                ratatui::style::Color::Reset => [0xd0, 0xd0, 0xd0],
                color => color_to_rgb(color, true),
            };
            let mut bg = color_to_rgb(c.bg, false);
            if reversed {
                std::mem::swap(&mut fg, &mut bg);
            }
            FrameCell {
                ch: c.glyph.clone(),
                fg,
                bg,
                bold: c.modifier.contains(ratatui::style::Modifier::BOLD),
                dim: c.modifier.contains(ratatui::style::Modifier::DIM),
                underline: c.modifier.contains(ratatui::style::Modifier::UNDERLINED),
                reversed,
                italic: c.modifier.contains(ratatui::style::Modifier::ITALIC),
                strike: c.modifier.contains(ratatui::style::Modifier::CROSSED_OUT),
            }
        })
        .collect()
}

/// One long-lived catalog page instance.
pub struct CatalogSession {
    app: App,
    profile: CatalogProfile,
    nav: Vec<NavEntry>,
    page: PageId,
    scenario: Option<CatalogScenario>,
    cols: u16,
    rows: u16,
    elapsed_ms: u64,
    semantic_revision: u64,
}

impl CatalogSession {
    pub fn mount(id: &str, cols: u16, rows: u16) -> Result<Self, String> {
        Self::mount_profile(id, cols, rows, CatalogProfile::TermRock)
    }

    /// Mount an entry under an explicit catalog profile. The reference
    /// profile exists for parity capture; it shares this session and renderer
    /// with the default TermRock catalog.
    pub fn mount_profile(
        id: &str,
        cols: u16,
        rows: u16,
        profile: CatalogProfile,
    ) -> Result<Self, String> {
        let (page, scenario, nav) = if let Some(scenario) = scenario_by_id(id) {
            if !page_is_visible(profile, scenario.page) {
                return Err(format!(
                    "catalog entry {id:?} is not available in the {profile:?} profile"
                ));
            }
            (scenario.page, Some(scenario), nav_entries(profile).to_vec())
        } else if let Some(page) = PageId::from_name(id, nav_entries(profile)) {
            (page, None, nav_entries(profile).to_vec())
        } else if profile == CatalogProfile::JunieReference {
            if let Some(source) = crate::scenarios::capture_scenarios().find(|s| s.id == id) {
                let Host::Catalog(page) = source.host else {
                    return Err(format!("source scenario {id:?} is not a catalog page"));
                };
                (page, None, crate::catalog::SOURCE_NAV.to_vec())
            } else {
                return Err(format!("unknown catalog entry {id:?}"));
            }
        } else {
            return Err(format!("unknown catalog entry {id:?}"));
        };
        let mut app = App::new_with_nav(profile, ColorCapability::Truecolor, nav.clone());
        app.goto(page);
        let mut session = Self {
            app,
            profile,
            nav,
            page,
            scenario,
            // A session smaller than the shell's own floor can only ever paint
            // the too-small screen, so mount never accepts one.
            cols: cols.max(crate::shell::MIN_WIDTH),
            rows: rows.max(crate::shell::MIN_HEIGHT),
            elapsed_ms: 0,
            semantic_revision: 0,
        };
        // Draw once at mount: the shell seeds page focus during its first
        // draw, and host input that arrives before that draw would otherwise
        // be dropped by a page with no focused control.
        session.frame();
        Ok(session)
    }

    pub fn reset(&mut self) {
        self.app = App::new_with_nav(self.profile, ColorCapability::Truecolor, self.nav.clone());
        self.app.goto(self.page);
        self.elapsed_ms = 0;
        self.semantic_revision = self.semantic_revision.saturating_add(1);
    }

    /// Whether the mounted story accepts input. Layout-kind components are
    /// passive paint: they render but their metadata promises no interaction.
    fn interactive(&self) -> bool {
        self.scenario.map_or_else(
            || self.app.page_metadata().interactive,
            |scenario| scenario.interactive,
        )
    }

    pub fn dispatch(&mut self, event: DemoEvent) -> Result<DemoUpdate, String> {
        // A scenario-declared passive story only paints. Feeding it input
        // would let it mutate state its own inventory declares
        // non-interactive, so only the events that shape the paint (size,
        // clock, host focus) still apply. Page mounts keep their input: shell
        // chrome (navigation, help) must work even on a passive page.
        if self.scenario.is_some_and(|scenario| !scenario.interactive)
            && !matches!(
                event,
                DemoEvent::Tick { .. } | DemoEvent::Resize { .. } | DemoEvent::Focus { .. }
            )
        {
            return Ok(self.update(false));
        }
        let before = self.frame();
        let before_cursor = self.app.last_cursor;
        let is_tick = matches!(event, DemoEvent::Tick { .. });
        let before_semantic = (
            self.app.page,
            self.app.focus,
            self.app.help_open,
            self.app.status.clone(),
            self.app.flash,
        );
        if let DemoEvent::Tick { elapsed_ms } = &event {
            self.elapsed_ms = *elapsed_ms;
        }
        let tick = FrameTick::manual(
            termrock::runtime::Instant::now(),
            std::time::Duration::from_millis(self.elapsed_ms),
            std::time::Duration::from_millis(16),
        );
        match event {
            DemoEvent::Tick { .. } => self.app.on_tick(tick),
            DemoEvent::Resize { cols, rows } => {
                // Same floor as mount: below it the shell can only paint the
                // too-small screen.
                self.cols = cols.max(crate::shell::MIN_WIDTH);
                self.rows = rows.max(crate::shell::MIN_HEIGHT);
                let _ = self.app.handle_event(
                    Event::Resize {
                        width: self.cols,
                        height: self.rows,
                    },
                    tick,
                );
            }
            DemoEvent::Paste { text } => {
                let _ = self.app.handle_event(Event::Paste(text), tick);
            }
            DemoEvent::Key {
                key,
                kind,
                shift,
                ctrl,
                alt,
                meta,
            } => {
                if let Some(code) = parse_key_code(&key) {
                    let mut mods = KeyModifiers::NONE;
                    if ctrl {
                        mods = mods.with_ctrl();
                    }
                    if alt {
                        mods = mods.with_alt();
                    }
                    if meta {
                        mods = mods.with_ctrl();
                    }
                    if shift {
                        mods = mods.with_shift();
                    }
                    let ke_kind = match kind.as_str() {
                        "release" => KeyEventKind::Release,
                        "repeat" => KeyEventKind::Repeat,
                        _ => KeyEventKind::Press,
                    };
                    let _ = self.app.handle_event(
                        Event::Key(KeyEvent {
                            code,
                            modifiers: mods,
                            kind: ke_kind,
                            state: Default::default(),
                        }),
                        tick,
                    );
                }
            }
            DemoEvent::Pointer { kind, x, y, button } => {
                let btn = match button.as_str() {
                    "right" => MouseButton::Right,
                    "middle" => MouseButton::Middle,
                    _ => MouseButton::Left,
                };
                let mk = match kind.as_str() {
                    "down" => MouseEventKind::Down(btn),
                    "up" => MouseEventKind::Up(btn),
                    "drag" => MouseEventKind::Drag(btn),
                    _ => MouseEventKind::Moved,
                };
                let _ = self.app.handle_event(
                    Event::Mouse(MouseEvent {
                        kind: mk,
                        position: Position { x, y },
                        modifiers: KeyModifiers::NONE,
                    }),
                    tick,
                );
            }
            DemoEvent::Wheel {
                delta_x,
                delta_y,
                x,
                y,
            } => {
                if delta_x != 0 {
                    let kind = if delta_x < 0 {
                        MouseEventKind::ScrollLeft
                    } else {
                        MouseEventKind::ScrollRight
                    };
                    let _ = self.app.handle_event(
                        Event::Mouse(MouseEvent {
                            kind,
                            position: Position { x, y },
                            modifiers: KeyModifiers::NONE,
                        }),
                        tick,
                    );
                }
                if delta_y != 0 {
                    let kind = if delta_y < 0 {
                        MouseEventKind::ScrollUp
                    } else {
                        MouseEventKind::ScrollDown
                    };
                    let _ = self.app.handle_event(
                        Event::Mouse(MouseEvent {
                            kind,
                            position: Position { x, y },
                            modifiers: KeyModifiers::NONE,
                        }),
                        tick,
                    );
                }
            }
            DemoEvent::Focus { focused } => {
                let event = if focused {
                    Event::FocusGained
                } else {
                    Event::FocusLost
                };
                let _ = self.app.handle_event(event, tick);
            }
        }
        let after = self.frame();
        let changed = before != after || before_cursor != self.app.last_cursor;
        let after_semantic = (
            self.app.page,
            self.app.focus,
            self.app.help_open,
            self.app.status.clone(),
            self.app.flash,
        );
        let semantic_changed = if is_tick {
            before_semantic != after_semantic
        } else {
            changed
        };
        if semantic_changed {
            self.semantic_revision = self.semantic_revision.saturating_add(1);
        }
        Ok(self.update(changed))
    }

    fn update(&self, changed: bool) -> DemoUpdate {
        let metadata = self.app.page_metadata();
        let hints = if self.scenario.is_some_and(|scenario| !scenario.interactive) {
            Vec::new()
        } else {
            metadata.hints.iter().map(|(key, _)| *key).collect()
        };
        let functional_deadline = self.app.flash.is_some() || self.app.status.is_some();
        let deadline_kind = if functional_deadline {
            Some("functional")
        } else if metadata.animating {
            Some("visual-motion")
        } else {
            None
        };
        DemoUpdate {
            changed,
            outcome: None,
            hints,
            interactive: self.interactive(),
            captures_text_input: metadata.captures_text_input,
            next_deadline_ms: deadline_kind.map(|_| 80),
            deadline_kind,
            semantic_revision: self.semantic_revision,
        }
    }

    pub fn frame(&mut self) -> TerminalFrame {
        let mut term = Terminal::new(TestBackend::new(self.cols, self.rows)).expect("backend");
        let tick = FrameTick::manual(
            termrock::runtime::Instant::now(),
            std::time::Duration::from_millis(self.elapsed_ms),
            std::time::Duration::from_millis(16),
        );
        term.draw(|f| self.app.render(f, tick)).expect("draw");
        let cursor = term.get_cursor_position().ok();
        let snap = Snapshot::from_buffer(term.backend().buffer(), cursor, false);
        let cells = frame_cells(&snap);
        let entry = self.app.nav().iter().find(|e| e.id == self.page);
        let story_id = self
            .scenario
            .map(|scenario| scenario.id.to_owned())
            .or_else(|| entry.map(|e| crate::catalog::normalize(e.label)))
            .unwrap_or_default();
        let title = self
            .scenario
            .map(|scenario| scenario.title.to_owned())
            .or_else(|| entry.map(|e| e.label.to_owned()))
            .unwrap_or_default();
        let component = self
            .scenario
            .map(|scenario| scenario.component.to_owned())
            .or_else(|| entry.map(|e| e.section.to_owned()))
            .unwrap_or_default();
        let interactive = self.interactive();
        TerminalFrame {
            story_id,
            title,
            component,
            cols: snap.cols,
            rows: snap.rows,
            story_cols: snap.cols,
            story_rows: snap.rows,
            cells,
            cursor: self
                .app
                .last_cursor
                .map(|position| [position.x, position.y]),
            cursor_visible: self.app.last_cursor.is_some(),
            interactive,
            theme: "junie".into(),
        }
    }
}

/// Headless adapter for the real TablePro application.
///
/// This owns only terminal-host concerns. Rendering and interaction remain in
/// [`TableProApp`], the same application mounted by the catalog page and the
/// standalone `tablepro` binary.
pub struct TableProFrameSession {
    app: TableProApp,
    cols: u16,
    rows: u16,
    elapsed_ms: u64,
}

impl TableProFrameSession {
    /// Mount TablePro at an exact headless terminal size.
    pub fn mount(connect: Option<&str>, cols: u16, rows: u16) -> Result<Self, String> {
        if cols == 0 || rows == 0 {
            return Err("tablepro frame requires positive dimensions".to_owned());
        }
        let mut app = TableProApp::new(ColorCapability::Truecolor);
        if let Some(name) = connect {
            app.connect_named(name)?;
        }
        Ok(Self {
            app,
            cols,
            rows,
            elapsed_ms: 0,
        })
    }

    /// Dispatch one CLI key token through TablePro's real event handler.
    pub fn dispatch_key(&mut self, key: &str) -> Result<(), String> {
        // A render registers the application's focus and hit-test scene before
        // keyboard routing, matching the native runtime's first frame.
        let _ = self.frame();
        let (code, modifiers) = parse_key_with_modifiers(key)?;
        let tick = FrameTick::manual(
            termrock::runtime::Instant::now(),
            std::time::Duration::from_millis(self.elapsed_ms),
            std::time::Duration::from_millis(16),
        );
        let _ = self.app.handle_event(
            Event::Key(KeyEvent {
                code,
                modifiers,
                kind: KeyEventKind::Press,
                state: Default::default(),
            }),
            tick,
        );
        Ok(())
    }

    /// Render the mounted TablePro application into the canonical frame grid.
    #[must_use]
    pub fn frame(&mut self) -> TerminalFrame {
        let mut term = Terminal::new(TestBackend::new(self.cols, self.rows)).expect("backend");
        let tick = FrameTick::manual(
            termrock::runtime::Instant::now(),
            std::time::Duration::from_millis(self.elapsed_ms),
            std::time::Duration::from_millis(16),
        );
        term.draw(|f| self.app.render(f, tick)).expect("draw");
        let cursor = self.app.last_cursor;
        let snap = Snapshot::from_buffer(term.backend().buffer(), cursor, false);
        TerminalFrame {
            story_id: "tablepro".to_owned(),
            title: "TablePro".to_owned(),
            component: "Applications".to_owned(),
            cols: snap.cols,
            rows: snap.rows,
            story_cols: snap.cols,
            story_rows: snap.rows,
            cells: frame_cells(&snap),
            cursor: cursor.map(|position| [position.x, position.y]),
            cursor_visible: cursor.is_some(),
            interactive: true,
            theme: "junie".to_owned(),
        }
    }
}

/// Render one real TablePro state into a serializable terminal frame.
pub fn tablepro_frame(
    connect: Option<&str>,
    cols: u16,
    rows: u16,
    keys: &[String],
) -> Result<TerminalFrame, String> {
    let mut session = TableProFrameSession::mount(connect, cols, rows)?;
    for key in keys {
        session.dispatch_key(key)?;
    }
    Ok(session.frame())
}

fn parse_key_code(key: &str) -> Option<KeyCode> {
    match key {
        "ArrowUp" | "Up" => Some(KeyCode::Up),
        "ArrowDown" | "Down" => Some(KeyCode::Down),
        "ArrowLeft" | "Left" => Some(KeyCode::Left),
        "ArrowRight" | "Right" => Some(KeyCode::Right),
        "Enter" => Some(KeyCode::Enter),
        "Escape" | "Esc" => Some(KeyCode::Esc),
        "Tab" => Some(KeyCode::Tab),
        "Backspace" => Some(KeyCode::Backspace),
        "Delete" => Some(KeyCode::Delete),
        "Home" => Some(KeyCode::Home),
        "End" => Some(KeyCode::End),
        "PageUp" => Some(KeyCode::PageUp),
        "PageDown" => Some(KeyCode::PageDown),
        " " | "Space" => Some(KeyCode::Char(' ')),
        s if s.chars().count() == 1 => s.chars().next().map(KeyCode::Char),
        _ => None,
    }
}

fn parse_key_with_modifiers(key: &str) -> Result<(KeyCode, KeyModifiers), String> {
    let mut parts = key.split('+').collect::<Vec<_>>();
    let base = parts
        .pop()
        .filter(|part| !part.is_empty())
        .ok_or_else(|| format!("unknown tablepro key {key:?}"))?;
    let mut modifiers = KeyModifiers::NONE;
    for modifier in parts {
        match modifier.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => modifiers = modifiers.with_ctrl(),
            "alt" => modifiers = modifiers.with_alt(),
            "shift" => modifiers = modifiers.with_shift(),
            "meta" | "super" | "cmd" | "command" => modifiers = modifiers.with_ctrl(),
            other => return Err(format!("unknown tablepro key modifier {other:?}")),
        }
    }
    let code = parse_key_code(base).ok_or_else(|| format!("unknown tablepro key {key:?}"))?;
    Ok((code, modifiers))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_lists_source_prefix_and_tablepro() {
        let c = catalog();
        assert!(c.iter().any(|d| d.id == "overview"));
        assert!(c.iter().any(|d| d.id == "tablepro"));
        assert!(c.iter().any(|d| d.title == "Buttons"));
        let overview = c.iter().find(|d| d.id == "overview").expect("overview");
        assert!(!overview.interactive);
        assert_eq!(
            overview.description,
            "Tokens and principles behind every component"
        );
        let json = serde_json::to_value(&c[0]).expect("descriptor json");
        assert!(json.get("interactionKind").is_some());
        assert!(json.get("interaction_kind").is_none());
    }

    #[test]
    fn catalog_exposes_all_canonical_scenario_ids_without_duplicates() {
        let entries = catalog();
        let ids: BTreeSet<_> = entries.iter().map(|entry| entry.id.as_str()).collect();
        assert_eq!(ids.len(), entries.len());

        for scenario in catalog_scenarios() {
            assert!(
                ids.contains(scenario.id),
                "scenario {} is missing from the host catalog",
                scenario.id
            );
        }
        assert_eq!(
            entries.len(),
            nav_entries(CatalogProfile::TermRock).len() + catalog_scenarios().len()
        );
    }

    #[test]
    fn profile_catalogs_are_scoped_and_unique() {
        let source_pages = nav_entries(CatalogProfile::JunieReference);
        let reference = catalog_for_profile(CatalogProfile::JunieReference);
        let reference_ids: BTreeSet<_> = reference.iter().map(|entry| entry.id.as_str()).collect();
        assert_eq!(reference_ids.len(), reference.len());
        for scenario in catalog_scenarios() {
            let source_visible = page_is_visible(CatalogProfile::JunieReference, scenario.page);
            assert_eq!(
                reference_ids.contains(scenario.id),
                source_visible,
                "scenario {} has incorrect JunieReference visibility",
                scenario.id
            );
        }
        assert_eq!(
            reference.len(),
            source_pages.len()
                + catalog_scenarios_for_profile(CatalogProfile::JunieReference).count()
        );
    }

    #[test]
    fn mount_overview_and_key() {
        let mut s = CatalogSession::mount("overview", 120, 40).unwrap();
        let frame = s.frame();
        assert_eq!(frame.cols, 120);
        assert!(frame.cells.iter().any(|c| c.ch.contains("Junie")
            || c.ch.contains("T")
            || c.ch.contains("▪")
            || c.ch.contains("O")));
        let event: DemoEvent =
            serde_json::from_str(r#"{"type":"key","key":"]","kind":"press"}"#).unwrap();
        let update = s.dispatch(event).unwrap();
        assert!(update.changed);
        assert!(update.semantic_revision > 0);
        let json = serde_json::to_value(s.frame()).expect("frame json");
        assert!(json.get("story_cols").is_some());
        assert!(json.get("story_rows").is_some());
        assert!(json.get("storyCols").is_none());
        assert!(json["cells"][0].get("underline").is_some());
        assert!(json["cells"][0].get("strike").is_some());
        assert!(
            serde_json::to_value(update)
                .expect("update json")
                .get("capturesTextInput")
                .is_some()
        );
        let update = s.dispatch(DemoEvent::Focus { focused: true }).unwrap();
        assert!(!update.changed);
    }

    #[test]
    fn representative_scenario_mount_uses_canonical_page_renderer() {
        let mut session = CatalogSession::mount("button/activation", 80, 24).unwrap();
        let frame = session.frame();
        assert_eq!(frame.story_id, "button/activation");
        assert_eq!(frame.component, "Button");
        assert_eq!(frame.title, "Button");
        assert_eq!(frame.cols, 80);
        assert_eq!(frame.rows, 24);
    }

    #[test]
    fn reference_profile_rejects_out_of_profile_scenarios() {
        let extension = catalog_scenarios()
            .into_iter()
            .find(|scenario| !page_is_visible(CatalogProfile::JunieReference, scenario.page))
            .expect("catalog has a TermRock-only scenario");
        let error =
            CatalogSession::mount_profile(extension.id, 80, 24, CatalogProfile::JunieReference)
                .err()
                .expect("JunieReference must reject TermRock-only scenarios");
        assert!(error.contains("not available"), "{error}");
        assert!(
            CatalogSession::mount_profile(
                "button/activation",
                80,
                24,
                CatalogProfile::JunieReference,
            )
            .is_ok()
        );
    }

    #[test]
    fn reference_profile_shares_session_renderer_and_source_navigation() {
        let mut reference =
            CatalogSession::mount_profile("overview", 120, 40, CatalogProfile::JunieReference)
                .unwrap();
        let frame = reference.frame();
        assert_eq!(frame.title, "Overview");
        assert_eq!(frame.cursor_visible, false);
        assert_eq!(reference.profile, CatalogProfile::JunieReference);
        assert!(reference.app.nav().iter().all(|entry| entry.id.0 < 20));
    }
}
