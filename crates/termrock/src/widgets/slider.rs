// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Slider and RangeSlider — terminal-native bounded numeric controls.
//!
//! **Mission.** Settings and filters need continuous or stepped values with a
//! visible track and handle. Progress bars are **read-only**; these widgets are
//! interactive.
//!
//! **vs [`ProgressBar`](crate::widgets::ProgressBar).** ProgressBar shows
//! completion; Slider edits a bounded value.
//!
//! **Precision.** Prefer a paired value field (built-in numeric face / edit, or
//! host TextInput) when exact precision matters. Tiny widths fall back to
//! numeric display automatically.
//!
//! Research: Radix Slider, TUI volume controls, btop, Textual sliders.
use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Modifier,
};

use crate::input::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use crate::interaction::{
    EventResult, SemanticNode, SemanticRole, SemanticScene, SemanticState, UiIntent,
    default_button_intent,
};
use crate::style::{ControlState, DesignSystem, Role};
use crate::text::{display_cols, take_display_cols};

/// Minimum track cells before falling back to numeric-only face.
pub const SLIDER_MIN_TRACK: u16 = 6;
/// Width at or below which the control paints numeric-only (no track).
pub const SLIDER_NUMERIC_FALLBACK_WIDTH: u16 = 10;

// ── Shared model ────────────────────────────────────────────────────────────

/// Orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SliderOrientation {
    /// Left → right (default).
    #[default]
    Horizontal,
    /// Bottom → top (volume / side panels).
    Vertical,
}

impl SliderOrientation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
        }
    }
}

/// Inclusive bounds + step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SliderBounds {
    /// Minimum value.
    pub min: f64,
    /// Maximum value.
    pub max: f64,
    /// Step size (> 0). Values snap to `min + n * step`.
    pub step: f64,
}

impl SliderBounds {
    /// `min..=max` with step.
    #[must_use]
    pub fn new(min: f64, max: f64, step: f64) -> Self {
        let step = if step.is_finite() && step > 0.0 {
            step
        } else {
            1.0
        };
        let (min, max) = if min <= max { (min, max) } else { (max, min) };
        Self { min, max, step }
    }

    /// Integer-friendly 0..=100 step 1.
    #[must_use]
    pub const fn percent() -> Self {
        Self {
            min: 0.0,
            max: 100.0,
            step: 1.0,
        }
    }

    /// Span.
    #[must_use]
    pub fn span(self) -> f64 {
        (self.max - self.min).max(self.step)
    }

    /// Clamp + snap to step.
    #[must_use]
    pub fn snap(self, value: f64) -> f64 {
        if !value.is_finite() {
            return self.min;
        }
        let v = value.clamp(self.min, self.max);
        let n = ((v - self.min) / self.step).round();
        let snapped = self.min + n * self.step;
        // Avoid float drift past max
        if snapped > self.max {
            self.max
        } else if snapped < self.min {
            self.min
        } else {
            // Clean near-integers
            if (snapped - snapped.round()).abs() < 1e-9 {
                snapped.round()
            } else {
                snapped
            }
        }
    }

    /// Fraction 0..=1.
    #[must_use]
    pub fn fraction(self, value: f64) -> f64 {
        let v = self.snap(value);
        if self.span() <= 0.0 {
            return 0.0;
        }
        ((v - self.min) / self.span()).clamp(0.0, 1.0)
    }

    /// Value from fraction.
    #[must_use]
    pub fn from_fraction(self, fraction: f64) -> f64 {
        let f = if fraction.is_finite() {
            fraction.clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.snap(self.min + f * self.span())
    }

    /// Page step: max(step, ~10% of span snapped).
    #[must_use]
    pub fn page_step(self) -> f64 {
        let raw = self.span() * 0.1;
        let pages = (raw / self.step).max(1.0).round() * self.step;
        pages.max(self.step)
    }
}

/// Optional mark on the track.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SliderMark<'a> {
    /// Value position.
    pub value: f64,
    /// Optional short label (marks row).
    pub label: Option<&'a str>,
}

impl<'a> SliderMark<'a> {
    /// Mark without label.
    #[must_use]
    pub const fn new(value: f64) -> Self {
        Self { value, label: None }
    }

    /// Mark with label.
    #[must_use]
    pub const fn labeled(value: f64, label: &'a str) -> Self {
        Self {
            value,
            label: Some(label),
        }
    }
}

// ── Track painting helpers ──────────────────────────────────────────────────

fn mono(system: &DesignSystem, colorless: bool) -> bool {
    colorless || system.mono()
}

fn handle_glyph(system: &DesignSystem, colorless: bool) -> &'static str {
    if mono(system, colorless) { "*" } else { "●" }
}

fn fill_glyph(system: &DesignSystem, colorless: bool) -> &'static str {
    if mono(system, colorless) { "=" } else { "━" }
}

fn empty_glyph(system: &DesignSystem, colorless: bool) -> &'static str {
    if mono(system, colorless) { "-" } else { "─" }
}

