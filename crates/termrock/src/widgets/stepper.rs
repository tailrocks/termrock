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
    style::{Modifier, Style},
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
    style::{ButtonRecipeVariant, ControlState, DesignSystem, Glyph, ListRowVisualState, Role},
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

    /// Non-color mark (always paired with style roles). One junie vocabulary.
    ///
    /// Progress marks, not checkbox wells: `[✓]` / `[ ]` belong to Checkbox.
    #[must_use]
    pub const fn mark(self) -> &'static str {
        match self {
            Self::Complete => Glyph::Success.resolve().text,
            Self::Current => Glyph::SelectionMarker.resolve().text,
            Self::Error => Glyph::Error.resolve().text,
            Self::Disabled | Self::Skipped => Glyph::Remove.resolve().text,
            // Optional is a `◦` suffix on the title in horizontal paint, not a well.
            Self::Optional | Self::Future => " ",
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
    vertical_scroll: usize,
    vertical_viewport: usize,
    vertical_show_descriptions: bool,
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
            vertical_scroll: 0,
            vertical_viewport: 0,
            vertical_show_descriptions: true,
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
        self.collection = std::mem::take(&mut self.collection).orientation(match o {
            StepperOrientation::Horizontal => RovingOrientation::Horizontal,
            StepperOrientation::Vertical => RovingOrientation::Vertical,
        });
        self
    }

    /// Set orientation.
    pub fn set_orientation(&mut self, o: StepperOrientation) {
        self.orientation = o;
        self.collection = self.collection.clone().orientation(match o {
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

    fn selected_index(&self, len: usize) -> usize {
        self.collection
            .active()
            .copied()
            .unwrap_or(self.current)
            .min(len.saturating_sub(1))
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

    fn uses_vertical_viewport(&self) -> bool {
        matches!(self.orientation, StepperOrientation::Vertical)
            && matches!(
                self.presentation,
                StepperPresentation::Expanded | StepperPresentation::Compact
            )
    }

    fn vertical_rows_to_cursor(&self, items: &[StepItem], start: usize, cursor: usize) -> usize {
        let expanded = matches!(self.presentation, StepperPresentation::Expanded);
        let mut rows = 0usize;
        for index in start..=cursor {
            rows = rows.saturating_add(1);
            if expanded && self.vertical_show_descriptions && items[index].description.is_some() {
                rows = rows.saturating_add(1);
            }
            if expanded && index < cursor && index + 1 < items.len() {
                rows = rows.saturating_add(1);
            }
        }
        rows
    }

    fn ensure_vertical_cursor_visible(&mut self, items: &[StepItem]) {
        if !self.uses_vertical_viewport() || items.is_empty() {
            self.vertical_scroll = 0;
            return;
        }
        let Some(cursor) = self.collection.active().copied() else {
            self.vertical_scroll = 0;
            return;
        };
        self.vertical_scroll = self.vertical_scroll.min(items.len().saturating_sub(1));
        if cursor < self.vertical_scroll {
            self.vertical_scroll = cursor;
        }
        let capacity = self.vertical_viewport.max(1);
        while self.vertical_scroll < cursor
            && self.vertical_rows_to_cursor(items, self.vertical_scroll, cursor) > capacity
        {
            self.vertical_scroll = self.vertical_scroll.saturating_add(1);
        }
    }

    fn configure_vertical_viewport(&mut self, items: &[StepItem], rows: usize) {
        if self.uses_vertical_viewport() {
            self.vertical_viewport = rows.max(1);
            self.ensure_vertical_cursor_visible(items);
        } else {
            self.vertical_scroll = 0;
        }
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
        self.ensure_vertical_cursor_visible(items);
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
        self.ensure_vertical_cursor_visible(items);

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
        self.ensure_vertical_cursor_visible(items);
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
                    self.ensure_vertical_cursor_visible(items);
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
            // Seeded from the system: a widget that defaults to false is
            // claiming the terminal has Unicode and colour before anyone
            // asked it. Builders below still force either way.
            colorless: system.mono(),
            show_descriptions: true,
        }
    }

    /// ASCII marks.
    #[must_use]
    /// Reduced color.
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
        state.vertical_show_descriptions = self.show_descriptions;
        state.configure_vertical_viewport(self.items, usize::from(area.height));

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

    fn status_style(&self, status: StepStatus, base: Style) -> Style {
        if self.colorless {
            return match status {
                StepStatus::Current | StepStatus::Complete | StepStatus::Error => {
                    base.add_modifier(Modifier::BOLD)
                }
                _ => base,
            };
        }
        match status {
            // The current step is stated by its glyph and its weight, not by a
            // reversed slab — a solid block reads as a selection the operator
            // made, not as where they are (plans/008 Step 4).
            StepStatus::Current => base.add_modifier(Modifier::BOLD),
            StepStatus::Complete => base.patch(self.system.style(Role::Success)),
            StepStatus::Error => base.patch(self.system.style(Role::Danger)),
            _ => base,
        }
    }

    fn paint_step_row(
        &self,
        rect: Rect,
        buffer: &mut Buffer,
        status: StepStatus,
        cursor: bool,
        focused: bool,
        enabled: bool,
    ) -> Style {
        let recipe = self.system.resolve_list_row(ListRowVisualState {
            selected: cursor,
            focused: cursor && focused,
            hovered: false,
            enabled: enabled && !matches!(status, StepStatus::Disabled),
            loading: false,
            checked: matches!(status, StepStatus::Complete),
            ..ListRowVisualState::default()
        });
        if recipe.use_tint {
            buffer.set_style(rect, recipe.tint);
        }
        self.status_style(status, recipe.label)
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
            let mark = status.mark();
            let title = take_display_cols(&step.title, max_title);
            let opt = if step.optional && !compact { "◦" } else { "" };
            let content = format!("{mark} {opt}{title}");
            let remaining = area.right().saturating_sub(x);
            if remaining == 0 {
                break;
            }
            let content_w = display_cols(&content) as u16;
            // Join lives between steps. Do not paint a dangling ` · ` when the
            // next title cannot start (separator consumed the leftover cells).
            let sep = if i + 1 < self.items.len() {
                self.system.glyphs.meta_join()
            } else {
                ""
            };
            let sep_w = display_cols(sep) as u16;
            let cell = if content_w < remaining
                && !sep.is_empty()
                && content_w.saturating_add(sep_w) < remaining
            {
                format!("{content}{sep}")
            } else {
                content
            };
            let w = (display_cols(&cell) as u16).min(remaining);
            if w == 0 {
                break;
            }
            let rect = Rect::new(x, y, w, 1);
            let style = self.paint_step_row(
                rect,
                buffer,
                status,
                cursor == i,
                surface,
                state.accepts_input,
            );
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
        for (i, step) in self.items.iter().enumerate().skip(state.vertical_scroll) {
            if y >= area.bottom() {
                break;
            }
            let status = state.statuses.get(i).copied().unwrap_or_default();
            let mark = status.mark();
            let title = take_display_cols(&step.title, usize::from(area.width.saturating_sub(6)));
            let line = format!("{mark} {title}");
            let rect = Rect::new(area.x, y, area.width, 1);
            let style = self.paint_step_row(
                rect,
                buffer,
                status,
                cursor == i,
                surface,
                state.accepts_input,
            );
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
                let conn = { " │" };
                buffer.set_stringn(area.x, y, conn, 2, self.system.style(Role::Border));
                y = y.saturating_add(1);
            }
        }
    }

    fn paint_numeric(&self, area: Rect, buffer: &mut Buffer, state: &mut StepperState) {
        let n = self.items.len().max(1);
        let selected = state.selected_index(self.items.len());
        let cur = selected.saturating_add(1).min(n);
        let status = state
            .statuses
            .get(selected)
            .copied()
            .unwrap_or(StepStatus::Current);
        let mark = status.mark();
        let title = self
            .items
            .get(selected)
            .map(|s| s.title.as_str())
            .unwrap_or("");
        let line = format!(
            "{mark} {cur}/{n} {}",
            take_display_cols(title, usize::from(area.width.saturating_sub(12)))
        );
        let recipe = self.system.button_recipe(
            ButtonRecipeVariant::Quiet,
            if !state.accepts_input || matches!(status, StepStatus::Disabled) {
                ControlState::Disabled
            } else if state.focused {
                ControlState::Focused
            } else {
                ControlState::Default
            },
            self.system.junie_theme().surface,
        );
        buffer.set_style(Rect::new(area.x, area.y, area.width, 1), recipe.fill);
        let style = self.status_style(status, recipe.label);
        buffer.set_stringn(
            area.x,
            area.y,
            &take_display_cols(&line, usize::from(area.width)),
            usize::from(area.width),
            style,
        );
        state
            .hits
            .push((selected, Rect::new(area.x, area.y, area.width, 1)));
    }

    fn paint_menu(&self, area: Rect, buffer: &mut Buffer, state: &mut StepperState) {
        let n = self.items.len().max(1);
        let selected = state.selected_index(self.items.len());
        let cur = selected.saturating_add(1).min(n);
        let title = self
            .items
            .get(selected)
            .map(|s| s.title.as_str())
            .unwrap_or("Step");
        let status = state
            .statuses
            .get(selected)
            .copied()
            .unwrap_or(StepStatus::Current);
        let mark = status.mark();
        let chev = if state.menu_open { "▾" } else { "▸" };
        let line = format!("{mark} {cur}/{n} {title} {chev}");
        let recipe = self.system.button_recipe(
            ButtonRecipeVariant::Quiet,
            if !state.accepts_input || matches!(status, StepStatus::Disabled) {
                ControlState::Disabled
            } else if state.focused {
                ControlState::Focused
            } else {
                ControlState::Default
            },
            self.system.junie_theme().surface,
        );
        buffer.set_style(Rect::new(area.x, area.y, area.width, 1), recipe.fill);
        let style = self.status_style(status, recipe.label);
        buffer.set_stringn(
            area.x,
            area.y,
            &take_display_cols(&line, usize::from(area.width)),
            usize::from(area.width),
            style,
        );
        state.menu_hit = Rect::new(area.x, area.y, area.width, 1);
        state.hits.push((selected, state.menu_hit));

        if state.menu_open && area.height > 1 {
            let mut y = area.y.saturating_add(1);
            let cursor = state.cursor();
            for (i, step) in self.items.iter().enumerate() {
                if y >= area.bottom() {
                    break;
                }
                let st = state.statuses.get(i).copied().unwrap_or_default();
                let m = st.mark();
                let row = format!(
                    "{} {}",
                    m,
                    take_display_cols(&step.title, usize::from(area.width.saturating_sub(5)))
                );
                let rect = Rect::new(area.x, y, area.width, 1);
                let style = self.paint_step_row(
                    rect,
                    buffer,
                    st,
                    cursor == i,
                    state.focused,
                    state.accepts_input,
                );
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
        assert_eq!(StepStatus::Complete.mark(), Glyph::Success.resolve().text);
        assert_eq!(
            StepStatus::Current.mark(),
            Glyph::SelectionMarker.resolve().text
        );
        assert_eq!(StepStatus::Error.mark(), Glyph::Error.resolve().text);
        assert_eq!(StepStatus::Disabled.mark(), Glyph::Remove.resolve().text);
        assert_eq!(StepStatus::Skipped.mark(), Glyph::Remove.resolve().text);
        assert_eq!(StepStatus::Optional.mark(), " ");
        assert_eq!(StepStatus::Future.mark(), " ");
        for status in [
            StepStatus::Complete,
            StepStatus::Current,
            StepStatus::Error,
            StepStatus::Disabled,
            StepStatus::Optional,
            StepStatus::Skipped,
            StepStatus::Future,
        ] {
            assert!(
                !status.mark().contains('[') && !status.mark().contains(']'),
                "checkbox well leaked from {status:?}: {:?}",
                status.mark()
            );
        }
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
    fn orientation_change_preserves_cursor_for_relative_movement() {
        let items = steps();
        let mut state = focused_linear(items.len()).policy(StepperNavPolicy::Free);
        state.set_current(2, items.len(), false);

        state.set_orientation(StepperOrientation::Vertical);

        assert_eq!(state.cursor(), 2);
        assert_eq!(
            state.handle_intent(UiIntent::Move(NavigationMove::Next), &items),
            StepperOutcome::CursorMoved { index: 3 }
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
        let _ = s.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &items);
        assert!(matches!(
            s.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &items),
            StepperOutcome::MenuToggled { open: false }
        ));
    }

    #[test]
    fn numeric_paint_and_mouse_follow_roving_cursor() {
        let system = DesignSystem::default();
        let items = steps();
        let mut state = focused_linear(items.len()).policy(StepperNavPolicy::Host);
        state.set_presentation_override(Some(StepperPresentation::Numeric));
        let area = Rect::new(0, 0, 28, 1);
        let widget = Stepper::new(&items, &system);

        let mut buffer = Buffer::empty(area);
        widget.paint(area, &mut buffer, &mut state);
        assert_eq!(
            state.handle_intent(UiIntent::Move(NavigationMove::Next), &items),
            StepperOutcome::CursorMoved { index: 1 }
        );

        let mut buffer = Buffer::empty(area);
        widget.paint(area, &mut buffer, &mut state);
        let text: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(text.contains("2/4 Region"), "{text}");
        assert_eq!(state.hits()[0].0, 1);
        let hit = state.hits()[0].1;
        assert_eq!(
            state.handle_mouse(
                MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    position: Position::new(hit.x, hit.y),
                    modifiers: KeyModifiers::NONE,
                },
                &items,
            ),
            StepperOutcome::StepActivated {
                index: 1,
                id: items[1].id.clone(),
            }
        );
        assert_eq!(state.current(), 0);
    }

    #[test]
    fn menu_paint_and_header_hit_follow_roving_cursor() {
        let system = DesignSystem::default();
        let items = steps();
        let mut state = focused_linear(items.len()).policy(StepperNavPolicy::Host);
        state.set_presentation_override(Some(StepperPresentation::Menu));
        state.menu_open = true;
        let area = Rect::new(0, 0, 28, 5);
        let widget = Stepper::new(&items, &system);

        let mut buffer = Buffer::empty(area);
        widget.paint(area, &mut buffer, &mut state);
        assert_eq!(
            state.handle_intent(UiIntent::Move(NavigationMove::Next), &items),
            StepperOutcome::CursorMoved { index: 1 }
        );

        let mut buffer = Buffer::empty(area);
        widget.paint(area, &mut buffer, &mut state);
        let text: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(text.contains("2/4 Region"), "{text}");
        assert_eq!(state.hits()[0].0, 1);
        let hit = state.hits()[0].1;
        assert_eq!(
            state.handle_mouse(
                MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    position: Position::new(hit.x, hit.y),
                    modifiers: KeyModifiers::NONE,
                },
                &items,
            ),
            StepperOutcome::StepActivated {
                index: 1,
                id: items[1].id.clone(),
            }
        );
        assert_eq!(state.current(), 0);
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
        Stepper::new(&items, &system).paint(area, &mut buf, &mut s);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("Account") || text.contains("Region"),
            "{text}"
        );
        assert!(
            text.contains(Glyph::Success.resolve().text) && text.contains(" Account"),
            "complete is catalog success with a space before the title: {text}"
        );
        assert!(
            text.contains(&format!("{} Region", Glyph::SelectionMarker.resolve().text))
                || text.contains(&format!(
                    "{} ◦Region",
                    Glyph::SelectionMarker.resolve().text
                )),
            "current is catalog selection marker with a space before the title: {text}"
        );
        assert!(
            text.contains(system.glyphs.meta_join()),
            "horizontal steps join with catalog meta_join: {text}"
        );
        assert!(
            !text.contains("[✓]")
                && !text.contains("[›]")
                && !text.contains("[ ]")
                && !text.contains(" → "),
            "invented wells / arrows leaked: {text}"
        );

        let mut s2 = focused_linear(items.len());
        s2.set_orientation(StepperOrientation::Vertical);
        // Wider than STEPPER_NARROW_MAX_WIDTH so this is a vertical list, not numeric.
        let area2 = Rect::new(0, 0, 56, 16);
        let mut buf2 = Buffer::empty(area2);
        Stepper::new(&items, &system).paint(area2, &mut buf2, &mut s2);
        let t2: String = buf2
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            t2.contains("Account")
                && t2.contains(&format!(
                    "{} Account",
                    Glyph::SelectionMarker.resolve().text
                )),
            "{t2}"
        );
        assert!(
            !t2.contains("[✓]") && !t2.contains("[›]") && !t2.contains("[ ]"),
            "invented wells leaked: {t2}"
        );
    }

    #[test]
    fn vertical_navigation_reveals_the_active_step() {
        let system = DesignSystem::default();
        let items = steps();
        let mut state = focused_linear(items.len()).policy(StepperNavPolicy::Free);
        state.set_orientation(StepperOrientation::Vertical);
        state.set_presentation_override(Some(StepperPresentation::Expanded));

        let area = Rect::new(0, 0, 48, 6);
        let mut buffer = Buffer::empty(area);
        Stepper::new(&items, &system).paint(area, &mut buffer, &mut state);
        for _ in 0..3 {
            assert!(matches!(
                state.handle_intent(UiIntent::Move(NavigationMove::Next), &items,),
                StepperOutcome::CursorMoved { .. }
            ));
        }

        let mut buffer = Buffer::empty(area);
        Stepper::new(&items, &system).paint(area, &mut buffer, &mut state);
        assert!(state.vertical_scroll > 0);
        let (_, hit) = state
            .hits
            .iter()
            .find(|(index, _)| *index == 3)
            .copied()
            .expect("active step must remain clickable after scrolling");
        let text: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(text.contains("Review"), "{text}");
        assert!(matches!(
            state.handle_mouse(
                MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    position: Position::new(hit.x, hit.y),
                    modifiers: KeyModifiers::NONE,
                },
                &items,
            ),
            StepperOutcome::StepActivated { index: 3, .. }
        ));
    }

    #[test]
    fn mouse_activates_only_painted_step_hit() {
        let system = DesignSystem::default();
        let items = steps();
        let mut state = focused_linear(items.len()).policy(StepperNavPolicy::Free);
        let area = Rect::new(0, 0, 72, 3);
        let mut buffer = Buffer::empty(area);
        Stepper::new(&items, &system).paint(area, &mut buffer, &mut state);
        let (index, hit) = state.hits[1];

        assert_eq!(
            state.handle_mouse(
                MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    position: Position::new(hit.x, hit.y),
                    modifiers: KeyModifiers::NONE,
                },
                &items,
            ),
            StepperOutcome::StepActivated {
                index,
                id: items[index].id.clone()
            }
        );
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
            .colorless(true)
            .paint(area, &mut buf, &mut s);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains(Glyph::Error.resolve().text),
            "colorless error still uses catalog error mark: {text}"
        );
        assert!(!text.contains("[!]"), "invented error well leaked: {text}");
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

    #[test]
    fn empty_stepper_is_safe_and_never_activates() {
        let system = DesignSystem::default();
        let items: [StepItem; 0] = [];
        let mut state = StepperState::with_len(0);
        state.set_focused(true);
        let area = Rect::new(0, 0, 1, 1);
        let mut buffer = Buffer::empty(area);

        Stepper::new(&items, &system).paint(area, &mut buffer, &mut state);

        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &items),
            StepperOutcome::Ignored
        );
    }
}
