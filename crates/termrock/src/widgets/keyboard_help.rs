// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **KeyboardHelp** — contextual, generated keyboard / interaction help.
//!
//! **Mission.** Surfaces must advertise live bindings from the active
//! [`Keymap`](crate::keymap::Keymap), focus zone, overlays, and semantic
//! actions — never stale hardcoded chords. Compact footer hints and a
//! searchable categorized modal share one entry model.
//!
//! **vs [`super::HintBar`].** HintBar paints a borrowed footer row. KeyboardHelp
//! *generates* entries from live sources and paints footer **or** modal help.
//! **vs [`super::ShortcutHint`].** ShortcutHint is one chord+label atom;
//! KeyboardHelp composes many of them into help chrome.
//!
//! Research: Zellij help, lazygit keybindings, Vim help, Textual bindings.

use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::StatefulWidget};

use crate::{
    input::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind},
    interaction::{
        CollectionItem, CollectionState, NavigationMove, OverlayOutcome, OverlaySize, OverlaySpec,
        OverlayStack, RovingOrientation, SemanticNode, SemanticRole, SemanticScene, SemanticState,
        UiIntent,
    },
    keymap::{KeyBinding, Keymap, Visibility},
    style::{DesignSystem, Role},
    text::{display_cols, take_display_cols},
    widgets::{
        ChordFormat, Panel, PanelChrome, PanelVariant, TextInputOutcome, TextInputState,
        format_binding,
    },
};

/// Default overlay id for modal keyboard help.
pub const KEYBOARD_HELP_OVERLAY_ID: &str = "termrock.keyboard_help";
/// Width under which modal becomes single-column compact.
pub const KEYBOARD_HELP_COMPACT_MAX_WIDTH: u16 = 48;
/// Width under which only a tiny summary / priority slice remains.
pub const KEYBOARD_HELP_TINY_MAX_WIDTH: u16 = 24;
/// Height under which modal drops search chrome.
pub const KEYBOARD_HELP_TINY_MAX_HEIGHT: u16 = 8;

// ── Placement ───────────────────────────────────────────────────────────────

/// Preferred modal size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyboardHelpSize {
    /// Width.
    pub width: u16,
    /// Height.
    pub height: u16,
}

impl Default for KeyboardHelpSize {
    fn default() -> Self {
        Self {
            width: 64,
            height: 18,
        }
    }
}

impl From<KeyboardHelpSize> for OverlaySize {
    fn from(value: KeyboardHelpSize) -> Self {
        Self {
            width: value.width,
            height: value.height,
            min_width: 28,
            min_height: 8,
            max_width: 0,
            max_height: 0,
        }
    }
}

/// Open modal help on the stack.
pub fn open_keyboard_help_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    preferred: KeyboardHelpSize,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.open(
        bounds,
        OverlaySpec::command_palette(
            KEYBOARD_HELP_OVERLAY_ID,
            OverlaySize::from(preferred),
            opener_focus,
        ),
    )
}

// ── Model ───────────────────────────────────────────────────────────────────

/// Help chrome mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum KeyboardHelpMode {
    /// Compact footer / status strip (default).
    #[default]
    Footer,
    /// Categorized searchable modal.
    Modal,
}

impl KeyboardHelpMode {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Footer => "footer",
            Self::Modal => "modal",
        }
    }
}

/// Layout density for modal / footer contraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum KeyboardHelpPresentation {
    /// Full categories + search (modal) or full priority list (footer).
    #[default]
    Full,
    /// Drop lower-priority rows; single category column.
    Compact,
    /// Tiny: top-N chords only or title + count.
    Tiny,
}

impl KeyboardHelpPresentation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Compact => "compact",
            Self::Tiny => "tiny",
        }
    }
}

/// Derive presentation from bounds.
#[must_use]
pub fn keyboard_help_presentation_for_bounds(bounds: Rect) -> KeyboardHelpPresentation {
    if bounds.width <= KEYBOARD_HELP_TINY_MAX_WIDTH
        || bounds.height <= KEYBOARD_HELP_TINY_MAX_HEIGHT
    {
        KeyboardHelpPresentation::Tiny
    } else if bounds.width <= KEYBOARD_HELP_COMPACT_MAX_WIDTH {
        KeyboardHelpPresentation::Compact
    } else {
        KeyboardHelpPresentation::Full
    }
}

/// Source tag for provenance (debugging / never-hardcoded proof).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum HelpEntrySource {
    /// Live keymap binding.
    Keymap,
    /// Semantic scene action / help line.
    Semantic,
    /// Overlay stack layer.
    Overlay,
    /// Conflict report from keymap.
    Conflict,
    /// Host-supplied contextual (must still carry live chord text).
    Context,
}

impl HelpEntrySource {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Keymap => "keymap",
            Self::Semantic => "semantic",
            Self::Overlay => "overlay",
            Self::Conflict => "conflict",
            Self::Context => "context",
        }
    }
}

/// One generated help row (always from live data — host rebuilds each frame).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpEntry {
    /// Stable id for selection / tests.
    pub id: String,
    /// Category header (Navigation, Edit, Overlay, …).
    pub category: String,
    /// Live chord display (from keymap formatting — not a hardcoded literal source).
    pub chord: String,
    /// Semantic action description.
    pub action: String,
    /// Optional mouse equivalent description.
    pub mouse: Option<String>,
    /// Focus zone label.
    pub zone: Option<String>,
    /// Lower first when contracting.
    pub priority: u8,
    /// Binding was remapped relative to a baseline (host flag).
    pub remapped: bool,
    /// Chord participates in a conflict.
    pub conflict: bool,
    /// Provenance.
    pub source: HelpEntrySource,
}

impl HelpEntry {
    /// Construct.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        category: impl Into<String>,
        chord: impl Into<String>,
        action: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            category: category.into(),
            chord: chord.into(),
            action: action.into(),
            mouse: None,
            zone: None,
            priority: 50,
            remapped: false,
            conflict: false,
            source: HelpEntrySource::Context,
        }
    }

    /// Mouse equivalent.
    #[must_use]
    pub fn mouse(mut self, m: impl Into<String>) -> Self {
        self.mouse = Some(m.into());
        self
    }

    /// Zone.
    #[must_use]
    pub fn zone(mut self, z: impl Into<String>) -> Self {
        self.zone = Some(z.into());
        self
    }

    /// Priority (0 = keep longest on narrow).
    #[must_use]
    pub const fn priority(mut self, p: u8) -> Self {
        self.priority = p;
        self
    }

    /// Conflict flag.
    #[must_use]
    pub const fn conflict(mut self, on: bool) -> Self {
        self.conflict = on;
        self
    }

    /// Source.
    #[must_use]
    pub const fn source(mut self, s: HelpEntrySource) -> Self {
        self.source = s;
        self
    }
}