fn format_value(value: f64) -> String {
    if (value - value.round()).abs() < 1e-9 {
        format!("{}", value.round() as i64)
    } else {
        // Trim trailing zeros
        let s = format!("{value:.4}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn parse_edit(buf: &str, bounds: SliderBounds) -> Option<f64> {
    let t = buf.trim();
    if t.is_empty() {
        return None;
    }
    t.parse::<f64>().ok().map(|v| bounds.snap(v))
}

// ── Single Slider ───────────────────────────────────────────────────────────

/// Paint geometry for [`Slider`].
#[derive(Debug, Clone, PartialEq)]
pub struct SliderParts {
    /// Full root.
    pub root: Rect,
    /// Track rect (empty when numeric-only).
    pub track: Option<Rect>,
    /// Handle cell.
    pub handle: Option<Rect>,
    /// Value text rect.
    pub value_area: Option<Rect>,
    /// Label rect.
    pub label_area: Option<Rect>,
    /// Whether numeric-only fallback is active.
    pub numeric_only: bool,
}

/// Outcomes for a single slider.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum SliderOutcome {
    /// No change.
    Ignored,
    /// Value changed (drag, key, click).
    ValueChanged {
        /// New snapped value.
        value: f64,
    },
    /// Numeric edit started.
    EditStarted,
    /// Numeric edit committed.
    EditCommitted {
        /// Parsed value.
        value: f64,
    },
    /// Numeric edit cancelled.
    EditCancelled,
}

/// Runtime state for [`Slider`].
#[derive(Debug, Clone, PartialEq)]
pub struct SliderState {
    /// Current value (host-projected; optimistically updated).
    pub value: f64,
    /// Focus.
    pub focused: bool,
    /// Enabled.
    pub enabled: bool,
    /// Read-only.
    pub read_only: bool,
    /// Direct numeric entry active.
    pub editing: bool,
    /// Edit buffer while editing.
    pub edit_buffer: String,
    /// Pointer dragging handle/track.
    pub dragging: bool,
    /// Hover.
    pub hovered: bool,
    /// Last parts.
    pub parts: Option<SliderParts>,
}

impl Default for SliderState {
    fn default() -> Self {
        Self::new(0.0)
    }
}

impl SliderState {
    /// Initial value.
    #[must_use]
    pub fn new(value: f64) -> Self {
        Self {
            value,
            focused: false,
            enabled: true,
            read_only: false,
            editing: false,
            edit_buffer: String::new(),
            dragging: false,
            hovered: false,
            parts: None,
        }
    }

    /// Focus.
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on;
        if !on {
            self.editing = false;
            self.edit_buffer.clear();
            self.dragging = false;
        }
    }

    /// Enabled.
    pub const fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
    }

    /// Read-only.
    pub fn set_read_only(&mut self, on: bool) {
        self.read_only = on;
        if on {
            self.editing = false;
            self.dragging = false;
        }
    }

    /// Controlled value.
    pub const fn set_value(&mut self, value: f64) {
        self.value = value;
    }

    /// Whether activatable.
    #[must_use]
    pub const fn can_edit(&self) -> bool {
        self.enabled && !self.read_only
    }
}

/// Single-thumb bounded slider.
///
/// Pair with a visible value (default) or host field when precision matters.
#[derive(Debug, Clone, Copy)]
pub struct Slider<'a> {
    bounds: SliderBounds,
    system: &'a DesignSystem,
    label: Option<&'a str>,
    marks: &'a [SliderMark<'a>],
    orientation: SliderOrientation,
    show_value: bool,
    page_step: Option<f64>,
    colorless: bool,
}

/// One chrome answer for both sliders.
///
/// Slider and RangeSlider disagreed: each resolved its own track, fill and
/// thumb styles, and both flooded the *track fill* with `Role::Accent` when
/// focused — a focused slider was a bar of brand green with a thumb hidden
/// inside it. The fill is data, so it wears a series role; the accent is spent
/// on the one cell the operator is moving (plans/008 Step 4).
#[derive(Debug, Clone, Copy)]
pub(crate) struct SliderChrome {
    /// Unfilled track.
    pub(crate) track: ratatui_core::style::Style,
    /// Filled portion of the track.
    pub(crate) fill: ratatui_core::style::Style,
    /// The thumb cell.
    pub(crate) thumb: ratatui_core::style::Style,
}

pub(crate) fn slider_chrome(system: &DesignSystem, enabled: bool, active: bool) -> SliderChrome {
    let recipe = system.input_recipe(
        if !enabled {
            ControlState::Disabled
        } else if active {
            ControlState::Focused
        } else {
            ControlState::Default
        },
        false,
    );
    let thumb = if active { recipe.cursor } else { recipe.value }.add_modifier(Modifier::BOLD);
    SliderChrome {
        track: recipe.border,
        fill: if enabled {
            system.style(Role::ChartSeries1)
        } else {
            recipe.value
        },
        thumb,
    }
}

impl<'a> Slider<'a> {
    /// Bounds + design system.
    #[must_use]
    pub const fn new(bounds: SliderBounds, system: &'a DesignSystem) -> Self {
        Self {
            bounds,
            system,
            label: None,
            marks: &[],
            orientation: SliderOrientation::Horizontal,
            show_value: true,
            page_step: None,
            colorless: false,
        }
    }

