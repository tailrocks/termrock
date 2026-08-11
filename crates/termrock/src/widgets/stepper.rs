// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **Stepper** — progress / navigation chrome for multi-step flows.
//!
//! **Mission.** Installers, onboarding, FormWizard, plans, and migrations need
//! a shared step indicator: current / complete / error / optional / disabled /
//! future states, horizontal or vertical layout, labels + descriptions, and
//! policy-gated jump navigation — with non-color marks and narrow contraction
//! to numeric status or a compact menu.
//!
//! **vs [`super::FormWizard`].** FormWizard owns flow gates, validation, review,
//! and nav buttons. Stepper is the reusable step list chrome FormWizard (and
//! hosts) embed. Hosts own domain data and whether a jump is allowed via
//! [`StepperNavPolicy`].
//!
//! Research: shadcn-style steppers, installers, CI pipeline views.

#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Modifier,
    widgets::StatefulWidget,
};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    interaction::{
        CollectionItem, CollectionState, NavigationMove, RovingOrientation, SemanticNode,
        SemanticRole, SemanticScene, SemanticState, UiIntent,
    },
    style::{DesignSystem, Role},
    text::{display_cols, take_display_cols},
};

/// Width under which Expanded becomes Compact (title + marks).
pub const STEPPER_COMPACT_MAX_WIDTH: u16 = 48;
/// Width under which Compact becomes Numeric (`3/7`) or Menu.
pub const STEPPER_NARROW_MAX_WIDTH: u16 = 28;
/// Height under which vertical expanded drops descriptions.
pub const STEPPER_COMPACT_MAX_HEIGHT: u16 = 8;

// ── Model ───────────────────────────────────────────────────────────────────

/// Visual / progress status of one step (non-color marks included).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum StepStatus {
    /// Not yet reached (future).
    #[default]
    Future,
    /// Active step.
    Current,
    /// Completed successfully.
    Complete,
    /// Failed / blocked.
    Error,
    /// Explicitly disabled (cannot visit).
    Disabled,
    /// Optional step available (not yet decided).
    Optional,
    /// Optional step skipped.
    Skipped,
}

impl StepStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Future => "future",
            Self::Current => "current",
            Self::Complete => "complete",
            Self::Error => "error",
            Self::Disabled => "disabled",
            Self::Optional => "optional",
            Self::Skipped => "skipped",
        }
    }

    /// Non-color mark (always paired with style roles).
    #[must_use]
    pub const fn mark(self, ascii: bool) -> &'static str {
        match (self, ascii) {
            (Self::Complete, true) => "[x]",
            (Self::Complete, false) => "[✓]",
            (Self::Current, true) => "[>]",
            (Self::Current, false) => "[›]",
            (Self::Error, true) => "[!]",
            (Self::Error, false) => "[!]",
            (Self::Disabled, true) => "[#]",
            (Self::Disabled, false) => "[⊘]",
            (Self::Optional, true) => "[?]",
            (Self::Optional, false) => "[◦]",
            (Self::Skipped, true) => "[-]",
            (Self::Skipped, false) => "[–]",
            (Self::Future, true) => "[ ]",
            (Self::Future, false) => "[ ]",
        }
    }

    /// Whether the step may receive activation under default linear policy.
    #[must_use]
    pub const fn is_terminal_ok(self) -> bool {
        matches!(self, Self::Complete | Self::Skipped)
    }
}

/// Alias used by FormWizard historically (`Upcoming` = [`StepStatus::Future`]).
#[allow(dead_code)]
pub type WizardStepStatus = StepStatus;

/// One step definition (host-projected; values stay outside).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepItem {
    /// Stable id.
    pub id: String,
    /// Title.
    pub title: String,
    /// Optional description (vertical / expanded).
    pub description: Option<String>,
    /// Step may be skipped by policy/host.
    pub optional: bool,
    /// Permanently unavailable.
    pub disabled: bool,
}

impl StepItem {
    /// Required enabled step.
    #[must_use]
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: None,
            optional: false,
            disabled: false,
        }
    }

    /// Description.
    #[must_use]
    pub fn description(mut self, d: impl Into<String>) -> Self {
        self.description = Some(d.into());
        self
    }

    /// Optional flag.
    #[must_use]
    pub const fn optional(mut self, on: bool) -> Self {
        self.optional = on;
        self
    }

    /// Disabled flag.
    #[must_use]
    pub const fn disabled(mut self, on: bool) -> Self {
        self.disabled = on;
        self
    }
}

/// FormWizard-compatible name.
#[allow(dead_code)]
pub type WizardStep = StepItem;

/// Layout orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum StepperOrientation {
    /// Left → right (default).
    #[default]
    Horizontal,
    /// Top → bottom with optional descriptions.
    Vertical,
}

impl StepperOrientation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
        }
    }
}

