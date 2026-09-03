// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **PromptQueue** — visible, editable queue of user messages waiting behind
//! active agent work.
//!
//! **Mission.** States: queued, sending, blocked, failed, cancelled, sent.
//! Reorder, edit, delete, send next, interrupt-and-send. Clear busy semantics.
//! Preserve attachment and mention **identities** (ids + labels). Compact
//! composer summary and expanded management view. **Persistence and drain
//! policy stay host-owned** — outcomes are requests only (KD-29: no auto-drain
//! on fail/cancel inside the library).
//!
//! **vs [`super::PromptComposer`].** Composer owns draft editing + enqueue;
//! PromptQueue owns queue presentation and mutation requests.
//! **vs OverlayStack queue.** Overlay FIFO is focus chrome, not user prompts.
//!
//! Research: async chat products, agent prompt queues, task schedulers.
//!
//! Teaches: how to compose visible, editable queue of user messages waiting
//! behind active agent work.
//!
//! Composes: [`crate::widgets::AgentBusyState`],
//! [`crate::widgets::ConfirmFocus`], [`crate::widgets::ConfirmPrompt`],
//! [`crate::widgets::Panel`], [`crate::widgets::PromptQueueItem`],
//! [`crate::widgets::PromptQueueRef`], [`crate::widgets::PromptQueueStatus`],
//! [`crate::widgets::StatefulWidget`], and 1 more.
//!
//! Copy-adapt: keep the widget composition and the focus routing;
//! replace the domain types, the wording, and the effects with your own.
use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::StatefulWidget};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    interaction::CursorWindow,
    style::{DesignSystem, PanelChrome, Role},
    widgets::{ConfirmFocus, ConfirmPrompt, Panel, SemanticStatus, StatusIndicator},
};

/// Overlay id for expanded queue manager.
pub const PROMPT_QUEUE_OVERLAY_ID: &str = "termrock.prompt_queue";
/// Max items painted in expanded list window.
pub const PROMPT_QUEUE_WINDOW: usize = 32;
/// Compact summary max preview chars.
pub const PROMPT_QUEUE_SUMMARY_PREVIEW: usize = 28;

// ── Domain ──────────────────────────────────────────────────────────────────

// Domain model lives in widgets (PromptComposer must not depend on patterns).
pub use crate::widgets::{AgentBusyState, PromptQueueItem, PromptQueueRef, PromptQueueStatus};

// ── Presentation ────────────────────────────────────────────────────────────

/// Compact strip vs expanded manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PromptQueuePresentation {
    /// One-line composer summary (`queue:N · next: …`).
    #[default]
    Compact,
    /// Full management list.
    Expanded,
}

/// Edit phase for one item.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum PromptQueuePhase {
    /// Browse / manage.
    #[default]
    Browse,
    /// Editing text of selected item.
    Edit {
        /// Item id.
        id: String,
    },
    /// Confirm delete.
    ConfirmDelete {
        /// Item id.
        id: String,
    },
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Outcomes — requests only; host owns drain/persistence policy.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PromptQueueOutcome {
    /// Ignored.
    Ignored,
    /// Selection moved.
    Selected {
        /// Id.
        id: String,
    },
    /// Presentation toggled.
    PresentationChanged(PromptQueuePresentation),
    /// Reorder: item moved from → to index in pending list.
    Reordered {
        /// Id.
        id: String,
        /// New index among items.
        index: usize,
    },
    /// Edit committed.
    Edited {
        /// Id.
        id: String,
        /// New text.
        text: String,
    },
    /// Edit cancelled (text discarded).
    EditCancelled {
        /// Id.
        id: String,
    },
    /// Delete requested (after confirm).
    Deleted {
        /// Id.
        id: String,
    },
    /// Confirm opened.
    ConfirmOpened {
        /// Id.
        id: String,
    },
    /// Confirm cancelled.
    ConfirmCancelled,
    /// Send this item next (host may interrupt current if needed).
    SendNext {
        /// Id.
        id: String,
    },
    /// Send front of queue (FIFO).
    SendFront,
    /// Interrupt active agent run and send selected/front.
    InterruptAndSend {
        /// Id to send after interrupt (or front if None semantics → selected).
        id: String,
    },
    /// Soft interrupt only (no send).
    Interrupt,
    /// Retry failed item (host).
    Retry {
        /// Id.
        id: String,
    },
    /// Clear terminal sent entries (host).
    ClearSent,
    /// Phase change.
    PhaseChanged,
}

// ── State ───────────────────────────────────────────────────────────────────