    /// Label.
    #[must_use]
    pub const fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    /// Marks.
    #[must_use]
    pub const fn marks(mut self, marks: &'a [SliderMark<'a>]) -> Self {
        self.marks = marks;
        self
    }

    /// Orientation.
    #[must_use]
    pub const fn orientation(mut self, orientation: SliderOrientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Vertical track.
    #[must_use]
    pub const fn vertical(mut self) -> Self {
        self.orientation = SliderOrientation::Vertical;
        self
    }

    /// Show trailing/current value text (default true).
    #[must_use]
    pub const fn show_value(mut self, on: bool) -> Self {
        self.show_value = on;
        self
    }

    /// Override page step.
    #[must_use]
    pub const fn page_step(mut self, step: f64) -> Self {
        self.page_step = Some(step);
        self
    }

    /// Monochrome emphasis.
    #[must_use]
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    fn page(&self) -> f64 {
        self.page_step
            .filter(|s| s.is_finite() && *s > 0.0)
            .unwrap_or_else(|| self.bounds.page_step())
    }

    fn numeric_only(&self, area: Rect) -> bool {
        match self.orientation {
            SliderOrientation::Horizontal => area.width < SLIDER_NUMERIC_FALLBACK_WIDTH,
            SliderOrientation::Vertical => area.height < SLIDER_NUMERIC_FALLBACK_WIDTH,
        }
    }

    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut SliderState) -> SliderParts {
        state.parts = None;
        if area.is_empty() {
            return SliderParts {
                root: area,
                track: None,
                handle: None,
                value_area: None,
                label_area: None,
                numeric_only: true,
            };
        }
        let value = self.bounds.snap(state.value);
        state.value = value;

        if self.numeric_only(area) || state.editing {
            return self.paint_numeric(area, buffer, state, value);
        }

        match self.orientation {
            SliderOrientation::Horizontal => self.paint_horizontal(area, buffer, state, value),
            SliderOrientation::Vertical => self.paint_vertical(area, buffer, state, value),
        }
    }

    fn paint_numeric(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut SliderState,
        value: f64,
    ) -> SliderParts {
        let mut y = area.y;
        let mut label_area = None;
        if let Some(lab) = self.label {
            if !lab.is_empty() && y < area.bottom() {
                let text = take_display_cols(lab, usize::from(area.width));
                let style = self.label_style(state);
                buffer.set_stringn(area.x, y, &text, usize::from(area.width), style);
                label_area = Some(Rect::new(
                    area.x,
                    y,
                    display_cols(&text).min(usize::from(area.width)) as u16,
                    1,
                ));
                y = y.saturating_add(1);
            }
        }
        let face = if state.editing {
            format!("{}_", state.edit_buffer)
        } else {
            format_value(value)
        };
        let text = take_display_cols(&face, usize::from(area.width));
        let style = self.value_style(state);
        if y < area.bottom() {
            buffer.set_stringn(area.x, y, &text, usize::from(area.width), style);
        }
        let value_area = Some(Rect::new(
            area.x,
            y.min(area.bottom().saturating_sub(1)),
            display_cols(&text).min(usize::from(area.width)) as u16,
            1,
        ));
        let parts = SliderParts {
            root: area,
            track: None,
            handle: None,
            value_area,
            label_area,
            numeric_only: true,
        };
        state.parts = Some(parts.clone());
        parts
    }

    fn paint_horizontal(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut SliderState,
        value: f64,
    ) -> SliderParts {
        let mut y = area.y;
        let mut label_area = None;
        if let Some(lab) = self.label {
            if !lab.is_empty() && area.height >= 2 {
                let text = take_display_cols(lab, usize::from(area.width));
                buffer.set_stringn(
                    area.x,
                    y,
                    &text,
                    usize::from(area.width),
                    self.label_style(state),
                );
                label_area = Some(Rect::new(
                    area.x,
                    y,
                    display_cols(&text).min(usize::from(area.width)) as u16,
                    1,
                ));
                y = y.saturating_add(1);
            }
        }

        let value_str = if self.show_value {
            format_value(value)
        } else {
            String::new()
        };
        let value_w = if value_str.is_empty() {
            0u16
        } else {
            (display_cols(&value_str) as u16).saturating_add(1)
        };
        let track_w = area
            .width
            .saturating_sub(value_w)
            .max(SLIDER_MIN_TRACK.min(area.width));
        let track = Rect::new(area.x, y.min(area.bottom().saturating_sub(1)), track_w, 1);
        let frac = self.bounds.fraction(value);
        let handle_idx = if track_w <= 1 {
            0u16
        } else {
            ((frac * f64::from(track_w.saturating_sub(1))).round() as u16).min(track_w - 1)
        };

        let fill = fill_glyph(self.system, self.colorless);
        let empty = empty_glyph(self.system, self.colorless);
        let handle = handle_glyph(self.system, self.colorless);
        let track_style = self.track_style(state);
        let fill_style = self.fill_style(state);

        for i in 0..track_w {
            let ch = if i == handle_idx {
                handle
            } else if i < handle_idx {
                fill
            } else {
                empty
            };
            let style = if i == handle_idx {
                self.handle_style(state)
            } else if i < handle_idx {
                fill_style
            } else {
                track_style
            };
            buffer.set_stringn(track.x.saturating_add(i), track.y, ch, 1, style);
        }

        // No-color tracks stay readable via the =/-/* glyphs alone.
        let mut value_area = None;
        if !value_str.is_empty() && track.right() < area.right() {
            let vx = track
                .right()
                .saturating_add(1)
                .min(area.right().saturating_sub(1));
            let vw = area.right().saturating_sub(vx);
            let text = take_display_cols(&value_str, usize::from(vw));
            buffer.set_stringn(vx, track.y, &text, usize::from(vw), self.value_style(state));
            value_area = Some(Rect::new(vx, track.y, display_cols(&text) as u16, 1));
        }

        // Marks row
        if area.height >= (if label_area.is_some() { 3 } else { 2 }) && !self.marks.is_empty() {
            let my = track.y.saturating_add(1);
            if my < area.bottom() {
                for m in self.marks {
                    let f = self.bounds.fraction(m.value);
                    let ix = if track_w <= 1 {
                        0
                    } else {
                        ((f * f64::from(track_w.saturating_sub(1))).round() as u16).min(track_w - 1)
                    };
                    let gx = track.x.saturating_add(ix);
                    let mark_ch = if mono(self.system, self.colorless) {
                        "|"
                    } else {
                        "┊"
                    };
                    buffer.set_stringn(gx, my, mark_ch, 1, self.system.style(Role::TextMuted));
                    if let Some(lab) = m.label {
                        if !lab.is_empty() && gx.saturating_add(1) < area.right() {
                            let t = take_display_cols(lab, 4);
                            buffer.set_stringn(
                                gx,
                                my,
                                &t,
                                4.min(usize::from(area.right().saturating_sub(gx))),
                                self.system.style(Role::TextMuted),
                            );
                        }
                    }
                }
            }
        }

        let handle_rect = Some(Rect::new(track.x.saturating_add(handle_idx), track.y, 1, 1));
        let parts = SliderParts {
            root: area,
            track: Some(track),
            handle: handle_rect,
            value_area,
            label_area,
            numeric_only: false,
        };
        state.parts = Some(parts.clone());
        parts
    }

    fn paint_vertical(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut SliderState,
        value: f64,
    ) -> SliderParts {
        // Bottom = min, top = max
        let track_h = area.height.max(1);
        let track = Rect::new(area.x, area.y, 1.min(area.width), track_h);
        let frac = self.bounds.fraction(value);
        // handle from bottom
        let handle_from_bottom = if track_h <= 1 {
            0u16
        } else {
            ((frac * f64::from(track_h.saturating_sub(1))).round() as u16).min(track_h - 1)
        };
        let handle_y = track
            .bottom()
            .saturating_sub(1)
            .saturating_sub(handle_from_bottom);

        let fill = fill_glyph(self.system, self.colorless);
        let empty = empty_glyph(self.system, self.colorless);
        let handle = handle_glyph(self.system, self.colorless);

        for row in 0..track_h {
            let y = track.y.saturating_add(row);
            let from_bottom = track.bottom().saturating_sub(1).saturating_sub(y);
            let ch = if y == handle_y {
                handle
            } else if from_bottom < handle_from_bottom {
                fill
            } else {
                empty
            };
            let style = if y == handle_y {
                self.handle_style(state)
            } else if from_bottom < handle_from_bottom {
                self.fill_style(state)
            } else {
                self.track_style(state)
            };
            buffer.set_stringn(track.x, y, ch, 1, style);
        }

        let mut value_area = None;
        if self.show_value && area.width >= 3 {
            let vs = format_value(value);
            let text = take_display_cols(&vs, usize::from(area.width.saturating_sub(2)));
            buffer.set_stringn(
                area.x.saturating_add(2),
                area.y,
                &text,
                usize::from(area.width.saturating_sub(2)),
                self.value_style(state),
            );
            value_area = Some(Rect::new(
                area.x.saturating_add(2),
                area.y,
                display_cols(&text) as u16,
                1,
            ));
        }

        let parts = SliderParts {
            root: area,
            track: Some(track),
            handle: Some(Rect::new(track.x, handle_y, 1, 1)),
            value_area,
            label_area: None,
            numeric_only: false,
        };
        state.parts = Some(parts.clone());
        parts
    }

    fn label_style(&self, state: &SliderState) -> ratatui_core::style::Style {
        let recipe = self.system.input_recipe(
            if !state.enabled {
                ControlState::Disabled
            } else if state.focused {
                ControlState::Focused
            } else {
                ControlState::Default
            },
            false,
        );
        if state.focused {
            recipe.value.add_modifier(Modifier::BOLD)
        } else {
            recipe.value
        }
    }

    fn value_style(&self, state: &SliderState) -> ratatui_core::style::Style {
        let recipe = self.system.input_recipe(
            if !state.enabled {
                ControlState::Disabled
            } else if state.focused {
                ControlState::Focused
            } else {
                ControlState::Default
            },
            state.editing,
        );
        if state.editing || state.focused {
            recipe.value.add_modifier(Modifier::BOLD)
        } else {
            recipe.placeholder
        }
    }

    fn chrome(&self, state: &SliderState) -> SliderChrome {
        slider_chrome(self.system, state.enabled, state.focused || state.dragging)
    }

    fn track_style(&self, state: &SliderState) -> ratatui_core::style::Style {
        self.chrome(state).track
    }

    fn fill_style(&self, state: &SliderState) -> ratatui_core::style::Style {
        self.chrome(state).fill
    }

    fn handle_style(&self, state: &SliderState) -> ratatui_core::style::Style {
        self.chrome(state).thumb
    }

    fn set_value(&self, state: &mut SliderState, value: f64) -> SliderOutcome {
        if !state.can_edit() {
            return SliderOutcome::Ignored;
        }
        let next = self.bounds.snap(value);
        if (next - state.value).abs() < 1e-12 {
            return SliderOutcome::Ignored;
        }
        state.value = next;
        SliderOutcome::ValueChanged { value: next }
    }

    /// Keys: arrows step, Page page, Home/End, Enter edit, digits.
    pub fn handle_key(&self, state: &mut SliderState, key: KeyEvent) -> SliderOutcome {
        if !state.focused || !key.is_press() {
            return SliderOutcome::Ignored;
        }
        if !state.can_edit() && !state.editing {
            return SliderOutcome::Ignored;
        }

        if state.editing {
            return self.handle_edit_key(state, key);
        }

        // Start edit on Enter or printable digit / minus / dot
        if matches!(key.code, KeyCode::Enter) {
            state.editing = true;
            state.edit_buffer = format_value(state.value);
            return SliderOutcome::EditStarted;
        }
        if let KeyCode::Char(c) = key.code {
            if c.is_ascii_digit() || c == '-' || c == '.' {
                state.editing = true;
                state.edit_buffer = String::new();
                state.edit_buffer.push(c);
                return SliderOutcome::EditStarted;
            }
        }

        let page = self.page();
        let step = self.bounds.step;
        match key.code {
            KeyCode::Left | KeyCode::Down | KeyCode::Char('h') | KeyCode::Char('-') => {
                self.set_value(state, state.value - step)
            }
            KeyCode::Right
            | KeyCode::Up
            | KeyCode::Char('l')
            | KeyCode::Char('+')
            | KeyCode::Char('=') => self.set_value(state, state.value + step),
            KeyCode::PageDown => self.set_value(state, state.value - page),
            KeyCode::PageUp => self.set_value(state, state.value + page),
            KeyCode::Home => self.set_value(state, self.bounds.min),
            KeyCode::End => self.set_value(state, self.bounds.max),
            _ => {
                if let Some(intent) = default_button_intent(key) {
                    if matches!(intent, UiIntent::Activate | UiIntent::Submit) {
                        state.editing = true;
                        state.edit_buffer = format_value(state.value);
                        return SliderOutcome::EditStarted;
                    }
                }
                SliderOutcome::Ignored
            }
        }
    }

    fn handle_edit_key(&self, state: &mut SliderState, key: KeyEvent) -> SliderOutcome {
        match key.code {
            KeyCode::Esc => {
                state.editing = false;
                state.edit_buffer.clear();
                SliderOutcome::EditCancelled
            }
            KeyCode::Enter => {
                if let Some(v) = parse_edit(&state.edit_buffer, self.bounds) {
                    state.editing = false;
                    state.edit_buffer.clear();
                    state.value = v;
                    SliderOutcome::EditCommitted { value: v }
                } else {
                    state.editing = false;
                    state.edit_buffer.clear();
                    SliderOutcome::EditCancelled
                }
            }
            KeyCode::Backspace => {
                state.edit_buffer.pop();
                SliderOutcome::Ignored
            }
            KeyCode::Char(c)
                if c.is_ascii_digit() || c == '-' || c == '.' || c == 'e' || c == 'E' =>
            {
                if state.edit_buffer.len() < 24 {
                    state.edit_buffer.push(c);
                }
                SliderOutcome::Ignored
            }
            _ => SliderOutcome::Ignored,
        }
    }

    /// Pointer: drag handle / click track; wheel steps.
    pub fn handle_mouse(&self, state: &mut SliderState, event: MouseEvent) -> SliderOutcome {
        if !state.can_edit() {
            return SliderOutcome::Ignored;
        }
        let Some(parts) = state.parts.clone() else {
            return SliderOutcome::Ignored;
        };
        if state.editing {
            return SliderOutcome::Ignored;
        }

        match event.kind {
            MouseEventKind::Moved | MouseEventKind::Drag(_) => {
                state.hovered = parts.root.contains(event.position);
                if state.dragging {
                    return self.value_from_pointer(state, &parts, event.position);
                }
                SliderOutcome::Ignored
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if parts.root.contains(event.position) {
                    state.focused = true;
                    state.dragging = true;
                    return self.value_from_pointer(state, &parts, event.position);
                }
                SliderOutcome::Ignored
            }
            MouseEventKind::Up(MouseButton::Left) => {
                state.dragging = false;
                SliderOutcome::Ignored
            }
            MouseEventKind::ScrollDown => {
                if parts.root.contains(event.position) {
                    state.focused = true;
                    return self.set_value(state, state.value - self.bounds.step);
                }
                SliderOutcome::Ignored
            }
            MouseEventKind::ScrollUp => {
                if parts.root.contains(event.position) {
                    state.focused = true;
                    return self.set_value(state, state.value + self.bounds.step);
                }
                SliderOutcome::Ignored
            }
            _ => SliderOutcome::Ignored,
        }
    }

    fn value_from_pointer(
        &self,
        state: &mut SliderState,
        parts: &SliderParts,
        pos: Position,
    ) -> SliderOutcome {
        if parts.numeric_only {
            return SliderOutcome::Ignored;
        }
        let Some(track) = parts.track else {
            return SliderOutcome::Ignored;
        };
        match self.orientation {
            SliderOrientation::Horizontal => {
                if track.width == 0 {
                    return SliderOutcome::Ignored;
                }
                let x = pos.x.clamp(track.x, track.right().saturating_sub(1));
                let idx = x.saturating_sub(track.x);
                let frac = if track.width <= 1 {
                    0.0
                } else {
                    f64::from(idx) / f64::from(track.width.saturating_sub(1))
                };
                self.set_value(state, self.bounds.from_fraction(frac))
            }
            SliderOrientation::Vertical => {
                if track.height == 0 {
                    return SliderOutcome::Ignored;
                }
                let y = pos.y.clamp(track.y, track.bottom().saturating_sub(1));
                // bottom = min
                let from_bottom = track.bottom().saturating_sub(1).saturating_sub(y);
                let frac = if track.height <= 1 {
                    0.0
                } else {
                    f64::from(from_bottom) / f64::from(track.height.saturating_sub(1))
                };
                self.set_value(state, self.bounds.from_fraction(frac))
            }
        }
    }

    /// EventResult wrapper.
    pub fn handle_key_result(
        &self,
        state: &mut SliderState,
        key: KeyEvent,
    ) -> EventResult<SliderOutcome> {
        match self.handle_key(state, key) {
            SliderOutcome::Ignored => EventResult::ignored(),
            other => EventResult::emit(other),
        }
    }

    /// Semantic.
    pub fn register_semantic<Id, Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        id: Id,
        area: Rect,
        state: &SliderState,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if area.is_empty() {
            return;
        }
        let label = self.label.unwrap_or("slider");
        let desc = format_value(state.value);
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Progress)
                .label(label)
                .description(&desc)
                .focusable(state.can_edit())
                .disabled(!state.enabled)
                .state(SemanticState {
                    selected: state.focused,
                    busy: state.editing,
                    pressed: state.dragging,
                    ..Default::default()
                }),
        );
    }
}

