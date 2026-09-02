// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Numeric field with draft text separate from committed value.
//!
//! **Mission.** Forms and settings need integer/decimal entry with min/max/step,
//! units, empty and intermediate invalid states, steppers, and safe overflow —
//! without locale-dependent storage.
//!
//! **vs [`TextInput`](super::TextInput).** Free text.
//! **vs [`Slider`](super::Slider).** Continuous scrub; NumberInput is typed entry
//! that can project into the same [`SliderBounds`](super::SliderBounds).
//!
//! Research: shadcn numeric inputs, Textual numeric fields, desktop form UX.
use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    interaction::{SemanticNode, SemanticRole, SemanticScene, SemanticState, UiIntent},
    style::{ButtonRecipeVariant, ControlState, DesignSystem},
    text::{display_cols, take_display_cols},
};
use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::StatefulWidget};

use super::{SliderBounds, TextInput, TextInputOutcome, TextInputState, Validation};

// ── Kind / constraints ──────────────────────────────────────────────────────

/// Integer vs fixed-precision decimal storage format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum NumberKind {
    /// Whole numbers only (`.` rejected).
    #[default]
    Integer,
    /// Decimal with at most `max_fraction_digits` after `.` (locale-independent).
    Decimal {
        /// Max digits after decimal point (clamped 0..=12).
        max_fraction_digits: u8,
    },
}

impl NumberKind {
    /// Decimal with two fraction digits.
    #[must_use]
    pub const fn decimal2() -> Self {
        Self::Decimal {
            max_fraction_digits: 2,
        }
    }

    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Integer => "integer",
            Self::Decimal { .. } => "decimal",
        }
    }
}

/// Min / max / step (locale-independent `f64` storage).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NumberConstraints {
    /// Inclusive minimum when set.
    pub min: Option<f64>,
    /// Inclusive maximum when set.
    pub max: Option<f64>,
    /// Step size for steppers / arrows (`> 0`).
    pub step: f64,
}

impl Default for NumberConstraints {
    fn default() -> Self {
        Self {
            min: None,
            max: None,
            step: 1.0,
        }
    }
}

impl NumberConstraints {
    /// Unbounded with step 1.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Min and max inclusive.
    #[must_use]
    pub fn bounded(min: f64, max: f64, step: f64) -> Self {
        let (min, max) = if min <= max { (min, max) } else { (max, min) };
        let step = sanitize_step(step);
        Self {
            min: Some(min),
            max: Some(max),
            step,
        }
    }

    /// From slider bounds (shared snap model).
    #[must_use]
    pub fn from_slider(bounds: SliderBounds) -> Self {
        Self {
            min: Some(bounds.min),
            max: Some(bounds.max),
            step: sanitize_step(bounds.step),
        }
    }

    /// Builder: min.
    #[must_use]
    pub const fn with_min(mut self, min: f64) -> Self {
        self.min = Some(min);
        self
    }

    /// Builder: max.
    #[must_use]
    pub const fn with_max(mut self, max: f64) -> Self {
        self.max = Some(max);
        self
    }

    /// Builder: step.
    #[must_use]
    pub fn with_step(mut self, step: f64) -> Self {
        self.step = sanitize_step(step);
        self
    }

    /// Clamp + optional snap to step grid when both bounds present.
    #[must_use]
    pub fn clamp_snap(self, value: f64) -> f64 {
        if !value.is_finite() {
            return self.min.or(self.max).unwrap_or(0.0);
        }
        let mut v = value;
        if let Some(min) = self.min {
            v = v.max(min);
        }
        if let Some(max) = self.max {
            v = v.min(max);
        }
        if let (Some(min), Some(max)) = (self.min, self.max) {
            return SliderBounds::new(min, max, self.step).snap(v);
        }
        // Step relative to 0 when unbounded on one side
        if self.step > 0.0 && self.step.is_finite() {
            let n = (v / self.step).round();
            let snapped = n * self.step;
            if snapped.is_finite() {
                return clean_float(snapped);
            }
        }
        clean_float(v)
    }

    /// Add `n` steps to `value`, clamped.
    #[must_use]
    pub fn stepped(self, value: f64, n: i32) -> f64 {
        if n == 0 {
            return self.clamp_snap(value);
        }
        let delta = self.step * f64::from(n);
        if !delta.is_finite() {
            return self.clamp_snap(value);
        }
        let base = if value.is_finite() {
            value
        } else {
            self.min.or(self.max).unwrap_or(0.0)
        };
        self.clamp_snap(base + delta)
    }
}

fn sanitize_step(step: f64) -> f64 {
    if step.is_finite() && step > 0.0 {
        step
    } else {
        1.0
    }
}

fn clean_float(v: f64) -> f64 {
    if !v.is_finite() {
        return 0.0;
    }
    if (v - v.round()).abs() < 1e-12 {
        v.round()
    } else {
        v
    }
}

// ── Parse / validity ────────────────────────────────────────────────────────