// ── Generators (live sources only) ──────────────────────────────────────────

/// Build help entries from a live keymap.
///
/// `describe` maps each action to `(id, category, action_label, mouse?, zone?, priority)`.
/// Only non-[`Visibility::Internal`] bindings are included. Chord text always
/// comes from [`format_binding`] on the live binding (including remaps).
#[must_use]
pub fn help_entries_from_keymap<A, F>(map: &Keymap<A>, mut describe: F) -> Vec<HelpEntry>
where
    A: Clone + Copy + PartialEq + 'static,
    F: FnMut(&A, &KeyBinding<A>) -> (String, String, String, Option<String>, Option<String>, u8),
{
    let fmt = ChordFormat::new();
    let conflict_chords: Vec<_> = map.conflicts().into_iter().map(|c| c.chord).collect();
    let mut out = Vec::new();
    for binding in map.bindings() {
        if matches!(binding.visibility(), Visibility::Internal) {
            continue;
        }
        // HiddenAlias entries are included for alias discoverability.
        let (id, category, action, mouse, zone, priority) = describe(binding.action(), binding);
        let chord = if let Some(g) = binding.glyph() {
            g.to_string()
        } else {
            format_binding(binding, fmt)
        };
        if chord.is_empty() && binding.hint().is_none() {
            continue;
        }
        let action = if action.is_empty() {
            binding.hint().unwrap_or("action").to_string()
        } else {
            action
        };
        let conflict = binding
            .chords()
            .iter()
            .any(|c| conflict_chords.iter().any(|cc| cc == c));
        let priority = if matches!(binding.visibility(), Visibility::HiddenAlias) {
            priority.saturating_add(20)
        } else {
            priority
        };
        out.push(HelpEntry {
            id,
            category,
            chord,
            action,
            mouse,
            zone,
            priority,
            remapped: false, // host may set via with_remapped_ids
            conflict,
            source: HelpEntrySource::Keymap,
        });
    }
    out
}

/// Mark entries whose ids appear in `remapped_ids` (host compares baseline chords).
pub fn mark_remapped_help_entries(entries: &mut [HelpEntry], remapped_ids: &[&str]) {
    for e in entries {
        if remapped_ids.iter().any(|id| *id == e.id) {
            e.remapped = true;
        }
    }
}

/// Build conflict-only entries from a live keymap.
#[must_use]
pub fn help_entries_from_conflicts<A, F>(map: &Keymap<A>, mut label_action: F) -> Vec<HelpEntry>
where
    A: Clone + Copy + PartialEq + 'static,
    F: FnMut(&A) -> String,
{
    let fmt = ChordFormat::new();
    let mut out = Vec::new();
    for (i, c) in map.conflicts().into_iter().enumerate() {
        let chord = crate::widgets::format_chord(c.chord, fmt);
        let a = label_action(c.first);
        let b = label_action(c.second);
        out.push(
            HelpEntry::new(
                format!("conflict-{i}"),
                "Conflicts",
                chord,
                format!("{a} ↔ {b}"),
            )
            .priority(5)
            .conflict(true)
            .source(HelpEntrySource::Conflict),
        );
    }
    out
}

/// Overlay stack layer summary entries (live stack — not hardcoded chords).
#[must_use]
pub fn help_entries_from_overlays<FocusId>(stack: &OverlayStack<FocusId>) -> Vec<HelpEntry> {
    let mut out = Vec::new();
    for (i, entry) in stack.entries().iter().enumerate() {
        let kind = format!("{:?}", entry.kind);
        out.push(
            HelpEntry::new(
                format!("overlay-{i}"),
                "Overlays",
                "Esc", // Esc law is structural, documented once as stack policy
                format!("Dismiss / peel: {kind} ({})", entry.id.as_str()),
            )
            .mouse("outside click (if dismissible)")
            .priority(30)
            .source(HelpEntrySource::Overlay)
            .zone("overlay"),
        );
    }
    // Only advertise Esc when stack non-empty; still generated from live stack.
    out
}

/// Merge multiple sources; stable sort by category then priority then action.
#[must_use]
pub fn merge_help_entries(parts: impl IntoIterator<Item = Vec<HelpEntry>>) -> Vec<HelpEntry> {
    let mut out: Vec<HelpEntry> = parts.into_iter().flatten().collect();
    out.sort_by(|a, b| {
        a.category
            .cmp(&b.category)
            .then_with(|| a.priority.cmp(&b.priority))
            .then_with(|| a.action.cmp(&b.action))
            .then_with(|| a.id.cmp(&b.id))
    });
    out
}

/// One entry's case-insensitive hit (category, action, chord, zone, mouse).
#[must_use]
pub fn help_entry_matches(entry: &HelpEntry, q: &str) -> bool {
    crate::text::contains_lower(&entry.action, q)
        || crate::text::contains_lower(&entry.chord, q)
        || crate::text::contains_lower(&entry.category, q)
        || entry
            .zone
            .as_ref()
            .is_some_and(|z| crate::text::contains_lower(&z, q))
        || entry
            .mouse
            .as_ref()
            .is_some_and(|m| crate::text::contains_lower(&m, q))
}

/// Filter entries by search query (category, action, chord, zone).
#[must_use]
pub fn filter_help_entries<'a>(entries: &'a [HelpEntry], query: &str) -> Vec<&'a HelpEntry> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return entries.iter().collect();
    }
    entries
        .iter()
        .filter(|e| help_entry_matches(e, &q))
        .collect()
}

/// Contract to top priorities for tiny layouts.
#[must_use]
pub fn contract_help_entries<'a>(entries: &[&'a HelpEntry], max: usize) -> Vec<&'a HelpEntry> {
    let mut v: Vec<&'a HelpEntry> = entries.to_vec();
    v.sort_by_key(|e| e.priority);
    v.truncate(max);
    v
}

