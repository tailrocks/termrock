// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Settings control that captures and validates user keybindings.
//!
//! **Mission.** Preferences UIs need a safe recorder: capture chords/sequences,
//! show normalized glyphs, surface conflicts/reserved/protocol limits, and
//! always provide an escape path. Hosts own the [`Keymap`](crate::keymap::Keymap)
//! and apply commits via [`Keymap::remap`](crate::keymap::Keymap::remap).
//!
//! **vs [`Kbd`](super::Kbd) / [`ShortcutHint`](super::ShortcutHint).** Display
//! only. Recorder is the capture + validate control.
//! **vs KeyboardHelp (future).** Help lists all bindings; recorder edits one.
//!
//! **Escape law.** Esc **always** exits recording (never traps). Host may also
//! bind a cancel chord; Esc remains hard-wired.
//!
//! Research: editor keybinding settings, terminal protocol limits (no F-keys in
//! neutral vocabulary, CSI ambiguity, Ctrl+C SIGINT).
use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::StatefulWidget};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers},
    interaction::{SemanticNode, SemanticRole, SemanticScene, SemanticState},
    keymap::{KeyBinding, KeyChord, Keymap, Visibility, raw_bytes_to_chord},
    style::{ControlState, DesignSystem, Role},
    text::take_display_cols,
};

use super::{
    ChordFormat, Panel, PanelChrome, PanelVariant, Platform, Validation, format_chord,
    format_sequence,
};

/// Default sequence separator in multi-chord display.
pub const KEYBINDING_SEQUENCE_SEP: &str = " ";

// ── Protocol / reserved model ───────────────────────────────────────────────

/// Why a chord cannot (or should not) be bound.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum BindingLimit {
    /// Host- or system-reserved chord.
    Reserved {
        /// Human reason.
        reason: String,
    },
    /// Collides with another action in the conflict table.
    Conflict {
        /// Other action label.
        with: String,
    },
    /// Neutral protocol cannot represent the key (e.g. F-keys → Unknown).
    Protocol {
        /// Human reason.
        reason: String,
    },
    /// Empty binding while empty disallowed.
    Empty,
    /// Sequence not finished / intermediate.
    Intermediate,
}

impl BindingLimit {
    /// Stable id.
    #[must_use]
    pub fn id(&self) -> &'static str {
        match self {
            Self::Reserved { .. } => "reserved",
            Self::Conflict { .. } => "conflict",
            Self::Protocol { .. } => "protocol",
            Self::Empty => "empty",
            Self::Intermediate => "intermediate",
        }
    }

    /// Display message.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::Reserved { reason } => format!("reserved: {reason}"),
            Self::Conflict { with } => format!("conflicts with “{with}”"),
            Self::Protocol { reason } => format!("protocol: {reason}"),
            Self::Empty => "empty binding".into(),
            Self::Intermediate => "recording…".into(),
        }
    }
}

/// Built-in terminal-hazard reserved chords (host may extend).
#[must_use]
pub fn default_reserved_chords() -> Vec<(KeyChord, String)> {
    vec![
        (
            KeyChord::ctrl(KeyCode::Char('c')),
            "often SIGINT / interrupt".into(),
        ),
        (
            KeyChord::ctrl(KeyCode::Char('z')),
            "often SIGTSTP / suspend".into(),
        ),
        (
            KeyChord::ctrl(KeyCode::Char('s')),
            "XON/XOFF stop (software flow control)".into(),
        ),
        (
            KeyChord::ctrl(KeyCode::Char('q')),
            "XON/XOFF start (software flow control)".into(),
        ),
        (KeyChord::ctrl(KeyCode::Char('\\')), "often SIGQUIT".into()),
    ]
}

/// Platform / protocol notes for help chrome.
#[must_use]
pub fn protocol_limitations(platform: Platform) -> Vec<&'static str> {
    let mut notes = vec![
        "Neutral KeyCode has no F-keys/media keys (arrive as Unknown).",
        "Shift on Char is folded into the character (Q vs q).",
        "Kitty/enhanced modifiers need host KeyEvent mapping into KeyChord.",
        "Esc always cancels recording (escape law).",
    ];
    if matches!(platform.resolve(), Platform::Mac) {
        notes.push("Mac Option may arrive as Alt; Cmd is not in KeyModifiers.");
    }
    notes
}

// ── Mode / outcome ──────────────────────────────────────────────────────────

/// Recorder interaction mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum KeybindingRecorderMode {
    /// Showing committed value; not capturing.
    #[default]
    Idle,
    /// Capturing chords (Esc cancels).
    Recording,
}

impl KeybindingRecorderMode {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Recording => "recording",
        }
    }
}