/// Parse result for draft text (locale-independent `.` decimal).
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum NumberParse {
    /// Empty draft.
    Empty,
    /// Valid prefix still being typed (`-`, `12.`, `+3`).
    Intermediate,
    /// Not a number.
    Invalid,
    /// Finite parsed value (not yet range-checked).
    Number(f64),
}

/// Field validity for chrome / submit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum NumberValidity {
    /// Empty allowed and draft empty.
    Empty,
    /// Intermediate typing (not ready to commit).
    Intermediate,
    /// Unparseable.
    Invalid,
    /// Parsed but outside min/max.
    OutOfRange,
    /// Committed or draft is valid in range.
    Valid,
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Interaction outcomes.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum NumberInputOutcome {
    /// No effect.
    Ignored,
    /// Draft text or caret changed (committed value unchanged).
    Changed,
    /// Committed numeric value changed (step, commit, set).
    ValueChanged {
        /// New committed value (`None` if cleared).
        value: Option<f64>,
    },
    /// Enter with valid value.
    Submitted {
        /// Committed value at submit.
        value: Option<f64>,
    },
    /// Esc / cancel editing.
    Cancelled,
    /// Host paste request.
    ClipboardPasteRequest,
    /// Copy for host.
    ClipboardCopy {
        /// Draft or formatted value text.
        text: String,
    },
}

// ── State ───────────────────────────────────────────────────────────────────

/// Runtime state: **draft text** ≠ **committed value**.
#[derive(Debug, Clone, PartialEq)]
pub struct NumberInputState {
    kind: NumberKind,
    constraints: NumberConstraints,
    /// Locale-independent committed value.
    value: Option<f64>,
    /// Editing buffer (always what the user sees while focused/editing).
    draft: TextInputState,
    /// When true, display follows draft; when false, draft mirrors value.
    editing: bool,
    allow_empty: bool,
    enabled: bool,
    read_only: bool,
    focused: bool,
    /// Last paint parts (field + steppers).
    parts: Option<NumberInputParts>,
}

impl Default for NumberInputState {
    fn default() -> Self {
        Self::new()
    }
}

impl NumberInputState {
    /// Empty integer field, empty allowed.
    #[must_use]
    pub fn new() -> Self {
        let mut draft = TextInputState::new("").with_allow_empty(true);
        draft.set_focused(false);
        Self {
            kind: NumberKind::Integer,
            constraints: NumberConstraints::default(),
            value: None,
            draft,
            editing: false,
            allow_empty: true,
            enabled: true,
            read_only: false,
            focused: false,
            parts: None,
        }
    }

    /// Seed committed value.
    #[must_use]
    pub fn with_value(mut self, value: f64) -> Self {
        self.set_value(Some(value));
        self
    }

    /// Kind.
    #[must_use]
    pub fn with_kind(mut self, kind: NumberKind) -> Self {
        self.kind = kind;
        if let Some(v) = self.value {
            self.set_value(Some(v));
        }
        self
    }

    /// Constraints.
    #[must_use]
    pub fn with_constraints(mut self, c: NumberConstraints) -> Self {
        self.constraints = c;
        if let Some(v) = self.value {
            self.set_value(Some(v));
        }
        self
    }

    /// Allow empty commit.
    #[must_use]
    pub const fn with_allow_empty(mut self, on: bool) -> Self {
        self.allow_empty = on;
        self
    }

    /// Kind.
    #[must_use]
    pub const fn kind(&self) -> NumberKind {
        self.kind
    }

    /// Constraints.
    #[must_use]
    pub const fn constraints(&self) -> NumberConstraints {
        self.constraints
    }

    /// Committed value.
    #[must_use]
    pub const fn value(&self) -> Option<f64> {
        self.value
    }

    /// Draft text.
    #[must_use]
    pub fn draft_text(&self) -> &str {
        self.draft.value()
    }

    /// Whether empty is allowed.
    #[must_use]
    pub const fn allow_empty(&self) -> bool {
        self.allow_empty
    }