/// Responsive presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum StepperPresentation {
    /// Full labels (+ descriptions when vertical).
    #[default]
    Expanded,
    /// Marks + short titles.
    Compact,
    /// `current/total` numeric only.
    Numeric,
    /// Single control opens step menu (dropdown-style list).
    Menu,
}

impl StepperPresentation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Expanded => "expanded",
            Self::Compact => "compact",
            Self::Numeric => "numeric",
            Self::Menu => "menu",
        }
    }
}

/// When users may activate a step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum StepperNavPolicy {
    /// Display only — keys/mouse never activate.
    #[default]
    DisplayOnly,
    /// Only current step and completed/skipped steps (linear wizard default).
    Linear,
    /// Any non-disabled step.
    Free,
    /// Always emit activation; host enforces gates (FormWizard).
    Host,
}

impl StepperNavPolicy {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::DisplayOnly => "display-only",
            Self::Linear => "linear",
            Self::Free => "free",
            Self::Host => "host",
        }
    }
}

/// Derive presentation from bounds + orientation.
#[must_use]
pub fn stepper_presentation_for_bounds(
    bounds: Rect,
    orientation: StepperOrientation,
) -> StepperPresentation {
    match orientation {
        StepperOrientation::Horizontal => {
            if bounds.width <= STEPPER_NARROW_MAX_WIDTH {
                StepperPresentation::Menu
            } else if bounds.width <= STEPPER_COMPACT_MAX_WIDTH {
                StepperPresentation::Compact
            } else {
                StepperPresentation::Expanded
            }
        }
        StepperOrientation::Vertical => {
            if bounds.width <= STEPPER_NARROW_MAX_WIDTH
                || bounds.height <= STEPPER_COMPACT_MAX_HEIGHT
            {
                StepperPresentation::Numeric
            } else if bounds.width <= STEPPER_COMPACT_MAX_WIDTH {
                StepperPresentation::Compact
            } else {
                StepperPresentation::Expanded
            }
        }
    }
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Typed outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum StepperOutcome {
    /// No change.
    Ignored,
    /// Roving cursor among steps moved (not domain current step).
    CursorMoved {
        /// Index.
        index: usize,
    },
    /// User requested navigation to a step (host/policy must apply).
    StepActivated {
        /// Index.
        index: usize,
        /// Step id.
        id: String,
    },
    /// Menu presentation toggled open/closed.
    MenuToggled {
        /// Open?
        open: bool,
    },
    /// Presentation changed after resize/sync.
    PresentationChanged {
        /// New presentation.
        presentation: StepperPresentation,
    },
}

// ── State ───────────────────────────────────────────────────────────────────

/// Stepper interaction state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepperState {
    /// Roving focus among interactive steps (independent of domain "current").
    collection: CollectionState<usize>,
    /// Domain current step index (host-synced).
    current: usize,
    /// Per-step statuses (parallel to items).
    statuses: Vec<StepStatus>,
    focused: bool,
    enabled: bool,
    accepts_input: bool,
    orientation: StepperOrientation,
    presentation: StepperPresentation,
    presentation_override: Option<StepperPresentation>,
    policy: StepperNavPolicy,
    /// Menu presentation open.
    menu_open: bool,
    hits: Vec<(usize, Rect)>,
    menu_hit: Rect,
    root: Rect,
}

impl Default for StepperState {
    fn default() -> Self {
        Self::new()
    }
}

impl StepperState {
    /// Empty stepper.
    #[must_use]
    pub fn new() -> Self {
        Self {
            collection: CollectionState::new().orientation(RovingOrientation::Horizontal),
            current: 0,
            statuses: Vec::new(),
            focused: false,
            enabled: true,
            accepts_input: true,
            orientation: StepperOrientation::Horizontal,
            presentation: StepperPresentation::Expanded,
            presentation_override: None,
            policy: StepperNavPolicy::Linear,
            menu_open: false,
            hits: Vec::new(),
            menu_hit: Rect::default(),
            root: Rect::default(),
        }
    }

    /// From step count (Future statuses; first Current).
    #[must_use]
    pub fn with_len(n: usize) -> Self {
        let mut s = Self::new();
        s.resize(n);
        s
    }

    /// Resize statuses to `n`.
    pub fn resize(&mut self, n: usize) {
        self.statuses.resize(n, StepStatus::Future);
        if n > 0 {
            if self.current >= n {
                self.current = n - 1;
            }
            if self
                .statuses
                .iter()
                .all(|s| matches!(s, StepStatus::Future))
            {
                self.statuses[self.current] = StepStatus::Current;
            }
        } else {
            self.current = 0;
        }
    }