// ── RangeSlider ─────────────────────────────────────────────────────────────

/// Which thumb is active for keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum RangeThumb {
    /// Lower bound.
    #[default]
    Start,
    /// Upper bound.
    End,
}

impl RangeThumb {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::End => "end",
        }
    }

    /// Toggle.
    #[must_use]
    pub const fn other(self) -> Self {
        match self {
            Self::Start => Self::End,
            Self::End => Self::Start,
        }
    }
}

/// Range paint geometry.
#[derive(Debug, Clone, PartialEq)]
pub struct RangeSliderParts {
    /// Root.
    pub root: Rect,
    /// Track.
    pub track: Option<Rect>,
    /// Start handle.
    pub start_handle: Option<Rect>,
    /// End handle.
    pub end_handle: Option<Rect>,
    /// Value text.
    pub value_area: Option<Rect>,
    /// Numeric-only fallback.
    pub numeric_only: bool,
}

/// Range outcomes.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum RangeSliderOutcome {
    /// No change.
    Ignored,
    /// Range changed.
    ValueChanged {
        /// Lower.
        start: f64,
        /// Upper.
        end: f64,
    },
    /// Active thumb switched.
    ThumbChanged {
        /// Active thumb.
        thumb: RangeThumb,
    },
}

/// Range slider state.
#[derive(Debug, Clone, PartialEq)]
pub struct RangeSliderState {
    /// Lower value.
    pub start: f64,
    /// Upper value.
    pub end: f64,
    /// Keyboard focus.
    pub focused: bool,
    /// Enabled.
    pub enabled: bool,
    /// Read-only.
    pub read_only: bool,
    /// Active thumb.
    pub active_thumb: RangeThumb,
    /// Dragging.
    pub dragging: bool,
    /// Hover.
    pub hovered: bool,
    /// Parts.
    pub parts: Option<RangeSliderParts>,
}