    /// Focused.
    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.focused
    }

    /// Enabled.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Read-only.
    #[must_use]
    pub const fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Editing draft (vs idle display of committed).
    #[must_use]
    pub const fn is_editing(&self) -> bool {
        self.editing
    }

    /// Paint geometry.
    #[must_use]
    pub const fn parts(&self) -> Option<&NumberInputParts> {
        self.parts.as_ref()
    }

    /// Enabled.
    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
        self.sync_draft_gates();
    }

    /// Read-only.
    pub fn set_read_only(&mut self, on: bool) {
        self.read_only = on;
        self.sync_draft_gates();
    }

    /// Focus; starts draft edit session.
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on;
        self.sync_draft_gates();
        if on {
            self.begin_edit();
        } else {
            let _ = self.commit_draft();
        }
    }

    fn sync_draft_gates(&mut self) {
        self.draft.set_enabled(self.enabled);
        self.draft.set_read_only(self.read_only);
        self.draft.set_focused(self.focused);
    }

    /// Replace committed value (snapped) and refresh draft.
    pub fn set_value(&mut self, value: Option<f64>) {
        self.value = value.map(|v| self.constraints.clamp_snap(normalize_kind(self.kind, v)));
        self.editing = false;
        self.sync_draft_from_value();
    }

    /// Project from slider bounds + value.
    pub fn set_from_slider(&mut self, bounds: SliderBounds, value: f64) {
        self.constraints = NumberConstraints::from_slider(bounds);
        self.set_value(Some(value));
    }

    /// Value for slider (defaults to min or 0).
    #[must_use]
    pub fn to_slider_value(&self, bounds: SliderBounds) -> f64 {
        let v = self.value.unwrap_or(bounds.min);
        bounds.snap(v)
    }

    fn sync_draft_from_value(&mut self) {
        let text = match self.value {
            None => String::new(),
            Some(v) => format_number(self.kind, v),
        };
        let mut draft = TextInputState::new(text).with_allow_empty(true);
        draft.set_enabled(self.enabled);
        draft.set_read_only(self.read_only);
        draft.set_focused(self.focused);
        self.draft = draft;
    }

    fn begin_edit(&mut self) {
        if !self.editing {
            self.sync_draft_from_value();
            self.editing = true;
        }
        self.sync_draft_gates();
    }

    /// Parse current draft.
    #[must_use]
    pub fn parse_draft(&self) -> NumberParse {
        parse_number_text(self.kind, self.draft.value())
    }

    /// Validity of draft relative to constraints.
    #[must_use]
    pub fn validity(&self) -> NumberValidity {
        match self.parse_draft() {
            NumberParse::Empty => {
                if self.allow_empty {
                    NumberValidity::Empty
                } else {
                    NumberValidity::Invalid
                }
            }
            NumberParse::Intermediate => NumberValidity::Intermediate,
            NumberParse::Invalid => NumberValidity::Invalid,
            NumberParse::Number(n) => {
                if !n.is_finite() {
                    return NumberValidity::Invalid;
                }
                if self.constraints.min.is_some_and(|m| n < m)
                    || self.constraints.max.is_some_and(|m| n > m)
                {
                    NumberValidity::OutOfRange
                } else {
                    NumberValidity::Valid
                }
            }
        }
    }

    /// Whether draft can be committed.
    #[must_use]
    pub fn can_commit(&self) -> bool {
        matches!(
            self.validity(),
            NumberValidity::Valid | NumberValidity::Empty
        )
    }

    /// Commit draft → value. Returns whether committed value changed.
    pub fn commit_draft(&mut self) -> bool {
        let before = self.value;
        match self.parse_draft() {
            NumberParse::Empty if self.allow_empty => {
                self.value = None;
                self.editing = false;
                self.sync_draft_from_value();
            }
            NumberParse::Number(n) if n.is_finite() => {
                let v = self.constraints.clamp_snap(normalize_kind(self.kind, n));
                // Out of range before clamp still commits clamped
                self.value = Some(v);
                self.editing = false;
                self.sync_draft_from_value();
            }
            NumberParse::Intermediate | NumberParse::Invalid | NumberParse::Empty => {
                // Restore draft from committed
                self.editing = false;
                self.sync_draft_from_value();
            }
            NumberParse::Number(_) => {
                self.editing = false;
                self.sync_draft_from_value();
            }
        }
        before != self.value
    }

    /// Cancel edit; restore draft from value.
    pub fn cancel_edit(&mut self) {
        self.editing = false;
        self.sync_draft_from_value();
    }

    /// Increment by `n` steps (commits current draft first if valid).
    pub fn step_by(&mut self, n: i32) -> NumberInputOutcome {
        if !self.enabled || self.read_only || n == 0 {
            return NumberInputOutcome::Ignored;
        }
        // Prefer committed; if editing valid number, use that as base
        let base = match self.parse_draft() {
            NumberParse::Number(v) if v.is_finite() => v,
            _ => self
                .value
                .unwrap_or_else(|| self.constraints.min.unwrap_or(0.0)),
        };
        let next = self.constraints.stepped(base, n);
        let next = normalize_kind(self.kind, next);
        if self.value == Some(next) && !self.editing {
            return NumberInputOutcome::Ignored;
        }
        self.value = Some(next);
        self.editing = false;
        self.sync_draft_from_value();
        NumberInputOutcome::ValueChanged { value: self.value }
    }

    /// Increment.
    pub fn increment(&mut self) -> NumberInputOutcome {
        self.step_by(1)
    }

    /// Decrement.
    pub fn decrement(&mut self) -> NumberInputOutcome {
        self.step_by(-1)
    }

    /// Page step (~10×).
    pub fn page_by(&mut self, forward: bool) -> NumberInputOutcome {
        let pages = 10i32;
        self.step_by(if forward { pages } else { -pages })
    }

    /// Insert text into draft (paste).
    pub fn insert_str(&mut self, text: &str) -> NumberInputOutcome {
        if !self.enabled || self.read_only {
            return NumberInputOutcome::Ignored;
        }
        self.begin_edit();
        let filtered = filter_numeric_paste(self.kind, text);
        match self.draft.insert_str(&filtered) {
            TextInputOutcome::Changed => NumberInputOutcome::Changed,
            _ => NumberInputOutcome::Ignored,
        }
    }

    /// Key adapter.
    pub fn handle_key(&mut self, key: KeyEvent) -> NumberInputOutcome {
        if key.kind == KeyEventKind::Release || !self.enabled {
            return NumberInputOutcome::Ignored;
        }
        self.sync_draft_gates();

        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);

        // Steppers via arrows / page (when not shifting selection in draft)
        if !ctrl && !alt && !shift {
            match key.code {
                KeyCode::Up => return self.increment(),
                KeyCode::Down => return self.decrement(),
                KeyCode::PageUp => return self.page_by(true),
                KeyCode::PageDown => return self.page_by(false),
                _ => {}
            }
        }

        if ctrl && !alt {
            match key.code {
                KeyCode::Char('c' | 'C') => {
                    let text = self.draft.value().to_owned();
                    if text.is_empty() {
                        return NumberInputOutcome::Ignored;
                    }
                    return NumberInputOutcome::ClipboardCopy { text };
                }
                KeyCode::Char('v' | 'V') if !self.read_only => {
                    return NumberInputOutcome::ClipboardPasteRequest;
                }
                _ => {}
            }
        }

        match key.code {
            KeyCode::Enter => {
                if self.commit_draft() {
                    return NumberInputOutcome::Submitted { value: self.value };
                }
                if self.can_commit() {
                    return NumberInputOutcome::Submitted { value: self.value };
                }
                NumberInputOutcome::Ignored
            }
            KeyCode::Esc => {
                self.cancel_edit();
                NumberInputOutcome::Cancelled
            }
            KeyCode::Char('+') if !ctrl && !alt && !self.read_only => self.increment(),
            KeyCode::Char('-')
                if !ctrl
                    && !alt
                    && !self.read_only
                    && self.draft.value().is_empty()
                    && !self.editing =>
            {
                // bare '-' starts negative draft
                self.begin_edit();
                match self.draft.handle_key(key) {
                    TextInputOutcome::Changed => NumberInputOutcome::Changed,
                    _ => NumberInputOutcome::Ignored,
                }
            }
            _ => {
                if self.read_only {
                    // navigation only
                    return match self.draft.handle_key(key) {
                        TextInputOutcome::Changed => NumberInputOutcome::Changed,
                        _ => NumberInputOutcome::Ignored,
                    };
                }
                // Filter illegal chars for kind before insert
                if let KeyCode::Char(c) = key.code {
                    if !ctrl && !alt && !is_allowed_char(self.kind, c, self.draft.value()) {
                        return NumberInputOutcome::Ignored;
                    }
                }
                self.begin_edit();
                match self.draft.handle_key(key) {
                    TextInputOutcome::Changed | TextInputOutcome::Cleared => {
                        NumberInputOutcome::Changed
                    }
                    TextInputOutcome::Submitted(_) => {
                        if self.commit_draft() {
                            NumberInputOutcome::Submitted { value: self.value }
                        } else if self.can_commit() {
                            NumberInputOutcome::Submitted { value: self.value }
                        } else {
                            NumberInputOutcome::Ignored
                        }
                    }
                    TextInputOutcome::Cancelled => {
                        self.cancel_edit();
                        NumberInputOutcome::Cancelled
                    }
                    TextInputOutcome::ClipboardPasteRequest => {
                        NumberInputOutcome::ClipboardPasteRequest
                    }
                    TextInputOutcome::ClipboardCopy { text } => {
                        NumberInputOutcome::ClipboardCopy { text }
                    }
                    TextInputOutcome::ClipboardCut { text } => {
                        NumberInputOutcome::ClipboardCopy { text }
                    }
                    TextInputOutcome::Ignored => NumberInputOutcome::Ignored,
                }
            }
        }
    }

    /// Intent path (step / submit).
    pub fn handle_intent(&mut self, intent: UiIntent) -> NumberInputOutcome {
        if !self.enabled {
            return NumberInputOutcome::Ignored;
        }
        match intent {
            UiIntent::Submit | UiIntent::Activate => {
                if self.commit_draft() || self.can_commit() {
                    NumberInputOutcome::Submitted { value: self.value }
                } else {
                    NumberInputOutcome::Ignored
                }
            }
            UiIntent::Cancel | UiIntent::Close => {
                self.cancel_edit();
                NumberInputOutcome::Cancelled
            }
            UiIntent::Page(crate::interaction::PageMove::Forward) => self.page_by(true),
            UiIntent::Page(crate::interaction::PageMove::Backward) => self.page_by(false),
            UiIntent::Move(crate::interaction::NavigationMove::Next)
            | UiIntent::Move(crate::interaction::NavigationMove::Right) => {
                self.begin_edit();
                match self.draft.handle_intent(intent) {
                    TextInputOutcome::Changed => NumberInputOutcome::Changed,
                    _ => NumberInputOutcome::Ignored,
                }
            }
            UiIntent::Move(crate::interaction::NavigationMove::Previous)
            | UiIntent::Move(crate::interaction::NavigationMove::Left) => {
                self.begin_edit();
                match self.draft.handle_intent(intent) {
                    TextInputOutcome::Changed => NumberInputOutcome::Changed,
                    _ => NumberInputOutcome::Ignored,
                }
            }
            _ => NumberInputOutcome::Ignored,
        }
    }

    /// Mouse: steppers + field.
    pub fn handle_mouse(&mut self, event: MouseEvent) -> NumberInputOutcome {
        if !self.enabled {
            return NumberInputOutcome::Ignored;
        }
        let Some(parts) = self.parts.clone() else {
            return NumberInputOutcome::Ignored;
        };
        if matches!(event.kind, MouseEventKind::Down(MouseButton::Left)) {
            if let Some(dec) = parts.decrement {
                if dec.contains(event.position) {
                    return self.decrement();
                }
            }
            if let Some(inc) = parts.increment {
                if inc.contains(event.position) {
                    return self.increment();
                }
            }
        }
        // Wheel over field
        if parts.field.contains(event.position) {
            match event.kind {
                MouseEventKind::ScrollUp => return self.increment(),
                MouseEventKind::ScrollDown => return self.decrement(),
                _ => {}
            }
            if !self.read_only {
                self.begin_edit();
                self.set_focused(true);
                return match self.draft.handle_mouse(event, parts.field) {
                    TextInputOutcome::Changed => NumberInputOutcome::Changed,
                    _ => NumberInputOutcome::Ignored,
                };
            }
        }
        NumberInputOutcome::Ignored
    }
}