    /// Orientation.
    #[must_use]
    pub fn orientation(mut self, o: StepperOrientation) -> Self {
        self.orientation = o;
        self.collection = self.collection.clone().orientation(match o {
            StepperOrientation::Horizontal => RovingOrientation::Horizontal,
            StepperOrientation::Vertical => RovingOrientation::Vertical,
        });
        self
    }

    /// Set orientation.
    pub fn set_orientation(&mut self, o: StepperOrientation) {
        self.orientation = o;
        self.collection = CollectionState::new().orientation(match o {
            StepperOrientation::Horizontal => RovingOrientation::Horizontal,
            StepperOrientation::Vertical => RovingOrientation::Vertical,
        });
    }

    /// Nav policy.
    #[must_use]
    pub const fn policy(mut self, p: StepperNavPolicy) -> Self {
        self.policy = p;
        self
    }

    /// Set policy.
    pub fn set_policy(&mut self, p: StepperNavPolicy) {
        self.policy = p;
    }

    /// Force presentation.
    pub fn set_presentation_override(&mut self, p: Option<StepperPresentation>) {
        self.presentation_override = p;
        if let Some(p) = p {
            self.presentation = p;
        }
    }

    /// Focus.
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Focused?
    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.focused
    }

    /// Enable.
    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
    }

    /// Input gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Domain current index.
    #[must_use]
    pub const fn current(&self) -> usize {
        self.current
    }

    /// Roving cursor.
    #[must_use]
    pub fn cursor(&self) -> usize {
        self.collection.active().copied().unwrap_or(0)
    }

    /// Statuses.
    #[must_use]
    pub fn statuses(&self) -> &[StepStatus] {
        &self.statuses
    }

    /// Presentation.
    #[must_use]
    pub const fn presentation(&self) -> StepperPresentation {
        self.presentation
    }

    /// Policy.
    #[must_use]
    pub const fn nav_policy(&self) -> StepperNavPolicy {
        self.policy
    }

    /// Menu open?
    #[must_use]
    pub const fn menu_open(&self) -> bool {
        self.menu_open
    }

    /// Painted hit regions `(step_index, rect)` after last paint.
    #[must_use]
    pub fn hits(&self) -> &[(usize, Rect)] {
        &self.hits
    }

    /// Sync domain current + recompute linear statuses (complete before, current, future after).
    ///
    /// Preserves Error / Skipped / Disabled / Optional host overrides when
    /// `preserve_special` is true.
    pub fn set_current(&mut self, index: usize, len: usize, preserve_special: bool) {
        if len == 0 {
            self.current = 0;
            self.statuses.clear();
            return;
        }
        self.resize(len);
        self.current = index.min(len - 1);
        for i in 0..len {
            let prev = self.statuses[i];
            if preserve_special
                && matches!(
                    prev,
                    StepStatus::Error
                        | StepStatus::Skipped
                        | StepStatus::Disabled
                        | StepStatus::Optional
                )
                && i != self.current
            {
                continue;
            }
            if i < self.current {
                if !matches!(prev, StepStatus::Skipped) {
                    self.statuses[i] = StepStatus::Complete;
                }
            } else if i == self.current {
                self.statuses[i] = StepStatus::Current;
            } else if !matches!(prev, StepStatus::Disabled | StepStatus::Optional) {
                self.statuses[i] = StepStatus::Future;
            }
        }
        self.collection.set_active(Some(self.current));
    }

    /// Host override one status.
    pub fn set_status(&mut self, index: usize, status: StepStatus) {
        if index < self.statuses.len() {
            self.statuses[index] = status;
        }
    }

    /// Replace all statuses (len must match or is resized).
    pub fn set_statuses(&mut self, statuses: impl IntoIterator<Item = StepStatus>) {
        self.statuses = statuses.into_iter().collect();
        if self.current >= self.statuses.len() && !self.statuses.is_empty() {
            self.current = self.statuses.len() - 1;
        }
    }

    fn live(&self) -> bool {
        self.enabled && self.accepts_input && self.focused
    }

    fn entries(items: &[StepItem], statuses: &[StepStatus]) -> Vec<CollectionItem<usize>> {
        items
            .iter()
            .enumerate()
            .map(|(i, it)| {
                let st = statuses.get(i).copied().unwrap_or_default();
                let enabled = !it.disabled && !matches!(st, StepStatus::Disabled);
                CollectionItem {
                    id: i,
                    enabled,
                    label: it.title.clone(),
                    parent: None,
                }
            })
            .collect()
    }

    fn can_activate(&self, index: usize, items: &[StepItem]) -> bool {
        if index >= items.len() {
            return false;
        }
        let item = &items[index];
        let st = self.statuses.get(index).copied().unwrap_or_default();
        if item.disabled || matches!(st, StepStatus::Disabled) {
            return false;
        }
        match self.policy {
            StepperNavPolicy::DisplayOnly => false,
            StepperNavPolicy::Host => true,
            StepperNavPolicy::Free => true,
            StepperNavPolicy::Linear => {
                index == self.current
                    || matches!(
                        st,
                        StepStatus::Complete | StepStatus::Skipped | StepStatus::Error
                    )
                    || (index < self.current)
            }
        }
    }

    /// Activate step if policy allows.
    pub fn activate(&mut self, index: usize, items: &[StepItem]) -> StepperOutcome {
        if !self.can_activate(index, items) {
            return StepperOutcome::Ignored;
        }
        let id = items[index].id.clone();
        self.collection.set_active(Some(index));
        if self.menu_open {
            self.menu_open = false;
        }
        StepperOutcome::StepActivated { index, id }
    }

    /// Keyboard.
    pub fn handle_key(&mut self, key: KeyEvent, items: &[StepItem]) -> StepperOutcome {
        if !self.live() || items.is_empty() || key.kind == KeyEventKind::Release {
            return StepperOutcome::Ignored;
        }
        let entries = Self::entries(items, &self.statuses);
        let _ = self.collection.reconcile(&entries);

        if matches!(self.presentation, StepperPresentation::Menu) && !self.menu_open {
            if matches!(
                key.code,
                KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Down
            ) {
                self.menu_open = true;
                return StepperOutcome::MenuToggled { open: true };
            }
        }

        if self.menu_open {
            match key.code {
                KeyCode::Esc => {
                    self.menu_open = false;
                    return StepperOutcome::MenuToggled { open: false };
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    return self.activate(self.cursor(), items);
                }
                _ => {}
            }
        }

        if let Some(intent) = default_stepper_intent(key, self.orientation) {
            return self.handle_intent(intent, items);
        }
        // Digit jump 1..9
        if let KeyCode::Char(c) = key.code {
            if c.is_ascii_digit() && c != '0' {
                let idx = (c as u8 - b'1') as usize;
                if idx < items.len() {
                    return self.activate(idx, items);
                }
            }
        }
        StepperOutcome::Ignored
    }

    /// Intent.
    pub fn handle_intent(&mut self, intent: UiIntent, items: &[StepItem]) -> StepperOutcome {
        if !self.live() || items.is_empty() {
            return StepperOutcome::Ignored;
        }
        let entries = Self::entries(items, &self.statuses);
        let _ = self.collection.reconcile(&entries);
        match intent {
            UiIntent::Move(
                NavigationMove::Next
                | NavigationMove::Previous
                | NavigationMove::First
                | NavigationMove::Last
                | NavigationMove::Left
                | NavigationMove::Right
                | NavigationMove::Up
                | NavigationMove::Down,
            ) => {
                let out = self.collection.handle_intent(intent, &entries);
                if out.active_changed() {
                    StepperOutcome::CursorMoved {
                        index: self.cursor(),
                    }
                } else {
                    StepperOutcome::Ignored
                }
            }
            UiIntent::Activate | UiIntent::Submit | UiIntent::Toggle => {
                self.activate(self.cursor(), items)
            }
            UiIntent::Cancel | UiIntent::Close if self.menu_open => {
                self.menu_open = false;
                StepperOutcome::MenuToggled { open: false }
            }
            _ => StepperOutcome::Ignored,
        }
    }

    /// Mouse.
    pub fn handle_mouse(&mut self, event: MouseEvent, items: &[StepItem]) -> StepperOutcome {
        if !self.enabled || !self.accepts_input || items.is_empty() {
            return StepperOutcome::Ignored;
        }
        if !matches!(event.kind, MouseEventKind::Down(MouseButton::Left)) {
            return StepperOutcome::Ignored;
        }
        self.focused = true;
        if matches!(self.presentation, StepperPresentation::Menu) && !self.menu_open {
            if rect_contains(self.menu_hit, event.position) {
                self.menu_open = true;
                return StepperOutcome::MenuToggled { open: true };
            }
        }
        for (i, rect) in self.hits.iter().rev() {
            if rect_contains(*rect, event.position) {
                return self.activate(*i, items);
            }
        }
        if self.menu_open && !rect_contains(self.root, event.position) {
            self.menu_open = false;
            return StepperOutcome::MenuToggled { open: false };
        }
        StepperOutcome::Ignored
    }

    /// Sync presentation from paint area.
    pub fn sync_presentation(&mut self, area: Rect) -> StepperOutcome {
        if self.presentation_override.is_some() {
            return StepperOutcome::Ignored;
        }
        let next = stepper_presentation_for_bounds(area, self.orientation);
        if next != self.presentation {
            self.presentation = next;
            if !matches!(next, StepperPresentation::Menu) {
                self.menu_open = false;
            }
            StepperOutcome::PresentationChanged { presentation: next }
        } else {
            StepperOutcome::Ignored
        }
    }
}

