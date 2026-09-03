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
use crate::shell::{App, PageMetadata};
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
    pub interactive: bool,
    pub theme: String,
}

/// Catalog pages the web host can mount.
#[must_use]
pub fn catalog() -> Vec<DemoDescriptor> {
    let mut app = App::new(CatalogProfile::TermRock, ColorCapability::Truecolor);
    nav_entries(CatalogProfile::TermRock)
        .iter()
        .map(|entry| {
            app.goto(entry.id);
            descriptor(entry, &app.page_metadata())
        })
        .collect()
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
        let before = self.frame();
        let before_cursor = self.app.last_cursor;
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
        if changed {
            self.semantic_revision = self.semantic_revision.saturating_add(1);
        }
        Ok(self.update(changed))
    }

    fn update(&self, changed: bool) -> DemoUpdate {
        let metadata = self.app.page_metadata();
        DemoUpdate {
            changed,
            outcome: None,
            hints: metadata.hints.iter().map(|(key, _)| *key).collect(),
            interactive: metadata.interactive,
            captures_text_input: metadata.captures_text_input,
            next_deadline_ms: metadata.animating.then_some(80),
            deadline_kind: metadata.animating.then_some("visual-motion"),
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
            .map(|c| {
                let reversed = c.modifier.contains(ratatui::style::Modifier::REVERSED);
                let mut fg = color_to_rgb(c.fg, true);
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
            story_cols: snap.cols,
            story_rows: snap.rows,
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
}