/// Interactive prompt queue state.
///
/// Does **not** auto-drain on Failed/Cancelled. Host calls
/// [`PromptQueueState::remove`] / status updates after policy decisions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptQueueState {
    /// Queue entries (order = send order for Queued).
    pub items: Vec<PromptQueueItem>,
    /// Cursor + scroll window.
    pub window: CursorWindow,
    /// Presentation.
    pub presentation: PromptQueuePresentation,
    /// Phase.
    pub phase: PromptQueuePhase,
    /// Agent busy chrome.
    pub agent: AgentBusyState,
    /// Edit draft text.
    pub edit_draft: String,
    /// Confirm proceed focused (false = Cancel default).
    pub confirm_proceed_focused: bool,
    /// Focused.
    pub focused: bool,
    accepts_input: bool,
    /// Row hits.
    pub row_hits: Vec<(String, Rect)>,
    /// Confirm hits.
    pub confirm_hits: Vec<(bool, Rect)>,
    /// Compact strip hit (expand).
    pub compact_hit: Option<Rect>,
}

impl Default for PromptQueueState {
    fn default() -> Self {
        Self::new()
    }
}

impl PromptQueueState {
    /// Empty.
    #[must_use]
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            window: CursorWindow::new(),
            presentation: PromptQueuePresentation::Compact,
            phase: PromptQueuePhase::Browse,
            agent: AgentBusyState::Idle,
            edit_draft: String::new(),
            confirm_proceed_focused: false,
            focused: true,
            accepts_input: true,
            row_hits: Vec::new(),
            confirm_hits: Vec::new(),
            compact_hit: None,
        }
    }

    /// Replace items.
    pub fn set_items(&mut self, items: Vec<PromptQueueItem>) {
        let keep = self.current_id();
        self.items = items;
        if let Some(id) = keep {
            if let Some(i) = self.items.iter().position(|e| e.id == id) {
                self.window
                    .set_cursor(i, self.items.len(), PROMPT_QUEUE_WINDOW);
            }
        }
        self.clamp_cursor();
    }

    /// Enqueue (host or composer bridge).
    pub fn enqueue(&mut self, item: PromptQueueItem) {
        self.items.push(item);
        if self.presentation == PromptQueuePresentation::Expanded {
            self.window.set_cursor(
                self.items.len().saturating_sub(1),
                self.items.len(),
                PROMPT_QUEUE_WINDOW,
            );
        }
    }

    /// Agent busy.
    pub const fn set_agent(&mut self, a: AgentBusyState) {
        self.agent = a;
    }

    /// Gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Focus.
    pub const fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Pending depth (queued/sending/blocked/failed/cancelled).
    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.items
            .iter()
            .filter(|i| i.status.counts_as_pending())
            .count()
    }

    /// Current id.
    #[must_use]
    pub fn current_id(&self) -> Option<String> {
        self.items.get(self.window.cursor()).map(|i| i.id.clone())
    }

    /// Current item.
    #[must_use]
    pub fn current(&self) -> Option<&PromptQueueItem> {
        self.items.get(self.window.cursor())
    }

    /// Remove by id.
    pub fn remove(&mut self, id: &str) -> Option<PromptQueueItem> {
        let i = self.items.iter().position(|e| e.id == id)?;
        let item = self.items.remove(i);
        self.clamp_cursor();
        Some(item)
    }

    /// Update status by id.
    pub fn set_status(&mut self, id: &str, status: PromptQueueStatus) {
        if let Some(e) = self.items.iter_mut().find(|e| e.id == id) {
            e.status = status;
        }
    }

    /// Pop front pending for host drain (only Queued/Failed by host choice).
    pub fn pop_front_sendable(&mut self) -> Option<PromptQueueItem> {
        let i = self.items.iter().position(|e| e.status.can_send())?;
        Some(self.items.remove(i))
    }

    fn clamp_cursor(&mut self) {
        self.window.clamp(self.items.len(), PROMPT_QUEUE_WINDOW);
    }

    fn move_cursor(&mut self, delta: isize) -> PromptQueueOutcome {
        self.window
            .move_by(delta, self.items.len(), PROMPT_QUEUE_WINDOW);
        match self.items.get(self.window.cursor()) {
            Some(item) => PromptQueueOutcome::Selected {
                id: item.id.clone(),
            },
            None => PromptQueueOutcome::Ignored,
        }
    }

    fn reorder(&mut self, delta: isize) -> PromptQueueOutcome {
        if self.items.is_empty() {
            return PromptQueueOutcome::Ignored;
        }
        let i = self.window.cursor();
        if !self.items[i].status.can_reorder() {
            return PromptQueueOutcome::Ignored;
        }
        let j = (i as isize + delta).clamp(0, self.items.len() as isize - 1) as usize;
        if i == j {
            return PromptQueueOutcome::Ignored;
        }
        self.items.swap(i, j);
        // Follow the moved item so it stays inside the painted window.
        self.window
            .set_cursor(j, self.items.len(), PROMPT_QUEUE_WINDOW);
        let id = self.items[j].id.clone();
        PromptQueueOutcome::Reordered { id, index: j }
    }

    /// Keyboard.
    pub fn handle_key(&mut self, key: KeyEvent) -> PromptQueueOutcome {
        if !self.focused || !self.accepts_input || !key.is_press() {
            return PromptQueueOutcome::Ignored;
        }

        match &self.phase {
            PromptQueuePhase::Edit { id } => {
                let id = id.clone();
                return self.handle_edit_key(key, &id);
            }
            PromptQueuePhase::ConfirmDelete { id } => {
                let id = id.clone();
                return self.handle_confirm_key(key, &id);
            }
            PromptQueuePhase::Browse => {}
        }

        // Compact: limited keys
        if self.presentation == PromptQueuePresentation::Compact {
            return self.handle_compact_key(key);
        }

        match key.code {
            KeyCode::Esc => {
                self.presentation = PromptQueuePresentation::Compact;
                PromptQueueOutcome::PresentationChanged(PromptQueuePresentation::Compact)
            }
            KeyCode::Up | KeyCode::Char('k') => self.move_cursor(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_cursor(1),
            KeyCode::Char('K') => self.reorder(-1),
            KeyCode::Char('J') => self.reorder(1),
            KeyCode::Char('[') => self.reorder(-1),
            KeyCode::Char(']') => self.reorder(1),
            KeyCode::Enter => {
                // Send selected next
                let Some(item) = self.current() else {
                    return PromptQueueOutcome::Ignored;
                };
                if !item.status.can_send() {
                    return PromptQueueOutcome::Ignored;
                }
                let id = item.id.clone();
                if self.agent.is_busy() {
                    // Prefer explicit interrupt path when busy
                    PromptQueueOutcome::SendNext { id }
                } else {
                    PromptQueueOutcome::SendNext { id }
                }
            }
            KeyCode::Char('s') => {
                if self.items.iter().any(|i| i.status.can_send()) {
                    PromptQueueOutcome::SendFront
                } else {
                    PromptQueueOutcome::Ignored
                }
            }
            KeyCode::Char('i') => {
                let id = self
                    .current_id()
                    .or_else(|| {
                        self.items
                            .iter()
                            .find(|i| i.status.can_send())
                            .map(|i| i.id.clone())
                    })
                    .unwrap_or_default();
                if id.is_empty() {
                    return PromptQueueOutcome::Interrupt;
                }
                PromptQueueOutcome::InterruptAndSend { id }
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                PromptQueueOutcome::Interrupt
            }
            KeyCode::Char('e') => {
                let Some(item) = self.current() else {
                    return PromptQueueOutcome::Ignored;
                };
                if matches!(
                    item.status,
                    PromptQueueStatus::Sending | PromptQueueStatus::Sent
                ) {
                    return PromptQueueOutcome::Ignored;
                }
                let id = item.id.clone();
                self.edit_draft = item.text.clone();
                self.phase = PromptQueuePhase::Edit { id: id.clone() };
                PromptQueueOutcome::PhaseChanged
            }
            KeyCode::Delete | KeyCode::Char('d') | KeyCode::Backspace
                if matches!(key.code, KeyCode::Delete)
                    || (key.modifiers.is_empty() && matches!(key.code, KeyCode::Char('d'))) =>
            {
                let Some(id) = self.current_id() else {
                    return PromptQueueOutcome::Ignored;
                };
                self.phase = PromptQueuePhase::ConfirmDelete { id: id.clone() };
                self.confirm_proceed_focused = false;
                PromptQueueOutcome::ConfirmOpened { id }
            }
            KeyCode::Char('r') => {
                let Some(item) = self.current() else {
                    return PromptQueueOutcome::Ignored;
                };
                if item.status != PromptQueueStatus::Failed {
                    return PromptQueueOutcome::Ignored;
                }
                PromptQueueOutcome::Retry {
                    id: item.id.clone(),
                }
            }
            KeyCode::Char('x') => {
                // clear sent
                PromptQueueOutcome::ClearSent
            }
            KeyCode::Char(' ') | KeyCode::Char('f') => {
                // already expanded; space no-op
                PromptQueueOutcome::Ignored
            }
            KeyCode::Char('y') => PromptQueueOutcome::Ignored,
            _ => PromptQueueOutcome::Ignored,
        }
    }

    fn handle_compact_key(&mut self, key: KeyEvent) -> PromptQueueOutcome {
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Char('e') | KeyCode::Char('f') => {
                self.presentation = PromptQueuePresentation::Expanded;
                PromptQueueOutcome::PresentationChanged(PromptQueuePresentation::Expanded)
            }
            KeyCode::Char('s') => {
                if self.items.iter().any(|i| i.status.can_send()) {
                    PromptQueueOutcome::SendFront
                } else {
                    PromptQueueOutcome::Ignored
                }
            }
            KeyCode::Char('i') => {
                let id = self
                    .items
                    .iter()
                    .find(|i| i.status.can_send())
                    .map(|i| i.id.clone())
                    .unwrap_or_default();
                if id.is_empty() {
                    PromptQueueOutcome::Interrupt
                } else {
                    PromptQueueOutcome::InterruptAndSend { id }
                }
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                PromptQueueOutcome::Interrupt
            }
            KeyCode::Char('y') => PromptQueueOutcome::Ignored,
            _ => PromptQueueOutcome::Ignored,
        }
    }

    fn handle_edit_key(&mut self, key: KeyEvent, id: &str) -> PromptQueueOutcome {
        match key.code {
            KeyCode::Esc => {
                self.phase = PromptQueuePhase::Browse;
                self.edit_draft.clear();
                PromptQueueOutcome::EditCancelled { id: id.into() }
            }
            KeyCode::Enter => {
                let text = self.edit_draft.clone();
                if let Some(item) = self.items.iter_mut().find(|e| e.id == id) {
                    item.text = text.clone();
                    // Failed → Queued after edit
                    if matches!(
                        item.status,
                        PromptQueueStatus::Failed | PromptQueueStatus::Cancelled
                    ) {
                        item.status = PromptQueueStatus::Queued;
                        item.error = None;
                    }
                }
                self.phase = PromptQueuePhase::Browse;
                self.edit_draft.clear();
                PromptQueueOutcome::Edited {
                    id: id.into(),
                    text,
                }
            }
            KeyCode::Backspace => {
                self.edit_draft.pop();
                PromptQueueOutcome::Ignored
            }
            KeyCode::Char(c)
                if !c.is_control() && !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.edit_draft.push(c);
                PromptQueueOutcome::Ignored
            }
            _ => PromptQueueOutcome::Ignored,
        }
    }

    fn handle_confirm_key(&mut self, key: KeyEvent, id: &str) -> PromptQueueOutcome {
        match key.code {
            KeyCode::Esc => {
                self.phase = PromptQueuePhase::Browse;
                PromptQueueOutcome::ConfirmCancelled
            }
            KeyCode::Left | KeyCode::Char('h') => {
                self.confirm_proceed_focused = false;
                PromptQueueOutcome::Ignored
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.confirm_proceed_focused = true;
                PromptQueueOutcome::Ignored
            }
            KeyCode::Tab => {
                self.confirm_proceed_focused = !self.confirm_proceed_focused;
                PromptQueueOutcome::Ignored
            }
            KeyCode::Enter => {
                if self.confirm_proceed_focused {
                    let _ = self.remove(id);
                    self.phase = PromptQueuePhase::Browse;
                    PromptQueueOutcome::Deleted { id: id.into() }
                } else {
                    self.phase = PromptQueuePhase::Browse;
                    PromptQueueOutcome::ConfirmCancelled
                }
            }
            KeyCode::Char('y') => PromptQueueOutcome::Ignored,
            _ => PromptQueueOutcome::Ignored,
        }
    }

    /// Mouse.
    pub fn handle_mouse(&mut self, ev: MouseEvent) -> PromptQueueOutcome {
        if !self.focused || !self.accepts_input {
            return PromptQueueOutcome::Ignored;
        }
        if !matches!(ev.kind, MouseEventKind::Down(MouseButton::Left)) {
            return PromptQueueOutcome::Ignored;
        }
        let pos = ev.position;
        if let PromptQueuePhase::ConfirmDelete { id } = &self.phase {
            let id = id.clone();
            for (proceed, r) in &self.confirm_hits {
                if r.contains(pos) {
                    self.confirm_proceed_focused = *proceed;
                    if *proceed {
                        let _ = self.remove(&id);
                        self.phase = PromptQueuePhase::Browse;
                        return PromptQueueOutcome::Deleted { id };
                    }
                    self.phase = PromptQueuePhase::Browse;
                    return PromptQueueOutcome::ConfirmCancelled;
                }
            }
            return PromptQueueOutcome::Ignored;
        }
        if self.presentation == PromptQueuePresentation::Compact {
            if self.compact_hit.is_some_and(|r| r.contains(pos)) {
                self.presentation = PromptQueuePresentation::Expanded;
                return PromptQueueOutcome::PresentationChanged(PromptQueuePresentation::Expanded);
            }
            return PromptQueueOutcome::Ignored;
        }
        let hit = self
            .row_hits
            .iter()
            .find(|(_, r)| r.contains(pos))
            .map(|(id, _)| id.clone());
        if let Some(id) = hit {
            if let Some(i) = self.items.iter().position(|e| e.id == id) {
                self.window
                    .set_cursor(i, self.items.len(), PROMPT_QUEUE_WINDOW);
                return PromptQueueOutcome::Selected { id };
            }
        }
        PromptQueueOutcome::Ignored
    }

    /// Compact summary line for composer status.
    #[must_use]
    pub fn compact_summary(&self) -> String {
        let n = self.pending_len();
        if n == 0 {
            return String::new();
        }
        let next = self
            .items
            .iter()
            .find(|i| i.status.can_send() || i.status == PromptQueueStatus::Sending)
            .map(|i| i.preview(PROMPT_QUEUE_SUMMARY_PREVIEW))
            .unwrap_or_default();
        let busy = if self.agent.is_busy() {
            format!(" · {}", self.agent.label())
        } else {
            String::new()
        };
        if next.is_empty() {
            format!("queue:{n}{busy}")
        } else {
            format!("queue:{n} · next:{next}{busy}")
        }
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Prompt queue painter.
#[derive(Debug, Clone, Copy)]
pub struct PromptQueue<'a> {
    system: &'a DesignSystem,
    colorless: bool,
}