fn rect_contains(rect: Rect, pos: Position) -> bool {
    pos.x >= rect.x
        && pos.y >= rect.y
        && pos.x < rect.x.saturating_add(rect.width)
        && pos.y < rect.y.saturating_add(rect.height)
}

/// Default intent map.
#[must_use]
pub fn default_stepper_intent(key: KeyEvent, orientation: StepperOrientation) -> Option<UiIntent> {
    if key.kind == KeyEventKind::Release {
        return None;
    }
    let is_press = key.kind == KeyEventKind::Press;
    match (orientation, key.code) {
        (_, KeyCode::Home) => Some(UiIntent::Move(NavigationMove::First)),
        (_, KeyCode::End) => Some(UiIntent::Move(NavigationMove::Last)),
        (_, KeyCode::Enter) if is_press => Some(UiIntent::Activate),
        (_, KeyCode::Char(' ')) if is_press => Some(UiIntent::Toggle),
        (_, KeyCode::Esc) if is_press => Some(UiIntent::Cancel),
        (StepperOrientation::Horizontal, KeyCode::Left | KeyCode::Char('h' | 'H')) => {
            Some(UiIntent::Move(NavigationMove::Previous))
        }
        (StepperOrientation::Horizontal, KeyCode::Right | KeyCode::Char('l' | 'L')) => {
            Some(UiIntent::Move(NavigationMove::Next))
        }
        (StepperOrientation::Vertical, KeyCode::Up | KeyCode::Char('k' | 'K')) => {
            Some(UiIntent::Move(NavigationMove::Previous))
        }
        (StepperOrientation::Vertical, KeyCode::Down | KeyCode::Char('j' | 'J')) => {
            Some(UiIntent::Move(NavigationMove::Next))
        }
        // Cross-orientation arrows still work
        (_, KeyCode::Left | KeyCode::Up) => Some(UiIntent::Move(NavigationMove::Previous)),
        (_, KeyCode::Right | KeyCode::Down) => Some(UiIntent::Move(NavigationMove::Next)),
        _ => None,
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Stepper paint.
#[derive(Debug, Clone, Copy)]
pub struct Stepper<'a> {
    items: &'a [StepItem],
    system: &'a DesignSystem,
    ascii: bool,
    colorless: bool,
    show_descriptions: bool,
}

impl<'a> Stepper<'a> {
    /// Items + design system.
    #[must_use]
    pub const fn new(items: &'a [StepItem], system: &'a DesignSystem) -> Self {
        Self {
            items,
            system,
            ascii: false,
            colorless: false,
            show_descriptions: true,
        }
    }

    /// ASCII marks.
    #[must_use]
    pub const fn ascii(mut self, on: bool) -> Self {
        self.ascii = on;
        self
    }

    /// Reduced color.
    #[must_use]
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    /// Vertical descriptions.
    #[must_use]
    pub const fn show_descriptions(mut self, on: bool) -> Self {
        self.show_descriptions = on;
        self
    }

    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut StepperState) {
        state.hits.clear();
        state.menu_hit = Rect::default();
        state.root = area;
        if area.is_empty() || self.items.is_empty() {
            return;
        }
        if state.statuses.len() != self.items.len() {
            state.resize(self.items.len());
        }
        let _ = state.sync_presentation(area);
        let entries = StepperState::entries(self.items, &state.statuses);
        let _ = state.collection.reconcile(&entries);

        match state.presentation {
            StepperPresentation::Numeric => self.paint_numeric(area, buffer, state),
            StepperPresentation::Menu => self.paint_menu(area, buffer, state),
            StepperPresentation::Expanded | StepperPresentation::Compact => {
                match state.orientation {
                    StepperOrientation::Horizontal => self.paint_horizontal(area, buffer, state),
                    StepperOrientation::Vertical => self.paint_vertical(area, buffer, state),
                }
            }
        }
    }

    fn style_for(
        &self,
        status: StepStatus,
        active_cursor: bool,
        focused: bool,
    ) -> ratatui_core::style::Style {
        if self.colorless {
            return match status {
                StepStatus::Current if focused => self
                    .system
                    .style(Role::TextStrong)
                    .add_modifier(Modifier::REVERSED | Modifier::BOLD),
                StepStatus::Complete => self.system.style(Role::TextStrong),
                StepStatus::Error => self
                    .system
                    .style(Role::TextStrong)
                    .add_modifier(Modifier::UNDERLINED),
                StepStatus::Disabled | StepStatus::Skipped | StepStatus::Future => {
                    self.system.style(Role::TextMuted)
                }
                StepStatus::Optional => self.system.style(Role::Text),
                StepStatus::Current => self.system.style(Role::TextStrong),
            };
        }
        let mut style = match status {
            StepStatus::Current => self
                .system
                .style(Role::Focus)
                .add_modifier(Modifier::BOLD | Modifier::REVERSED),
            StepStatus::Complete => self.system.style(Role::Success),
            StepStatus::Error => self.system.style(Role::Danger),
            StepStatus::Disabled => self.system.style(Role::TextDisabled),
            StepStatus::Skipped | StepStatus::Future => self.system.style(Role::TextMuted),
            StepStatus::Optional => self.system.style(Role::Text),
        };
        if active_cursor && focused && !matches!(status, StepStatus::Current) {
            style = style.add_modifier(Modifier::UNDERLINED);
        }
        style
    }

    fn paint_horizontal(&self, area: Rect, buffer: &mut Buffer, state: &mut StepperState) {
        let compact = matches!(state.presentation, StepperPresentation::Compact);
        let max_title = if compact { 8 } else { 14 };
        let mut x = area.x;
        let y = area.y;
        let cursor = state.cursor();
        let surface = state.focused && state.accepts_input;
        for (i, step) in self.items.iter().enumerate() {
            if x >= area.right() {
                break;
            }
            let status = state.statuses.get(i).copied().unwrap_or_default();
            let mark = status.mark(self.ascii);
            let title = take_display_cols(&step.title, max_title);
            let opt = if step.optional && !compact {
                if self.ascii { "?" } else { "◦" }
            } else {
                ""
            };
            let sep = if i + 1 < self.items.len() {
                if self.ascii { " > " } else { " → " }
            } else {
                ""
            };
            let cell = format!("{mark}{opt}{title}{sep}");
            let w = (display_cols(&cell) as u16).min(area.right().saturating_sub(x));
            if w == 0 {
                break;
            }
            let rect = Rect::new(x, y, w, 1);
            let style = self.style_for(status, cursor == i, surface);
            buffer.set_stringn(
                rect.x,
                rect.y,
                &take_display_cols(&cell, usize::from(w)),
                usize::from(w),
                style,
            );
            state.hits.push((i, rect));
            x = x.saturating_add(w);
        }
    }

    fn paint_vertical(&self, area: Rect, buffer: &mut Buffer, state: &mut StepperState) {
        let compact = matches!(state.presentation, StepperPresentation::Compact);
        let cursor = state.cursor();
        let surface = state.focused && state.accepts_input;
        let mut y = area.y;
        for (i, step) in self.items.iter().enumerate() {
            if y >= area.bottom() {
                break;
            }
            let status = state.statuses.get(i).copied().unwrap_or_default();
            let mark = status.mark(self.ascii);
            let title = take_display_cols(&step.title, usize::from(area.width.saturating_sub(6)));
            let line = format!("{mark} {title}");
            let rect = Rect::new(area.x, y, area.width, 1);
            let style = self.style_for(status, cursor == i, surface);
            buffer.set_stringn(
                rect.x,
                rect.y,
                &take_display_cols(&line, usize::from(area.width)),
                usize::from(area.width),
                style,
            );
            state.hits.push((i, rect));
            y = y.saturating_add(1);
            if self.show_descriptions
                && !compact
                && let Some(desc) = &step.description
            {
                if y >= area.bottom() {
                    break;
                }
                let d = format!(
                    "    {}",
                    take_display_cols(desc, usize::from(area.width.saturating_sub(4)))
                );
                buffer.set_stringn(
                    area.x,
                    y,
                    &take_display_cols(&d, usize::from(area.width)),
                    usize::from(area.width),
                    self.system.style(Role::TextMuted),
                );
                y = y.saturating_add(1);
            }
            // connector
            if i + 1 < self.items.len() && !compact && y < area.bottom() {
                let conn = if self.ascii { " |" } else { " │" };
                buffer.set_stringn(area.x, y, conn, 2, self.system.style(Role::Border));
                y = y.saturating_add(1);
            }
        }
    }

    fn paint_numeric(&self, area: Rect, buffer: &mut Buffer, state: &mut StepperState) {
        let n = self.items.len().max(1);
        let cur = state.current.saturating_add(1).min(n);
        let status = state
            .statuses
            .get(state.current)
            .copied()
            .unwrap_or(StepStatus::Current);
        let mark = status.mark(self.ascii);
        let title = self
            .items
            .get(state.current)
            .map(|s| s.title.as_str())
            .unwrap_or("");
        let line = format!(
            "{mark} {cur}/{n} {}",
            take_display_cols(title, usize::from(area.width.saturating_sub(12)))
        );
        let style = self.style_for(status, true, state.focused);
        buffer.set_stringn(
            area.x,
            area.y,
            &take_display_cols(&line, usize::from(area.width)),
            usize::from(area.width),
            style,
        );
        state
            .hits
            .push((state.current, Rect::new(area.x, area.y, area.width, 1)));
    }

    fn paint_menu(&self, area: Rect, buffer: &mut Buffer, state: &mut StepperState) {
        let n = self.items.len().max(1);
        let cur = state.current.saturating_add(1).min(n);
        let title = self
            .items
            .get(state.current)
            .map(|s| s.title.as_str())
            .unwrap_or("Step");
        let status = state
            .statuses
            .get(state.current)
            .copied()
            .unwrap_or(StepStatus::Current);
        let mark = status.mark(self.ascii);
        let chev = if state.menu_open {
            if self.ascii { "v" } else { "▾" }
        } else if self.ascii {
            ">"
        } else {
            "▸"
        };
        let line = format!("{mark} {cur}/{n} {title} {chev}");
        let style = self.style_for(status, true, state.focused);
        buffer.set_stringn(
            area.x,
            area.y,
            &take_display_cols(&line, usize::from(area.width)),
            usize::from(area.width),
            style,
        );
        state.menu_hit = Rect::new(area.x, area.y, area.width, 1);
        state.hits.push((state.current, state.menu_hit));

        if state.menu_open && area.height > 1 {
            let mut y = area.y.saturating_add(1);
            let cursor = state.cursor();
            for (i, step) in self.items.iter().enumerate() {
                if y >= area.bottom() {
                    break;
                }
                let st = state.statuses.get(i).copied().unwrap_or_default();
                let m = st.mark(self.ascii);
                let row = format!(
                    "{} {}",
                    m,
                    take_display_cols(&step.title, usize::from(area.width.saturating_sub(5)))
                );
                let rect = Rect::new(area.x, y, area.width, 1);
                let style = self.style_for(st, cursor == i, state.focused);
                buffer.set_stringn(
                    rect.x,
                    rect.y,
                    &take_display_cols(&row, usize::from(area.width)),
                    usize::from(area.width),
                    style,
                );
                state.hits.push((i, rect));
                y = y.saturating_add(1);
            }
        }
    }

    /// Semantic registration.
    pub fn register_semantic<Id, Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        id: Id,
        area: Rect,
        state: &StepperState,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if area.is_empty() {
            return;
        }
        let desc = format!(
            "stepper current={} presentation={} policy={} steps={}",
            state.current(),
            state.presentation().id(),
            state.nav_policy().id(),
            self.items.len()
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Control)
                .label("stepper")
                .description(desc)
                .focusable(state.enabled)
                .disabled(!state.enabled)
                .state(SemanticState {
                    selected: state.focused,
                    expanded: state.menu_open,
                    ..Default::default()
                }),
        );
    }
}

