// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Host event JSON shape extracted from termrock-lookbook demo.rs (Apache-2.0).

//! Persistent catalog sessions for the WASM / browser host.

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Position;
use serde::{Deserialize, Serialize};
use termrock::input::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use termrock::runtime::FrameTick;
use termrock::style::{ColorCapability, color_to_rgb};

use crate::catalog::{CatalogProfile, NavEntry, PageId, nav_entries};
use crate::shell::App;
use crate::snapshot::Snapshot;

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
    pub next_deadline_ms: Option<u64>,
    pub deadline_kind: Option<&'static str>,
    pub semantic_revision: u64,
}

/// One truecolor cell for the browser host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FrameCell {
    pub ch: String,
    pub fg: [u8; 3],
    pub bg: [u8; 3],
    #[serde(default)]
    pub bold: bool,
}

/// Full terminal frame for a catalog page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalFrame {
    pub story_id: String,
    pub title: String,
    pub component: String,
    pub cols: u16,
    pub rows: u16,
    pub cells: Vec<FrameCell>,
    pub interactive: bool,
    pub theme: String,
}

/// Catalog pages the web host can mount.
#[must_use]
pub fn catalog() -> Vec<DemoDescriptor> {
    nav_entries(CatalogProfile::TermRock)
        .iter()
        .map(descriptor)
        .collect()
}

fn descriptor(e: &NavEntry) -> DemoDescriptor {
    DemoDescriptor {
        id: crate::catalog::normalize(e.label),
        title: e.label,
        component: e.section,
        description: e.label,
        cols: 120,
        rows: 40,
        interactive: true,
        hints: vec!["Tab", "Enter", "[", "]"],
    }
}

/// One long-lived catalog page instance.
pub struct CatalogSession {
    app: App,
    page: PageId,
    cols: u16,
    rows: u16,
    elapsed_ms: u64,
    semantic_revision: u64,
}

impl CatalogSession {
    pub fn mount(id: &str, cols: u16, rows: u16) -> Result<Self, String> {
        let profile = CatalogProfile::TermRock;
        let page = PageId::from_name(id, nav_entries(profile))
            .ok_or_else(|| format!("unknown catalog page {id:?}"))?;
        let mut app = App::new(profile, ColorCapability::Truecolor);
        app.goto(page);
        Ok(Self {
            app,
            page,
            cols: cols.max(8),
            rows: rows.max(4),
            elapsed_ms: 0,
            semantic_revision: 0,
        })
    }

    pub fn reset(&mut self) {
        let profile = CatalogProfile::TermRock;
        self.app = App::new(profile, ColorCapability::Truecolor);
        self.app.goto(self.page);
        self.elapsed_ms = 0;
        self.semantic_revision = self.semantic_revision.saturating_add(1);
    }

    pub fn dispatch(&mut self, event: DemoEvent) -> Result<DemoUpdate, String> {
        let tick = FrameTick::manual(
            termrock::runtime::Instant::now(),
            std::time::Duration::from_millis(self.elapsed_ms),
            std::time::Duration::from_millis(16),
        );
        match event {
            DemoEvent::Tick { elapsed_ms } => {
                self.elapsed_ms = elapsed_ms;
                self.app.on_tick(tick);
            }
            DemoEvent::Resize { cols, rows } => {
                self.cols = cols.max(8);
                self.rows = rows.max(4);
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
                    if alt || meta {
                        mods = mods.with_alt();
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
            DemoEvent::Wheel { delta_y, x, y, .. } => {
                let mk = if delta_y < 0 {
                    MouseEventKind::ScrollUp
                } else {
                    MouseEventKind::ScrollDown
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
            DemoEvent::Focus { .. } => {}
        }
        self.semantic_revision = self.semantic_revision.saturating_add(1);
        Ok(self.update(true))
    }

    fn update(&self, changed: bool) -> DemoUpdate {
        DemoUpdate {
            changed,
            outcome: None,
            hints: vec!["Tab", "Enter", "[", "]"],
            interactive: true,
            next_deadline_ms: None,
            deadline_kind: Some("functional"),
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
        let cells = snap
            .cells
            .iter()
            .map(|c| FrameCell {
                ch: if c.glyph.is_empty() {
                    " ".into()
                } else {
                    c.glyph.clone()
                },
                fg: color_to_rgb(c.fg, true),
                bg: color_to_rgb(c.bg, false),
                bold: c.modifier.contains(ratatui::style::Modifier::BOLD),
            })
            .collect();
        let entry = nav_entries(CatalogProfile::TermRock)
            .iter()
            .find(|e| e.id == self.page);
        TerminalFrame {
            story_id: entry
                .map(|e| crate::catalog::normalize(e.label))
                .unwrap_or_default(),
            title: entry.map(|e| e.label.to_owned()).unwrap_or_default(),
            component: entry.map(|e| e.section.to_owned()).unwrap_or_default(),
            cols: snap.cols,
            rows: snap.rows,
            cells,
            interactive: true,
            theme: "junie".into(),
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_lists_source_prefix_and_tablepro() {
        let c = catalog();
        assert!(c.iter().any(|d| d.id == "overview"));
        assert!(c.iter().any(|d| d.id == "tablepro"));
        assert!(c.iter().any(|d| d.title == "Buttons"));
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
    }
}