impl<'a> PromptQueue<'a> {
    /// System only — items live in state.
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self {
            system,
            colorless: false,
        }
    }

    /// ASCII.
    #[must_use]
    /// Colorless.
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut PromptQueueState) {
        state.row_hits.clear();
        state.confirm_hits.clear();
        state.compact_hit = None;
        if area.is_empty() {
            return;
        }
        match state.presentation {
            PromptQueuePresentation::Compact => self.paint_compact(area, buffer, state),
            PromptQueuePresentation::Expanded => self.paint_expanded(area, buffer, state),
        }
    }

    fn paint_compact(&self, area: Rect, buffer: &mut Buffer, state: &mut PromptQueueState) {
        let summary = state.compact_summary();
        if summary.is_empty() {
            return;
        }
        let semantic = if state.agent.is_busy() {
            state.agent.semantic()
        } else {
            SemanticStatus::Queued
        };
        let indicator = StatusIndicator::new(semantic, self.system)
            .label(&summary)
            .colorless(self.colorless);
        indicator.paint(Rect::new(area.x, area.y, area.width, 1), buffer, None);
        state.compact_hit = Some(Rect {
            x: area.x,
            y: area.y,
            width: area.width.min(indicator.measure_width(None)).max(1),
            height: 1,
        });
    }

    fn paint_expanded(&self, area: Rect, buffer: &mut Buffer, state: &mut PromptQueueState) {
        let title = format!(
            "Prompt queue · {} · {}",
            state.agent.label(),
            state.pending_len()
        );
        let emphasis = if state.focused {
            PanelChrome::Focused
        } else {
            PanelChrome::Normal
        };
        let panel = Panel::new(self.system)
            .title(title.as_str())
            .emphasis(emphasis);
        let inner = panel.inner(area);
        panel.paint(area, buffer, None);
        if inner.is_empty() {
            return;
        }

        let mut y = inner.y;
        let w = usize::from(inner.width);
        let max_y = inner.bottom();

        // Semantics banner
        if y < max_y {
            let banner = if state.agent.is_busy() {
                "agent busy · Enter sends next when free · i interrupt+send · no auto-drain on fail"
            } else {
                "Enter send · e edit · d delete · J/K reorder · Esc compact"
            };
            self.system.paint_row(
                buffer,
                Rect::new(inner.x, y, inner.width, 1),
                banner,
                self.system.style(Role::TextMuted),
            );
            y = y.saturating_add(1);
        }

        // Edit / confirm chrome
        match &state.phase {
            PromptQueuePhase::Edit { .. } if y < max_y => {
                let line = format!("edit › {}▎", state.edit_draft);
                self.system.paint_row(
                    buffer,
                    Rect::new(inner.x, y, inner.width, 1),
                    &line,
                    self.system.style(Role::Accent),
                );
                y = y.saturating_add(1);
            }
            PromptQueuePhase::ConfirmDelete { .. } => {}
            _ => {}
        }

        let footer = if matches!(state.phase, PromptQueuePhase::ConfirmDelete { .. }) {
            2u16
        } else {
            1u16
        };
        let list_bottom = max_y.saturating_sub(footer);

        if state.items.is_empty() {
            if y < list_bottom {
                self.system.paint_row(
                    buffer,
                    Rect::new(inner.x, y, inner.width, 1),
                    "(queue empty)",
                    self.system.style(Role::TextMuted),
                );
            }
        } else {
            let viewport = list_bottom.saturating_sub(y) as usize;
            // Read-only projection: re-derive the visible slice against the
            // painted viewport without mutating state during paint.
            let mut view = state.window;
            view.clamp(state.items.len(), viewport);
            let offset = view.scroll();

            for (i, item) in state.items.iter().enumerate().skip(offset) {
                if y >= list_bottom {
                    break;
                }
                let selected = i == state.window.cursor();
                let mark = if selected { "›" } else { " " };
                let att = if item.attachments.is_empty() && item.mentions.is_empty() {
                    String::new()
                } else {
                    format!(" [{}+{}]", item.attachments.len(), item.mentions.len())
                };
                let preview = item.preview(w.saturating_sub(12));
                let indicator = StatusIndicator::new(item.status.semantic(), self.system)
                    .label(item.status.id())
                    .colorless(self.colorless);
                let status_text = indicator.text(None);
                let text = format!("{mark}{status_text} · {preview}{att}");
                let style = self.system.style(Role::Text).add_modifier(if selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                });
                self.system
                    .paint_row(buffer, Rect::new(inner.x, y, inner.width, 1), &text, style);
                self.system.paint_row(
                    buffer,
                    Rect::new(inner.x, y, 1.min(inner.width), 1),
                    mark,
                    self.system.style(if selected && !self.colorless {
                        Role::Focus
                    } else {
                        Role::TextStrong
                    }),
                );
                if inner.width > 1 {
                    indicator.paint(
                        Rect::new(
                            inner.x.saturating_add(1),
                            y,
                            inner.width.saturating_sub(1),
                            1,
                        ),
                        buffer,
                        None,
                    );
                }
                state.row_hits.push((
                    item.id.clone(),
                    Rect {
                        x: inner.x,
                        y,
                        width: inner.width,
                        height: 1,
                    },
                ));
                y = y.saturating_add(1);
                // detail line
                if y < list_bottom && (selected || item.status == PromptQueueStatus::Failed) {
                    let detail = item
                        .error
                        .as_deref()
                        .or(item.blocked_reason.as_deref())
                        .or(item.when.as_deref())
                        .unwrap_or("");
                    if !detail.is_empty() {
                        self.system.paint_row(
                            buffer,
                            Rect::new(inner.x, y, inner.width, 1),
                            &format!("   {detail}"),
                            self.system.style(Role::TextMuted),
                        );
                        y = y.saturating_add(1);
                    }
                }
            }
        }

        if matches!(state.phase, PromptQueuePhase::ConfirmDelete { .. }) {
            self.paint_confirm(inner, buffer, state);
        } else if max_y > inner.y {
            let fy = max_y.saturating_sub(1);
            self.system.paint_row(
                buffer,
                Rect::new(inner.x, fy, inner.width, 1),
                "enter send · i interrupt+send · e edit · d del · esc close",
                self.system.style(Role::TextMuted),
            );
        }
    }

    fn paint_confirm(&self, area: Rect, buffer: &mut Buffer, state: &mut PromptQueueState) {
        let hits = ConfirmPrompt::new("Delete queue entry", "Delete", self.system)
            .detail("the host may drop its persistence")
            .focus(if state.confirm_proceed_focused {
                ConfirmFocus::Confirm
            } else {
                ConfirmFocus::Cancel
            })
            .paint(area, buffer);
        if let Some(cancel) = hits.cancel {
            state.confirm_hits.push((false, cancel));
        }
        if let Some(confirm) = hits.confirm {
            state.confirm_hits.push((true, confirm));
        }
    }
}