fn normalize_kind(kind: NumberKind, v: f64) -> f64 {
    match kind {
        NumberKind::Integer => clean_float(v.round()),
        NumberKind::Decimal {
            max_fraction_digits,
        } => {
            let d = u32::from(max_fraction_digits.min(12));
            let factor = 10f64.powi(d as i32);
            if !factor.is_finite() || factor == 0.0 {
                return clean_float(v);
            }
            clean_float((v * factor).round() / factor)
        }
    }
}

fn format_number(kind: NumberKind, v: f64) -> String {
    let v = normalize_kind(kind, v);
    match kind {
        NumberKind::Integer => format!("{}", v as i64),
        NumberKind::Decimal {
            max_fraction_digits,
        } => {
            let d = max_fraction_digits.min(12) as usize;
            let s = format!("{v:.d$}");
            // Trim trailing zeros but keep at least one integer digit
            if s.contains('.') {
                let t = s.trim_end_matches('0');
                t.trim_end_matches('.').to_owned()
            } else {
                s
            }
        }
    }
}

fn parse_number_text(kind: NumberKind, text: &str) -> NumberParse {
    let t = text.trim();
    if t.is_empty() {
        return NumberParse::Empty;
    }
    // Intermediate: sole sign or trailing dot
    if matches!(t, "-" | "+" | "." | "-." | "+.") {
        return NumberParse::Intermediate;
    }
    if t.ends_with('.') && kind != NumberKind::Integer {
        let head = &t[..t.len() - 1];
        if head == "-" || head == "+" || head.is_empty() {
            return NumberParse::Intermediate;
        }
        if parse_finite(head).is_some() {
            return NumberParse::Intermediate;
        }
    }
    if kind == NumberKind::Integer && t.contains('.') {
        return NumberParse::Invalid;
    }
    // Multiple dots
    if t.chars().filter(|c| *c == '.').count() > 1 {
        return NumberParse::Invalid;
    }
    match parse_finite(t) {
        Some(n) => {
            if let NumberKind::Decimal {
                max_fraction_digits,
            } = kind
            {
                if let Some(dot) = t.find('.') {
                    let frac = t.len() - dot - 1;
                    if frac > usize::from(max_fraction_digits.min(12)) {
                        return NumberParse::Invalid;
                    }
                }
            }
            NumberParse::Number(n)
        }
        None => {
            // Still typing exponent-like junk → invalid
            if t.chars()
                .all(|c| c.is_ascii_digit() || matches!(c, '+' | '-' | '.'))
            {
                NumberParse::Intermediate
            } else {
                NumberParse::Invalid
            }
        }
    }
}