// ── Outcomes / state ────────────────────────────────────────────────────────

/// Typed outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum KeyboardHelpOutcome {
    /// No change.
    Ignored,
    /// Search query changed (modal).
    QueryChanged {
        /// Query.
        query: String,
    },
    /// Cursor moved in modal list.
    CursorMoved {
        /// Index into visible entries.
        index: usize,
    },
    /// Modal opened.
    Opened,
    /// Modal closed.
    Closed,
    /// Mode switched.
    ModeChanged {
        /// New mode.
        mode: KeyboardHelpMode,
    },
    /// Presentation changed after resize.
    PresentationChanged {
        /// New presentation.
        presentation: KeyboardHelpPresentation,
    },
}

/// Interaction state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyboardHelpState {
    mode: KeyboardHelpMode,
    open: bool,
    query: TextInputState,
    collection: CollectionState<usize>,
    focused: bool,
    accepts_input: bool,
    presentation: KeyboardHelpPresentation,
    presentation_override: Option<KeyboardHelpPresentation>,
    hits: Vec<(usize, Rect)>,
    scroll: usize,
    painted_rows: u16,
    /// Footer max hints when contracting.
    footer_max: usize,
}

impl Default for KeyboardHelpState {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyboardHelpState {
    /// Footer mode, closed modal.
    #[must_use]
    pub fn new() -> Self {
        Self {
            mode: KeyboardHelpMode::Footer,
            open: false,
            query: TextInputState::new("")
                .with_allow_empty(true)
                .with_editing(),
            collection: CollectionState::new().orientation(RovingOrientation::Vertical),
            focused: true,
            accepts_input: true,
            presentation: KeyboardHelpPresentation::Full,
            presentation_override: None,
            hits: Vec::new(),
            scroll: 0,
            painted_rows: 0,
            footer_max: 8,
        }
    }

    /// Modal mode factory.
    #[must_use]
    pub fn modal() -> Self {
        let mut s = Self::new();
        s.mode = KeyboardHelpMode::Modal;
        s.open = true;
        s
    }

    /// Open modal help.
    pub fn open_modal(&mut self) -> KeyboardHelpOutcome {
        self.mode = KeyboardHelpMode::Modal;
        self.open = true;
        self.query = TextInputState::new("")
            .with_allow_empty(true)
            .with_editing();
        KeyboardHelpOutcome::Opened
    }

    /// Close modal (footer may still paint).
    pub fn close_modal(&mut self) -> KeyboardHelpOutcome {
        self.open = false;
        self.query = TextInputState::new("")
            .with_allow_empty(true)
            .with_editing();
        KeyboardHelpOutcome::Closed
    }

    /// Mode.
    #[must_use]
    pub const fn mode(&self) -> KeyboardHelpMode {
        self.mode
    }

    /// Set mode.
    pub fn set_mode(&mut self, mode: KeyboardHelpMode) -> KeyboardHelpOutcome {
        if self.mode == mode {
            return KeyboardHelpOutcome::Ignored;
        }
        self.mode = mode;
        if matches!(mode, KeyboardHelpMode::Modal) {
            self.open = true;
        }
        KeyboardHelpOutcome::ModeChanged { mode }
    }

    /// Open?
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.open
    }

