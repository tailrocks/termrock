// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **ContextMeter** — trustworthy context / token / resource budget display.
//!
//! **Mission.** Used/available, compaction threshold, model limit, included
//! sources, cached content, pending attachments, warning, expandable breakdown.
//! Avoid **false precision** when estimates are approximate. Concise
//! composer/status form and detailed popover. Show what action reduces usage or
//! triggers compaction. Generic measurement units beyond tokens.
//!
//! **vs [`super::TokenMeter`](crate::widgets::TokenMeter).** TokenMeter is the
//! thin used/limit label paint. ContextMeter is the full budget surface.
//! **vs charts/Gauge.** Domain-neutral viz; this is AI/context budget semantics.
//!
//! Research: Amp compaction, OpenCode context displays, AI chat token meters.

#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    style::{DesignSystem, Role},
    text::{display_cols, take_display_cols},
};

/// Max sources painted in expanded breakdown.
pub const CONTEXT_METER_SOURCE_CAP: usize = 8;
/// Warning fraction default when threshold unset.
pub const CONTEXT_METER_WARN_FRACTION: f64 = 0.75;
/// Danger fraction default.
pub const CONTEXT_METER_DANGER_FRACTION: f64 = 0.90;

// ── Domain ──────────────────────────────────────────────────────────────────

/// Measurement unit (tokens or other resource budgets).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum BudgetUnit {
    /// Model tokens.
    #[default]
    Tokens,
    /// Bytes.
    Bytes,
    /// Characters / codepoints (host-defined).
    Characters,
    /// Messages / turns.
    Messages,
    /// Custom (use [`BudgetMeasure::unit_label`]).
    Custom,
}

impl BudgetUnit {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Tokens => "tokens",
            Self::Bytes => "bytes",
            Self::Characters => "characters",
            Self::Messages => "messages",
            Self::Custom => "custom",
        }
    }

    /// Default short label.
    #[must_use]
    pub const fn default_label(self) -> &'static str {
        match self {
            Self::Tokens => "tok",
            Self::Bytes => "B",
            Self::Characters => "ch",
            Self::Messages => "msg",
            Self::Custom => "units",
        }
    }
}

/// How exact the numbers are (drives formatting).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum BudgetPrecision {
    /// Exact counts.
    #[default]
    Exact,
    /// Approximate — show `~`, avoid false 1% precision.
    Approximate,
    /// Limit/used unknown — indeterminate meter, never claim 100%.
    Unknown,
}

impl BudgetPrecision {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Approximate => "approx",
            Self::Unknown => "unknown",
        }
    }
}

/// Kind of included source in the breakdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ContextSourceKind {
    /// Conversation messages.
    #[default]
    Message,
    /// Tool results.
    Tool,
    /// Attachments / files.
    Attachment,
    /// Cached / prompt cache.
    Cache,
    /// System / policy.
    System,
    /// Other.
    Other,
}

impl ContextSourceKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Message => "message",
            Self::Tool => "tool",
            Self::Attachment => "attachment",
            Self::Cache => "cache",
            Self::System => "system",
            Self::Other => "other",
        }
    }

    /// Letter.
    #[must_use]
    pub const fn letter(self) -> char {
        match self {
            Self::Message => 'M',
            Self::Tool => 'T',
            Self::Attachment => 'A',
            Self::Cache => 'C',
            Self::System => 'S',
            Self::Other => '·',
        }
    }
}

/// One included budget slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextSource {
    /// Stable id.
    pub id: String,
    /// Label.
    pub label: String,
    /// Amount in budget units.
    pub amount: u64,
    /// Kind.
    pub kind: ContextSourceKind,
}

impl ContextSource {
    /// Construct.
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>, amount: u64) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            amount,
            kind: ContextSourceKind::Message,
        }
    }

    /// Kind.
    #[must_use]
    pub const fn kind(mut self, k: ContextSourceKind) -> Self {
        self.kind = k;
        self
    }
}

