// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Persistent demo sessions shared by native Lookbook and web hosts.

use std::cell::Cell;

use ratatui::{
    Terminal,
    backend::TestBackend,
    layout::{Position, Rect},
    style::Style,
    widgets::{Block, Clear},
};
use serde::{Deserialize, Serialize};
use termrock::{
    input::{
        Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers, MouseButton,
        MouseEvent, MouseEventKind,
    },
    style::{PREVIEW_CARD, RolePalette},
};

use crate::{
    frame::{STORY_PAD, TerminalFrame, encode_buffer, story_by_id},
    interactors::StoryInteraction,
    stories::{Story, stories},
};

thread_local! {
    static RENDER_ELAPSED_MS: Cell<Option<u64>> = const { Cell::new(None) };
}

/// Build a deterministic frame tick from the active demo host time.
pub(crate) fn demo_tick(default_elapsed_ms: u64) -> termrock::runtime::FrameTick {
    let elapsed_ms = RENDER_ELAPSED_MS
        .with(Cell::get)
        .unwrap_or(default_elapsed_ms);
    termrock::runtime::FrameTick::manual(
        termrock::runtime::Instant::now(),
        std::time::Duration::from_millis(elapsed_ms),
        std::time::Duration::from_millis(16),
    )
}

/// Static catalog metadata available before mounting a demo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DemoDescriptor {
    /// Stable story/demo identifier.
    pub id: &'static str,
    /// Human-readable story title.
    pub title: &'static str,
    /// Public component or pattern type demonstrated.
    pub component: &'static str,
    /// Short purpose statement.
    pub description: &'static str,
    /// Preferred inner width in cells.
    pub cols: u16,
    /// Preferred inner height in cells.
    pub rows: u16,
    /// Whether this demo accepts real input.
    pub interactive: bool,
    /// High-level interaction family.
    pub interaction_kind: &'static str,
    /// Actions supported by the current demo.
    pub hints: Vec<&'static str>,
}

impl From<Story> for DemoDescriptor {
    fn from(story: Story) -> Self {
        let interactor_hints = story.make_interactor().hints();
        Self {
            id: story.id,
            title: story.title,
            component: story.component,
            description: story.description,
            cols: story.width,
            rows: story.height,
            interactive: story.interactive,
            interaction_kind: interaction_kind(story.component, story.interactive),
            hints: if interactor_hints.is_empty() {
                hints_for(story.component, story.interactive)
            } else {
                interactor_hints
            },
        }
    }
}

/// Host-to-demo event serialized by the WASM adapter.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum DemoEvent {
    /// Keyboard lifecycle event.
    Key {
        /// Browser-style key name (`ArrowDown`, `Enter`, or one character).
        key: String,
        /// `press`, `repeat`, or `release`.
        #[serde(default = "default_key_kind")]
        kind: String,
        /// Shift modifier.
        #[serde(default)]
        shift: bool,
        /// Control modifier.
        #[serde(default)]
        ctrl: bool,
        /// Alt/Option modifier.
        #[serde(default)]
        alt: bool,
        /// Meta/Super modifier; maps to Alt in the neutral vocabulary.
        #[serde(default)]
        meta: bool,
    },
    /// Pointer motion/button/drag at an exact terminal cell.
    Pointer {
        /// `move`, `down`, `up`, or `drag`.
        kind: String,
        /// Terminal column including preview padding.
        x: u16,
        /// Terminal row including preview padding.
        y: u16,
        /// `left`, `right`, or `middle`.
        #[serde(default = "default_button")]
        button: String,
    },
    /// Discrete wheel input. Negative vertical values scroll up.
    Wheel {
        /// Horizontal wheel delta.
        #[serde(default)]
        delta_x: i16,
        /// Vertical wheel delta.
        #[serde(default)]
        delta_y: i16,
        /// Terminal column including preview padding.
        x: u16,
        /// Terminal row including preview padding.
        y: u16,
    },
    /// Bracketed-paste text.
    Paste {
        /// Exact pasted Unicode text.
        text: String,
    },
    /// New inner demo grid size.
    Resize {
        /// New inner width.
        cols: u16,
        /// New inner height.
        rows: u16,
    },
    /// Host focus transition.
    Focus {
        /// True on focus gained, false on focus lost.
        focused: bool,
    },
    /// Advance deterministic host time.
    Tick {
        /// Monotonic milliseconds supplied by the host.
        elapsed_ms: u64,
    },
}

/// Result of one event dispatch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DemoUpdate {
    /// True when persistent demo state changed.
    pub changed: bool,
    /// Latest visible host outcome, when an action changed state.
    pub outcome: Option<String>,
    /// Current valid action hints.
    pub hints: Vec<&'static str>,
    /// Whether the demo accepts input.
    pub interactive: bool,
    /// Whether the current state accepts literal text and paste payloads.
    pub captures_text_input: bool,
    /// Whether the host should keep ticking this demo.
    pub next_deadline_ms: Option<u64>,
}

/// Which shipped preset a palette is, for frame metadata.
///
/// The demo used to report `"phosphor"` whatever the host had picked, so every
/// docs preview claimed the same theme (plans/011 Step 4).
fn theme_id_for(palette: &RolePalette) -> &'static str {
    for (id, preset) in [
        ("phosphor", RolePalette::tailrocks_phosphor()),
        ("slate", RolePalette::slate()),
        ("paper", RolePalette::paper()),
        ("ansi", RolePalette::ansi()),
        ("high-contrast", RolePalette::high_contrast()),
    ] {
        if palette == &preset {
            return id;
        }
    }
    "custom"
}

/// One long-lived Rust demo instance.
pub struct DemoSession {
    story: Story,
    interactor: Box<dyn StoryInteraction>,
    theme: RolePalette,
    /// Stable id of the palette the host picked, for honest frame metadata.
    theme_id: String,
    cols: u16,
    rows: u16,
    focused: bool,
    elapsed_ms: u64,
    outcome: Option<String>,
}

impl DemoSession {
    /// Mount a fresh session by stable demo id.
    pub fn mount(id: &str, cols: Option<u16>, rows: Option<u16>) -> Result<Self, String> {
        let story = story_by_id(id).ok_or_else(|| format!("unknown demo id: {id}"))?;
        let theme = RolePalette::default();
        let mut interactor = story.make_interactor();
        interactor.set_theme(theme.clone());
        let theme_id = theme_id_for(&theme).to_string();
        Ok(Self {
            story,
            interactor,
            theme,
            theme_id,
            cols: cols.unwrap_or(story.width).max(8),
            rows: rows.unwrap_or(story.height).max(4),
            focused: false,
            elapsed_ms: 0,
            outcome: None,
        })
    }