fn parse_finite(text: &str) -> Option<f64> {
    let n: f64 = text.parse().ok()?;
    if n.is_finite() { Some(n) } else { None }
}

fn is_allowed_char(kind: NumberKind, c: char, draft: &str) -> bool {
    if c.is_ascii_digit() {
        return true;
    }
    match c {
        '+' | '-' => draft.is_empty(),
        '.' => matches!(kind, NumberKind::Decimal { .. }) && !draft.contains('.'),
        _ => false,
    }
}

fn filter_numeric_paste(kind: NumberKind, text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut started = false;
    let mut saw_dot = false;
    for c in text.chars() {
        if c == '\n' || c == '\r' || c.is_control() {
            break;
        }
        if c.is_ascii_digit() {
            out.push(c);
            started = true;
            continue;
        }
        if matches!(c, '+' | '-') && !started && out.is_empty() {
            out.push(c);
            started = true;
            continue;
        }
        if c == '.' && matches!(kind, NumberKind::Decimal { .. }) && !saw_dot {
            out.push('.');
            saw_dot = true;
            started = true;
        }
    }
    out
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Hit geometry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumberInputParts {
    /// Root.
    pub root: Rect,
    /// Editable field.
    pub field: Rect,
    /// Decrement stepper.
    pub decrement: Option<Rect>,
    /// Increment stepper.
    pub increment: Option<Rect>,
    /// Unit suffix area.
    pub unit: Option<Rect>,
    /// Cursor.
    pub cursor: Option<Rect>,
}

/// Numeric input chrome.
#[derive(Debug, Clone, Copy)]
pub struct NumberInput<'a> {
    label: &'a str,
    placeholder: &'a str,
    unit: Option<&'a str>,
    validation: Validation<'a>,
    system: &'a DesignSystem,
    show_steppers: bool,
}