/// Core used/limit measurement (generic units).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetMeasure {
    /// Used amount.
    pub used: u64,
    /// Remaining if known (preferred over limit − used when host has it).
    pub available: Option<u64>,
    /// Hard limit if known.
    pub limit: Option<u64>,
    /// Unit.
    pub unit: BudgetUnit,
    /// Custom unit label when unit is Custom (or override).
    pub unit_label: Option<String>,
    /// Precision.
    pub precision: BudgetPrecision,
}

impl BudgetMeasure {
    /// Exact tokens used/limit.
    #[must_use]
    pub fn tokens(used: u64, limit: u64) -> Self {
        Self {
            used,
            available: limit.checked_sub(used),
            limit: Some(limit),
            unit: BudgetUnit::Tokens,
            unit_label: None,
            precision: BudgetPrecision::Exact,
        }
    }

    /// Approximate (estimator).
    #[must_use]
    pub fn approximate(used: u64, limit: Option<u64>, unit: BudgetUnit) -> Self {
        Self {
            used,
            available: limit.and_then(|l| l.checked_sub(used)),
            limit,
            unit,
            unit_label: None,
            precision: BudgetPrecision::Approximate,
        }
    }

    /// Unknown totals — indeterminate.
    #[must_use]
    pub fn unknown(unit: BudgetUnit) -> Self {
        Self {
            used: 0,
            available: None,
            limit: None,
            unit,
            unit_label: None,
            precision: BudgetPrecision::Unknown,
        }
    }

    /// Unit label text.
    #[must_use]
    pub fn label(&self) -> &str {
        self.unit_label
            .as_deref()
            .unwrap_or_else(|| self.unit.default_label())
    }

    /// Fraction 0.0–1.0 if computable; **None** when unknown / no limit.
    #[must_use]
    pub fn fraction(&self) -> Option<f64> {
        if matches!(self.precision, BudgetPrecision::Unknown) {
            return None;
        }
        let limit = self.limit?;
        if limit == 0 {
            return None;
        }
        Some((self.used as f64 / limit as f64).clamp(0.0, 1.0))
    }

    /// Pressure role from fraction + thresholds.
    #[must_use]
    pub fn pressure_role(&self, warn: f64, danger: f64) -> Role {
        match self.fraction() {
            None => Role::TextMuted,
            Some(f) if f >= danger => Role::Danger,
            Some(f) if f >= warn => Role::Warning,
            Some(_) => Role::TextMuted,
        }
    }
}

/// Full context budget projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextBudget {
    /// Core measure.
    pub measure: BudgetMeasure,
    /// Compaction threshold fraction (0–100 as percent int optional).
    pub compaction_threshold_pct: Option<u8>,
    /// Model context limit (may differ from measure.limit).
    pub model_limit: Option<u64>,
    /// Model name.
    pub model: Option<String>,
    /// Included sources.
    pub sources: Vec<ContextSource>,
    /// Cached amount.
    pub cached: Option<u64>,
    /// Pending attachments amount.
    pub pending_attachments: Option<u64>,
    /// Warning banner.
    pub warning: Option<String>,
    /// Host action that reduces usage.
    pub reduce_action: Option<String>,
    /// Host action that triggers compaction.
    pub compact_action: Option<String>,
}

impl ContextBudget {
    /// From measure.
    #[must_use]
    pub fn new(measure: BudgetMeasure) -> Self {
        Self {
            measure,
            compaction_threshold_pct: None,
            model_limit: None,
            model: None,
            sources: Vec::new(),
            cached: None,
            pending_attachments: None,
            warning: None,
            reduce_action: None,
            compact_action: None,
        }
    }

    /// Tokens used/limit helper.
    #[must_use]
    pub fn tokens(used: u64, limit: u64) -> Self {
        Self::new(BudgetMeasure::tokens(used, limit))
    }

    /// Compaction threshold percent 0–100.
    #[must_use]
    pub fn compaction_threshold_pct(mut self, pct: u8) -> Self {
        self.compaction_threshold_pct = Some(pct.min(100));
        self
    }

    /// Model.
    #[must_use]
    pub fn model(mut self, m: impl Into<String>, limit: Option<u64>) -> Self {
        self.model = Some(m.into());
        self.model_limit = limit;
        self
    }

    /// Source.
    #[must_use]
    pub fn source(mut self, s: ContextSource) -> Self {
        self.sources.push(s);
        self
    }