impl StatefulWidget for &Stepper<'_> {
    type State = StepperState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl StatefulWidget for Stepper<'_> {
    type State = StepperState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

// ── Helpers for FormWizard / plans ──────────────────────────────────────────

/// Project FormWizard-style steps into [`StepItem`] (identity copy).
#[must_use]
pub fn step_items_from_titles(titles: &[&str]) -> Vec<StepItem> {
    titles
        .iter()
        .enumerate()
        .map(|(i, t)| StepItem::new(format!("step-{i}"), *t))
        .collect()
}

/// Sample onboarding steps.
#[must_use]
pub fn example_onboarding_steps() -> Vec<StepItem> {
    vec![
        StepItem::new("account", "Account").description("Identity and email"),
        StepItem::new("region", "Region")
            .description("Deployment region")
            .optional(true),
        StepItem::new("plan", "Plan").description("Billing plan"),
        StepItem::new("review", "Review").description("Confirm and finish"),
    ]
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyModifiers;

    fn steps() -> Vec<StepItem> {
        example_onboarding_steps()
    }

    fn focused_linear(n: usize) -> StepperState {
        let mut s = StepperState::with_len(n).policy(StepperNavPolicy::Linear);
        s.set_focused(true);
        s.set_current(0, n, false);
        s
    }

    #[test]
    fn marks_non_color() {
        assert_eq!(StepStatus::Complete.mark(true), "[x]");
        assert_eq!(StepStatus::Error.mark(false), "[!]");
        assert_eq!(StepStatus::Disabled.mark(true), "[#]");
        assert_eq!(StepStatus::Future.mark(true), "[ ]");
    }

    #[test]
    fn linear_blocks_future_jump() {
        let items = steps();
        let mut s = focused_linear(items.len());
        assert!(matches!(s.activate(2, &items), StepperOutcome::Ignored));
        s.set_current(1, items.len(), false);
        s.set_status(0, StepStatus::Complete);
        assert!(matches!(
            s.activate(0, &items),
            StepperOutcome::StepActivated { index: 0, .. }
        ));
    }

    #[test]
    fn free_policy_allows_any_enabled() {
        let items = steps();
        let mut s = focused_linear(items.len()).policy(StepperNavPolicy::Free);
        s.set_focused(true);
        assert!(matches!(
            s.activate(3, &items),
            StepperOutcome::StepActivated { index: 3, .. }
        ));
    }

    #[test]
    fn disabled_step_blocked() {
        let mut items = steps();
        items[1].disabled = true;
        let mut s = focused_linear(items.len()).policy(StepperNavPolicy::Free);
        s.set_focused(true);
        assert!(matches!(s.activate(1, &items), StepperOutcome::Ignored));
    }

    #[test]
    fn presentation_width() {
        assert_eq!(
            stepper_presentation_for_bounds(Rect::new(0, 0, 20, 5), StepperOrientation::Horizontal),
            StepperPresentation::Menu
        );
        assert_eq!(
            stepper_presentation_for_bounds(Rect::new(0, 0, 40, 5), StepperOrientation::Horizontal),
            StepperPresentation::Compact
        );
        assert_eq!(
            stepper_presentation_for_bounds(Rect::new(0, 0, 80, 5), StepperOrientation::Horizontal),
            StepperPresentation::Expanded
        );
    }

    #[test]
    fn menu_toggle_and_activate() {
        let items = steps();
        let mut s = focused_linear(items.len()).policy(StepperNavPolicy::Host);
        s.set_focused(true);
        s.set_presentation_override(Some(StepperPresentation::Menu));
        assert!(matches!(
            s.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &items),
            StepperOutcome::MenuToggled { open: true }
        ));
        let _ = s.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &items);
        assert!(matches!(
            s.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &items),
            StepperOutcome::StepActivated { .. }
        ));
    }

    #[test]
    fn digit_jump_host_policy() {
        let items = steps();
        let mut s = focused_linear(items.len()).policy(StepperNavPolicy::Host);
        s.set_focused(true);
        assert!(matches!(
            s.handle_key(
                KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE),
                &items
            ),
            StepperOutcome::StepActivated { index: 2, .. }
        ));
    }

    #[test]
    fn paint_horizontal_and_vertical() {
        let system = DesignSystem::default();
        let items = steps();
        let mut s = focused_linear(items.len());
        s.set_status(0, StepStatus::Complete);
        s.set_current(1, items.len(), true);
        let area = Rect::new(0, 0, 72, 3);
        let mut buf = Buffer::empty(area);
        Stepper::new(&items, &system)
            .ascii(true)
            .paint(area, &mut buf, &mut s);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("Account") || text.contains("[x]") || text.contains("Region"),
            "{text}"
        );

        let mut s2 = focused_linear(items.len());
        s2.set_orientation(StepperOrientation::Vertical);
        let area2 = Rect::new(0, 0, 24, 12);
        let mut buf2 = Buffer::empty(area2);
        Stepper::new(&items, &system)
            .ascii(true)
            .paint(area2, &mut buf2, &mut s2);
        let t2: String = buf2
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(t2.contains("Account") || t2.contains("[>]"), "{t2}");
    }

    #[test]
    fn colorless_error_still_marked() {
        let system = DesignSystem::default();
        let items = steps();
        let mut s = focused_linear(items.len());
        s.set_status(1, StepStatus::Error);
        let area = Rect::new(0, 0, 60, 2);
        let mut buf = Buffer::empty(area);
        Stepper::new(&items, &system)
            .ascii(true)
            .colorless(true)
            .paint(area, &mut buf, &mut s);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(text.contains("[!]"), "{text}");
    }

    #[test]
    fn fuzz_keys() {
        let items = steps();
        let mut s = focused_linear(items.len()).policy(StepperNavPolicy::Host);
        s.set_focused(true);
        let keys = [
            KeyCode::Left,
            KeyCode::Right,
            KeyCode::Enter,
            KeyCode::Char('1'),
            KeyCode::Char('4'),
            KeyCode::Esc,
            KeyCode::Home,
            KeyCode::End,
        ];
        let mut seed = 3u64;
        for _ in 0..200 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let k = keys[(seed as usize) % keys.len()];
            let _ = s.handle_key(KeyEvent::new(k, KeyModifiers::NONE), &items);
        }
    }

    #[test]
    fn semantic_registers() {
        let system = DesignSystem::default();
        let items = steps();
        let s = focused_linear(items.len());
        let mut scene = SemanticScene::<&str, ()>::default();
        Stepper::new(&items, &system).register_semantic(
            &mut scene,
            "st",
            Rect::new(0, 0, 40, 2),
            &s,
        );
        assert!(
            scene
                .nodes()
                .iter()
                .any(|n| n.label.as_deref() == Some("stepper"))
        );
    }

    #[test]
    fn display_only_never_activates() {
        let items = steps();
        let mut s = focused_linear(items.len()).policy(StepperNavPolicy::DisplayOnly);
        s.set_focused(true);
        assert!(matches!(
            s.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &items),
            StepperOutcome::Ignored
        ));
    }
}