/// Outcomes for host settings loops.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum KeybindingRecorderOutcome {
    /// No effect.
    Ignored,
    /// Chrome / draft display changed.
    Changed,
    /// Entered recording (keys will be captured).
    RecordingStarted,
    /// Left recording without commit (Esc / cancel).
    RecordingCancelled {
        /// Draft discarded.
        restored: Vec<KeyChord>,
    },
    /// One chord appended while recording.
    ChordCaptured {
        /// Chord.
        chord: KeyChord,
        /// Full draft sequence so far.
        sequence: Vec<KeyChord>,
    },
    /// Committed binding (Enter while recording, or idle commit).
    Committed {
        /// Final chords (may be multi-chord sequence or alternates as sequence).
        chords: Vec<KeyChord>,
    },
    /// Cleared to empty.
    Cleared,
    /// Restored factory default chords.
    RestoredDefault {
        /// Default chords.
        chords: Vec<KeyChord>,
    },
    /// Commit blocked.
    ValidationFailed {
        /// Limit.
        limit: BindingLimit,
    },
    /// Focus left control.
    Blurred,
}

// ── State ───────────────────────────────────────────────────────────────────

/// Runtime state for [`KeybindingRecorder`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeybindingRecorderState {
    /// Stable action id (host settings key).
    action_id: String,
    /// Human action label.
    action_label: String,
    /// Committed chords.
    value: Vec<KeyChord>,
    /// Factory default (restore).
    default: Vec<KeyChord>,
    /// Draft while recording.
    draft: Vec<KeyChord>,
    mode: KeybindingRecorderMode,
    /// Reserved chords → reason.
    reserved: Vec<(KeyChord, String)>,
    /// Occupied chords from other actions → label (conflict table).
    occupied: Vec<(KeyChord, String)>,
    /// Allow multi-chord sequences (g g style).
    allow_sequences: bool,
    /// Allow empty value.
    allow_empty: bool,
    /// Treat conflicts as hard errors (vs warning-only).
    hard_conflicts: bool,
    /// Treat reserved as hard errors.
    hard_reserved: bool,
    format: ChordFormat,
    focused: bool,
    enabled: bool,
    last_limit: Option<BindingLimit>,
    /// Capture count while recording (for chrome).
    capture_count: u32,
    root: Rect,
}

impl Default for KeybindingRecorderState {
    fn default() -> Self {
        Self::new("action", "Action")
    }
}

impl KeybindingRecorderState {
    /// Recorder for one action.
    #[must_use]
    pub fn new(action_id: impl Into<String>, action_label: impl Into<String>) -> Self {
        Self {
            action_id: action_id.into(),
            action_label: action_label.into(),
            value: Vec::new(),
            default: Vec::new(),
            draft: Vec::new(),
            mode: KeybindingRecorderMode::Idle,
            reserved: default_reserved_chords(),
            occupied: Vec::new(),
            allow_sequences: true,
            allow_empty: true,
            hard_conflicts: true,
            hard_reserved: true,
            format: ChordFormat::footer(),
            focused: false,
            enabled: true,
            last_limit: None,
            capture_count: 0,
            root: Rect::default(),
        }
    }

    /// Initial + default chords.
    #[must_use]
    pub fn with_chords(mut self, chords: impl IntoIterator<Item = KeyChord>) -> Self {
        let v: Vec<_> = chords.into_iter().collect();
        self.value = v.clone();
        self.default = v;
        self
    }
    /// Chord display format.
    #[must_use]
    pub const fn with_format(mut self, fmt: ChordFormat) -> Self {
        self.format = fmt;
        self
    }

    /// Multi-chord sequences.
    #[must_use]
    pub const fn with_sequences(mut self, on: bool) -> Self {
        self.allow_sequences = on;
        self
    }

    /// Allow empty commit.
    #[must_use]
    pub const fn with_allow_empty(mut self, on: bool) -> Self {
        self.allow_empty = on;
        self
    }
    /// Load occupied chords from a [`Keymap`], skipping `skip` action.
    pub fn load_occupied_from_keymap<A>(
        &mut self,
        map: &Keymap<A>,
        skip: A,
        label: impl Fn(&A) -> String,
    ) where
        A: Clone + Copy + PartialEq + 'static,
    {
        let mut occ = Vec::new();
        for b in map.bindings() {
            if b.action() == &skip {
                continue;
            }
            let lab = label(b.action());
            for c in b.chords() {
                occ.push((*c, lab.clone()));
            }
        }
        self.occupied = occ;
    }

    /// Apply committed value onto a keymap action (host commit helper).
    pub fn apply_to_keymap<A>(&self, map: &mut Keymap<A>, action: A) -> bool
    where
        A: Clone + Copy + PartialEq + 'static,
    {
        if self.value.is_empty() {
            map.disable(action)
        } else {
            map.remap(action, self.value.clone())
        }
    }
    // ── accessors ───────────────────────────────────────────────────────────