    /// Cached.
    #[must_use]
    pub const fn cached(mut self, n: u64) -> Self {
        self.cached = Some(n);
        self
    }

    /// Pending attachments.
    #[must_use]
    pub const fn pending_attachments(mut self, n: u64) -> Self {
        self.pending_attachments = Some(n);
        self
    }

    /// Warning.
    #[must_use]
    pub fn warning(mut self, w: impl Into<String>) -> Self {
        self.warning = Some(w.into());
        self
    }

    /// Reduce action hint.
    #[must_use]
    pub fn reduce_action(mut self, a: impl Into<String>) -> Self {
        self.reduce_action = Some(a.into());
        self
    }

    /// Compact action hint.
    #[must_use]
    pub fn compact_action(mut self, a: impl Into<String>) -> Self {
        self.compact_action = Some(a.into());
        self
    }

    /// Effective warn fraction.
    #[must_use]
    pub fn warn_fraction(&self) -> f64 {
        self.compaction_threshold_pct
            .map(|p| f64::from(p) / 100.0)
            .unwrap_or(CONTEXT_METER_WARN_FRACTION)
    }

    /// At or past compaction threshold?
    #[must_use]
    pub fn at_compaction(&self) -> bool {
        self.measure
            .fraction()
            .is_some_and(|f| f >= self.warn_fraction())
    }
}

// ── Formatting (no false precision) ─────────────────────────────────────────

/// Format count with precision rules.
#[must_use]
pub fn format_budget_count(n: u64, precision: BudgetPrecision) -> String {
    match precision {
        BudgetPrecision::Unknown => "—".into(),
        BudgetPrecision::Approximate => {
            if n >= 1_000_000 {
                format!("~{:.1}M", n as f64 / 1_000_000.0)
            } else if n >= 10_000 {
                format!("~{}k", n / 1000)
            } else if n >= 1000 {
                format!("~{:.1}k", n as f64 / 1000.0)
            } else {
                format!("~{n}")
            }
        }
        BudgetPrecision::Exact => {
            if n >= 1_000_000 {
                format!("{:.1}M", n as f64 / 1_000_000.0)
            } else if n >= 10_000 {
                format!("{}k", n / 1000)
            } else {
                n.to_string()
            }
        }
    }
}

/// Format percent; approximate rounds to 5%; unknown is `—`.
///
/// **Never** returns `100%` when fraction is None.
#[must_use]
pub fn format_budget_percent(fraction: Option<f64>, precision: BudgetPrecision) -> String {
    match (fraction, precision) {
        (None, _) | (_, BudgetPrecision::Unknown) => "—".into(),
        (Some(f), BudgetPrecision::Approximate) => {
            let pct = ((f * 100.0 / 5.0).round() * 5.0).clamp(0.0, 100.0);
            format!("~{pct:.0}%")
        }
        (Some(f), BudgetPrecision::Exact) => {
            let pct = (f * 100.0).clamp(0.0, 100.0);
            // avoid claiming 100% from float noise unless truly at/over limit
            if f >= 1.0 {
                "100%".into()
            } else if pct >= 99.5 && f < 1.0 {
                format!("{:.0}%", pct.floor())
            } else {
                format!("{pct:.0}%")
            }
        }
    }
}

/// Compact status line: `tok 12k/200k (6%)` or `tok ~12k/~200k (~5%)` or `tok —`.
#[must_use]
pub fn format_budget_compact(measure: &BudgetMeasure) -> String {
    let label = measure.label();
    let frac = measure.fraction();
    let pct = format_budget_percent(frac, measure.precision);
    match (measure.limit, measure.precision) {
        (_, BudgetPrecision::Unknown) | (None, _) => {
            if measure.used > 0 && !matches!(measure.precision, BudgetPrecision::Unknown) {
                format!(
                    "{label} {} (?)",
                    format_budget_count(measure.used, measure.precision)
                )
            } else {
                format!("{label} —")
            }
        }
        (Some(limit), prec) => format!(
            "{label} {}/{} ({pct})",
            format_budget_count(measure.used, prec),
            format_budget_count(limit, prec),
        ),
    }
}