impl StatefulWidget for &PromptQueue<'_> {
    type State = PromptQueueState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl StatefulWidget for PromptQueue<'_> {
    type State = PromptQueueState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

// ── Bridges ─────────────────────────────────────────────────────────────────

/// Project composer chip ids + text into a queue item (identities preserved as chips).
#[must_use]
pub fn queue_item_from_composer(
    id: impl Into<String>,
    text: impl Into<String>,
    chips: &[(String, String, String)], // id, kind, label
) -> PromptQueueItem {
    let mut attachments = Vec::new();
    let mut mentions = Vec::new();
    for (cid, kind, label) in chips {
        let r = PromptQueueRef::new(cid.clone(), kind.clone(), label.clone());
        if kind == "mention" {
            mentions.push(r);
        } else {
            attachments.push(r);
        }
    }
    PromptQueueItem::new(id, text)
        .attachments(attachments)
        .mentions(mentions)
}

// ── Examples ────────────────────────────────────────────────────────────────

/// Demo queue.
#[must_use]
pub fn example_prompt_queue() -> Vec<PromptQueueItem> {
    vec![
        PromptQueueItem::new("q1", "Summarize the auth module")
            .when("now")
            .attachments(vec![PromptQueueRef::file("f1", "auth/mod.rs")])
            .status(PromptQueueStatus::Sending),
        PromptQueueItem::new("q2", "Then add unit tests for token expiry")
            .when("queued")
            .mentions(vec![PromptQueueRef::mention("m1", "@test")])
            .attachments(vec![PromptQueueRef::paste("p1", "paste 2kb")]),
        PromptQueueItem::new("q3", "Deploy canary")
            .blocked("waiting permission")
            .when("blocked"),
        PromptQueueItem::new("q4", "Broken path earlier")
            .failed("tool error: exit 1")
            .when("failed"),
    ]
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Paint stress.
pub mod bench {
    /// Frames.
    pub const PAINT_FRAMES: u32 = 30;
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::tests::{click, press};

    fn open() -> PromptQueueState {
        let mut st = PromptQueueState::new();
        st.set_items(example_prompt_queue());
        st.presentation = PromptQueuePresentation::Expanded;
        st.agent = AgentBusyState::Busy;
        st
    }

    #[test]
    fn compact_summary_shows_depth() {
        let mut st = open();
        st.presentation = PromptQueuePresentation::Compact;
        let s = st.compact_summary();
        assert!(s.contains("queue:"), "{s}");
        assert!(s.contains("busy") || s.contains("next"), "{s}");
    }

    #[test]
    fn expand_from_compact() {
        let mut st = open();
        st.presentation = PromptQueuePresentation::Compact;
        let out = st.handle_key(press(KeyCode::Enter));
        assert!(matches!(
            out,
            PromptQueueOutcome::PresentationChanged(PromptQueuePresentation::Expanded)
        ));
    }

    #[test]
    fn reorder_moves_queued() {
        let mut st = open();
        // q2 is index 1, can reorder
        st.window.set_cursor(1, st.items.len(), PROMPT_QUEUE_WINDOW);
        let id = st.current_id().unwrap();
        let out = st.handle_key(press(KeyCode::Char('J')));
        match out {
            PromptQueueOutcome::Reordered { id: rid, .. } => assert_eq!(rid, id),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn send_next() {
        let mut st = open();
        st.window.set_cursor(1, st.items.len(), PROMPT_QUEUE_WINDOW);
        let out = st.handle_key(press(KeyCode::Enter));
        assert!(matches!(
            out,
            PromptQueueOutcome::SendNext { ref id } if id == "q2"
        ));
    }

    #[test]
    fn interrupt_and_send() {
        let mut st = open();
        st.window.set_cursor(1, st.items.len(), PROMPT_QUEUE_WINDOW);
        let out = st.handle_key(press(KeyCode::Char('i')));
        assert!(matches!(
            out,
            PromptQueueOutcome::InterruptAndSend { ref id } if id == "q2"
        ));
    }

    #[test]
    fn edit_preserves_attachments() {
        let mut st = open();
        st.window.set_cursor(1, st.items.len(), PROMPT_QUEUE_WINDOW);
        let att_before = st.current().unwrap().attachments.clone();
        let _ = st.handle_key(press(KeyCode::Char('e')));
        assert!(matches!(st.phase, PromptQueuePhase::Edit { .. }));
        st.edit_draft = "edited text".into();
        let out = st.handle_key(press(KeyCode::Enter));
        assert!(matches!(
            out,
            PromptQueueOutcome::Edited { ref text, .. } if text == "edited text"
        ));
        assert_eq!(st.items[1].attachments, att_before);
        assert_eq!(st.items[1].mentions.len(), 1);
    }

    #[test]
    fn delete_confirm_cancel_default() {
        let mut st = open();
        st.window.set_cursor(1, st.items.len(), PROMPT_QUEUE_WINDOW);
        let out = st.handle_key(press(KeyCode::Char('d')));
        assert!(matches!(out, PromptQueueOutcome::ConfirmOpened { .. }));
        assert!(!st.confirm_proceed_focused);
        let out = st.handle_key(press(KeyCode::Enter));
        assert!(matches!(out, PromptQueueOutcome::ConfirmCancelled));
        assert_eq!(st.items.len(), 4);
    }

    #[test]
    fn delete_confirm_proceed() {
        let mut st = open();
        st.window.set_cursor(1, st.items.len(), PROMPT_QUEUE_WINDOW);
        let _ = st.handle_key(press(KeyCode::Char('d')));
        st.confirm_proceed_focused = true;
        let out = st.handle_key(press(KeyCode::Enter));
        assert!(matches!(
            out,
            PromptQueueOutcome::Deleted { ref id } if id == "q2"
        ));
        assert_eq!(st.items.len(), 3);
    }

    #[test]
    fn failed_not_auto_removed() {
        let mut st = open();
        assert!(
            st.items
                .iter()
                .any(|i| i.status == PromptQueueStatus::Failed)
        );
        // no drain on set_agent etc
        st.set_agent(AgentBusyState::Idle);
        assert!(
            st.items
                .iter()
                .any(|i| i.status == PromptQueueStatus::Failed)
        );
    }

    #[test]
    fn retry_failed() {
        let mut st = open();
        let i = st
            .items
            .iter()
            .position(|e| e.status == PromptQueueStatus::Failed)
            .unwrap();
        st.window.set_cursor(i, st.items.len(), PROMPT_QUEUE_WINDOW);
        let out = st.handle_key(press(KeyCode::Char('r')));
        assert!(matches!(out, PromptQueueOutcome::Retry { ref id } if id == "q4"));
    }

    #[test]
    fn chip_ids_bridge() {
        let item = PromptQueueItem::from_text_and_chip_ids("q", "hi", ["a", "b"]);
        assert_eq!(item.chip_ids(), vec!["a", "b"]);
    }

    #[test]
    fn queue_item_from_composer_splits_mentions() {
        let item = queue_item_from_composer(
            "q",
            "text",
            &[
                ("f1".into(), "file".into(), "f".into()),
                ("m1".into(), "mention".into(), "@x".into()),
            ],
        );
        assert_eq!(item.attachments.len(), 1);
        assert_eq!(item.mentions.len(), 1);
    }

    #[test]
    fn y_unbound() {
        let mut st = open();
        assert!(matches!(
            st.handle_key(press(KeyCode::Char('y'))),
            PromptQueueOutcome::Ignored
        ));
    }

    #[test]
    fn no_auto_drain_policy_in_source() {
        let src = include_str!("prompt_queue.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        assert!(body.contains("KD-29") || body.contains("auto-drain"));
        for f in ["std::process", "Command::new", "openai"] {
            assert!(!body.contains(f), "{f}");
        }
    }

    #[test]
    fn accepts_input_gate() {
        let mut st = open();
        st.set_accepts_input(false);
        assert!(matches!(
            st.handle_key(press(KeyCode::Enter)),
            PromptQueueOutcome::Ignored
        ));
    }

    #[test]
    fn paint_compact_and_expanded() {
        let system = DesignSystem::default();
        let mut st = open();
        let area = Rect::new(0, 0, 56, 14);
        let mut buf = Buffer::empty(area);
        st.presentation = PromptQueuePresentation::Compact;
        PromptQueue::new(&system).paint(area, &mut buf, &mut st);
        st.presentation = PromptQueuePresentation::Expanded;
        PromptQueue::new(&system)
            .colorless(true)
            .paint(area, &mut buf, &mut st);
        st.phase = PromptQueuePhase::ConfirmDelete { id: "q2".into() };
        PromptQueue::new(&system).paint(area, &mut buf, &mut st);
    }

    #[test]
    fn paint_perf() {
        let system = DesignSystem::default();
        let mut st = PromptQueueState::new();
        let many: Vec<_> = (0..80)
            .map(|i| PromptQueueItem::new(format!("q{i}"), format!("message {i}")))
            .collect();
        st.set_items(many);
        st.presentation = PromptQueuePresentation::Expanded;
        let area = Rect::new(0, 0, 60, 18);
        let mut buf = Buffer::empty(area);
        let start = std::time::Instant::now();
        for _ in 0..bench::PAINT_FRAMES {
            PromptQueue::new(&system).paint(area, &mut buf, &mut st);
        }
        assert!(start.elapsed().as_secs() < 3, "{:?}", start.elapsed());
    }

    #[test]
    fn fuzz_status() {
        for s in [
            PromptQueueStatus::Queued,
            PromptQueueStatus::Sending,
            PromptQueueStatus::Blocked,
            PromptQueueStatus::Failed,
            PromptQueueStatus::Cancelled,
            PromptQueueStatus::Sent,
        ] {
            assert!(!s.id().is_empty());
            let _ = s.glyph(true);
            let _ = s.can_send();
        }
    }

    #[test]
    fn mouse_select() {
        let system = DesignSystem::default();
        let mut st = open();
        let area = Rect::new(0, 0, 56, 14);
        let mut buf = Buffer::empty(area);
        PromptQueue::new(&system).paint(area, &mut buf, &mut st);
        assert!(!st.row_hits.is_empty());
        let (id, r) = st.row_hits[0].clone();
        let out = st.handle_mouse(click(r.x, r.y));
        assert!(
            matches!(out, PromptQueueOutcome::Selected { .. }),
            "{out:?} {id}"
        );
    }

    #[test]
    fn unicode_text() {
        let system = DesignSystem::default();
        let mut st = PromptQueueState::new();
        st.set_items(vec![
            PromptQueueItem::new("u1", "検査して 🔍")
                .attachments(vec![PromptQueueRef::file("f", "日本語.rs")]),
        ]);
        st.presentation = PromptQueuePresentation::Expanded;
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        PromptQueue::new(&system).paint(area, &mut buf, &mut st);
    }

    #[test]
    fn sending_cannot_reorder() {
        let mut st = open();
        st.window.set_cursor(0, st.items.len(), PROMPT_QUEUE_WINDOW); // sending
        assert!(matches!(
            st.handle_key(press(KeyCode::Char('J'))),
            PromptQueueOutcome::Ignored
        ));
    }

    #[test]
    fn pop_front_sendable_skips_sending() {
        let mut st = open();
        let front = st.pop_front_sendable().unwrap();
        assert_eq!(front.id, "q2"); // q1 is Sending
    }
}