impl<'a> NumberInput<'a> {
    /// Labeled number field.
    #[must_use]
    pub const fn new(label: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            label,
            placeholder: "",
            unit: None,
            validation: Validation::Valid,
            system,
            show_steppers: true,
        }
    }

    /// Placeholder.
    #[must_use]
    pub const fn placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = placeholder;
        self
    }

    /// Unit suffix (`px`, `%`, `ms`).
    #[must_use]
    pub const fn unit(mut self, unit: &'a str) -> Self {
        self.unit = Some(unit);
        self
    }

    /// External validation message.
    #[must_use]
    pub const fn validation(mut self, validation: Validation<'a>) -> Self {
        self.validation = validation;
        self
    }

    /// Show +/- steppers.
    #[must_use]
    pub const fn show_steppers(mut self, on: bool) -> Self {
        self.show_steppers = on;
        self
    }

    /// ASCII steppers `-` / `+`.
    #[must_use]
    /// Paint.
    pub fn paint(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut NumberInputState,
    ) -> NumberInputParts {
        state.parts = None;
        state.sync_draft_gates();
        if area.is_empty() {
            return NumberInputParts {
                root: area,
                field: area,
                decrement: None,
                increment: None,
                unit: None,
                cursor: None,
            };
        }

        let invalid = matches!(
            state.validity(),
            NumberValidity::Invalid | NumberValidity::OutOfRange
        ) || matches!(self.validation, Validation::Invalid(_));
        let field_recipe = self.system.input_recipe(
            if !state.enabled {
                ControlState::Disabled
            } else if state.focused {
                ControlState::Focused
            } else {
                ControlState::Default
            },
            invalid,
        );

        let mut y = area.y;
        if area.height >= 2 && !self.label.is_empty() {
            let mut style = field_recipe.value;
            if state.focused {
                style = style.add_modifier(Modifier::BOLD);
            }
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(self.label, usize::from(area.width)),
                usize::from(area.width),
                style,
            );
            y = y.saturating_add(1);
        }

        let row = Rect::new(
            area.x,
            y.min(area.bottom().saturating_sub(1)),
            area.width,
            1,
        );
        let mut x = row.x;
        let mut right = row.right();
        let mut dec = None;
        let mut inc = None;
        let mut unit_rect = None;

        let steppers = self.show_steppers && row.width >= 8;
        if steppers {
            let step_recipe = self.system.button_recipe(
                ButtonRecipeVariant::Quiet,
                if !state.enabled || state.read_only {
                    ControlState::Disabled
                } else {
                    ControlState::Default
                },
                self.system.junie_theme().surface,
            );
            let step_style = step_recipe.fill.patch(step_recipe.label);
            dec = Some(Rect::new(x, row.y, 1, 1));
            let dec_g = { "−" };
            buffer.set_stringn(x, row.y, dec_g, 1, step_style);
            x = x.saturating_add(2);
            right = right.saturating_sub(2);
            inc = Some(Rect::new(right.saturating_add(1), row.y, 1, 1));
            buffer.set_stringn(right.saturating_add(1), row.y, "+", 1, step_style);
        }

        if let Some(unit) = self.unit {
            if !unit.is_empty() && right > x.saturating_add(3) {
                let uw = display_cols(unit).min(6) as u16;
                right = right.saturating_sub(uw.saturating_add(1));
                unit_rect = Some(Rect::new(right.saturating_add(1), row.y, uw, 1));
                buffer.set_stringn(
                    right.saturating_add(1),
                    row.y,
                    take_display_cols(unit, usize::from(uw)),
                    usize::from(uw),
                    field_recipe.placeholder,
                );
            }
        }

        let field = Rect::new(x, row.y, right.saturating_sub(x).max(1), 1);
        // Paint field via TextInput (no label)
        let placeholder = if self.placeholder.is_empty() {
            match state.kind {
                NumberKind::Integer => "0",
                NumberKind::Decimal { .. } => "0.0",
            }
        } else {
            self.placeholder
        };
        let ext_validation = if invalid && matches!(self.validation, Validation::Valid) {
            match state.validity() {
                NumberValidity::OutOfRange => Validation::Invalid("out of range"),
                NumberValidity::Invalid => Validation::Invalid("invalid number"),
                _ => self.validation,
            }
        } else {
            self.validation
        };
        let input = TextInput::new("", self.system)
            .placeholder(placeholder)
            .validation(ext_validation);
        // Temporarily ensure draft shows committed when not editing
        if !state.editing && !state.focused {
            state.sync_draft_from_value();
        }
        let ti = input.paint(field, buffer, &mut state.draft);

        // Validation row
        if ti.field.y.saturating_add(1) < area.bottom() {
            if let Validation::Invalid(msg) = self.validation {
                crate::widgets::field_message::paint_field_message(
                    buffer,
                    Rect::new(area.x, ti.field.y.saturating_add(1), area.width, 1),
                    self.system,
                    crate::widgets::label::DescriptionKind::Error,
                    msg,
                );
            } else if matches!(
                state.validity(),
                NumberValidity::OutOfRange | NumberValidity::Invalid
            ) && state.focused
            {
                let msg = match state.validity() {
                    NumberValidity::OutOfRange => "out of range",
                    _ => "invalid",
                };
                crate::widgets::field_message::paint_field_message(
                    buffer,
                    Rect::new(area.x, ti.field.y.saturating_add(1), area.width, 1),
                    self.system,
                    crate::widgets::label::DescriptionKind::Error,
                    msg,
                );
            }
        }

        let parts = NumberInputParts {
            root: area,
            field: ti.field,
            decrement: dec,
            increment: inc,
            unit: unit_rect,
            cursor: ti.cursor,
        };
        state.parts = Some(parts.clone());
        let _ = ti;
        parts
    }

    /// Semantic registration.
    pub fn register_semantic<Id, Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        id: Id,
        area: Rect,
        state: &NumberInputState,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if area.is_empty() {
            return;
        }
        let desc = match state.validity() {
            NumberValidity::Empty => "number empty",
            NumberValidity::Intermediate => "number intermediate",
            NumberValidity::Invalid => "number invalid",
            NumberValidity::OutOfRange => "number out of range",
            NumberValidity::Valid => "number",
        };
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Input)
                .label(if self.label.is_empty() {
                    "number"
                } else {
                    self.label
                })
                .description(desc)
                .focusable(state.enabled)
                .disabled(!state.enabled)
                .state(SemanticState {
                    selected: state.focused,
                    invalid: matches!(
                        state.validity(),
                        NumberValidity::Invalid | NumberValidity::OutOfRange
                    ) || matches!(self.validation, Validation::Invalid(_)),
                    ..Default::default()
                }),
        );
    }
}