    /// Focus.
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Input gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }
    /// Presentation override.
    pub fn set_presentation_override(&mut self, p: Option<KeyboardHelpPresentation>) {
        self.presentation_override = p;
        if let Some(p) = p {
            self.presentation = p;
        }
    }

    /// Presentation.
    #[must_use]
    pub const fn presentation(&self) -> KeyboardHelpPresentation {
        self.presentation
    }

    /// Query.
    #[must_use]
    pub fn query_text(&self) -> &str {
        self.query.value()
    }

    /// Query mut.
    pub const fn query_mut(&mut self) -> &mut TextInputState {
        &mut self.query
    }

    /// Cursor.
    #[must_use]
    pub fn cursor_index(&self) -> usize {
        self.collection.active().copied().unwrap_or(0)
    }

    fn live(&self) -> bool {
        self.accepts_input && self.focused
    }

    fn entries_coll<'a>(visible: &'a [&HelpEntry]) -> Vec<CollectionItem<'a, usize>> {
        visible
            .iter()
            .enumerate()
            .map(|(i, e)| CollectionItem {
                id: i,
                enabled: true,
                label: &e.action,
                parent: None,
            })
            .collect()
    }

    fn projected_entries<'a>(&self, visible: &'a [&'a HelpEntry]) -> Vec<&'a HelpEntry> {
        let q = self.query_text().trim().to_ascii_lowercase();
        let mut projected: Vec<&'a HelpEntry> = if q.is_empty() {
            visible.to_vec()
        } else {
            visible
                .iter()
                .copied()
                .filter(|e| help_entry_matches(e, &q))
                .collect()
        };
        if matches!(self.presentation, KeyboardHelpPresentation::Tiny) {
            return contract_help_entries(&projected, 6);
        }
        projected.sort_by(|a, b| a.category.cmp(&b.category));
        projected
    }

    fn reconcile_projected(&mut self, projected: &[&HelpEntry]) {
        let entries = Self::entries_coll(projected);
        let _ = self.collection.reconcile(&entries);
        self.scroll = self.scroll.min(projected.len().saturating_sub(1));
    }

    fn rows_to_cursor(&self, projected: &[&HelpEntry], start: usize, cursor: usize) -> usize {
        let mut rows = 0usize;
        let mut last_category = projected
            .get(start.saturating_sub(1))
            .map(|entry| entry.category.as_str());
        for entry in projected
            .iter()
            .skip(start)
            .take(cursor.saturating_sub(start).saturating_add(1))
        {
            if !matches!(self.presentation, KeyboardHelpPresentation::Tiny)
                && last_category != Some(entry.category.as_str())
            {
                rows = rows.saturating_add(1);
                last_category = Some(entry.category.as_str());
            }
            rows = rows.saturating_add(1);
        }
        rows
    }

    fn ensure_cursor_visible(&mut self, projected: &[&HelpEntry]) {
        let Some(cursor) = self.collection.active().copied() else {
            self.scroll = 0;
            return;
        };
        if projected.is_empty() {
            self.scroll = 0;
            return;
        }
        self.scroll = self.scroll.min(projected.len().saturating_sub(1));
        if cursor < self.scroll {
            self.scroll = cursor;
        }
        let capacity = usize::from(self.painted_rows.max(1));
        while self.scroll < cursor && self.rows_to_cursor(projected, self.scroll, cursor) > capacity
        {
            self.scroll = self.scroll.saturating_add(1);
        }
    }

    /// Reconcile after host rebuilds visible list.
    pub fn reconcile(&mut self, visible: &[&HelpEntry]) {
        let projected = self.projected_entries(visible);
        self.reconcile_projected(&projected);
    }

    /// Keyboard (modal primarily; footer is mostly display).
    pub fn handle_key(&mut self, key: KeyEvent, visible: &[&HelpEntry]) -> KeyboardHelpOutcome {
        if !self.live() || key.is_release() {
            return KeyboardHelpOutcome::Ignored;
        }
        // Escape closes the modal exactly once. Consume repeats before they
        // reach TextInputState, whose Escape path also cancels its draft.
        if key.code == KeyCode::Esc && !key.is_press() {
            return KeyboardHelpOutcome::Ignored;
        }
        // ? opens modal from footer context when host routes here
        if matches!(self.mode, KeyboardHelpMode::Footer)
            && matches!(key.code, KeyCode::Char('?'))
            && key.modifiers.is_empty()
            && key.is_press()
        {
            return self.open_modal();
        }

        if !matches!(self.mode, KeyboardHelpMode::Modal) || !self.open {
            return KeyboardHelpOutcome::Ignored;
        }

        self.reconcile(visible);

        if key.code == KeyCode::Esc && key.is_press() {
            return self.close_modal();
        }

        if matches!(
            key.code,
            KeyCode::Down
                | KeyCode::Up
                | KeyCode::PageDown
                | KeyCode::PageUp
                | KeyCode::Home
                | KeyCode::End
        ) {
            if let Some(intent) = default_keyboard_help_intent(key) {
                return self.handle_intent(intent, visible);
            }
        }

        match self.query.handle_key(key) {
            TextInputOutcome::Changed => {
                self.reconcile(visible);
                KeyboardHelpOutcome::QueryChanged {
                    query: self.query_text().to_string(),
                }
            }
            TextInputOutcome::Cancelled => self.close_modal(),
            TextInputOutcome::Ignored => {
                if let Some(intent) = default_keyboard_help_intent(key) {
                    self.handle_intent(intent, visible)
                } else {
                    KeyboardHelpOutcome::Ignored
                }
            }
            _ => KeyboardHelpOutcome::Ignored,
        }
    }

    /// Intent.
    pub fn handle_intent(
        &mut self,
        intent: UiIntent,
        visible: &[&HelpEntry],
    ) -> KeyboardHelpOutcome {
        if !self.live() || !self.open || !matches!(self.mode, KeyboardHelpMode::Modal) {
            return KeyboardHelpOutcome::Ignored;
        }
        let projected = self.projected_entries(visible);
        self.reconcile_projected(&projected);
        match intent {
            UiIntent::Move(
                NavigationMove::Next
                | NavigationMove::Previous
                | NavigationMove::First
                | NavigationMove::Last
                | NavigationMove::Up
                | NavigationMove::Down,
            ) => {
                if projected.is_empty() {
                    return KeyboardHelpOutcome::Ignored;
                }
                let entries = Self::entries_coll(&projected);
                let out = self.collection.handle_intent(intent, &entries);
                if out.active_changed() {
                    self.ensure_cursor_visible(&projected);
                    let cur = self.cursor_index();
                    KeyboardHelpOutcome::CursorMoved { index: cur }
                } else {
                    KeyboardHelpOutcome::Ignored
                }
            }
            UiIntent::Cancel | UiIntent::Close => self.close_modal(),
            _ => KeyboardHelpOutcome::Ignored,
        }
    }

    /// Mouse.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        visible: &[&HelpEntry],
    ) -> KeyboardHelpOutcome {
        if !self.live() || !self.open {
            return KeyboardHelpOutcome::Ignored;
        }
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                for (idx, rect) in &self.hits {
                    if rect.contains(event.position) {
                        self.collection.set_active(Some(*idx));
                        return KeyboardHelpOutcome::CursorMoved { index: *idx };
                    }
                }
                KeyboardHelpOutcome::Ignored
            }
            MouseEventKind::ScrollDown => {
                self.handle_intent(UiIntent::Move(NavigationMove::Next), visible)
            }
            MouseEventKind::ScrollUp => {
                self.handle_intent(UiIntent::Move(NavigationMove::Previous), visible)
            }
            _ => KeyboardHelpOutcome::Ignored,
        }
    }

    /// Sync presentation.
    pub fn sync_presentation(&mut self, area: Rect) -> KeyboardHelpOutcome {
        if self.presentation_override.is_some() {
            return KeyboardHelpOutcome::Ignored;
        }
        let next = keyboard_help_presentation_for_bounds(area);
        if next != self.presentation {
            self.presentation = next;
            KeyboardHelpOutcome::PresentationChanged { presentation: next }
        } else {
            KeyboardHelpOutcome::Ignored
        }
    }
}

/// Default intents for modal list.
#[must_use]
pub fn default_keyboard_help_intent(key: KeyEvent) -> Option<UiIntent> {
    if key.is_release() {
        return None;
    }
    let is_press = key.is_press();
    match key.code {
        KeyCode::Down | KeyCode::Char('j' | 'J') => Some(UiIntent::Move(NavigationMove::Next)),
        KeyCode::Up | KeyCode::Char('k' | 'K') => Some(UiIntent::Move(NavigationMove::Previous)),
        KeyCode::Home => Some(UiIntent::Move(NavigationMove::First)),
        KeyCode::End => Some(UiIntent::Move(NavigationMove::Last)),
        KeyCode::Esc if is_press => Some(UiIntent::Cancel),
        _ => None,
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Keyboard help paint (footer strip or modal).
#[derive(Debug, Clone, Copy)]
pub struct KeyboardHelp<'a> {
    entries: &'a [&'a HelpEntry],
    system: &'a DesignSystem,
    title: &'a str,
    colorless: bool,
}