impl RangeSliderState {
    /// Initial range (unordered inputs are sorted).
    #[must_use]
    pub fn new(start: f64, end: f64) -> Self {
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        Self {
            start,
            end,
            focused: false,
            enabled: true,
            read_only: false,
            active_thumb: RangeThumb::Start,
            dragging: false,
            hovered: false,
            parts: None,
        }
    }

    /// Focus.
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on;
        if !on {
            self.dragging = false;
        }
    }

    /// Enabled.
    pub const fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
    }

    /// Read-only.
    pub fn set_read_only(&mut self, on: bool) {
        self.read_only = on;
        if on {
            self.dragging = false;
        }
    }

    /// Controlled range.
    pub fn set_range(&mut self, start: f64, end: f64) {
        if start <= end {
            self.start = start;
            self.end = end;
        } else {
            self.start = end;
            self.end = start;
        }
    }

    /// Whether editable.
    #[must_use]
    pub const fn can_edit(&self) -> bool {
        self.enabled && !self.read_only
    }
}

/// Dual-thumb range slider.
#[derive(Debug, Clone, Copy)]
pub struct RangeSlider<'a> {
    bounds: SliderBounds,
    system: &'a DesignSystem,
    label: Option<&'a str>,
    marks: &'a [SliderMark<'a>],
    show_value: bool,
    page_step: Option<f64>,
    colorless: bool,
}

impl<'a> RangeSlider<'a> {
    /// Bounds + system.
    #[must_use]
    pub const fn new(bounds: SliderBounds, system: &'a DesignSystem) -> Self {
        Self {
            bounds,
            system,
            label: None,
            marks: &[],
            show_value: true,
            page_step: None,
            colorless: false,
        }
    }