impl StatefulWidget for &NumberInput<'_> {
    type State = NumberInputState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        let _ = self.paint(area, buffer, state);
    }
}

impl StatefulWidget for NumberInput<'_> {
    type State = NumberInputState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::RolePalette;

    #[test]
    fn draft_separate_from_committed() {
        let mut state = NumberInputState::new().with_value(10.0);
        assert_eq!(state.value(), Some(10.0));
        state.set_focused(true);
        for c in "99".chars() {
            let _ = state.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        // Committed stays until commit
        assert_eq!(state.value(), Some(10.0));
        assert_eq!(state.draft_text(), "1099"); // draft started from "10" then typed
    }

    #[test]
    fn commit_and_submit() {
        let mut state = NumberInputState::new().with_value(1.0);
        state.set_focused(true);
        state.draft = TextInputState::new("42").with_allow_empty(true);
        state.editing = true;
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            NumberInputOutcome::Submitted { value: Some(42.0) }
        );
        assert_eq!(state.value(), Some(42.0));
    }

    #[test]
    fn intermediate_trailing_dot() {
        let mut state = NumberInputState::new().with_kind(NumberKind::decimal2());
        state.set_focused(true);
        let _ = state.handle_key(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE));
        let _ = state.handle_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE));
        let _ = state.handle_key(KeyEvent::new(KeyCode::Char('.'), KeyModifiers::NONE));
        assert_eq!(state.parse_draft(), NumberParse::Intermediate);
        assert_eq!(state.validity(), NumberValidity::Intermediate);
        assert_eq!(state.value(), None); // never committed
    }

    #[test]
    fn integer_rejects_dot() {
        let mut state = NumberInputState::new().with_kind(NumberKind::Integer);
        state.set_focused(true);
        let _ = state.handle_key(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE));
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Char('.'), KeyModifiers::NONE)),
            NumberInputOutcome::Ignored
        );
        assert_eq!(state.draft_text(), "1");
    }

    #[test]
    fn min_max_step_and_steppers() {
        let mut state = NumberInputState::new()
            .with_constraints(NumberConstraints::bounded(0.0, 10.0, 2.0))
            .with_value(4.0);
        assert_eq!(
            state.increment(),
            NumberInputOutcome::ValueChanged { value: Some(6.0) }
        );
        assert_eq!(
            state.increment(),
            NumberInputOutcome::ValueChanged { value: Some(8.0) }
        );
        assert_eq!(
            state.increment(),
            NumberInputOutcome::ValueChanged { value: Some(10.0) }
        );
        assert_eq!(state.increment(), NumberInputOutcome::Ignored);
        assert_eq!(
            state.decrement(),
            NumberInputOutcome::ValueChanged { value: Some(8.0) }
        );
    }

    #[test]
    fn overflow_clamps_to_max() {
        let mut state =
            NumberInputState::new().with_constraints(NumberConstraints::bounded(0.0, 100.0, 1.0));
        state.set_value(Some(1e308));
        assert_eq!(state.value(), Some(100.0));
    }

    #[test]
    fn empty_allowed_and_disallowed() {
        let mut state = NumberInputState::new().with_allow_empty(true);
        state.set_focused(true);
        assert_eq!(state.validity(), NumberValidity::Empty);
        assert!(state.commit_draft() || state.can_commit());

        let mut state = NumberInputState::new().with_allow_empty(false);
        state.set_focused(true);
        assert_eq!(state.validity(), NumberValidity::Invalid);
    }

    #[test]
    fn cancel_restores_committed() {
        let mut state = NumberInputState::new().with_value(7.0);
        state.set_focused(true);
        let _ = state.handle_key(KeyEvent::new(KeyCode::Char('9'), KeyModifiers::NONE));
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            NumberInputOutcome::Cancelled
        );
        assert_eq!(state.value(), Some(7.0));
        assert_eq!(state.draft_text(), "7");
    }

    #[test]
    fn slider_roundtrip() {
        let bounds = SliderBounds::new(0.0, 100.0, 5.0);
        let mut state = NumberInputState::new().with_kind(NumberKind::Integer);
        state.set_from_slider(bounds, 47.0);
        assert_eq!(state.value(), Some(45.0)); // snapped
        assert_eq!(state.to_slider_value(bounds), 45.0);
    }

    #[test]
    fn paint_steppers_and_unit() {
        let system = DesignSystem::from_palette(RolePalette::default());
        let mut state = NumberInputState::new().with_value(3.0);
        state.set_focused(true);
        let area = Rect::new(0, 0, 28, 2);
        let mut buf = Buffer::empty(area);
        let parts = NumberInput::new("Opacity", &system)
            .unit("%")
            .paint(area, &mut buf, &mut state);
        assert!(parts.decrement.is_some());
        assert!(parts.increment.is_some());
        assert!(parts.unit.is_some());
    }

    #[test]
    fn mouse_stepper_hits() {
        let system = DesignSystem::default();
        let mut state = NumberInputState::new()
            .with_constraints(NumberConstraints::bounded(0.0, 5.0, 1.0))
            .with_value(1.0);
        state.set_focused(true);
        let area = Rect::new(0, 0, 20, 2);
        let mut buf = Buffer::empty(area);
        let parts = NumberInput::new("N", &system).paint(area, &mut buf, &mut state);
        let dec = parts.decrement.unwrap();
        assert_eq!(
            state.handle_mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: ratatui_core::layout::Position::new(dec.x, dec.y),
                modifiers: KeyModifiers::NONE,
            }),
            NumberInputOutcome::ValueChanged { value: Some(0.0) }
        );
    }

    #[test]
    fn semantic_no_panic() {
        let system = DesignSystem::default();
        let state = NumberInputState::new().with_value(1.0);
        let mut scene = SemanticScene::<&str, ()>::default();
        NumberInput::new("Count", &system).register_semantic(
            &mut scene,
            "n",
            Rect::new(0, 0, 20, 2),
            &state,
        );
        assert!(scene.get(&"n").is_some());
    }

    #[test]
    fn fuzz_keys_keep_finite_commit() {
        let mut state = NumberInputState::new()
            .with_kind(NumberKind::decimal2())
            .with_constraints(NumberConstraints::bounded(-1000.0, 1000.0, 0.5));
        state.set_focused(true);
        let keys = [
            KeyEvent::new(KeyCode::Char('9'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('.'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('5'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('-'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
        ];
        for key in keys.iter().cycle().take(40) {
            let _ = state.handle_key(*key);
            if let Some(v) = state.value() {
                assert!(v.is_finite());
                assert!((-1000.0..=1000.0).contains(&v));
            }
        }
    }

    #[test]
    fn paint_hot_path() {
        let system = DesignSystem::default();
        let mut state = NumberInputState::new().with_value(42.0);
        state.set_focused(true);
        let area = Rect::new(0, 0, 24, 2);
        let mut buf = Buffer::empty(area);
        let w = NumberInput::new("N", &system);
        for _ in 0..200 {
            let _ = w.paint(area, &mut buf, &mut state);
        }
    }
}