    /// Stable descriptor for this mounted demo.
    #[must_use]
    pub fn descriptor(&self) -> DemoDescriptor {
        let mut descriptor: DemoDescriptor = self.story.into();
        let hints = self.interactor.hints();
        if !hints.is_empty() {
            descriptor.hints = hints;
        }
        descriptor
    }

    /// Reset all demo-owned state while preserving host size and theme.
    pub fn reset(&mut self) {
        self.interactor = self.story.make_interactor();
        self.interactor.set_theme(self.theme.clone());
        self.focused = false;
        self.elapsed_ms = 0;
        self.outcome = Some("Demo reset".to_owned());
    }

    /// Replace the semantic palette used by both preview hosts.
    pub fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme.clone();
        self.theme_id = theme_id_for(&theme).to_string();
        self.interactor.set_theme(theme);
    }

    /// Dispatch one normalized host event.
    pub fn dispatch(&mut self, event: DemoEvent) -> Result<DemoUpdate, String> {
        if let DemoEvent::Tick { elapsed_ms } = event {
            return Ok(self.tick(elapsed_ms));
        }
        let neutral = self.decode_event(event)?;
        Ok(match neutral {
            Some(event) => self.dispatch_event(event),
            None => self.update(false),
        })
    }

    /// Dispatch one already-normalized TermRock event.
    ///
    /// Native Lookbook uses this path; the WASM adapter decodes browser JSON
    /// into the same event vocabulary before reaching the same state machine.
    pub fn dispatch_event(&mut self, event: Event) -> DemoUpdate {
        self.dispatch_event_in(event, self.preview_area())
    }

    /// Dispatch a normalized event using the host's actual paint area.
    ///
    /// This keeps native absolute hit geometry and the browser's padded local
    /// geometry on one state machine without translating widget hit logic.
    pub fn dispatch_event_in(&mut self, event: Event, preview_area: Rect) -> DemoUpdate {
        let (changed, announce_fallback) = match event {
            Event::Resize { width, height } => {
                let next_cols = width.max(8);
                let next_rows = height.max(4);
                let changed = (self.cols, self.rows) != (next_cols, next_rows);
                self.cols = next_cols;
                self.rows = next_rows;
                (changed, false)
            }
            Event::FocusGained => {
                let changed = !self.focused;
                self.focused = true;
                (changed, false)
            }
            Event::FocusLost => {
                let changed = self.focused;
                self.focused = false;
                (changed, false)
            }
            Event::Mouse(mouse) if self.story.interactive => {
                let announce_fallback = !matches!(mouse.kind, MouseEventKind::Moved);
                (
                    self.interactor
                        .handle_event(Event::Mouse(mouse), preview_area),
                    announce_fallback,
                )
            }
            Event::Key(key) if self.story.interactive => {
                let announce_fallback = key.kind != KeyEventKind::Release;
                (
                    self.interactor.handle_event(Event::Key(key), preview_area),
                    announce_fallback,
                )
            }
            event if self.story.interactive => {
                (self.interactor.handle_event(event, preview_area), true)
            }
            _ => (false, false),
        };
        if let Some(outcome) = self.interactor.take_outcome() {
            self.outcome = Some(outcome);
        } else if changed && announce_fallback && self.outcome.is_none() {
            self.outcome = Some(format!("{} updated", self.story.component));
        }
        self.update(changed)
    }

    /// Advance this demo using host-injected monotonic time.
    pub fn tick(&mut self, elapsed_ms: u64) -> DemoUpdate {
        let changed = self.interactor.handle_tick(elapsed_ms)
            || (self.elapsed_ms != elapsed_ms && timed_component(self.story.component));
        self.elapsed_ms = elapsed_ms;
        if let Some(outcome) = self.interactor.take_outcome() {
            self.outcome = Some(outcome);
        }
        self.update(changed)
    }

    /// Paint current persistent state into the existing truecolor cell format.
    #[must_use]
    pub fn frame(&mut self) -> TerminalFrame {
        let width = self.cols.saturating_add(STORY_PAD * 2);
        let height = self.rows.saturating_add(STORY_PAD * 2);
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("in-memory terminal");
        terminal
            .draw(|frame| {
                let area = frame.area();
                frame.render_widget(
                    Block::default().style(Style::default().bg(PREVIEW_CARD)),
                    area,
                );
                let inner = self.preview_area();
                frame.render_widget(Clear, inner);
                // The story's ground is the palette's canvas (plans/011).
                frame.buffer_mut().set_style(
                    inner,
                    crate::design::lookbook_system(self.theme.clone())
                        .style(termrock::style::Role::Canvas),
                );
                self.render_into(frame, inner);
            })
            .expect("in-memory draw");
        let (cols, rows, cells) = encode_buffer(terminal.backend().buffer());
        TerminalFrame {
            story_id: self.story.id.into(),
            title: self.story.title.into(),
            component: self.story.component.into(),
            cols,
            rows,
            story_cols: self.cols,
            story_rows: self.rows,
            cells,
            interactive: self.story.interactive,
            theme: self.theme_id.clone(),
        }
    }

    /// Paint the mounted demo directly into a native Ratatui frame.
    pub fn render_into(&mut self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        RENDER_ELAPSED_MS.with(|elapsed| {
            let previous = elapsed.replace(Some(self.elapsed_ms));
            self.interactor.render(frame, area);
            elapsed.set(previous);
        });
    }

    /// Current deterministic configuration controls used by native Lookbook.
    #[must_use]
    pub fn knobs(&self) -> &[crate::knobs::Knob] {
        self.interactor.knobs()
    }

    /// Edit one selected native configuration control.
    pub fn handle_knob_key(&mut self, selected: usize, key: KeyEvent) -> DemoUpdate {
        let changed = self.interactor.handle_knob_key(selected, key);
        if let Some(outcome) = self.interactor.take_outcome() {
            self.outcome = Some(outcome);
        }
        self.update(changed)
    }

    /// Paint the rich editor for one selected native configuration control.
    pub fn render_knob_editor(
        &mut self,
        selected: usize,
        frame: &mut ratatui::Frame<'_>,
        area: Rect,
    ) {
        self.interactor.render_knob_editor(selected, frame, area);
    }

    /// Let the mounted demo consume Escape before native shell navigation.
    pub fn dispatch_preview_escape(&mut self, key: KeyEvent) -> DemoUpdate {
        let changed = self.interactor.handle_preview_escape(key);
        if let Some(outcome) = self.interactor.take_outcome() {
            self.outcome = Some(outcome);
        }
        self.update(changed)
    }

    /// Whether literal text and paste currently belong to the mounted demo.
    #[must_use]
    pub fn captures_text_input(&self) -> bool {
        self.interactor.captures_text_input()
    }

    /// Whether the selected native control currently captures literal text.
    #[must_use]
    pub fn knob_captures_text_input(&self, selected: usize) -> bool {
        self.interactor.knob_captures_text_input(selected)
    }

    /// Current state-aware hints and visible outcome without mutating state.
    #[must_use]
    pub fn current_update(&self) -> DemoUpdate {
        self.update(false)
    }

    fn preview_area(&self) -> Rect {
        Rect::new(STORY_PAD, STORY_PAD, self.cols, self.rows)
    }

    fn update(&self, changed: bool) -> DemoUpdate {
        let interactor_hints = self.interactor.hints();
        DemoUpdate {
            changed,
            outcome: self.outcome.clone(),
            hints: if interactor_hints.is_empty() {
                hints_for(self.story.component, self.story.interactive)
            } else {
                interactor_hints
            },
            interactive: self.story.interactive,
            captures_text_input: self.interactor.captures_text_input(),
            next_deadline_ms: self
                .interactor
                .next_deadline_ms(self.elapsed_ms)
                .or_else(|| {
                    timed_component(self.story.component)
                        .then_some(self.elapsed_ms.saturating_add(100))
                }),
        }
    }

    fn decode_event(&mut self, event: DemoEvent) -> Result<Option<Event>, String> {
        match event {
            DemoEvent::Key {
                key,
                kind,
                shift,
                ctrl,
                alt,
                meta,
            } => {
                let Some(code) = decode_key_code(&key) else {
                    return Ok(None);
                };
                let mut modifiers = KeyModifiers::NONE;
                if shift {
                    modifiers |= KeyModifiers::SHIFT;
                }
                // Browser Command is the platform accelerator equivalent of
                // terminal Control for editor shortcuts. Option remains Alt.
                if ctrl || meta {
                    modifiers |= KeyModifiers::CONTROL;
                }
                if alt {
                    modifiers |= KeyModifiers::ALT;
                }
                let kind = match kind.as_str() {
                    "press" => KeyEventKind::Press,
                    "repeat" => KeyEventKind::Repeat,
                    "release" => KeyEventKind::Release,
                    other => return Err(format!("unknown key event kind: {other}")),
                };
                Ok(Some(Event::Key(KeyEvent {
                    code,
                    modifiers,
                    kind,
                    state: KeyEventState::NONE,
                })))
            }
            DemoEvent::Pointer { kind, x, y, button } => {
                let button = decode_button(&button)?;
                let kind = match kind.as_str() {
                    "move" => MouseEventKind::Moved,
                    "down" => MouseEventKind::Down(button),
                    "up" => MouseEventKind::Up(button),
                    "drag" => MouseEventKind::Drag(button),
                    other => return Err(format!("unknown pointer event kind: {other}")),
                };
                Ok(Some(Event::Mouse(MouseEvent {
                    kind,
                    position: Position::new(x, y),
                    modifiers: KeyModifiers::NONE,
                })))
            }
            DemoEvent::Wheel {
                delta_x,
                delta_y,
                x,
                y,
            } => {
                let kind = if delta_y < 0 {
                    MouseEventKind::ScrollUp
                } else if delta_y > 0 {
                    MouseEventKind::ScrollDown
                } else if delta_x < 0 {
                    MouseEventKind::ScrollLeft
                } else if delta_x > 0 {
                    MouseEventKind::ScrollRight
                } else {
                    return Ok(None);
                };
                Ok(Some(Event::Mouse(MouseEvent {
                    kind,
                    position: Position::new(x, y),
                    modifiers: KeyModifiers::NONE,
                })))
            }
            DemoEvent::Paste { text } => Ok(Some(Event::Paste(text))),
            DemoEvent::Resize { cols, rows } => Ok(Some(Event::Resize {
                width: cols,
                height: rows,
            })),
            DemoEvent::Focus { focused: true } => Ok(Some(Event::FocusGained)),
            DemoEvent::Focus { focused: false } => Ok(Some(Event::FocusLost)),
            DemoEvent::Tick { .. } => unreachable!("tick handled before neutral decoding"),
        }
    }
}