impl<'a> KeyboardHelp<'a> {
    /// Live entries (host-filtered) + design system.
    #[must_use]
    pub const fn new(entries: &'a [&'a HelpEntry], system: &'a DesignSystem) -> Self {
        Self {
            entries,
            system,
            title: "Keyboard",
            colorless: false,
        }
    }

    /// Title for modal.
    #[must_use]
    pub const fn title(mut self, t: &'a str) -> Self {
        self.title = t;
        self
    }

    /// ASCII.
    #[must_use]
    /// Colorless.
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    /// Paint according to state mode.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut KeyboardHelpState) {
        state.hits.clear();
        if area.is_empty() {
            return;
        }
        let _ = state.sync_presentation(area);
        match state.mode {
            KeyboardHelpMode::Footer => self.paint_footer(area, buffer, state),
            KeyboardHelpMode::Modal => {
                if state.open {
                    self.paint_modal(area, buffer, state);
                }
            }
        }
    }

    fn paint_footer(&self, area: Rect, buffer: &mut Buffer, state: &KeyboardHelpState) {
        let max = match state.presentation {
            KeyboardHelpPresentation::Tiny => 3.min(state.footer_max),
            KeyboardHelpPresentation::Compact => 5.min(state.footer_max),
            KeyboardHelpPresentation::Full => state.footer_max,
        };
        let mut sorted: Vec<&HelpEntry> = self.entries.iter().copied().collect();
        sorted.sort_by_key(|e| e.priority);
        let slice: Vec<&HelpEntry> = sorted.into_iter().take(max).collect();

        // Build owned Hint-compatible paint without HintBar lifetime issues:
        // paint key/label pairs with priority drop already applied.
        let mut x = area.x;
        let y = area.y;
        // The widget's own ASCII switch outranks the system profile here.
        let glyphs = { self.system.glyphs };
        let sep = glyphs.meta_join();
        for (i, e) in slice.iter().enumerate() {
            if x >= area.right() {
                break;
            }
            if i > 0 {
                let sw = display_cols(sep) as u16;
                if x + sw >= area.right() {
                    break;
                }
                buffer.set_stringn(
                    x,
                    y,
                    sep,
                    usize::from(sw),
                    self.system.style(Role::HintSeparator),
                );
                x = x.saturating_add(sw);
            }
            let key_style = if self.colorless {
                self.system
                    .style(Role::TextStrong)
                    .add_modifier(Modifier::BOLD)
            } else {
                self.system.style(Role::HintKey)
            };
            let text_style = if self.colorless {
                self.system.style(Role::TextMuted)
            } else {
                self.system.style(Role::HintText)
            };
            let mut chord = e.chord.as_str();
            if e.conflict {
                // non-color conflict cue
                chord = e.chord.as_str();
            }
            let kw = display_cols(chord) as u16;
            if x + kw >= area.right() {
                break;
            }
            buffer.set_stringn(x, y, chord, usize::from(kw), key_style);
            x = x.saturating_add(kw);
            let label = format!(
                " {}{}",
                take_display_cols(&e.action, 16),
                if e.remapped {
                    "∗"
                } else if e.conflict {
                    "⚠"
                } else {
                    ""
                }
            );
            let lw = display_cols(&label) as u16;
            let avail = area.right().saturating_sub(x);
            if avail == 0 {
                break;
            }
            let lw = lw.min(avail);
            buffer.set_stringn(
                x,
                y,
                take_display_cols(&label, usize::from(lw)).as_ref(),
                usize::from(lw),
                text_style,
            );
            x = x.saturating_add(lw);
        }
    }

    fn paint_modal(&self, area: Rect, buffer: &mut Buffer, state: &mut KeyboardHelpState) {
        let surface = state.focused && state.accepts_input;
        let panel = Panel::new(self.system)
            .variant(PanelVariant::Bordered)
            .overlay(true)
            .title(self.title)
            .emphasis(if surface {
                PanelChrome::Focused
            } else {
                PanelChrome::Normal
            });
        let inner = panel.inner(area);
        panel.paint(area, buffer, None);
        if inner.is_empty() {
            return;
        }

        let mut y = inner.y;
        let show_search =
            !matches!(state.presentation, KeyboardHelpPresentation::Tiny) && inner.height >= 4;

        if show_search {
            state.query.set_focused(surface);
            let _ = crate::widgets::TextInput::new("", self.system)
                .placeholder("Filter bindings…")
                .paint(
                    Rect::new(inner.x, y, inner.width, 1),
                    buffer,
                    &mut state.query,
                );
            y = y.saturating_add(1);
            let line = { "─".repeat(usize::from(inner.width)) };
            buffer.set_stringn(
                inner.x,
                y,
                &line,
                usize::from(inner.width),
                self.system.style(Role::Border),
            );
            y = y.saturating_add(1);
        }

        let flat = state.projected_entries(self.entries);

        let list_h = inner.bottom().saturating_sub(y);
        if list_h == 0 {
            return;
        }
        state.painted_rows = list_h;
        state.reconcile_projected(&flat);
        state.ensure_cursor_visible(&flat);

        let cursor = state.cursor_index();
        let mut last_cat = flat
            .get(state.scroll.saturating_sub(1))
            .map_or("", |entry| entry.category.as_str());
        for (flat_i, e) in flat.iter().enumerate().skip(state.scroll) {
            if y >= inner.bottom() {
                break;
            }
            if !matches!(state.presentation, KeyboardHelpPresentation::Tiny)
                && e.category != last_cat
            {
                last_cat = e.category.as_str();
                buffer.set_stringn(
                    inner.x,
                    y,
                    take_display_cols(&e.category, usize::from(inner.width)).as_ref(),
                    usize::from(inner.width),
                    self.system
                        .style(Role::TextStrong)
                        .add_modifier(Modifier::BOLD),
                );
                y = y.saturating_add(1);
                if y >= inner.bottom() {
                    break;
                }
            }

            let active = flat_i == cursor && surface;
            let rect = Rect::new(inner.x, y, inner.width, 1);
            state.hits.push((flat_i, rect));

            let flags = format!(
                "{}{}",
                if e.remapped { "∗" } else { "" },
                if e.conflict { "!" } else { "" }
            );
            let mouse = e
                .mouse
                .as_ref()
                .map(|m| format!("  {}", take_display_cols(m, 18)))
                .unwrap_or_default();
            let style = if self.colorless {
                if active {
                    self.system
                        .style(Role::TextStrong)
                        .add_modifier(Modifier::BOLD)
                } else if e.conflict {
                    // A conflicting binding is a warning, and says so.
                    self.system
                        .style(Role::Warning)
                        .add_modifier(Modifier::BOLD)
                } else {
                    self.system.style(Role::Text)
                }
            } else if active {
                self.system
                    .style(Role::TextStrong)
                    .patch(self.system.style(Role::SelectionTint))
            } else if e.conflict {
                self.system.style(Role::Danger)
            } else if e.remapped {
                self.system.style(Role::Focus)
            } else {
                self.system.style(Role::Text)
            };
            // Chord emphasis
            let chord_w = display_cols(&e.chord).min(10) as u16;
            buffer.set_stringn(
                inner.x,
                y,
                take_display_cols(&e.chord, 10).as_ref(),
                usize::from(chord_w.min(inner.width)),
                if self.colorless {
                    self.system.style(Role::TextStrong)
                } else {
                    self.system.style(Role::HintKey)
                },
            );
            let rest_x = inner
                .x
                .saturating_add(chord_w.saturating_add(1).min(inner.width));
            let rest_w = inner.right().saturating_sub(rest_x);
            if rest_w > 0 {
                let rest = format!(
                    "{}{}{}",
                    take_display_cols(&e.action, usize::from(rest_w.saturating_sub(4))),
                    flags,
                    if matches!(state.presentation, KeyboardHelpPresentation::Full) {
                        mouse
                    } else {
                        String::new()
                    }
                );
                buffer.set_stringn(
                    rest_x,
                    y,
                    take_display_cols(&rest, usize::from(rest_w)).as_ref(),
                    usize::from(rest_w),
                    style,
                );
            }
            y = y.saturating_add(1);
        }
    }

    /// Semantic registration.
    pub fn register_semantic<Id, Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        id: Id,
        area: Rect,
        state: &KeyboardHelpState,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if area.is_empty() {
            return;
        }
        let desc = format!(
            "keyboard-help mode={} open={} presentation={} entries={}",
            state.mode().id(),
            state.is_open(),
            state.presentation().id(),
            self.entries.len()
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Chrome)
                .label("keyboard-help")
                .description(desc)
                .focusable(true)
                .state(SemanticState {
                    selected: state.focused,
                    expanded: state.open,
                    ..Default::default()
                }),
        );
    }
}