/// Meter bar glyphs (filled / empty / hatch for unknown).
#[must_use]
pub fn meter_bar(fraction: Option<f64>, width: usize, ascii: bool, indeterminate: bool) -> String {
    if width == 0 {
        return String::new();
    }
    let fill_ch = if ascii { '=' } else { '█' };
    let empty_ch = if ascii { '-' } else { '░' };
    let hatch_ch = if ascii { '~' } else { '▒' };
    if indeterminate || fraction.is_none() {
        return hatch_ch.to_string().repeat(width);
    }
    let f = fraction.unwrap_or(0.0).clamp(0.0, 1.0);
    let filled = ((f * width as f64).round() as usize).min(width);
    // never paint full bar when fraction < 1.0
    let filled = if f < 1.0 && filled == width {
        width.saturating_sub(1)
    } else {
        filled
    };
    let mut s = fill_ch.to_string().repeat(filled);
    s.push_str(&empty_ch.to_string().repeat(width.saturating_sub(filled)));
    s
}

// ── Presentation / state / outcomes ─────────────────────────────────────────

/// Display form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ContextMeterPresentation {
    /// Composer / status bar one-liner (+ optional mini bar).
    #[default]
    Compact,
    /// Expanded breakdown inline.
    Expanded,
    /// Popover-dense detail (host places in overlay).
    Popover,
}

impl ContextMeterPresentation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Expanded => "expanded",
            Self::Popover => "popover",
        }
    }
}

/// Outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ContextMeterOutcome {
    /// Ignored.
    Ignored,
    /// Activated (open detail / host popover).
    Activated,
    /// Expand toggled.
    ExpandToggled {
        /// Expanded.
        expanded: bool,
    },
    /// Host should compact context.
    CompactRequested,
    /// Host should reduce usage (drop attachments etc.).
    ReduceRequested,
    /// Open full breakdown.
    OpenBreakdown,
}

/// Interactive state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextMeterState {
    /// Presentation.
    pub presentation: ContextMeterPresentation,
    /// Focused.
    pub focused: bool,
    accepts_input: bool,
    /// Hit region.
    pub hit: Rect,
}

impl Default for ContextMeterState {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextMeterState {
    /// Compact default.
    #[must_use]
    pub fn new() -> Self {
        Self {
            presentation: ContextMeterPresentation::Compact,
            focused: false,
            accepts_input: true,
            hit: Rect::default(),
        }
    }

    /// Gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Focus.
    pub const fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Toggle expand.
    pub fn toggle_expand(&mut self) -> ContextMeterOutcome {
        match self.presentation {
            ContextMeterPresentation::Compact => {
                self.presentation = ContextMeterPresentation::Expanded;
                ContextMeterOutcome::ExpandToggled { expanded: true }
            }
            ContextMeterPresentation::Expanded | ContextMeterPresentation::Popover => {
                self.presentation = ContextMeterPresentation::Compact;
                ContextMeterOutcome::ExpandToggled { expanded: false }
            }
        }
    }

    /// Keys.
    pub fn handle_key(&mut self, key: KeyEvent, budget: &ContextBudget) -> ContextMeterOutcome {
        if !self.accepts_input || key.kind != KeyEventKind::Press {
            return ContextMeterOutcome::Ignored;
        }
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') if key.modifiers.is_empty() => {
                if self.focused {
                    ContextMeterOutcome::Activated
                } else {
                    ContextMeterOutcome::Ignored
                }
            }
            KeyCode::Char('e') if key.modifiers.is_empty() => self.toggle_expand(),
            KeyCode::Char('c')
                if key.modifiers.is_empty()
                    && (budget.compact_action.is_some() || budget.at_compaction()) =>
            {
                ContextMeterOutcome::CompactRequested
            }
            KeyCode::Char('r') if key.modifiers.is_empty() && budget.reduce_action.is_some() => {
                ContextMeterOutcome::ReduceRequested
            }
            KeyCode::Char('b') if key.modifiers.is_empty() => ContextMeterOutcome::OpenBreakdown,
            KeyCode::Esc
                if matches!(
                    self.presentation,
                    ContextMeterPresentation::Expanded | ContextMeterPresentation::Popover
                ) =>
            {
                self.presentation = ContextMeterPresentation::Compact;
                ContextMeterOutcome::ExpandToggled { expanded: false }
            }
            _ => ContextMeterOutcome::Ignored,
        }
    }

    /// Mouse.
    pub fn handle_mouse(&mut self, event: MouseEvent) -> ContextMeterOutcome {
        if !self.accepts_input || event.kind != MouseEventKind::Down(MouseButton::Left) {
            return ContextMeterOutcome::Ignored;
        }
        if self.hit.contains(event.position) {
            return ContextMeterOutcome::Activated;
        }
        ContextMeterOutcome::Ignored
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Context / resource budget meter.
#[derive(Debug, Clone, Copy)]
pub struct ContextMeter<'a> {
    budget: &'a ContextBudget,
    system: &'a DesignSystem,
    ascii: bool,
    colorless: bool,
    mono: bool,
}