    /// Committed chords.
    #[must_use]
    pub fn value(&self) -> &[KeyChord] {
        &self.value
    }
    /// Draft while recording (else empty).
    #[must_use]
    pub fn draft(&self) -> &[KeyChord] {
        &self.draft
    }

    /// Mode.
    #[must_use]
    pub const fn mode(&self) -> KeybindingRecorderMode {
        self.mode
    }

    /// Recording?
    #[must_use]
    pub const fn is_recording(&self) -> bool {
        matches!(self.mode, KeybindingRecorderMode::Recording)
    }

    /// Normalized display of committed value.
    #[must_use]
    pub fn display_value(&self) -> String {
        self.format_sequence(&self.value)
    }

    /// Normalized display of draft or value for paint.
    #[must_use]
    pub fn display_live(&self) -> String {
        if self.is_recording() {
            if self.draft.is_empty() {
                "…".into()
            } else {
                self.format_sequence(&self.draft)
            }
        } else if self.value.is_empty() {
            "—".into()
        } else {
            self.format_sequence(&self.value)
        }
    }

    fn format_sequence(&self, chords: &[KeyChord]) -> String {
        if chords.is_empty() {
            return String::new();
        }
        if chords.len() == 1 {
            format_chord(chords[0], self.format)
        } else {
            format_sequence(chords, self.format, KEYBINDING_SEQUENCE_SEP)
        }
    }