impl StatefulWidget for &KeyboardHelp<'_> {
    type State = KeyboardHelpState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl StatefulWidget for KeyboardHelp<'_> {
    type State = KeyboardHelpState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

// ── Sample / demo generation ────────────────────────────────────────────────

/// Demo action set for stories / tests (bindings live in a Keymap).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemoHelpAction {
    /// Quit.
    Quit,
    /// Save.
    Save,
    /// Help.
    Help,
    /// Next.
    Next,
    /// Prev.
    Prev,
}

/// Live demo keymap — **not** painted as hardcoded strings; stories call
/// [`help_entries_from_keymap`] every frame.
#[must_use]
pub fn example_help_keymap() -> Keymap<DemoHelpAction> {
    use crate::input::{KeyCode, KeyModifiers};
    use crate::keymap::{KeyBinding, KeyChord, Visibility};
    Keymap::from_owned(vec![
        KeyBinding::owned(
            vec![KeyChord {
                key: KeyCode::Char('q'),
                mods: KeyModifiers::CONTROL,
            }],
            DemoHelpAction::Quit,
            Some("Quit application".into()),
            Visibility::Shown,
            None,
        ),
        KeyBinding::owned(
            vec![KeyChord {
                key: KeyCode::Char('s'),
                mods: KeyModifiers::CONTROL,
            }],
            DemoHelpAction::Save,
            Some("Save document".into()),
            Visibility::Shown,
            None,
        ),
        KeyBinding::owned(
            vec![KeyChord {
                key: KeyCode::Char('?'),
                mods: KeyModifiers::NONE,
            }],
            DemoHelpAction::Help,
            Some("Open keyboard help".into()),
            Visibility::Shown,
            None,
        ),
        KeyBinding::owned(
            vec![KeyChord {
                key: KeyCode::Down,
                mods: KeyModifiers::NONE,
            }],
            DemoHelpAction::Next,
            Some("Next item".into()),
            Visibility::Shown,
            None,
        ),
        KeyBinding::owned(
            vec![KeyChord {
                key: KeyCode::Up,
                mods: KeyModifiers::NONE,
            }],
            DemoHelpAction::Prev,
            Some("Previous item".into()),
            Visibility::Shown,
            None,
        ),
        // Alias creating intentional conflict with Next for demo of conflict section
        KeyBinding::owned(
            vec![KeyChord {
                key: KeyCode::Char('j'),
                mods: KeyModifiers::NONE,
            }],
            DemoHelpAction::Next,
            Some("Next item (alias)".into()),
            Visibility::HiddenAlias,
            None,
        ),
    ])
}