/// Full shared catalog used by both hosts.
#[must_use]
pub fn catalog() -> Vec<DemoDescriptor> {
    stories()
        .into_iter()
        .map(|story| {
            let mut descriptor: DemoDescriptor = story.into();
            let hints = story.make_interactor().hints();
            if !hints.is_empty() {
                descriptor.hints = hints;
            }
            descriptor
        })
        .collect()
}

fn default_key_kind() -> String {
    "press".to_owned()
}

fn default_button() -> String {
    "left".to_owned()
}

fn decode_button(value: &str) -> Result<MouseButton, String> {
    match value {
        "left" => Ok(MouseButton::Left),
        "right" => Ok(MouseButton::Right),
        "middle" => Ok(MouseButton::Middle),
        other => Err(format!("unknown pointer button: {other}")),
    }
}

fn decode_key_code(value: &str) -> Option<KeyCode> {
    match value {
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
        raw if raw.chars().count() == 1 => raw.chars().next().map(KeyCode::Char),
        _ => None,
    }
}

fn interaction_kind(component: &str, interactive: bool) -> &'static str {
    if timed_component(component) {
        return "timed-state";
    }
    if !interactive {
        return "passive-paint";
    }
    match component {
        "TextArea" | "TextInput" | "PasswordInput" | "NumberInput" | "SearchInput"
        | "PathInput" | "Combobox" | "QuickOpen" | "Form" | "FormWizard" | "PromptComposer"
        | "CommandPalette" | "HistoryPicker" | "KeybindingRecorder" | "QuestionFlow"
        | "TokenField" | "ObjectInspector" => "editor-form",
        "ChoiceDialog" | "Dialog" | "AlertDialog" | "DropdownMenu" | "Popover" | "Accordion"
        | "Collapsible" | "Select" | "MultiSelect" | "DateTimePicker" | "FilePicker"
        | "KeyboardHelp" | "JumpOverlay" | "JumpMode" | "PermissionPrompt" | "Drawer" | "Sheet"
        | "FullscreenViewer" | "EmptyState" | "ErrorState" | "OfflineBanner" | "OfflineSurface"
        | "PreviewCard" => "disclosure-overlay",
        "SplitPane" | "Slider" | "RangeSlider" | "ResizablePanelGroup" => "drag-continuous-value",
        "Tree" | "TreeTable" | "TreeNavigation" | "List" | "Picker" | "Table" | "DataTable"
        | "Tabs" | "ThemePicker" | "Menu" | "MenuBar" | "CompletionMenu" | "NotificationCenter"
        | "Sidebar" | "ToggleGroup" | "SegmentedControl" | "Pagination" | "NavigationList"
        | "RadioGroup" | "Breadcrumbs" | "Toolbar" | "ButtonGroup" | "ProgressSteps"
        | "Stepper" | "CheckpointTimeline" | "DiffReview" | "KeyValueList" | "ModeRibbon" => {
            "selection-navigation"
        }
        "KeyValueTable" => "editor-form",
        "LogPane" | "Transcript" | "VirtualGrid" | "VirtualList" | "CodeBlock" | "DiffView"
        | "DiagnosticView" | "EventStream" | "HexViewer" | "LogStream" | "MarkdownView"
        | "TerminalOutput" | "Timeline" | "DetailTable" => "scrolling-virtualization",
        "Toast" => "timed-state",
        _ => "activation",
    }
}