    /// Focus.
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Enabled.
    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
        if !on && self.is_recording() {
            let _ = self.cancel_recording();
        }
    }

    /// Set committed chords without validation (host load).
    pub fn set_value(&mut self, chords: impl IntoIterator<Item = KeyChord>) {
        self.value = chords.into_iter().collect();
        self.last_limit = None;
    }

    /// Validate a sequence against reserved/conflict/empty rules.
    #[must_use]
    pub fn validate(&self, chords: &[KeyChord]) -> Result<(), BindingLimit> {
        if chords.is_empty() {
            return if self.allow_empty {
                Ok(())
            } else {
                Err(BindingLimit::Empty)
            };
        }
        for c in chords {
            if matches!(c.key, KeyCode::Unknown) {
                return Err(BindingLimit::Protocol {
                    reason: "key not representable in neutral KeyCode (F-keys/media/etc.)".into(),
                });
            }
            if let Some((_, reason)) = self.reserved.iter().find(|(r, _)| r == c) {
                if self.hard_reserved {
                    return Err(BindingLimit::Reserved {
                        reason: reason.clone(),
                    });
                }
            }
            if let Some((_, with)) = self.occupied.iter().find(|(o, _)| o == c) {
                if self.hard_conflicts {
                    return Err(BindingLimit::Conflict { with: with.clone() });
                }
            }
        }
        Ok(())
    }

    /// Soft issues for display (even when not hard-failing).
    #[must_use]
    pub fn soft_issues(&self, chords: &[KeyChord]) -> Vec<BindingLimit> {
        let mut out = Vec::new();
        for c in chords {
            if matches!(c.key, KeyCode::Unknown) {
                out.push(BindingLimit::Protocol {
                    reason: "Unknown key code".into(),
                });
            }
            if let Some((_, reason)) = self.reserved.iter().find(|(r, _)| r == c) {
                out.push(BindingLimit::Reserved {
                    reason: reason.clone(),
                });
            }
            if let Some((_, with)) = self.occupied.iter().find(|(o, _)| o == c) {
                out.push(BindingLimit::Conflict { with: with.clone() });
            }
        }
        out
    }

    /// Start recording (clears draft).
    pub fn start_recording(&mut self) -> KeybindingRecorderOutcome {
        if !self.enabled {
            return KeybindingRecorderOutcome::Ignored;
        }
        self.mode = KeybindingRecorderMode::Recording;
        self.draft.clear();
        self.capture_count = 0;
        self.last_limit = Some(BindingLimit::Intermediate);
        KeybindingRecorderOutcome::RecordingStarted
    }

    /// Cancel recording; restore display to committed value.
    pub fn cancel_recording(&mut self) -> KeybindingRecorderOutcome {
        let restored = self.value.clone();
        self.mode = KeybindingRecorderMode::Idle;
        self.draft.clear();
        self.capture_count = 0;
        self.last_limit = None;
        KeybindingRecorderOutcome::RecordingCancelled { restored }
    }

    /// Clear committed binding.
    pub fn clear(&mut self) -> KeybindingRecorderOutcome {
        if !self.enabled {
            return KeybindingRecorderOutcome::Ignored;
        }
        if self.is_recording() {
            let _ = self.cancel_recording();
        }
        if !self.allow_empty {
            self.last_limit = Some(BindingLimit::Empty);
            return KeybindingRecorderOutcome::ValidationFailed {
                limit: BindingLimit::Empty,
            };
        }
        self.value.clear();
        self.last_limit = None;
        KeybindingRecorderOutcome::Cleared
    }

    /// Restore factory default.
    pub fn restore_default(&mut self) -> KeybindingRecorderOutcome {
        if !self.enabled {
            return KeybindingRecorderOutcome::Ignored;
        }
        if self.is_recording() {
            let _ = self.cancel_recording();
        }
        match self.validate(&self.default) {
            Ok(()) => {
                self.value = self.default.clone();
                self.last_limit = None;
                KeybindingRecorderOutcome::RestoredDefault {
                    chords: self.value.clone(),
                }
            }
            Err(limit) => {
                // Still restore value; report soft fail if hard rules block
                self.value = self.default.clone();
                self.last_limit = Some(limit.clone());
                if self.hard_conflicts || self.hard_reserved {
                    KeybindingRecorderOutcome::ValidationFailed { limit }
                } else {
                    KeybindingRecorderOutcome::RestoredDefault {
                        chords: self.value.clone(),
                    }
                }
            }
        }
    }

    /// Commit `chords` (or draft if recording).
    pub fn commit(&mut self) -> KeybindingRecorderOutcome {
        let chords = if self.is_recording() {
            self.draft.clone()
        } else {
            self.value.clone()
        };
        match self.validate(&chords) {
            Ok(()) => {
                self.value = chords.clone();
                self.mode = KeybindingRecorderMode::Idle;
                self.draft.clear();
                self.capture_count = 0;
                self.last_limit = None;
                KeybindingRecorderOutcome::Committed { chords }
            }
            Err(limit) => {
                self.last_limit = Some(limit.clone());
                KeybindingRecorderOutcome::ValidationFailed { limit }
            }
        }
    }

    /// Capture one chord (recording only).
    pub fn capture_chord(&mut self, chord: KeyChord) -> KeybindingRecorderOutcome {
        if !self.is_recording() || !self.enabled {
            return KeybindingRecorderOutcome::Ignored;
        }
        // Escape law: Esc never becomes a binding via capture path
        if chord.key == KeyCode::Esc && chord.mods.is_empty() {
            return self.cancel_recording();
        }
        if matches!(chord.key, KeyCode::Unknown) {
            let limit = BindingLimit::Protocol {
                reason: "key not representable in neutral KeyCode".into(),
            };
            self.last_limit = Some(limit.clone());
            return KeybindingRecorderOutcome::ValidationFailed { limit };
        }
        // Soft/hard checks for this chord alone
        if let Err(limit) = self.validate(std::slice::from_ref(&chord)) {
            // Still allow viewing reserved as fail if hard
            self.last_limit = Some(limit.clone());
            if matches!(
                limit,
                BindingLimit::Reserved { .. } | BindingLimit::Conflict { .. }
            ) {
                return KeybindingRecorderOutcome::ValidationFailed { limit };
            }
        }
        if self.allow_sequences {
            self.draft.push(chord);
        } else {
            self.draft = vec![chord];
        }
        self.capture_count = self.capture_count.saturating_add(1);
        self.last_limit = self.soft_issues(&self.draft).into_iter().next();
        // Single-chord mode: auto-commit on first valid capture
        if !self.allow_sequences {
            return self.commit();
        }
        KeybindingRecorderOutcome::ChordCaptured {
            chord,
            sequence: self.draft.clone(),
        }
    }

    /// Capture from raw PTY bytes (conventional protocol path).
    pub fn capture_bytes(&mut self, bytes: &[u8]) -> KeybindingRecorderOutcome {
        match raw_bytes_to_chord(bytes) {
            Some(chord) => self.capture_chord(chord),
            None => {
                let limit = BindingLimit::Protocol {
                    reason: "unrecognized PTY sequence".into(),
                };
                self.last_limit = Some(limit.clone());
                KeybindingRecorderOutcome::ValidationFailed { limit }
            }
        }
    }

    /// Pop last draft chord (Backspace while recording).
    pub fn pop_draft(&mut self) -> KeybindingRecorderOutcome {
        if !self.is_recording() {
            return KeybindingRecorderOutcome::Ignored;
        }
        if self.draft.pop().is_some() {
            self.last_limit = self.soft_issues(&self.draft).into_iter().next();
            KeybindingRecorderOutcome::Changed
        } else {
            KeybindingRecorderOutcome::Ignored
        }
    }

    /// Key adapter.
    pub fn handle_key(&mut self, key: KeyEvent) -> KeybindingRecorderOutcome {
        // Each captured chord and recorder action represents one physical
        // key press. Repeats must not append duplicate chords or retrigger a
        // mode transition while a key is held.
        if !key.is_press() || !self.enabled {
            return KeybindingRecorderOutcome::Ignored;
        }
        if !self.focused {
            return KeybindingRecorderOutcome::Ignored;
        }

        let chord = KeyChord::from(key);

        if self.is_recording() {
            // Escape law first — plain Esc always cancels
            if key.code == KeyCode::Esc && key.modifiers.is_empty() {
                return self.cancel_recording();
            }
            // Enter commits sequence
            if key.code == KeyCode::Enter && key.modifiers.is_empty() {
                return self.commit();
            }
            // Backspace pops
            if key.code == KeyCode::Backspace && key.modifiers.is_empty() {
                return self.pop_draft();
            }
            // All other keys captured (including modified Esc)
            return self.capture_chord(chord);
        }

        // Idle
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') if key.modifiers.is_empty() => {
                self.start_recording()
            }
            KeyCode::Char('c') | KeyCode::Char('C')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                // Ctrl+C in idle is NOT captured — host may still get it; we clear
                self.clear()
            }
            KeyCode::Delete | KeyCode::Backspace if key.modifiers.is_empty() => self.clear(),
            KeyCode::Char('r') | KeyCode::Char('R') if key.modifiers.is_empty() => {
                self.restore_default()
            }
            KeyCode::Esc if key.modifiers.is_empty() => KeybindingRecorderOutcome::Blurred,
            _ => KeybindingRecorderOutcome::Ignored,
        }
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Keybinding recorder chrome.
#[derive(Debug, Clone, Copy)]
pub struct KeybindingRecorder<'a> {
    system: &'a DesignSystem,
    show_limits: bool,
    show_hints: bool,
}