/// Generate demo help entries from the example keymap (live format).
#[must_use]
pub fn example_help_entries() -> Vec<HelpEntry> {
    let map = example_help_keymap();
    let mut entries = help_entries_from_keymap(&map, |a, _b| {
        let (cat, pri) = match a {
            DemoHelpAction::Quit | DemoHelpAction::Save => ("App", 10),
            DemoHelpAction::Help => ("Help", 5),
            DemoHelpAction::Next | DemoHelpAction::Prev => ("Navigation", 20),
        };
        let id = match a {
            DemoHelpAction::Quit => "quit",
            DemoHelpAction::Save => "save",
            DemoHelpAction::Help => "help",
            DemoHelpAction::Next => "next",
            DemoHelpAction::Prev => "prev",
        };
        let mouse = match a {
            DemoHelpAction::Next => Some("click next / scroll down".into()),
            DemoHelpAction::Prev => Some("click prev / scroll up".into()),
            DemoHelpAction::Save => Some("toolbar Save".into()),
            _ => None,
        };
        (
            id.into(),
            cat.into(),
            String::new(), // use binding hint
            mouse,
            Some("main".into()),
            pri,
        )
    });
    let conflicts = help_entries_from_conflicts(&map, |a| format!("{a:?}"));
    // No conflict if same action aliases — conflicts() only different actions.
    // Add a host context row still sourced live:
    entries = merge_help_entries([entries, conflicts]);
    mark_remapped_help_entries(&mut entries, &["save"]); // demo remapped mark
    entries
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyEventKind;
    use crate::input::KeyModifiers;
    use crate::keymap::{KeyBinding, KeyChord, Visibility};
    use crate::widgets::tests::click;

    fn refs(v: &[HelpEntry]) -> Vec<&HelpEntry> {
        v.iter().collect()
    }

    fn system() -> DesignSystem {
        DesignSystem::default()
    }

    #[test]
    fn entries_from_live_keymap_not_empty_chords() {
        let map = example_help_keymap();
        let entries = help_entries_from_keymap(&map, |a, _| {
            (
                format!("{a:?}"),
                "Gen".into(),
                String::new(),
                None,
                None,
                10,
            )
        });
        assert!(!entries.is_empty());
        assert!(
            entries
                .iter()
                .all(|e| !e.chord.is_empty() || e.action.is_empty() == false)
        );
        assert!(entries.iter().all(|e| e.source == HelpEntrySource::Keymap));
    }

    #[test]
    fn remap_changes_chord_text() {
        let mut map = example_help_keymap();
        let before = help_entries_from_keymap(&map, |a, _| {
            (format!("{a:?}"), "App".into(), String::new(), None, None, 1)
        });
        let save_before = before
            .iter()
            .find(|e| e.action.contains("Save"))
            .map(|e| e.chord.clone())
            .unwrap();
        assert!(map.remap(
            DemoHelpAction::Save,
            vec![KeyChord {
                key: KeyCode::Char('w'),
                mods: KeyModifiers::CONTROL,
            }]
        ));
        let after = help_entries_from_keymap(&map, |a, _| {
            (format!("{a:?}"), "App".into(), String::new(), None, None, 1)
        });
        let save_after = after
            .iter()
            .find(|e| e.action.contains("Save"))
            .map(|e| e.chord.clone())
            .unwrap();
        assert_ne!(save_before, save_after);
        assert!(save_after.to_ascii_lowercase().contains('w') || save_after.contains("W"));
    }

    #[test]
    fn conflicts_reported() {
        let map = Keymap::from_owned(vec![
            KeyBinding::owned(
                vec![KeyChord {
                    key: KeyCode::Char('x'),
                    mods: KeyModifiers::CONTROL,
                }],
                DemoHelpAction::Save,
                Some("Save".into()),
                Visibility::Shown,
                None,
            ),
            KeyBinding::owned(
                vec![KeyChord {
                    key: KeyCode::Char('x'),
                    mods: KeyModifiers::CONTROL,
                }],
                DemoHelpAction::Quit,
                Some("Quit".into()),
                Visibility::Shown,
                None,
            ),
        ]);
        let c = help_entries_from_conflicts(&map, |a| format!("{a:?}"));
        assert_eq!(c.len(), 1);
        assert!(c[0].conflict);
        assert_eq!(c[0].source, HelpEntrySource::Conflict);
    }

    #[test]
    fn filter_and_contract() {
        let e = example_help_entries();
        let f = filter_help_entries(&e, "save");
        assert!(
            f.iter()
                .all(|x| x.action.to_ascii_lowercase().contains("save")
                    || x.chord.to_ascii_lowercase().contains('s'))
        );
        let c = contract_help_entries(&refs(&e), 2);
        assert!(c.len() <= 2);
    }

    #[test]
    fn modal_query_projection_drives_reconcile_and_paint() {
        let sys = system();
        let entries = vec![
            HelpEntry::new("save", "Edit", "ctrl+s", "Save file"),
            HelpEntry::new("quit", "Navigation", "q", "Quit application"),
        ];
        let mut state = KeyboardHelpState::modal();
        for character in "save".chars() {
            assert!(matches!(
                state.handle_key(
                    KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE),
                    &refs(&entries),
                ),
                KeyboardHelpOutcome::QueryChanged { .. }
            ));
        }

        let area = Rect::new(0, 0, 64, 16);
        let mut buffer = Buffer::empty(area);
        KeyboardHelp::new(&refs(&entries), &sys).paint(area, &mut buffer, &mut state);

        assert_eq!(state.hits.len(), 1);
        assert_eq!(state.hits[0].0, 0);
        let text: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(text.contains("Save file"), "{text}");
        assert!(!text.contains("Quit application"), "{text}");
    }

    #[test]
    fn tiny_projection_keeps_mouse_hits_in_projected_order() {
        let sys = system();
        let entries = vec![
            HelpEntry::new("slow", "Other", "s", "Slow").priority(80),
            HelpEntry::new("best", "Priority", "b", "Best").priority(1),
        ];
        let mut state = KeyboardHelpState::modal();
        state.set_presentation_override(Some(KeyboardHelpPresentation::Tiny));

        let area = Rect::new(0, 0, 20, 10);
        let mut buffer = Buffer::empty(area);
        KeyboardHelp::new(&refs(&entries), &sys).paint(area, &mut buffer, &mut state);

        let (index, rect) = state.hits[0];
        assert_eq!(index, 0);
        assert_eq!(
            state.handle_mouse(click(rect.x, rect.y), &refs(&entries),),
            KeyboardHelpOutcome::CursorMoved { index: 0 }
        );
        assert_eq!(state.cursor_index(), 0);
    }

    #[test]
    fn modal_scroll_accounts_for_category_headers() {
        let sys = system();
        let entries = vec![
            HelpEntry::new("a1", "A", "a", "A one"),
            HelpEntry::new("a2", "A", "b", "A two"),
            HelpEntry::new("b1", "B", "c", "B one"),
            HelpEntry::new("b2", "B", "d", "B two"),
        ];
        let mut state = KeyboardHelpState::modal();
        state.set_presentation_override(Some(KeyboardHelpPresentation::Full));
        state.reconcile(&refs(&entries));
        state.collection.set_active(Some(2));

        let area = Rect::new(0, 0, 40, 4);
        let mut buffer = Buffer::empty(area);
        KeyboardHelp::new(&refs(&entries), &sys).paint(area, &mut buffer, &mut state);

        assert_eq!(state.painted_rows, 2);
        assert_eq!(state.scroll, 2);
        assert!(state.hits.iter().any(|(index, _)| *index == 2));
    }

    #[test]
    fn footer_and_modal_paint() {
        let sys = system();
        let e = example_help_entries();
        let mut st = KeyboardHelpState::new();
        let area = Rect::new(0, 0, 60, 1);
        let mut buf = Buffer::empty(area);
        KeyboardHelp::new(&refs(&e), &sys).paint(area, &mut buf, &mut st);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(!text.trim().is_empty(), "{text:?}");

        let mut st2 = KeyboardHelpState::modal();
        let area2 = Rect::new(0, 0, 64, 16);
        let mut buf2 = Buffer::empty(area2);
        KeyboardHelp::new(&refs(&e), &sys).paint(area2, &mut buf2, &mut st2);
        let t2: String = buf2
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            t2.contains("Keyboard") || t2.contains("App") || t2.contains("Save"),
            "{t2}"
        );
    }

    #[test]
    fn modal_esc_closes() {
        let mut st = KeyboardHelpState::modal();
        let e = example_help_entries();
        assert!(matches!(
            st.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &refs(&e)),
            KeyboardHelpOutcome::Closed
        ));
        assert!(!st.is_open());
    }

    #[test]
    fn question_opens_modal_from_footer() {
        let mut st = KeyboardHelpState::new();
        assert!(matches!(
            st.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE), &[]),
            KeyboardHelpOutcome::Opened
        ));
        assert!(st.is_open());
        assert_eq!(st.mode(), KeyboardHelpMode::Modal);
    }

    #[test]
    fn repeated_modal_triggers_do_not_open_or_close() {
        let entries = example_help_entries();

        let mut footer = KeyboardHelpState::new();
        let mut repeat_question = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
        repeat_question.kind = KeyEventKind::Repeat;
        assert_eq!(
            footer.handle_key(repeat_question, &refs(&entries)),
            KeyboardHelpOutcome::Ignored
        );
        assert!(!footer.is_open());
        assert_eq!(footer.mode(), KeyboardHelpMode::Footer);

        let mut modal = KeyboardHelpState::modal();
        let mut repeat_escape = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        repeat_escape.kind = KeyEventKind::Repeat;
        assert_eq!(
            modal.handle_key(repeat_escape, &refs(&entries)),
            KeyboardHelpOutcome::Ignored
        );
        assert!(modal.is_open());
    }

    #[test]
    fn overlay_entries_from_live_stack() {
        let mut stack = OverlayStack::<()>::new();
        let bounds = Rect::new(0, 0, 80, 24);
        let _ = open_keyboard_help_overlay(&mut stack, bounds, KeyboardHelpSize::default(), None);
        let e = help_entries_from_overlays(&stack);
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].source, HelpEntrySource::Overlay);
        assert!(e[0].action.contains("CommandPalette") || e[0].action.contains("overlay"));
    }

    #[test]
    fn overlay_dismiss_restores_focus() {
        let mut stack = OverlayStack::<&'static str>::new();
        let bounds = Rect::new(0, 0, 80, 24);
        let _ = open_keyboard_help_overlay(
            &mut stack,
            bounds,
            KeyboardHelpSize::default(),
            Some("editor"),
        );
        assert!(matches!(
            stack.handle_escape(),
            OverlayOutcome::Dismissed {
                focus: Some("editor"),
                ..
            }
        ));
    }

    #[test]
    fn presentation_bounds() {
        assert_eq!(
            keyboard_help_presentation_for_bounds(Rect::new(0, 0, 20, 20)),
            KeyboardHelpPresentation::Tiny
        );
        assert_eq!(
            keyboard_help_presentation_for_bounds(Rect::new(0, 0, 40, 20)),
            KeyboardHelpPresentation::Compact
        );
    }

    #[test]
    fn colorless_footer() {
        let sys = system();
        let e = example_help_entries();
        let mut st = KeyboardHelpState::new();
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = Buffer::empty(area);
        KeyboardHelp::new(&refs(&e), &sys)
            .colorless(true)
            .paint(area, &mut buf, &mut st);
    }

    #[test]
    fn mouse_hit_moves_the_modal_help_cursor() {
        let entries = example_help_entries();
        let mut state = KeyboardHelpState::modal();
        state.hits = vec![(1, Rect::new(2, 3, 18, 1))];
        assert_eq!(
            state.handle_mouse(click(2, 3), &refs(&entries),),
            KeyboardHelpOutcome::CursorMoved { index: 1 }
        );
    }

    #[test]
    fn fuzz_modal_keys() {
        let e = example_help_entries();
        let mut st = KeyboardHelpState::modal();
        let keys = [
            KeyCode::Down,
            KeyCode::Up,
            KeyCode::Char('a'),
            KeyCode::Esc,
            KeyCode::Char('?'),
        ];
        let mut seed = 5u64;
        for _ in 0..200 {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let k = keys[(seed as usize) % keys.len()];
            if !st.is_open() {
                let _ = st.open_modal();
            }
            let _ = st.handle_key(KeyEvent::new(k, KeyModifiers::NONE), &refs(&e));
        }
    }

    #[test]
    fn semantic_registers() {
        let sys = system();
        let st = KeyboardHelpState::new();
        let mut scene = SemanticScene::<&str, ()>::default();
        KeyboardHelp::new(&[], &sys).register_semantic(
            &mut scene,
            "kh",
            Rect::new(0, 0, 40, 2),
            &st,
        );
        assert!(
            scene
                .nodes()
                .iter()
                .any(|n| n.label.as_deref() == Some("keyboard-help"))
        );
    }

    #[test]
    fn generators_use_live_bindings_only() {
        // Chords on keymap-sourced entries must equal format_binding output.
        let map = example_help_keymap();
        let fmt = ChordFormat::new();
        let entries = help_entries_from_keymap(&map, |a, _| {
            (format!("{a:?}"), "T".into(), String::new(), None, None, 1)
        });
        for e in &entries {
            if e.source != HelpEntrySource::Keymap {
                continue;
            }
            // Every keymap entry's chord must match some binding's formatted form.
            let ok = map.bindings().iter().any(|b| {
                let g = b
                    .glyph()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format_binding(b, fmt));
                g == e.chord
            });
            assert!(ok, "stale chord {:?} for {}", e.chord, e.action);
        }
    }
}