impl<'a> ContextMeter<'a> {
    /// Budget + system.
    #[must_use]
    pub const fn new(budget: &'a ContextBudget, system: &'a DesignSystem) -> Self {
        Self {
            budget,
            system,
            ascii: false,
            colorless: false,
            mono: false,
        }
    }

    /// ASCII glyphs.
    #[must_use]
    pub const fn ascii(mut self, on: bool) -> Self {
        self.ascii = on;
        self
    }

    /// Colorless.
    #[must_use]
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    /// Monochrome density bar (no role color).
    #[must_use]
    pub const fn mono(mut self, on: bool) -> Self {
        self.mono = on;
        self
    }

    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut ContextMeterState) {
        state.hit = area;
        if area.is_empty() {
            return;
        }
        let b = self.budget;
        let m = &b.measure;
        let frac = m.fraction();
        let indeterminate = matches!(m.precision, BudgetPrecision::Unknown) || frac.is_none();
        let warn = b.warn_fraction();
        let role = if self.mono || self.colorless {
            Role::Text
        } else {
            m.pressure_role(warn, CONTEXT_METER_DANGER_FRACTION)
        };
        let mut style = self.system.style(role);
        if state.focused && !self.colorless {
            style = style.add_modifier(Modifier::BOLD);
        } else if state.focused && self.colorless {
            style = style.add_modifier(Modifier::REVERSED);
        }

        match state.presentation {
            ContextMeterPresentation::Compact => {
                self.paint_compact(area, buffer, style, frac, indeterminate);
            }
            ContextMeterPresentation::Expanded | ContextMeterPresentation::Popover => {
                self.paint_detail(area, buffer, style, frac, indeterminate);
            }
        }
    }

    fn paint_compact(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        style: ratatui_core::style::Style,
        frac: Option<f64>,
        indeterminate: bool,
    ) {
        let line = format_budget_compact(&self.budget.measure);
        let w = usize::from(area.width);
        if area.height == 1 || w < 16 {
            buffer.set_stringn(area.x, area.y, take_display_cols(&line, w), w, style);
            return;
        }
        // line 0: bar, line 1: numbers
        let bar_w = w.saturating_sub(2).min(24);
        let bar = meter_bar(frac, bar_w, self.ascii || self.colorless, indeterminate);
        let bar_line = format!("[{bar}]");
        buffer.set_stringn(area.x, area.y, take_display_cols(&bar_line, w), w, style);
        if area.height > 1 {
            buffer.set_stringn(
                area.x,
                area.y.saturating_add(1),
                take_display_cols(&line, w),
                w,
                style,
            );
        }
    }

    fn paint_detail(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        style: ratatui_core::style::Style,
        frac: Option<f64>,
        indeterminate: bool,
    ) {
        let b = self.budget;
        let m = &b.measure;
        let w = usize::from(area.width);
        let mut y = area.y;
        let max_y = area.bottom();
        let muted = if self.colorless {
            self.system.style(Role::Text)
        } else {
            self.system.style(Role::TextMuted)
        };

        // bar + compact
        let bar_w = w.saturating_sub(2).min(32);
        let bar = meter_bar(frac, bar_w, self.ascii || self.colorless, indeterminate);
        buffer.set_stringn(
            area.x,
            y,
            take_display_cols(&format!("[{bar}]"), w),
            w,
            style,
        );
        y = y.saturating_add(1);

        if y < max_y {
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&format_budget_compact(m), w),
                w,
                style,
            );
            y = y.saturating_add(1);
        }

        // model / limit
        if y < max_y {
            if let Some(model) = &b.model {
                let lim = b
                    .model_limit
                    .or(m.limit)
                    .map(|l| format_budget_count(l, m.precision))
                    .unwrap_or_else(|| "—".into());
                buffer.set_stringn(
                    area.x,
                    y,
                    take_display_cols(&format!("model {model} · lim {lim}"), w),
                    w,
                    muted,
                );
                y = y.saturating_add(1);
            }
        }

        // available / cached / pending
        if y < max_y {
            let mut bits = Vec::new();
            if let Some(a) = m.available {
                bits.push(format!("avail {}", format_budget_count(a, m.precision)));
            }
            if let Some(c) = b.cached {
                bits.push(format!("cache {}", format_budget_count(c, m.precision)));
            }
            if let Some(p) = b.pending_attachments {
                bits.push(format!("pend+{}", format_budget_count(p, m.precision)));
            }
            if !bits.is_empty() {
                buffer.set_stringn(area.x, y, take_display_cols(&bits.join(" · "), w), w, muted);
                y = y.saturating_add(1);
            }
        }

        // threshold
        if let Some(pct) = b.compaction_threshold_pct {
            if y < max_y {
                let mark = if b.at_compaction() { "!" } else { "·" };
                buffer.set_stringn(
                    area.x,
                    y,
                    take_display_cols(&format!("{mark} compact ≥{pct}%"), w),
                    w,
                    if b.at_compaction() && !self.colorless {
                        self.system.style(Role::Warning)
                    } else {
                        muted
                    },
                );
                y = y.saturating_add(1);
            }
        }

        // sources breakdown
        for src in b.sources.iter().take(CONTEXT_METER_SOURCE_CAP) {
            if y >= max_y {
                break;
            }
            let line = format!(
                "  {} {} {}",
                src.kind.letter(),
                take_display_cols(&src.label, 16),
                format_budget_count(src.amount, m.precision)
            );
            buffer.set_stringn(area.x, y, take_display_cols(&line, w), w, muted);
            y = y.saturating_add(1);
        }

        // warning
        if let Some(warn) = &b.warning {
            if y < max_y {
                buffer.set_stringn(
                    area.x,
                    y,
                    take_display_cols(warn, w),
                    w,
                    if self.colorless {
                        self.system.style(Role::Text)
                    } else {
                        self.system.style(Role::Warning)
                    },
                );
                y = y.saturating_add(1);
            }
        }

        // actions that reduce / compact
        if y < max_y {
            let mut acts = Vec::new();
            if let Some(a) = &b.reduce_action {
                acts.push(format!("r:{a}"));
            }
            if let Some(a) = &b.compact_action {
                acts.push(format!("c:{a}"));
            } else if b.at_compaction() {
                acts.push("c:compact".into());
            }
            if !acts.is_empty() {
                buffer.set_stringn(
                    area.x,
                    y,
                    take_display_cols(&format!("→ {}", acts.join(" · ")), w),
                    w,
                    if self.colorless {
                        self.system.style(Role::Text)
                    } else {
                        self.system.style(Role::Accent)
                    },
                );
            }
        }
        let _ = display_cols;
    }

    /// Render alias.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut ContextMeterState) {
        self.paint(area, buffer, state);
    }
}