impl<'a> KeybindingRecorder<'a> {
    /// Create.
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self {
            system,
            show_limits: true,
            show_hints: true,
        }
    }

    /// ASCII-only marks.
    #[must_use]
    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut KeybindingRecorderState) {
        state.root = area;
        if area.is_empty() {
            return;
        }

        let panel = Panel::new(self.system)
            .variant(PanelVariant::Bordered)
            .emphasis(if state.focused {
                PanelChrome::Focused
            } else {
                PanelChrome::Normal
            });
        let inner = panel.inner(area);
        let title = if state.is_recording() {
            "Recording — Esc cancel · Enter accept"
        } else {
            state.action_label.as_str()
        };
        panel.title(title).paint(area, buffer, None);
        if inner.is_empty() {
            return;
        }

        let mut y = inner.y;

        // Action id (muted)
        if inner.height >= 3 {
            let id_line = format!("id: {}", state.action_id);
            buffer.set_stringn(
                inner.x,
                y,
                take_display_cols(&id_line, usize::from(inner.width)),
                usize::from(inner.width),
                self.system.style(Role::TextMuted),
            );
            y = y.saturating_add(1);
        }

        // Live chord display via Kbd-like paint
        if y < inner.bottom() {
            let invalid = state
                .last_limit
                .as_ref()
                .is_some_and(|limit| !matches!(limit, BindingLimit::Intermediate));
            let recipe = self.system.input_recipe(
                if !state.enabled {
                    ControlState::Disabled
                } else if state.focused {
                    ControlState::Focused
                } else {
                    ControlState::Default
                },
                state.is_recording(),
            );
            let live = state.display_live();
            let rec_mark = if state.is_recording() { "● " } else { "" };
            let line = format!("{rec_mark}{live}");
            let mut style = if state.is_recording() {
                // The caret recipe is already the explicit pair; stacking a
                // reversal on top swapped it back to invisible.
                recipe.cursor
            } else {
                recipe.value.add_modifier(Modifier::BOLD)
            };
            if invalid {
                style = style.patch(self.system.style(Role::Danger));
            }
            let live_row = Rect::new(inner.x, y, inner.width, 1);
            buffer.set_style(live_row, recipe.fill);
            if let Some((glyph, prompt_style)) = recipe.prompt {
                buffer.set_stringn(inner.x, y, glyph, 1, prompt_style);
            }
            let value_x = inner.x.saturating_add(1).min(inner.right());
            let value_width = inner.width.saturating_sub(1);
            buffer.set_stringn(
                value_x,
                y,
                take_display_cols(&line, usize::from(value_width)),
                usize::from(value_width),
                style,
            );
            y = y.saturating_add(1);
        }

        // Default line
        if y < inner.bottom() && !state.default.is_empty() {
            let def = format!("default: {}", state.format_sequence(&state.default));
            buffer.set_stringn(
                inner.x,
                y,
                take_display_cols(&def, usize::from(inner.width)),
                usize::from(inner.width),
                self.system.style(Role::TextMuted),
            );
            y = y.saturating_add(1);
        }

        // Limit / validation
        if self.show_limits && y < inner.bottom() {
            if let Some(limit) = &state.last_limit {
                let msg = limit.message();
                crate::widgets::field_message::paint_field_message(
                    buffer,
                    Rect::new(inner.x, y, inner.width, 1),
                    self.system,
                    match limit {
                        BindingLimit::Intermediate => crate::widgets::label::DescriptionKind::Meta,
                        _ => crate::widgets::label::DescriptionKind::Error,
                    },
                    &msg,
                );
            } else if !state.is_recording() {
                // soft issues on value
                let soft = state.soft_issues(&state.value);
                if let Some(issue) = soft.first() {
                    crate::widgets::field_message::paint_field_message(
                        buffer,
                        Rect::new(inner.x, y, inner.width, 1),
                        self.system,
                        crate::widgets::label::DescriptionKind::Warning,
                        &issue.message(),
                    );
                }
            }
            // Validation owns a permanent row so limits do not move the
            // protocol note or hint bar when they appear.
            y = y.saturating_add(1);
        }

        // Protocol notes (compact, one line)
        if self.show_limits && y < inner.bottom() && state.is_recording() {
            let note = { "protocol: no F-keys in neutral map · Esc cancels" };
            buffer.set_stringn(
                inner.x,
                y,
                take_display_cols(note, usize::from(inner.width)),
                usize::from(inner.width),
                self.system.style(Role::TextMuted),
            );
            y = y.saturating_add(1);
        }

        // Hints
        if self.show_hints && y < inner.bottom() && !state.is_recording() {
            let hints = { "Enter/Space record · r restore · Del clear · Esc blur" };
            buffer.set_stringn(
                inner.x,
                y,
                take_display_cols(hints, usize::from(inner.width)),
                usize::from(inner.width),
                self.system.style(Role::TextMuted),
            );
        }

        // Validation role for field chrome parity
        let _ = Validation::Valid;
    }

    /// Semantic registration.
    pub fn register_semantic<Id, Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        id: Id,
        area: Rect,
        state: &KeybindingRecorderState,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if area.is_empty() {
            return;
        }
        let desc = format!(
            "keybinding-recorder {} mode={} value={}",
            state.action_id,
            state.mode.id(),
            state.display_value()
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Input)
                .label(&state.action_label)
                .description(desc)
                .focusable(state.enabled)
                .disabled(!state.enabled)
                .state(SemanticState {
                    selected: state.focused,
                    busy: state.is_recording(),
                    invalid: state
                        .last_limit
                        .as_ref()
                        .is_some_and(|l| !matches!(l, BindingLimit::Intermediate)),
                    expanded: state.is_recording(),
                    ..Default::default()
                }),
        );
    }
}