    /// Label.
    #[must_use]
    pub const fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    /// Marks.
    #[must_use]
    pub const fn marks(mut self, marks: &'a [SliderMark<'a>]) -> Self {
        self.marks = marks;
        self
    }

    /// Show value text.
    #[must_use]
    pub const fn show_value(mut self, on: bool) -> Self {
        self.show_value = on;
        self
    }

    /// Page step.
    #[must_use]
    pub const fn page_step(mut self, step: f64) -> Self {
        self.page_step = Some(step);
        self
    }

    /// Colorless.
    #[must_use]
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    fn page(&self) -> f64 {
        self.page_step
            .filter(|s| s.is_finite() && *s > 0.0)
            .unwrap_or_else(|| self.bounds.page_step())
    }

    /// Paint range.
    pub fn paint(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut RangeSliderState,
    ) -> RangeSliderParts {
        state.parts = None;
        if area.is_empty() {
            return RangeSliderParts {
                root: area,
                track: None,
                start_handle: None,
                end_handle: None,
                value_area: None,
                numeric_only: true,
            };
        }
        state.start = self.bounds.snap(state.start);
        state.end = self.bounds.snap(state.end);
        if state.start > state.end {
            core::mem::swap(&mut state.start, &mut state.end);
        }

        if area.width < SLIDER_NUMERIC_FALLBACK_WIDTH {
            let face = format!("{}–{}", format_value(state.start), format_value(state.end));
            let text = take_display_cols(&face, usize::from(area.width));
            let recipe = self.system.input_recipe(
                if !state.enabled {
                    ControlState::Disabled
                } else if state.focused {
                    ControlState::Focused
                } else {
                    ControlState::Default
                },
                false,
            );
            let style = if state.focused {
                recipe.value.add_modifier(Modifier::BOLD)
            } else {
                recipe.value
            };
            buffer.set_stringn(area.x, area.y, &text, usize::from(area.width), style);
            let parts = RangeSliderParts {
                root: area,
                track: None,
                start_handle: None,
                end_handle: None,
                value_area: Some(Rect::new(area.x, area.y, display_cols(&text) as u16, 1)),
                numeric_only: true,
            };
            state.parts = Some(parts.clone());
            return parts;
        }

        let mut y = area.y;
        if let Some(lab) = self.label {
            if !lab.is_empty() && area.height >= 2 {
                let text = take_display_cols(lab, usize::from(area.width));
                buffer.set_stringn(
                    area.x,
                    y,
                    &text,
                    usize::from(area.width),
                    self.system
                        .input_recipe(
                            if state.focused {
                                ControlState::Focused
                            } else {
                                ControlState::Default
                            },
                            false,
                        )
                        .value,
                );
                y = y.saturating_add(1);
            }
        }

        let value_str = if self.show_value {
            format!("{}–{}", format_value(state.start), format_value(state.end))
        } else {
            String::new()
        };
        let value_w = if value_str.is_empty() {
            0u16
        } else {
            (display_cols(&value_str) as u16).saturating_add(1)
        };
        let track_w = area
            .width
            .saturating_sub(value_w)
            .max(SLIDER_MIN_TRACK.min(area.width));
        let track = Rect::new(area.x, y.min(area.bottom().saturating_sub(1)), track_w, 1);

        let start_i = self.idx_for(state.start, track_w);
        let end_i = self.idx_for(state.end, track_w).max(start_i);

        let fill = fill_glyph(self.system, self.colorless);
        let empty = empty_glyph(self.system, self.colorless);
        let handle = handle_glyph(self.system, self.colorless);

        for i in 0..track_w {
            let is_start = i == start_i;
            let is_end = i == end_i;
            let in_range = i >= start_i && i <= end_i;
            let ch = if is_start || is_end {
                handle
            } else if in_range {
                fill
            } else {
                empty
            };
            let active = match state.active_thumb {
                RangeThumb::Start => is_start,
                RangeThumb::End => is_end,
            };
            // Same chrome answer as Slider: the thumb the operator is moving
            // carries the accent, the range between them is data
            // (plans/008 Step 4).
            let chrome = slider_chrome(self.system, state.enabled, state.focused && active);
            let style = if is_start || is_end {
                chrome.thumb
            } else if in_range {
                chrome.fill
            } else {
                chrome.track
            };
            buffer.set_stringn(track.x.saturating_add(i), track.y, ch, 1, style);
        }

        let mut value_area = None;
        if !value_str.is_empty() && track.right() < area.right() {
            let vx = track.right().saturating_add(1);
            let vw = area.right().saturating_sub(vx);
            let text = take_display_cols(&value_str, usize::from(vw));
            buffer.set_stringn(
                vx,
                track.y,
                &text,
                usize::from(vw),
                self.system
                    .input_recipe(
                        if state.focused {
                            ControlState::Focused
                        } else {
                            ControlState::Default
                        },
                        false,
                    )
                    .placeholder,
            );
            value_area = Some(Rect::new(vx, track.y, display_cols(&text) as u16, 1));
        }

        let parts = RangeSliderParts {
            root: area,
            track: Some(track),
            start_handle: Some(Rect::new(track.x.saturating_add(start_i), track.y, 1, 1)),
            end_handle: Some(Rect::new(track.x.saturating_add(end_i), track.y, 1, 1)),
            value_area,
            numeric_only: false,
        };
        state.parts = Some(parts.clone());
        parts
    }

    fn idx_for(&self, value: f64, track_w: u16) -> u16 {
        if track_w <= 1 {
            return 0;
        }
        let f = self.bounds.fraction(value);
        ((f * f64::from(track_w.saturating_sub(1))).round() as u16).min(track_w - 1)
    }

    fn emit_range(&self, state: &mut RangeSliderState) -> RangeSliderOutcome {
        if state.start > state.end {
            core::mem::swap(&mut state.start, &mut state.end);
        }
        RangeSliderOutcome::ValueChanged {
            start: state.start,
            end: state.end,
        }
    }

    /// Keys: Tab switches thumb; arrows move active thumb.
    pub fn handle_key(&self, state: &mut RangeSliderState, key: KeyEvent) -> RangeSliderOutcome {
        if !state.focused || !state.can_edit() || !key.is_press() {
            return RangeSliderOutcome::Ignored;
        }
        match key.code {
            KeyCode::Tab => {
                state.active_thumb = state.active_thumb.other();
                return RangeSliderOutcome::ThumbChanged {
                    thumb: state.active_thumb,
                };
            }
            KeyCode::BackTab => {
                state.active_thumb = state.active_thumb.other();
                return RangeSliderOutcome::ThumbChanged {
                    thumb: state.active_thumb,
                };
            }
            _ => {}
        }
        // Shift+Tab already BackTab in some adapters
        if key.modifiers.contains(KeyModifiers::SHIFT) && matches!(key.code, KeyCode::Tab) {
            state.active_thumb = state.active_thumb.other();
            return RangeSliderOutcome::ThumbChanged {
                thumb: state.active_thumb,
            };
        }

        let step = self.bounds.step;
        let page = self.page();
        let delta = match key.code {
            KeyCode::Left | KeyCode::Down | KeyCode::Char('h') | KeyCode::Char('-') => -step,
            KeyCode::Right | KeyCode::Up | KeyCode::Char('l') | KeyCode::Char('+') => step,
            KeyCode::PageDown => -page,
            KeyCode::PageUp => page,
            KeyCode::Home => {
                match state.active_thumb {
                    RangeThumb::Start => {
                        state.start = self.bounds.min;
                    }
                    RangeThumb::End => {
                        state.end = state.start;
                    }
                }
                return self.emit_range(state);
            }
            KeyCode::End => {
                match state.active_thumb {
                    RangeThumb::Start => {
                        state.start = state.end;
                    }
                    RangeThumb::End => {
                        state.end = self.bounds.max;
                    }
                }
                return self.emit_range(state);
            }
            _ => return RangeSliderOutcome::Ignored,
        };

        match state.active_thumb {
            RangeThumb::Start => {
                let next = self.bounds.snap(state.start + delta);
                state.start = next.min(state.end);
            }
            RangeThumb::End => {
                let next = self.bounds.snap(state.end + delta);
                state.end = next.max(state.start);
            }
        }
        self.emit_range(state)
    }

    /// Mouse drag / click.
    pub fn handle_mouse(
        &self,
        state: &mut RangeSliderState,
        event: MouseEvent,
    ) -> RangeSliderOutcome {
        if !state.can_edit() {
            return RangeSliderOutcome::Ignored;
        }
        let Some(parts) = state.parts.clone() else {
            return RangeSliderOutcome::Ignored;
        };
        match event.kind {
            MouseEventKind::Moved | MouseEventKind::Drag(_) => {
                state.hovered = parts.root.contains(event.position);
                if state.dragging {
                    return self.pointer_set(state, &parts, event.position);
                }
                RangeSliderOutcome::Ignored
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if !parts.root.contains(event.position) {
                    return RangeSliderOutcome::Ignored;
                }
                state.focused = true;
                state.dragging = true;
                // Prefer nearer handle
                if let (Some(s), Some(e)) = (parts.start_handle, parts.end_handle) {
                    let ds = event.position.x.abs_diff(s.x);
                    let de = event.position.x.abs_diff(e.x);
                    state.active_thumb = if ds <= de {
                        RangeThumb::Start
                    } else {
                        RangeThumb::End
                    };
                }
                self.pointer_set(state, &parts, event.position)
            }
            MouseEventKind::Up(MouseButton::Left) => {
                state.dragging = false;
                RangeSliderOutcome::Ignored
            }
            _ => RangeSliderOutcome::Ignored,
        }
    }

    fn pointer_set(
        &self,
        state: &mut RangeSliderState,
        parts: &RangeSliderParts,
        pos: Position,
    ) -> RangeSliderOutcome {
        if parts.numeric_only {
            return RangeSliderOutcome::Ignored;
        }
        let Some(track) = parts.track else {
            return RangeSliderOutcome::Ignored;
        };
        if track.width == 0 {
            return RangeSliderOutcome::Ignored;
        }
        let x = pos.x.clamp(track.x, track.right().saturating_sub(1));
        let idx = x.saturating_sub(track.x);
        let frac = if track.width <= 1 {
            0.0
        } else {
            f64::from(idx) / f64::from(track.width.saturating_sub(1))
        };
        let v = self.bounds.from_fraction(frac);
        match state.active_thumb {
            RangeThumb::Start => state.start = v.min(state.end),
            RangeThumb::End => state.end = v.max(state.start),
        }
        self.emit_range(state)
    }

    /// EventResult.
    pub fn handle_key_result(
        &self,
        state: &mut RangeSliderState,
        key: KeyEvent,
    ) -> EventResult<RangeSliderOutcome> {
        match self.handle_key(state, key) {
            RangeSliderOutcome::Ignored => EventResult::ignored(),
            other => EventResult::emit(other),
        }
    }

    /// Semantic.
    pub fn register_semantic<Id, Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        id: Id,
        area: Rect,
        state: &RangeSliderState,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if area.is_empty() {
            return;
        }
        let desc = format!("{}–{}", format_value(state.start), format_value(state.end));
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Progress)
                .label(self.label.unwrap_or("range slider"))
                .description(&desc)
                .focusable(state.can_edit())
                .disabled(!state.enabled)
                .state(SemanticState {
                    selected: state.focused,
                    pressed: state.dragging,
                    ..Default::default()
                }),
        );
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_snap_and_fraction() {
        let b = SliderBounds::new(0.0, 10.0, 0.5);
        assert_eq!(b.snap(3.2), 3.0);
        assert_eq!(b.snap(3.3), 3.5);
        assert!((b.fraction(5.0) - 0.5).abs() < 1e-9);
        assert_eq!(b.from_fraction(1.0), 10.0);
    }

    #[test]
    fn slider_arrow_steps() {
        let system = DesignSystem::default();
        let s = Slider::new(SliderBounds::percent(), &system);
        let mut state = SliderState::new(50.0);
        state.set_focused(true);
        let out = s.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
        );
        assert!(matches!(out, SliderOutcome::ValueChanged { value: 51.0 }));
        let out = s.handle_key(&mut state, KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
        assert!(matches!(out, SliderOutcome::ValueChanged { value: 0.0 }));
        let out = s.handle_key(&mut state, KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
        assert!(matches!(out, SliderOutcome::ValueChanged { value: 100.0 }));
    }

    #[test]
    fn slider_page_and_readonly() {
        let system = DesignSystem::default();
        let s = Slider::new(SliderBounds::percent(), &system);
        let mut state = SliderState::new(50.0);
        state.set_focused(true);
        let out = s.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE),
        );
        assert!(matches!(out, SliderOutcome::ValueChanged { .. }));
        state.set_read_only(true);
        assert!(matches!(
            s.handle_key(&mut state, KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)),
            SliderOutcome::Ignored
        ));
    }

    #[test]
    fn slider_numeric_edit() {
        let system = DesignSystem::default();
        let s = Slider::new(SliderBounds::percent(), &system);
        let mut state = SliderState::new(10.0);
        state.set_focused(true);
        assert!(matches!(
            s.handle_key(
                &mut state,
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
            ),
            SliderOutcome::EditStarted
        ));
        state.edit_buffer = "75".into();
        let out = s.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        );
        assert!(matches!(out, SliderOutcome::EditCommitted { value: 75.0 }));
        assert_eq!(state.value, 75.0);
        assert!(matches!(
            s.handle_key(
                &mut state,
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
            ),
            SliderOutcome::EditStarted
        ));
        assert!(matches!(
            s.handle_key(&mut state, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            SliderOutcome::EditCancelled
        ));
        assert_eq!(state.value, 75.0);
    }

    #[test]
    fn slider_tiny_numeric_fallback() {
        let system = DesignSystem::default();
        let s = Slider::new(SliderBounds::percent(), &system);
        let mut state = SliderState::new(42.0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 6, 1));
        let parts = s.paint(Rect::new(0, 0, 6, 1), &mut buf, &mut state);
        assert!(parts.numeric_only);
        assert!(parts.track.is_none());
    }

    #[test]
    fn slider_mouse_click_track() {
        let system = DesignSystem::default();
        let s = Slider::new(SliderBounds::percent(), &system);
        let mut state = SliderState::new(0.0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 1));
        let parts = s.paint(Rect::new(0, 0, 30, 1), &mut buf, &mut state);
        let track = parts.track.unwrap();
        let pos = Position {
            x: track.x.saturating_add(track.width.saturating_sub(1)),
            y: track.y,
        };
        let out = s.handle_mouse(
            &mut state,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: pos,
                modifiers: KeyModifiers::NONE,
            },
        );
        assert!(matches!(out, SliderOutcome::ValueChanged { .. }));
        assert!(state.value > 50.0);
    }

    #[test]
    fn range_slider_move_thumbs() {
        let system = DesignSystem::default();
        let s = RangeSlider::new(SliderBounds::percent(), &system);
        let mut state = RangeSliderState::new(20.0, 80.0);
        state.set_focused(true);
        let out = s.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
        );
        assert!(matches!(
            out,
            RangeSliderOutcome::ValueChanged {
                start: 21.0,
                end: 80.0
            }
        ));
        let out = s.handle_key(&mut state, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert!(matches!(
            out,
            RangeSliderOutcome::ThumbChanged {
                thumb: RangeThumb::End
            }
        ));
        let out = s.handle_key(&mut state, KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        assert!(matches!(
            out,
            RangeSliderOutcome::ValueChanged {
                start: 21.0,
                end: 79.0
            }
        ));
    }

    #[test]
    fn range_paint_two_handles() {
        let system = DesignSystem::default();
        let s = RangeSlider::new(SliderBounds::percent(), &system).label("Range");
        let mut state = RangeSliderState::new(25.0, 75.0);
        state.set_focused(true);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 2));
        let parts = s.paint(Rect::new(0, 0, 40, 2), &mut buf, &mut state);
        assert!(parts.start_handle.is_some());
        assert!(parts.end_handle.is_some());
        assert_ne!(parts.start_handle, parts.end_handle);
    }

    #[test]
    fn range_slider_mouse_chooses_nearest_painted_thumb() {
        let system = DesignSystem::default();
        let slider = RangeSlider::new(SliderBounds::percent(), &system);
        let mut state = RangeSliderState::new(20.0, 80.0);
        let area = Rect::new(0, 0, 40, 2);
        let mut buffer = Buffer::empty(area);
        let parts = slider.paint(area, &mut buffer, &mut state);
        let end = parts.end_handle.expect("end handle");

        assert!(matches!(
            slider.handle_mouse(
                &mut state,
                MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    position: Position::new(end.x, end.y),
                    modifiers: KeyModifiers::NONE,
                },
            ),
            RangeSliderOutcome::ValueChanged { .. }
        ));
        assert_eq!(state.active_thumb, RangeThumb::End);
        assert!(state.dragging);
    }

    #[test]
    fn range_start_cannot_pass_end() {
        let system = DesignSystem::default();
        let s = RangeSlider::new(SliderBounds::percent(), &system);
        let mut state = RangeSliderState::new(50.0, 50.0);
        state.set_focused(true);
        state.active_thumb = RangeThumb::Start;
        let _ = s.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
        );
        assert!(state.start <= state.end);
    }

    #[test]
    fn marks_and_vertical() {
        let system = DesignSystem::default();
        let marks = [
            SliderMark::labeled(0.0, "0"),
            SliderMark::labeled(50.0, "50"),
            SliderMark::new(100.0),
        ];
        let s = Slider::new(SliderBounds::percent(), &system)
            .marks(&marks)
            .label("Gain");
        let mut state = SliderState::new(50.0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 36, 3));
        let _ = s.paint(Rect::new(0, 0, 36, 3), &mut buf, &mut state);

        let v = Slider::new(SliderBounds::percent(), &system).vertical();
        let mut st = SliderState::new(75.0);
        let mut buf2 = Buffer::empty(Rect::new(0, 0, 4, 12));
        let parts = v.paint(Rect::new(0, 0, 4, 12), &mut buf2, &mut st);
        assert!(parts.track.is_some());
        assert!(parts.handle.is_some());
    }

    #[test]
    fn semantic_and_hot_path() {
        let system = DesignSystem::default();
        let s = Slider::new(SliderBounds::percent(), &system).label("Hot");
        let mut state = SliderState::new(30.0);
        state.set_focused(true);
        let area = Rect::new(0, 0, 32, 1);
        let mut buf = Buffer::empty(area);
        for _ in 0..300 {
            let _ = s.paint(area, &mut buf, &mut state);
        }
        let mut scene = SemanticScene::<&str, ()>::default();
        s.register_semantic(&mut scene, "hot", area, &state);
        assert!(scene.len() >= 1);
    }

    #[test]
    fn disabled_ignores() {
        let system = DesignSystem::default();
        let s = Slider::new(SliderBounds::percent(), &system);
        let mut state = SliderState::new(10.0);
        state.set_focused(true);
        state.set_enabled(false);
        assert!(matches!(
            s.handle_key(
                &mut state,
                KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)
            ),
            SliderOutcome::Ignored
        ));
    }
}