// ── TokenMeter bridge ───────────────────────────────────────────────────────

/// Build a simple token budget for elevated hosts still using used/limit pairs.
#[must_use]
pub fn context_budget_from_tokens(used: u64, limit: u64) -> ContextBudget {
    ContextBudget::tokens(used, limit)
}

// ── Examples ────────────────────────────────────────────────────────────────

/// Demo budgets for stories.
#[must_use]
pub fn example_context_budgets() -> Vec<ContextBudget> {
    vec![
        ContextBudget::tokens(12_000, 200_000)
            .model("grok-4", Some(200_000))
            .source(ContextSource::new("m", "messages", 8_000))
            .source(ContextSource::new("t", "tools", 3_000).kind(ContextSourceKind::Tool))
            .source(ContextSource::new("a", "files", 1_000).kind(ContextSourceKind::Attachment))
            .cached(2_000)
            .reduce_action("drop attachments")
            .compact_action("compact"),
        ContextBudget::tokens(160_000, 200_000)
            .compaction_threshold_pct(75)
            .model("grok-4", Some(200_000))
            .warning("near limit — compact soon")
            .compact_action("compact now")
            .pending_attachments(5_000),
        ContextBudget::new(BudgetMeasure::unknown(BudgetUnit::Tokens)).model("unknown", None),
        ContextBudget::new(BudgetMeasure::approximate(
            48_000,
            Some(128_000),
            BudgetUnit::Tokens,
        ))
        .model("est", Some(128_000))
        .source(ContextSource::new("m", "approx msgs", 48_000)),
        ContextBudget::new(BudgetMeasure {
            used: 40 * 1024 * 1024,
            available: Some(60 * 1024 * 1024),
            limit: Some(100 * 1024 * 1024),
            unit: BudgetUnit::Bytes,
            unit_label: None,
            precision: BudgetPrecision::Exact,
        })
        .reduce_action("clear cache"),
    ]
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Paint stress.
pub mod bench {
    /// Frames.
    pub const PAINT_FRAMES: u32 = 40;
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_core::layout::Position;

    #[test]
    fn never_100_percent_without_limit() {
        let m = BudgetMeasure::unknown(BudgetUnit::Tokens);
        assert!(m.fraction().is_none());
        assert_eq!(format_budget_percent(None, BudgetPrecision::Exact), "—");
        assert_eq!(format_budget_percent(None, BudgetPrecision::Unknown), "—");
        let compact = format_budget_compact(&m);
        assert!(!compact.contains("100%"));
        assert!(compact.contains('—') || compact.contains("?"));
    }

    #[test]
    fn zero_limit_safe() {
        let m = BudgetMeasure::tokens(10, 0);
        // limit 0 → fraction None
        assert!(m.fraction().is_none() || m.limit == Some(0));
        let f = if m.limit == Some(0) {
            None
        } else {
            m.fraction()
        };
        // our tokens() sets limit Some(0); fraction returns None for limit 0
        assert!(BudgetMeasure::tokens(10, 0).fraction().is_none());
        let _ = f;
        let s = format_budget_compact(&BudgetMeasure::tokens(10, 0));
        assert!(!s.contains("100%"));
    }

    #[test]
    fn approximate_avoids_false_precision() {
        let pct = format_budget_percent(Some(0.73), BudgetPrecision::Approximate);
        assert!(pct.starts_with('~'));
        // rounded to 5%
        assert!(pct.contains("70%") || pct.contains("75%"));
        let c = format_budget_count(12_345, BudgetPrecision::Approximate);
        assert!(c.starts_with('~'));
    }

    #[test]
    fn meter_bar_never_full_when_partial() {
        let bar = meter_bar(Some(0.999), 10, true, false);
        assert!(bar.contains('-'));
        assert_ne!(bar, "==========");
        let ind = meter_bar(None, 8, true, true);
        assert!(ind.chars().all(|c| c == '~'));
    }

    #[test]
    fn pressure_roles() {
        let low = BudgetMeasure::tokens(10, 100);
        assert_eq!(low.pressure_role(0.75, 0.9), Role::TextMuted);
        let mid = BudgetMeasure::tokens(80, 100);
        assert_eq!(mid.pressure_role(0.75, 0.9), Role::Warning);
        let hi = BudgetMeasure::tokens(95, 100);
        assert_eq!(hi.pressure_role(0.75, 0.9), Role::Danger);
    }

    #[test]
    fn keys_compact_reduce_expand() {
        let budget = ContextBudget::tokens(160_000, 200_000)
            .compaction_threshold_pct(75)
            .compact_action("compact")
            .reduce_action("drop files");
        let mut st = ContextMeterState::new();
        st.focused = true;
        assert!(matches!(
            st.handle_key(
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
                &budget
            ),
            ContextMeterOutcome::CompactRequested
        ));
        assert!(matches!(
            st.handle_key(
                KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
                &budget
            ),
            ContextMeterOutcome::ReduceRequested
        ));
        assert!(matches!(
            st.handle_key(
                KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE),
                &budget
            ),
            ContextMeterOutcome::ExpandToggled { expanded: true }
        ));
        assert!(matches!(
            st.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &budget),
            ContextMeterOutcome::Activated
        ));
    }

    #[test]
    fn paint_compact_expanded_indeterminate() {
        let system = DesignSystem::default();
        let budgets = example_context_budgets();
        let area = Rect::new(0, 0, 40, 8);
        let mut buf = Buffer::empty(area);
        for b in &budgets {
            let mut st = ContextMeterState::new();
            st.presentation = ContextMeterPresentation::Compact;
            ContextMeter::new(b, &system).paint(area, &mut buf, &mut st);
            st.presentation = ContextMeterPresentation::Expanded;
            ContextMeter::new(b, &system)
                .ascii(true)
                .paint(area, &mut buf, &mut st);
        }
    }

    #[test]
    fn mouse_activates() {
        let system = DesignSystem::default();
        let b = ContextBudget::tokens(1, 10);
        let mut st = ContextMeterState::new();
        let area = Rect::new(0, 0, 20, 1);
        let mut buf = Buffer::empty(area);
        ContextMeter::new(&b, &system).paint(area, &mut buf, &mut st);
        let ev = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position::new(area.x, area.y),
            modifiers: KeyModifiers::NONE,
        };
        assert!(matches!(
            st.handle_mouse(ev),
            ContextMeterOutcome::Activated
        ));
    }

    #[test]
    fn non_token_bytes_unit() {
        let b = &example_context_budgets()[4];
        assert_eq!(b.measure.unit, BudgetUnit::Bytes);
        let s = format_budget_compact(&b.measure);
        assert!(s.contains('B') || s.contains('M') || s.contains('k'));
    }

    #[test]
    fn token_meter_bridge() {
        let b = context_budget_from_tokens(128_000, 200_000);
        assert_eq!(b.measure.used, 128_000);
        assert_eq!(b.measure.limit, Some(200_000));
    }

    #[test]
    fn never_process() {
        let src = include_str!("context_meter.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        for f in ["std::process", "Command::new", "portable_pty", "openai"] {
            assert!(!body.contains(f), "{f}");
        }
    }

    #[test]
    fn accepts_input_gate() {
        let b = ContextBudget::tokens(1, 2);
        let mut st = ContextMeterState::new();
        st.focused = true;
        st.set_accepts_input(false);
        assert!(matches!(
            st.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &b),
            ContextMeterOutcome::Ignored
        ));
    }

    #[test]
    fn paint_perf_budget() {
        let system = DesignSystem::default();
        let b = example_context_budgets()[1].clone();
        let area = Rect::new(0, 0, 48, 10);
        let mut buf = Buffer::empty(area);
        let start = std::time::Instant::now();
        for _ in 0..bench::PAINT_FRAMES {
            let mut st = ContextMeterState::new();
            st.presentation = ContextMeterPresentation::Expanded;
            ContextMeter::new(&b, &system).paint(area, &mut buf, &mut st);
        }
        assert!(start.elapsed().as_secs() < 3, "{:?}", start.elapsed());
    }

    #[test]
    fn fuzz_units_precision() {
        for u in [
            BudgetUnit::Tokens,
            BudgetUnit::Bytes,
            BudgetUnit::Characters,
            BudgetUnit::Messages,
            BudgetUnit::Custom,
        ] {
            assert!(!u.id().is_empty());
            let _ = BudgetMeasure::unknown(u);
        }
        for p in [
            BudgetPrecision::Exact,
            BudgetPrecision::Approximate,
            BudgetPrecision::Unknown,
        ] {
            assert!(!p.id().is_empty());
        }
    }

    #[test]
    fn at_compaction_threshold() {
        let b = ContextBudget::tokens(80, 100).compaction_threshold_pct(75);
        assert!(b.at_compaction());
        let b2 = ContextBudget::tokens(10, 100).compaction_threshold_pct(75);
        assert!(!b2.at_compaction());
    }
}