impl StatefulWidget for &KeybindingRecorder<'_> {
    type State = KeybindingRecorderState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        let _ = self.paint(area, buffer, state);
    }
}

impl StatefulWidget for KeybindingRecorder<'_> {
    type State = KeybindingRecorderState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

/// Build a one-off binding for KeyboardHelp / Keymap from recorder value.
#[must_use]
pub fn binding_from_recorder<A: Clone + 'static>(
    state: &KeybindingRecorderState,
    action: A,
    visibility: Visibility,
) -> KeyBinding<A> {
    KeyBinding::owned(
        state.value.clone(),
        action,
        Some(state.action_label.clone()),
        visibility,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyEventKind;
    use crate::style::RolePalette;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Act {
        Save,
        Quit,
        Find,
    }

    fn sample_map() -> Keymap<Act> {
        Keymap::from_owned(vec![
            KeyBinding::owned(
                vec![KeyChord::ctrl(KeyCode::Char('s'))],
                Act::Save,
                Some("save".into()),
                Visibility::Shown,
                None,
            ),
            KeyBinding::owned(
                vec![KeyChord::ctrl(KeyCode::Char('q'))],
                Act::Quit,
                Some("quit".into()),
                Visibility::Shown,
                None,
            ),
            KeyBinding::owned(
                vec![KeyChord::plain(KeyCode::Char('/'))],
                Act::Find,
                Some("find".into()),
                Visibility::Shown,
                None,
            ),
        ])
    }

    #[test]
    fn record_single_chord_sequence_commit() {
        let mut state = KeybindingRecorderState::new("save", "Save")
            .with_chords([KeyChord::ctrl(KeyCode::Char('s'))])
            .with_sequences(true);
        // avoid reserved/conflict on Ctrl+S for this test
        state.reserved.clear();
        state.set_focused(true);
        assert!(matches!(
            state.start_recording(),
            KeybindingRecorderOutcome::RecordingStarted
        ));
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL)),
            KeybindingRecorderOutcome::ChordCaptured { .. }
        ));
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            KeybindingRecorderOutcome::Committed { chords }
                if chords == [KeyChord::ctrl(KeyCode::Char('x'))]
        ));
        assert!(!state.is_recording());
        assert_eq!(state.value(), &[KeyChord::ctrl(KeyCode::Char('x'))]);
    }

    #[test]
    fn esc_always_cancels_recording() {
        let mut state = KeybindingRecorderState::new("a", "A")
            .with_chords([KeyChord::plain(KeyCode::Char('a'))]);
        state.set_focused(true);
        let _ = state.start_recording();
        let _ = state.capture_chord(KeyChord::plain(KeyCode::Char('z')));
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            KeybindingRecorderOutcome::RecordingCancelled { restored }
                if restored == [KeyChord::plain(KeyCode::Char('a'))]
        ));
        assert_eq!(state.value(), &[KeyChord::plain(KeyCode::Char('a'))]);
    }

    #[test]
    fn reserved_blocks_commit() {
        let mut state = KeybindingRecorderState::new("x", "X").with_sequences(false);
        state.set_focused(true);
        let _ = state.start_recording();
        assert!(matches!(
            state.capture_chord(KeyChord::ctrl(KeyCode::Char('c'))),
            KeybindingRecorderOutcome::ValidationFailed {
                limit: BindingLimit::Reserved { .. }
            }
        ));
    }

    #[test]
    fn conflict_from_keymap() {
        let map = sample_map();
        let mut state = KeybindingRecorderState::new("find", "Find")
            .with_chords([KeyChord::plain(KeyCode::Char('/'))])
            .with_sequences(false);
        state.reserved.clear();
        state.load_occupied_from_keymap(&map, Act::Find, |a| format!("{a:?}"));
        state.set_focused(true);
        let _ = state.start_recording();
        // try to steal Save's Ctrl+S
        assert!(matches!(
            state.capture_chord(KeyChord::ctrl(KeyCode::Char('s'))),
            KeybindingRecorderOutcome::ValidationFailed {
                limit: BindingLimit::Conflict { .. }
            }
        ));
    }

    #[test]
    fn restore_default_and_clear() {
        let mut state = KeybindingRecorderState::new("a", "A")
            .with_chords([KeyChord::plain(KeyCode::Char('a'))]);
        state.reserved.clear();
        state.set_focused(true);
        state.set_value([KeyChord::plain(KeyCode::Char('b'))]);
        assert!(matches!(
            state.restore_default(),
            KeybindingRecorderOutcome::RestoredDefault { chords }
                if chords == [KeyChord::plain(KeyCode::Char('a'))]
        ));
        assert!(matches!(state.clear(), KeybindingRecorderOutcome::Cleared));
        assert!(state.value().is_empty());
    }

    #[test]
    fn apply_to_keymap() {
        let mut map = sample_map();
        let mut state = KeybindingRecorderState::new("save", "Save")
            .with_chords([KeyChord::ctrl(KeyCode::Char('s'))]);
        state.reserved.clear();
        state.set_value([KeyChord::alt(KeyCode::Char('s'))]);
        assert!(state.apply_to_keymap(&mut map, Act::Save));
        assert_eq!(
            map.dispatch(KeyChord::alt(KeyCode::Char('s'))),
            Some(Act::Save)
        );
    }

    #[test]
    fn pty_bytes_capture() {
        let mut state = KeybindingRecorderState::new("q", "Quit").with_sequences(false);
        state.reserved.clear();
        state.set_focused(true);
        let _ = state.start_recording();
        // Ctrl+X = 0x18
        assert!(matches!(
            state.capture_bytes(&[0x18]),
            KeybindingRecorderOutcome::Committed { chords }
                if chords == [KeyChord::ctrl(KeyCode::Char('x'))]
        ));
    }

    #[test]
    fn unknown_key_protocol_limit() {
        let mut state = KeybindingRecorderState::new("a", "A");
        state.set_focused(true);
        let _ = state.start_recording();
        assert!(matches!(
            state.capture_chord(KeyChord::plain(KeyCode::Unknown)),
            KeybindingRecorderOutcome::ValidationFailed {
                limit: BindingLimit::Protocol { .. }
            }
        ));
    }

    #[test]
    fn display_normalized() {
        let state = KeybindingRecorderState::new("s", "Save")
            .with_chords([KeyChord::ctrl(KeyCode::Char('s'))])
            .with_format(ChordFormat::footer());
        let d = state.display_value();
        assert!(
            d.contains('s') || d.contains('S') || d.contains("C-"),
            "{d}"
        );
    }

    #[test]
    fn sequence_multi_chord() {
        let mut state = KeybindingRecorderState::new("gg", "Top").with_sequences(true);
        state.reserved.clear();
        state.set_focused(true);
        let _ = state.start_recording();
        let _ = state.capture_chord(KeyChord::plain(KeyCode::Char('g')));
        let _ = state.capture_chord(KeyChord::plain(KeyCode::Char('g')));
        assert!(matches!(
            state.commit(),
            KeybindingRecorderOutcome::Committed { chords } if chords.len() == 2
        ));
    }

    #[test]
    fn paint_recording_and_idle() {
        let system = DesignSystem::new(RolePalette::default());
        let mut state = KeybindingRecorderState::new("save", "Save file")
            .with_chords([KeyChord::ctrl(KeyCode::Char('s'))])
            .with_format(ChordFormat::footer());
        state.reserved.clear();
        state.set_focused(true);
        let area = Rect::new(0, 0, 48, 8);
        let mut buf = Buffer::empty(area);
        let _ = KeybindingRecorder::new(&system).paint(area, &mut buf, &mut state);
        let _ = state.start_recording();
        let _ = KeybindingRecorder::new(&system).paint(area, &mut buf, &mut state);
    }

    #[test]
    fn fuzz_keys() {
        let mut state = KeybindingRecorderState::new("a", "A").with_sequences(true);
        state.reserved.clear();
        state.set_focused(true);
        let keys = [
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL),
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE),
        ];
        for key in keys.iter().cycle().take(40) {
            let _ = state.handle_key(*key);
        }
    }

    #[test]
    fn paint_hot_path() {
        let system = DesignSystem::default();
        let mut state =
            KeybindingRecorderState::new("a", "A").with_chords([KeyChord::alt(KeyCode::Char('x'))]);
        state.set_focused(true);
        let area = Rect::new(0, 0, 40, 6);
        let mut buf = Buffer::empty(area);
        let w = KeybindingRecorder::new(&system);
        for _ in 0..50 {
            let _ = w.paint(area, &mut buf, &mut state);
        }
    }

    #[test]
    fn semantic() {
        let system = DesignSystem::default();
        let state = KeybindingRecorderState::new("a", "A");
        let mut scene = SemanticScene::<&str, ()>::default();
        KeybindingRecorder::new(&system).register_semantic(
            &mut scene,
            "kb",
            Rect::new(0, 0, 30, 5),
            &state,
        );
        assert!(scene.get(&"kb").is_some());
    }

    #[test]
    fn binding_for_help() {
        let state = KeybindingRecorderState::new("save", "Save")
            .with_chords([KeyChord::ctrl(KeyCode::Char('s'))]);
        let b = binding_from_recorder(&state, Act::Save, Visibility::Shown);
        assert_eq!(b.chords().len(), 1);
        assert_eq!(b.hint(), Some("Save"));
    }

    #[test]
    fn protocol_notes_nonempty() {
        assert!(!protocol_limitations(Platform::Other).is_empty());
        assert!(!default_reserved_chords().is_empty());
    }

    #[test]
    fn idle_enter_starts_record() {
        let mut state = KeybindingRecorderState::new("a", "A");
        state.set_focused(true);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            KeybindingRecorderOutcome::RecordingStarted
        ));
    }

    #[test]
    fn repeated_physical_events_are_ignored() {
        let mut state = KeybindingRecorderState::new("a", "A")
            .with_chords([KeyChord::plain(KeyCode::Char('a'))]);
        state.reserved.clear();
        state.set_focused(true);

        let mut repeat_enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        repeat_enter.kind = KeyEventKind::Repeat;
        let before = state.clone();
        assert_eq!(
            state.handle_key(repeat_enter),
            KeybindingRecorderOutcome::Ignored
        );
        assert_eq!(state, before);

        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            KeybindingRecorderOutcome::RecordingStarted
        );
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)),
            KeybindingRecorderOutcome::ChordCaptured {
                chord: KeyChord::plain(KeyCode::Char('x')),
                sequence: vec![KeyChord::plain(KeyCode::Char('x'))],
            }
        );

        for (code, modifiers) in [
            (KeyCode::Char('x'), KeyModifiers::NONE),
            (KeyCode::Backspace, KeyModifiers::NONE),
            (KeyCode::Enter, KeyModifiers::NONE),
            (KeyCode::Esc, KeyModifiers::NONE),
        ] {
            let mut repeat = KeyEvent::new(code, modifiers);
            repeat.kind = KeyEventKind::Repeat;
            let before = state.clone();
            assert_eq!(state.handle_key(repeat), KeybindingRecorderOutcome::Ignored);
            assert_eq!(state, before, "{code:?} repeat mutated recorder state");
        }

        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            KeybindingRecorderOutcome::RecordingCancelled { .. }
        ));
        for (code, modifiers) in [
            (KeyCode::Delete, KeyModifiers::NONE),
            (KeyCode::Char('r'), KeyModifiers::NONE),
            (KeyCode::Esc, KeyModifiers::NONE),
        ] {
            let mut repeat = KeyEvent::new(code, modifiers);
            repeat.kind = KeyEventKind::Repeat;
            let before = state.clone();
            assert_eq!(state.handle_key(repeat), KeybindingRecorderOutcome::Ignored);
            assert_eq!(state, before, "{code:?} repeat mutated idle recorder state");
        }
    }
}