fn hints_for(component: &str, interactive: bool) -> Vec<&'static str> {
    if !interactive {
        return Vec::new();
    }
    match component {
        "TextArea" | "TextInput" | "PasswordInput" | "Form" | "FormWizard" | "PromptComposer"
        | "CommandPalette" => {
            vec!["click to focus", "type to edit", "paste", "arrow keys"]
        }
        "ChoiceDialog" => vec!["←→ choose", "Enter activate", "click action"],
        "SplitPane" => vec!["←→ resize", "drag divider"],
        "Tabs" => vec!["←→ change tab", "click tab"],
        "Tree" => vec!["↑↓ select", "←→ collapse/expand", "click row"],
        "LogPane" | "Transcript" | "VirtualGrid" => vec!["wheel to scroll", "↑↓ navigate"],
        "Toast" => vec!["use controls", "type message"],
        _ => vec!["arrow keys", "Enter activate", "click"],
    }
}

fn timed_component(component: &str) -> bool {
    matches!(
        component,
        "Spinner" | "Progress" | "Skeleton" | "LoadingOverlay" | "Toast"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stories::{PATTERN_DEMO_IDS, stories};

    fn key(value: &str) -> DemoEvent {
        DemoEvent::Key {
            key: value.to_owned(),
            kind: "press".to_owned(),
            shift: false,
            ctrl: false,
            alt: false,
            meta: false,
        }
    }

    fn pointer(kind: &str, x: u16, y: u16) -> DemoEvent {
        DemoEvent::Pointer {
            kind: kind.to_owned(),
            x,
            y,
            button: "left".to_owned(),
        }
    }

    #[test]
    fn persistent_tabs_session_changes_real_widget_frame() {
        let mut session = DemoSession::mount("tabs/status", Some(40), Some(8)).unwrap();
        let before = session.frame();
        let update = session.dispatch(key("ArrowRight")).unwrap();
        let after = session.frame();
        assert!(update.changed);
        assert_ne!(before.cells, after.cells);
    }

    #[test]
    fn passive_demo_rejects_fake_interaction() {
        let mut session = DemoSession::mount("accent-rail/actors", None, None).unwrap();
        assert!(!session.descriptor().interactive);
        let update = session.dispatch(key("ArrowDown")).unwrap();
        assert!(!update.changed);
        assert!(update.hints.is_empty());
    }

    #[test]
    fn browser_meta_maps_to_terminal_control_without_inventing_alt() {
        let mut session = DemoSession::mount("text-input/basic", None, None).unwrap();
        let event = session
            .decode_event(DemoEvent::Key {
                key: "a".to_owned(),
                kind: "press".to_owned(),
                shift: false,
                ctrl: false,
                alt: false,
                meta: true,
            })
            .unwrap()
            .unwrap();
        let Event::Key(key) = event else {
            panic!("expected key event");
        };
        assert!(key.modifiers.contains(KeyModifiers::CONTROL));
        assert!(!key.modifiers.contains(KeyModifiers::ALT));
    }

    #[test]
    fn action_link_hover_and_activation_are_real_state() {
        let mut session = DemoSession::mount("action-link/basic", Some(40), Some(3)).unwrap();
        let before = session.frame();
        let hovered = session.dispatch(pointer("move", 2, 1)).unwrap();
        assert!(hovered.changed);
        assert_eq!(
            hovered.outcome, None,
            "hover is state, not an action outcome"
        );
        assert_ne!(before.cells, session.frame().cells);
        let clicked = session.dispatch(pointer("down", 2, 1)).unwrap();
        assert_eq!(
            clicked.outcome.as_deref(),
            Some("Action activated: cargo test")
        );
    }

    #[test]
    fn host_lifecycle_events_never_invent_user_outcomes() {
        let mut session = DemoSession::mount("dialog/message", Some(48), Some(12)).unwrap();
        for event in [
            DemoEvent::Resize { cols: 72, rows: 11 },
            DemoEvent::Focus { focused: true },
            DemoEvent::Focus { focused: false },
        ] {
            assert_eq!(session.dispatch(event).unwrap().outcome, None);
        }
        assert_eq!(
            session.dispatch(key("Enter")).unwrap().outcome.as_deref(),
            Some("Dialog opened")
        );
    }

    #[test]
    fn button_loading_blocks_then_completes_from_host_time() {
        let mut session = DemoSession::mount("button/activation", Some(28), Some(3)).unwrap();
        let activated = session.dispatch(key("Enter")).unwrap();
        assert_eq!(activated.outcome.as_deref(), Some("Save started"));
        assert!(activated.hints.iter().any(|hint| hint.contains("loading")));
        assert_eq!(activated.next_deadline_ms, Some(100));
        let blocked = session.dispatch(key("Enter")).unwrap();
        assert!(!blocked.changed);
        let started = session
            .dispatch(DemoEvent::Tick { elapsed_ms: 100 })
            .unwrap();
        assert!(!started.changed);
        assert_eq!(started.next_deadline_ms, Some(900));
        let completed = session
            .dispatch(DemoEvent::Tick { elapsed_ms: 900 })
            .unwrap();
        assert_eq!(completed.outcome.as_deref(), Some("Saved successfully"));
        assert_eq!(completed.next_deadline_ms, None);
    }

    #[test]
    fn dialog_and_choice_dialog_have_trigger_lifecycles() {
        let mut dialog = DemoSession::mount("dialog/message", Some(48), Some(12)).unwrap();
        assert_eq!(
            dialog.dispatch(key("Enter")).unwrap().outcome.as_deref(),
            Some("Dialog opened")
        );
        assert_eq!(
            dialog.dispatch(key("Escape")).unwrap().outcome.as_deref(),
            Some("Dialog closed: Escape; focus restored to Open dialog")
        );

        let mut continued = DemoSession::mount("choice-dialog/basic", Some(48), Some(10)).unwrap();
        continued.dispatch(key("Enter")).unwrap();
        let continued = continued.dispatch(key("Enter")).unwrap();
        assert_eq!(continued.outcome.as_deref(), Some("You chose continue"));

        let mut cancelled = DemoSession::mount("choice-dialog/basic", Some(48), Some(10)).unwrap();
        cancelled.dispatch(key("Enter")).unwrap();
        let cancelled = cancelled.dispatch(key("Escape")).unwrap();
        assert_eq!(cancelled.outcome.as_deref(), Some("You chose cancel"));
    }

    #[test]
    fn editor_slider_split_tree_and_virtual_list_accept_real_events() {
        let mut input = DemoSession::mount("text-input/basic", Some(40), Some(3)).unwrap();
        let pasted = input
            .dispatch(DemoEvent::Paste {
                text: " λ🚀".to_owned(),
            })
            .unwrap();
        assert!(pasted.changed);
        assert!(
            pasted
                .outcome
                .as_deref()
                .is_some_and(|value| value.contains("λ🚀"))
        );

        let mut slider = DemoSession::mount("slider/basic", Some(44), Some(3)).unwrap();
        let adjusted = slider.dispatch(key("ArrowRight")).unwrap();
        assert_eq!(adjusted.outcome.as_deref(), Some("Volume: 63%"));

        let mut split = DemoSession::mount("split-pane/horizontal", Some(52), Some(10)).unwrap();
        assert!(split.dispatch(key("ArrowRight")).unwrap().changed);

        let mut tree = DemoSession::mount("tree-table/process", Some(64), Some(12)).unwrap();
        let collapsed = tree.dispatch(key("ArrowLeft")).unwrap();
        assert!(
            collapsed
                .outcome
                .as_deref()
                .is_some_and(|value| value.contains("collapsed"))
        );

        let mut list = DemoSession::mount("virtual-list/million", Some(52), Some(16)).unwrap();
        let scrolled = list
            .dispatch(DemoEvent::Wheel {
                delta_x: 0,
                delta_y: 1,
                x: 2,
                y: 4,
            })
            .unwrap();
        assert!(scrolled.changed);
        assert!(
            scrolled
                .outcome
                .as_deref()
                .is_some_and(|value| value.contains("250001"))
        );
    }

    #[test]
    fn missing_interaction_families_use_real_public_state() {
        let mut password = DemoSession::mount("password-input/basic", Some(40), Some(4)).unwrap();
        let pasted = password
            .dispatch(DemoEvent::Paste {
                text: "λ-secret".to_owned(),
            })
            .unwrap();
        assert!(pasted.changed);
        assert!(
            pasted
                .outcome
                .as_deref()
                .is_some_and(|value| value.contains("graphemes"))
        );
        let revealed = password
            .dispatch(DemoEvent::Key {
                key: "r".to_owned(),
                kind: "press".to_owned(),
                shift: false,
                ctrl: false,
                alt: true,
                meta: false,
            })
            .unwrap();
        assert_eq!(revealed.outcome.as_deref(), Some("Secret revealed locally"));

        let mut range = DemoSession::mount("range-slider/basic", Some(44), Some(4)).unwrap();
        assert!(range.dispatch(key("ArrowRight")).unwrap().changed);
        assert!(range.dispatch(key("Tab")).unwrap().changed);

        let mut panels =
            DemoSession::mount("resizable-panel-group/workbench", Some(80), Some(16)).unwrap();
        let before = panels.frame();
        assert!(panels.dispatch(key("ArrowRight")).unwrap().changed);
        assert_ne!(before.cells, panels.frame().cells);

        let mut menu = DemoSession::mount("dropdown-menu/basic", Some(40), Some(14)).unwrap();
        assert_eq!(
            menu.dispatch(key("Enter")).unwrap().outcome.as_deref(),
            Some("Dropdown menu opened")
        );
        assert!(menu.dispatch(key("ArrowDown")).unwrap().changed);
        assert_eq!(
            menu.dispatch(key("Escape")).unwrap().outcome.as_deref(),
            Some("Dropdown menu closed; focus restored")
        );

        let mut popover = DemoSession::mount("popover/basic", Some(36), Some(10)).unwrap();
        assert_eq!(
            popover.dispatch(key("Enter")).unwrap().outcome.as_deref(),
            Some("Popover opened")
        );
        assert!(
            popover
                .dispatch(key("Escape"))
                .unwrap()
                .outcome
                .as_deref()
                .is_some_and(|value| value.contains("focus restored"))
        );

        let mut sidebar = DemoSession::mount("sidebar/settings", Some(28), Some(14)).unwrap();
        assert!(sidebar.dispatch(key("ArrowDown")).unwrap().changed);
        assert_eq!(
            sidebar.dispatch(key("[")).unwrap().outcome.as_deref(),
            Some("Sidebar collapsed to rail")
        );

        let mut wizard = DemoSession::mount("blocks/form-wizard", Some(56), Some(12)).unwrap();
        let first = wizard.frame();
        assert!(wizard.dispatch(key("Enter")).unwrap().changed);
        assert_ne!(first.cells, wizard.frame().cells);

        let mut alert = DemoSession::mount("alert-dialog/delete", Some(56), Some(16)).unwrap();
        assert_eq!(
            alert.dispatch(key("Enter")).unwrap().outcome.as_deref(),
            Some("Destructive alert opened on safe Cancel")
        );
        assert!(
            alert
                .dispatch(key("Escape"))
                .unwrap()
                .outcome
                .as_deref()
                .is_some_and(|value| value.contains("resolved safely"))
        );
    }

    #[test]
    fn core_controls_preserve_public_state_and_typed_outcomes() {
        for id in ["checkbox/switch", "switch/basic", "toggle/pressed"] {
            let mut session = DemoSession::mount(id, None, None).unwrap();
            let before = session.frame();
            let update = session.dispatch(key("Enter")).unwrap();
            assert!(update.changed, "{id} must accept its public activation key");
            assert!(
                update.outcome.is_some(),
                "{id} must expose a visible typed outcome"
            );
            assert_ne!(
                before.cells,
                session.frame().cells,
                "{id} must repaint from persistent state"
            );
        }

        let mut group = DemoSession::mount("toggle-group/format", None, None).unwrap();
        assert!(group.dispatch(key("ArrowRight")).unwrap().changed);
        assert!(group.dispatch(key("Enter")).unwrap().changed);

        let mut segmented = DemoSession::mount("segmented-control/basic", None, None).unwrap();
        assert!(segmented.dispatch(key("ArrowRight")).unwrap().changed);
        assert_eq!(
            segmented.descriptor().interaction_kind,
            "selection-navigation"
        );

        let mut accordion = DemoSession::mount("accordion/section", None, None).unwrap();
        assert!(accordion.dispatch(key("ArrowDown")).unwrap().changed);
        assert!(accordion.dispatch(key("ArrowRight")).unwrap().changed);

        let mut collapsible = DemoSession::mount("collapsible/inline", None, None).unwrap();
        let before = collapsible.frame();
        assert!(collapsible.dispatch(key("Enter")).unwrap().changed);
        assert_ne!(before.cells, collapsible.frame().cells);

        let mut number = DemoSession::mount("number-input/basic", None, None).unwrap();
        assert!(number.dispatch(key("ArrowUp")).unwrap().changed);
        assert!(number.dispatch(key("Enter")).unwrap().changed);

        let mut select = DemoSession::mount("select/basic", None, None).unwrap();
        assert!(select.dispatch(key("Enter")).unwrap().changed);
        assert!(select.dispatch(key("ArrowDown")).unwrap().changed);
        assert!(select.dispatch(key("Enter")).unwrap().changed);

        let mut multi = DemoSession::mount("multi-select/basic", None, None).unwrap();
        assert!(multi.dispatch(key("Enter")).unwrap().changed);
        assert!(multi.dispatch(key("ArrowDown")).unwrap().changed);
        assert!(multi.dispatch(key(" ")).unwrap().changed);

        let mut pagination = DemoSession::mount("pagination/full", None, None).unwrap();
        assert!(pagination.dispatch(key("]")).unwrap().changed);
        assert!(pagination.dispatch(key("g")).unwrap().changed);
        assert!(pagination.dispatch(key("8")).unwrap().changed);
        assert!(pagination.dispatch(key("Enter")).unwrap().changed);
    }

    #[test]
    fn toast_appears_dismisses_and_expires() {
        let mut toast = DemoSession::mount("toast/success", Some(44), Some(8)).unwrap();
        assert_eq!(
            toast.dispatch(key("Enter")).unwrap().outcome.as_deref(),
            Some("Toast appeared")
        );
        assert_eq!(
            toast.dispatch(key("Escape")).unwrap().outcome.as_deref(),
            Some("Toast dismissed")
        );
        toast.dispatch(key("Enter")).unwrap();
        let expired = toast
            .dispatch(DemoEvent::Tick { elapsed_ms: 2_100 })
            .unwrap();
        assert_eq!(expired.outcome.as_deref(), Some("Toast expired"));
    }

    #[test]
    fn rich_catalog_widgets_use_persistent_public_interactors() {
        for (id, event) in [
            ("search-input/basic", key("x")),
            ("path-input/basic", key("x")),
            ("combobox/basic", key("ArrowDown")),
            ("completion-menu/basic", key("ArrowDown")),
            ("data-table/toolbar", key("ArrowDown")),
            (
                "date-time-picker/date",
                DemoEvent::Key {
                    key: "ArrowDown".to_owned(),
                    kind: "press".to_owned(),
                    shift: false,
                    ctrl: false,
                    alt: true,
                    meta: false,
                },
            ),
            ("file-picker/unix", key("ArrowDown")),
            ("quick-open/basic", key("ArrowDown")),
            ("menu-bar/basic", key("Enter")),
            ("tree-navigation/project", key("ArrowDown")),
            ("notification-center/drawer", key("ArrowDown")),
        ] {
            let mut session = DemoSession::mount(id, None, None).unwrap();
            assert!(session.descriptor().interactive, "{id}");
            let before = session.frame();
            let update = session.dispatch(event).unwrap();
            assert!(update.changed, "{id}");
            assert!(update.outcome.is_some(), "{id}");
            assert_ne!(before.cells, session.frame().cells, "{id}");
        }
    }

    #[test]
    fn complete_public_interaction_catalog_accepts_real_widget_events() {
        for (id, event) in [
            ("section/quiet", key("Enter")),
            ("button-group/dialog", key("ArrowRight")),
            ("toolbar/basic", key("Enter")),
            ("navigation-list/basic", key("ArrowDown")),
            ("stepper/horizontal", key("ArrowRight")),
            ("history-picker/basic", key("ArrowDown")),
            ("keyboard-help/footer", key("?")),
            ("breadcrumbs/path", key("ArrowRight")),
            ("progress-steps/pipeline", key("ArrowDown")),
            ("detail-table/basic", key("ArrowDown")),
            ("object-inspector/flat", key("ArrowDown")),
            ("log-stream/follow", key("f")),
            ("event-stream/basic", key("ArrowDown")),
            ("diff-review/hunks", key("ArrowDown")),
            ("diagnostic/list", key("ArrowDown")),
            ("terminal-output/running", key("f")),
            ("hex-viewer/basic", key("ArrowRight")),
            ("diff/basic", key("ArrowDown")),
            ("empty-state/basic", key("Enter")),
            ("connectivity/banner", key("r")),
            ("connectivity/reconnecting", key("r")),
            ("error-state/network", key("d")),
            ("jump-overlay/basic", key("f")),
            ("jump-mode/multi", key("f")),
            ("code-block/basic", key("ArrowDown")),
            ("markdown-view/basic", key("ArrowDown")),
            ("timeline/basic", key("ArrowDown")),
            ("checkpoint-timeline/basic", key("ArrowUp")),
            ("button/icon", key("Enter")),
            ("radio-group/basic", key("ArrowDown")),
            ("tag/removable", key("ArrowRight")),
            ("chip/filter", key("Enter")),
            ("badge/basic", key("Enter")),
            ("alert/danger", key("d")),
            ("drawer/basic", key("Enter")),
            ("drawer/sheet", key("Enter")),
            ("fullscreen-viewer/basic", key("Enter")),
            ("preview-card/file", key("p")),
            ("key-value-list/basic", key("ArrowDown")),
            ("link/basic", key("Enter")),
            ("permission-prompt/basic", key("ArrowRight")),
            ("mode-ribbon/basic", key("ArrowRight")),
            ("question-flow/basic", key("Enter")),
            ("key-value-table/http", key("ArrowDown")),
            (
                "token-field/basic",
                DemoEvent::Paste {
                    text: "bob@example.com".into(),
                },
            ),
            ("keybinding-recorder/idle", key("Enter")),
        ] {
            let mut session = DemoSession::mount(id, None, None).unwrap();
            assert!(session.descriptor().interactive, "{id}");
            let _ = session.frame();
            let update = session.dispatch(event).unwrap();
            assert!(update.changed, "{id} must accept its advertised action");
            assert!(
                update.outcome.is_some(),
                "{id} must expose its typed outcome"
            );
        }
    }

    #[test]
    fn dismissible_demo_lifecycles_stay_closed_until_explicitly_reopened() {
        let mut alert = DemoSession::mount("alert/danger", None, None).unwrap();
        assert!(alert.dispatch(key("Escape")).unwrap().changed);
        assert!(!alert.dispatch(key("Enter")).unwrap().changed);
        assert_eq!(
            alert.dispatch(key("o")).unwrap().outcome.as_deref(),
            Some("Alert: Shown")
        );

        let mut chip = DemoSession::mount("chip/filter", None, None).unwrap();
        assert!(chip.dispatch(key("Delete")).unwrap().changed);
        assert!(!chip.dispatch(key("Enter")).unwrap().changed);
        assert_eq!(
            chip.dispatch(key("r")).unwrap().outcome.as_deref(),
            Some("Chip: Restored")
        );

        let mut drawer = DemoSession::mount("drawer/basic", None, None).unwrap();
        assert!(drawer.dispatch(key("Enter")).unwrap().changed);
        assert!(drawer.dispatch(key("Escape")).unwrap().changed);
        assert!(drawer.dispatch(key("Enter")).unwrap().changed);
    }

    #[test]
    fn every_interactive_story_has_an_executable_typed_outcome_path() {
        let key_candidates = [
            "Enter",
            "ArrowDown",
            "ArrowRight",
            "ArrowUp",
            "ArrowLeft",
            "Tab",
            " ",
            "/",
            "?",
            "f",
            "d",
            "r",
            "o",
            "p",
            "s",
            "c",
            "e",
            "x",
            "1",
            "Escape",
        ];
        let mut missing = Vec::new();

        for story in stories().into_iter().filter(|story| story.interactive) {
            let mut accepted = false;
            for value in key_candidates {
                let mut session = DemoSession::mount(story.id, None, None).unwrap();
                let _ = session.frame();
                let update = session.dispatch(key(value)).unwrap();
                if update.changed && update.outcome.is_some() {
                    accepted = true;
                    break;
                }
            }
            if !accepted {
                let mut session = DemoSession::mount(story.id, None, None).unwrap();
                let _ = session.frame();
                let update = session
                    .dispatch(DemoEvent::Paste {
                        text: "λ demo".into(),
                    })
                    .unwrap();
                accepted = update.changed && update.outcome.is_some();
            }
            if !accepted {
                let points = [
                    (2, 2),
                    (story.width / 4 + 1, story.height / 4 + 1),
                    (story.width / 2 + 1, story.height / 2 + 1),
                    (
                        story.width.saturating_sub(1),
                        story.height.saturating_sub(1),
                    ),
                ];
                for (x, y) in points {
                    let mut session = DemoSession::mount(story.id, None, None).unwrap();
                    let _ = session.frame();
                    let update = session.dispatch(pointer("down", x, y)).unwrap();
                    if update.changed && update.outcome.is_some() {
                        accepted = true;
                        break;
                    }
                }
            }
            if !accepted {
                missing.push(story.id);
            }
        }

        assert!(
            missing.is_empty(),
            "interactive stories without an executable typed outcome path: {}",
            missing.join(", ")
        );
    }

    #[test]
    fn every_pattern_demo_is_real_interactive_state_without_sample_app_fallbacks() {
        const BANNED: &[&str] = &[
            "Enter open action",
            "Application row",
            "Application filter",
            "Sample application",
            "Sidebar region collapsed",
        ];
        for id in PATTERN_DEMO_IDS {
            let mut session = DemoSession::mount(id, Some(72), Some(20))
                .unwrap_or_else(|error| panic!("{id}: {error}"));
            let descriptor = session.descriptor();
            assert!(descriptor.interactive, "{id}");
            assert!(!descriptor.hints.is_empty(), "{id}");
            let contract = format!("{} {:?}", descriptor.description, descriptor.hints);
            assert!(
                BANNED.iter().all(|fake| !contract.contains(fake)),
                "{id}: {contract}"
            );
            assert!(!session.frame().cells.is_empty(), "{id}");
        }
    }

    #[test]
    fn representative_pattern_flows_change_public_pattern_state() {
        let flows = [
            ("activity-shelf/statuses", "ArrowRight"),
            ("agent-workbench/basic", "o"),
            ("app-shell/workbench", "s"),
            ("approval-queue/basic", "ArrowDown"),
            ("auth-entry/basic", "a"),
            ("connection-manager/full", "ArrowDown"),
            ("database-workbench/basic", "Tab"),
            ("file-manager/basic", "ArrowDown"),
            ("git-workbench/basic", "?"),
            ("metrics-dashboard/basic", "ArrowRight"),
            ("plan-review/basic", "Tab"),
            ("process-table/basic", "ArrowDown"),
            ("prompt-queue/compact", "Enter"),
            ("query-editor/basic", "x"),
            ("result-grid/basic", "ArrowRight"),
            ("session-picker/basic", "ArrowDown"),
            ("setup-wizard/welcome", "Enter"),
            ("terminal-run-card/running", "Enter"),
        ];
        for (id, input) in flows {
            let mut session = DemoSession::mount(id, None, None).unwrap();
            let before = session.frame();
            let update = session.dispatch(key(input)).unwrap();
            let after = session.frame();
            assert!(update.changed, "{id}: {input} was ignored");
            assert!(update.outcome.is_some(), "{id}: missing typed outcome");
            assert_ne!(before.cells, after.cells, "{id}: frame stayed static");
        }
    }

    #[test]
    fn browser_and_native_event_paths_have_identical_state_and_frames() {
        fn native_key(value: &str, kind: KeyEventKind) -> Event {
            let code = decode_key_code(value).expect("test key must decode");
            Event::Key(KeyEvent {
                code,
                modifiers: KeyModifiers::NONE,
                kind,
                state: KeyEventState::NONE,
            })
        }

        fn native_pointer(kind: MouseEventKind, x: u16, y: u16) -> Event {
            Event::Mouse(MouseEvent {
                kind,
                position: Position::new(x, y),
                modifiers: KeyModifiers::NONE,
            })
        }

        fn same(
            browser: &mut DemoSession,
            native: &mut DemoSession,
            browser_event: DemoEvent,
            native_event: Event,
        ) {
            let browser_update = browser.dispatch(browser_event).unwrap();
            let native_update = native.dispatch_event(native_event);
            assert_eq!(browser_update, native_update);
            assert_eq!(browser.frame(), native.frame());
        }

        let mut browser = DemoSession::mount("text-input/basic", Some(40), Some(4)).unwrap();
        let mut native = DemoSession::mount("text-input/basic", Some(40), Some(4)).unwrap();
        same(
            &mut browser,
            &mut native,
            DemoEvent::Focus { focused: true },
            Event::FocusGained,
        );
        same(
            &mut browser,
            &mut native,
            key("λ"),
            native_key("λ", KeyEventKind::Press),
        );
        same(
            &mut browser,
            &mut native,
            DemoEvent::Key {
                key: "λ".to_owned(),
                kind: "repeat".to_owned(),
                shift: false,
                ctrl: false,
                alt: false,
                meta: false,
            },
            native_key("λ", KeyEventKind::Repeat),
        );
        same(
            &mut browser,
            &mut native,
            DemoEvent::Paste {
                text: "-paste".to_owned(),
            },
            Event::Paste("-paste".to_owned()),
        );
        same(
            &mut browser,
            &mut native,
            DemoEvent::Resize { cols: 48, rows: 5 },
            Event::Resize {
                width: 48,
                height: 5,
            },
        );
        same(
            &mut browser,
            &mut native,
            DemoEvent::Key {
                key: "λ".to_owned(),
                kind: "release".to_owned(),
                shift: false,
                ctrl: false,
                alt: false,
                meta: false,
            },
            native_key("λ", KeyEventKind::Release),
        );
        same(
            &mut browser,
            &mut native,
            DemoEvent::Focus { focused: false },
            Event::FocusLost,
        );

        let mut browser = DemoSession::mount("action-link/basic", Some(40), Some(3)).unwrap();
        let mut native = DemoSession::mount("action-link/basic", Some(40), Some(3)).unwrap();
        let _ = browser.frame();
        let _ = native.frame();
        same(
            &mut browser,
            &mut native,
            pointer("move", 2, 1),
            native_pointer(MouseEventKind::Moved, 2, 1),
        );
        same(
            &mut browser,
            &mut native,
            pointer("down", 2, 1),
            native_pointer(MouseEventKind::Down(MouseButton::Left), 2, 1),
        );
        same(
            &mut browser,
            &mut native,
            pointer("up", 2, 1),
            native_pointer(MouseEventKind::Up(MouseButton::Left), 2, 1),
        );

        let mut browser = DemoSession::mount("dialog/message", Some(48), Some(12)).unwrap();
        let mut native = DemoSession::mount("dialog/message", Some(48), Some(12)).unwrap();
        same(
            &mut browser,
            &mut native,
            key("Enter"),
            native_key("Enter", KeyEventKind::Press),
        );
        same(
            &mut browser,
            &mut native,
            key("Escape"),
            native_key("Escape", KeyEventKind::Press),
        );

        let mut browser = DemoSession::mount("split-pane/horizontal", Some(52), Some(10)).unwrap();
        let mut native = DemoSession::mount("split-pane/horizontal", Some(52), Some(10)).unwrap();
        let _ = browser.frame();
        let _ = native.frame();
        same(
            &mut browser,
            &mut native,
            key("ArrowRight"),
            native_key("ArrowRight", KeyEventKind::Press),
        );
        for (browser_event, native_event) in [
            (
                pointer("move", 22, 5),
                native_pointer(MouseEventKind::Moved, 22, 5),
            ),
            (
                pointer("down", 22, 5),
                native_pointer(MouseEventKind::Down(MouseButton::Left), 22, 5),
            ),
            (
                pointer("drag", 29, 5),
                native_pointer(MouseEventKind::Drag(MouseButton::Left), 29, 5),
            ),
            (
                pointer("up", 29, 5),
                native_pointer(MouseEventKind::Up(MouseButton::Left), 29, 5),
            ),
        ] {
            same(&mut browser, &mut native, browser_event, native_event);
        }

        let mut browser = DemoSession::mount("tree-table/process", Some(64), Some(12)).unwrap();
        let mut native = DemoSession::mount("tree-table/process", Some(64), Some(12)).unwrap();
        for value in ["ArrowLeft", "ArrowRight"] {
            same(
                &mut browser,
                &mut native,
                key(value),
                native_key(value, KeyEventKind::Press),
            );
        }

        let mut browser = DemoSession::mount("virtual-list/million", Some(52), Some(16)).unwrap();
        let mut native = DemoSession::mount("virtual-list/million", Some(52), Some(16)).unwrap();
        same(
            &mut browser,
            &mut native,
            DemoEvent::Wheel {
                delta_x: 0,
                delta_y: 1,
                x: 2,
                y: 4,
            },
            native_pointer(MouseEventKind::ScrollDown, 2, 4),
        );

        let mut browser = DemoSession::mount("spinner/labeled", Some(24), Some(4)).unwrap();
        let mut native = DemoSession::mount("spinner/labeled", Some(24), Some(4)).unwrap();
        assert_eq!(
            browser
                .dispatch(DemoEvent::Tick { elapsed_ms: 400 })
                .unwrap(),
            native.tick(400)
        );
        assert_eq!(browser.frame(), native.frame());

        let mut browser = DemoSession::mount("toast/success", Some(44), Some(8)).unwrap();
        let mut native = DemoSession::mount("toast/success", Some(44), Some(8)).unwrap();
        same(
            &mut browser,
            &mut native,
            key("Enter"),
            native_key("Enter", KeyEventKind::Press),
        );
        assert_eq!(
            browser
                .dispatch(DemoEvent::Tick { elapsed_ms: 2_100 })
                .unwrap(),
            native.tick(2_100)
        );
        assert_eq!(browser.frame(), native.frame());

        let mut browser =
            DemoSession::mount("connection-manager/full", Some(72), Some(18)).unwrap();
        let mut native = DemoSession::mount("connection-manager/full", Some(72), Some(18)).unwrap();
        same(
            &mut browser,
            &mut native,
            key("ArrowDown"),
            native_key("ArrowDown", KeyEventKind::Press),
        );
        same(
            &mut browser,
            &mut native,
            DemoEvent::Resize { cols: 36, rows: 18 },
            Event::Resize {
                width: 36,
                height: 18,
            },
        );

        let mut browser = DemoSession::mount("accent-rail/actors", None, None).unwrap();
        let mut native = DemoSession::mount("accent-rail/actors", None, None).unwrap();
        same(
            &mut browser,
            &mut native,
            key("ArrowDown"),
            native_key("ArrowDown", KeyEventKind::Press),
        );
    }
}
