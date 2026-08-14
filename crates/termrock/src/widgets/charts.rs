// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **Visualization family** — Sparkline, Chart (line + **area**), Gauge,
//! Histogram (+ BarSeries, SegmentedMeter), MetricRadar (multi-axis peer).
//!
//! **Mission.** Coherent terminal data-viz: time series, filled area under
//! series, bars, stacked bars, histogram buckets, gauges, multi-metric axis
//! comparison (radar substitute), thresholds, labels, legends, missing data,
//! and selected points. Braille / block / ASCII capability fallbacks with
//! **consistent scale semantics**. Autoscale, fixed scale, log (where
//! justified), time-window behavior. No-color mode uses line styles, glyphs,
//! labels, and ordering — never color alone.
//!
//! Research: btop, bottom, gping, Ratatui charts, shadcn Recharts demos (area/
//! bar/line/pie/radar peers), observability dashboards.

#![allow(unused_imports)] // test-only imports retained
use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::Widget};

use crate::{
    style::{DesignSystem, GlyphSet, Role, RolePalette},
    text::{display_cols, take_display_cols},
};

// ── Shared scale & glyphs ───────────────────────────────────────────────────

/// How domain values map to the plot range.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum ScaleMode {
    /// Fit to finite samples (ignore missing).
    Auto,
    /// Fixed domain `[min, max]`.
    Fixed {
        /// Domain min.
        min: f64,
        /// Domain max.
        max: f64,
    },
    /// Logarithmic (positive finite values only); decades over `[min, max]` or auto.
    Log {
        /// Optional fixed domain (both ends must be > 0).
        fixed: Option<(f64, f64)>,
    },
}

impl Default for ScaleMode {
    fn default() -> Self {
        Self::Auto
    }
}

impl ScaleMode {
    /// Stable id.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Fixed { .. } => "fixed",
            Self::Log { .. } => "log",
        }
    }
}

/// Resolved numeric domain for mapping.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScaleDomain {
    /// Inclusive min.
    pub min: f64,
    /// Inclusive max.
    pub max: f64,
    /// Whether log mapping is active.
    pub log: bool,
}

impl ScaleDomain {
    /// Empty/degenerate → unit domain.
    #[must_use]
    pub fn unit() -> Self {
        Self {
            min: 0.0,
            max: 1.0,
            log: false,
        }
    }

    /// Map value to `0.0..=1.0` (missing → None).
    #[must_use]
    pub fn normalize(self, value: f64) -> Option<f64> {
        if !value.is_finite() {
            return None;
        }
        if self.log {
            if value <= 0.0 || self.min <= 0.0 || self.max <= 0.0 {
                return None;
            }
            let lo = self.min.ln();
            let hi = self.max.ln();
            let span = (hi - lo).abs().max(f64::EPSILON);
            let t = (value.ln() - lo) / span;
            return Some(t.clamp(0.0, 1.0));
        }
        let span = (self.max - self.min).abs().max(f64::EPSILON);
        Some(((value - self.min) / span).clamp(0.0, 1.0))
    }
}

/// Compute domain from samples under a scale mode.
#[must_use]
pub fn resolve_domain(mode: ScaleMode, values: impl Iterator<Item = f64>) -> ScaleDomain {
    match mode {
        ScaleMode::Fixed { min, max } => {
            let (a, b) = if min <= max { (min, max) } else { (max, min) };
            let (a, b) = if (b - a).abs() < f64::EPSILON {
                (a, a + 1.0)
            } else {
                (a, b)
            };
            ScaleDomain {
                min: a,
                max: b,
                log: false,
            }
        }
        ScaleMode::Log {
            fixed: Some((min, max)),
        } => {
            let min = if min > 0.0 && min.is_finite() {
                min
            } else {
                f64::MIN_POSITIVE
            };
            let max = if max > min && max.is_finite() {
                max
            } else {
                min * 10.0
            };
            ScaleDomain {
                min,
                max,
                log: true,
            }
        }
        ScaleMode::Log { fixed: None } => {
            let mut lo = f64::INFINITY;
            let mut hi = f64::NEG_INFINITY;
            for v in values {
                if v.is_finite() && v > 0.0 {
                    lo = lo.min(v);
                    hi = hi.max(v);
                }
            }
            if !lo.is_finite() || !hi.is_finite() {
                return ScaleDomain {
                    min: 1.0,
                    max: 10.0,
                    log: true,
                };
            }
            if (hi - lo).abs() < f64::EPSILON {
                hi = lo * 10.0;
            }
            ScaleDomain {
                min: lo,
                max: hi,
                log: true,
            }
        }
        ScaleMode::Auto => {
            let mut lo = f64::INFINITY;
            let mut hi = f64::NEG_INFINITY;
            let mut any = false;
            for v in values {
                if v.is_finite() {
                    any = true;
                    lo = lo.min(v);
                    hi = hi.max(v);
                }
            }
            if !any {
                return ScaleDomain::unit();
            }
            if (hi - lo).abs() < f64::EPSILON {
                // constant series: show mid with headroom
                return ScaleDomain {
                    min: lo - 0.5,
                    max: hi + 0.5,
                    log: false,
                };
            }
            ScaleDomain {
                min: lo,
                max: hi,
                log: false,
            }
        }
    }
}

/// Glyph ladder for density plots (capability-aware).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum VizGlyphSet {
    /// Prefer braille/block (Unicode / Enhanced).
    #[default]
    Auto,
    /// Block elements ` ▁▂▃▄▅▆▇█`.
    Block,
    /// Braille 2×4 density (Unicode).
    Braille,
    /// ASCII ` .:-=+*#%@`.
    Ascii,
}

impl VizGlyphSet {
    /// Resolve against DesignSystem glyph capability.
    #[must_use]
    pub fn resolve(self, glyphs: GlyphSet) -> VizGlyphSet {
        match self {
            Self::Auto => match glyphs {
                GlyphSet::Ascii => Self::Ascii,
                GlyphSet::Enhanced | GlyphSet::Unicode => Self::Block,
            },
            other => {
                if matches!(glyphs, GlyphSet::Ascii) && !matches!(other, Self::Ascii) {
                    Self::Ascii
                } else {
                    other
                }
            }
        }
    }

    /// Ladder characters low→high (index 0 = empty/missing track).
    #[must_use]
    pub fn ladder(self) -> &'static [char] {
        match self {
            Self::Ascii => &[' ', '.', ':', '-', '=', '+', '*', '#', '%', '@'],
            Self::Block | Self::Auto => crate::style::BLOCK_RAMP,
            // Braille vertical density approximation (single cell heights)
            Self::Braille => crate::style::BRAILLE_RAMP,
        }
    }

    /// No-color line style cycle for multi-series (glyph, not color).
    #[must_use]
    pub fn series_marker(self, series_index: usize) -> char {
        const ASCII: &[char] = &['*', '+', 'x', 'o', '#'];
        const UNI: &[char] = &['●', '◆', '▲', '■', '○'];
        match self {
            Self::Ascii => ASCII[series_index % ASCII.len()],
            _ => UNI[series_index % UNI.len()],
        }
    }

    /// Threshold / tick glyph.
    #[must_use]
    pub fn threshold_mark(self) -> char {
        match self {
            Self::Ascii => '|',
            _ => '│',
        }
    }

    /// Missing-data placeholder.
    #[must_use]
    pub fn missing_mark(self) -> char {
        match self {
            Self::Ascii => '?',
            _ => '·',
        }
    }
}

/// Map `0..=1` fraction to ladder glyph.
#[must_use]
pub fn glyph_for_fraction(fraction: f64, ladder: &[char], missing: bool, missing_ch: char) -> char {
    if missing || !fraction.is_finite() {
        return missing_ch;
    }
    if ladder.is_empty() {
        return ' ';
    }
    let t = fraction.clamp(0.0, 1.0);
    let idx = ((t * (ladder.len() - 1) as f64).round() as usize).min(ladder.len() - 1);
    ladder[idx]
}

/// Sample window for streaming series (take last `width` points).
#[must_use]
pub fn window_samples(samples: &[f64], width: usize) -> &[f64] {
    if width == 0 || samples.is_empty() {
        return samples;
    }
    if samples.len() <= width {
        samples
    } else {
        &samples[samples.len() - width..]
    }
}

/// Chart series color roles (cycle).
const SERIES_ROLES: &[Role] = &[
    Role::ChartSeries1,
    Role::ChartSeries2,
    Role::ChartSeries3,
    Role::ChartSeries4,
    Role::Accent,
    Role::Info,
];

fn series_role(i: usize) -> Role {
    SERIES_ROLES[i % SERIES_ROLES.len()]
}

// ── Sparkline ───────────────────────────────────────────────────────────────

/// One-row time/value sparkline.
///
/// Samples may be pre-normalized (`0..=1`) or raw values with [`ScaleMode`].
/// Non-finite values are **missing** (distinct glyph).
#[derive(Debug, Clone, Copy)]
pub struct Sparkline<'a> {
    samples: &'a [f64],
    system: &'a DesignSystem,
    scale: ScaleMode,
    glyphs: VizGlyphSet,
    /// Optional threshold in domain units (horizontal tick mark in density).
    threshold: Option<f64>,
    /// Selected sample index (absolute into samples, not windowed).
    selected: Option<usize>,
    /// When true, treat values as already `0..=1` (legacy).
    pre_normalized: bool,
    /// Time-window: only last N samples (0 = all / width fit).
    window: usize,
    role: Role,
}

impl<'a> Sparkline<'a> {
    /// Creates a sparkline (auto-scale raw values; legacy 0..=1 still works).
    #[must_use]
    pub const fn new(samples: &'a [f64], system: &'a DesignSystem) -> Self {
        Self {
            samples,
            system,
            scale: ScaleMode::Auto,
            glyphs: VizGlyphSet::Auto,
            threshold: None,
            selected: None,
            pre_normalized: false,
            window: 0,
            role: Role::Accent,
        }
    }

    /// Force pre-normalized `0..=1` path (old API semantics).
    #[must_use]
    pub const fn pre_normalized(mut self, on: bool) -> Self {
        self.pre_normalized = on;
        self
    }

    /// Scale mode.
    #[must_use]
    pub const fn scale(mut self, scale: ScaleMode) -> Self {
        self.scale = scale;
        self
    }

    /// Glyph set.
    #[must_use]
    pub const fn glyphs(mut self, glyphs: VizGlyphSet) -> Self {
        self.glyphs = glyphs;
        self
    }

    /// Threshold in domain units.
    #[must_use]
    pub const fn threshold(mut self, t: f64) -> Self {
        self.threshold = Some(t);
        self
    }

    /// Selected absolute index.
    #[must_use]
    pub const fn selected(mut self, index: usize) -> Self {
        self.selected = Some(index);
        self
    }

    /// Streaming window (last N points). `0` = fit paint width.
    #[must_use]
    pub const fn window(mut self, n: usize) -> Self {
        self.window = n;
        self
    }

    /// Series role.
    #[must_use]
    pub const fn role(mut self, role: Role) -> Self {
        self.role = role;
        self
    }
}

impl Widget for &Sparkline<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() || self.samples.is_empty() {
            return;
        }
        let width = usize::from(area.width);
        let win_n = if self.window == 0 {
            width.max(1)
        } else {
            self.window
        };
        let samples = window_samples(self.samples, win_n);
        let base = self.samples.len().saturating_sub(samples.len());

        let domain = if self.pre_normalized {
            ScaleDomain::unit()
        } else {
            resolve_domain(self.scale, samples.iter().copied())
        };
        let gset = self.glyphs.resolve(self.system.glyphs);
        let ladder = gset.ladder();
        let miss = gset.missing_mark();
        let thr_f = self
            .threshold
            .and_then(|t| domain.normalize(t))
            .unwrap_or(-1.0);

        for col in 0..width {
            let index = col * samples.len() / width.max(1);
            let abs_i = base + index;
            let sample = samples.get(index).copied().unwrap_or(f64::NAN);
            let missing = !sample.is_finite();
            let fraction = if self.pre_normalized && sample.is_finite() {
                Some(sample.clamp(0.0, 1.0))
            } else {
                domain.normalize(sample)
            };
            let mut glyph = glyph_for_fraction(fraction.unwrap_or(0.0), ladder, missing, miss);
            // threshold band: use threshold mark when close
            if thr_f >= 0.0 {
                if let Some(f) = fraction {
                    if (f - thr_f).abs() < 0.08 {
                        glyph = gset.threshold_mark();
                    }
                }
            }
            let selected = self.selected == Some(abs_i);
            if selected {
                glyph = if matches!(gset, VizGlyphSet::Ascii) {
                    'X'
                } else {
                    '◆'
                };
            }
            let style = if selected {
                self.system
                    .style(Role::TextStrong)
                    .add_modifier(Modifier::BOLD)
            } else if matches!(
                self.system.capability,
                crate::style::ColorCapability::Monochrome
            ) {
                // no-color: density only
                self.system.style(Role::Text)
            } else {
                self.system.style(self.role)
            };
            buffer.set_stringn(
                area.x.saturating_add(col as u16),
                area.y,
                glyph.to_string(),
                1,
                style,
            );
        }
    }
}

impl Widget for Sparkline<'_> {
    #[expect(
        clippy::needless_borrows_for_generic_args,
        reason = "explicitly delegate the owned contract to the borrowed renderer"
    )]
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

// ── Chart (multi-series) ────────────────────────────────────────────────────

/// How the chart fills the plot under series (shadcn area-chart peer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ChartFill {
    /// Line/spark only (default line chart).
    #[default]
    None,
    /// Fill from plot baseline (bottom) up to each series value (area chart).
    Area,
    /// Stack series (sum) then fill under the cumulative outline.
    AreaStacked,
}

impl ChartFill {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Area => "area",
            Self::AreaStacked => "area-stacked",
        }
    }
}

/// How multi-column samples map along X (shadcn line linear/step peer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ChartInterpolation {
    /// Nearest sample index `col * n / width` (historical default).
    #[default]
    Nearest,
    /// Linear lerp between adjacent samples (smooth TUI polyline).
    Linear,
    /// Hold floor sample until next boundary (step chart).
    Step,
}

impl ChartInterpolation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Nearest => "nearest",
            Self::Linear => "linear",
            Self::Step => "step",
        }
    }

    /// Sample value at column `col` of `width` for `samples` (non-empty).
    #[must_use]
    pub fn sample_at(self, samples: &[f64], col: usize, width: usize) -> f64 {
        let n = samples.len();
        if n == 0 {
            return f64::NAN;
        }
        if n == 1 || width <= 1 {
            return samples[0];
        }
        let w = width.max(1);
        match self {
            Self::Nearest => {
                let index = col.saturating_mul(n) / w;
                samples.get(index.min(n - 1)).copied().unwrap_or(f64::NAN)
            }
            Self::Step => {
                // Hold sample i across columns mapping to floor(pos).
                let pos = col as f64 * (n - 1) as f64 / (w - 1) as f64;
                let index = pos.floor() as usize;
                samples.get(index.min(n - 1)).copied().unwrap_or(f64::NAN)
            }
            Self::Linear => {
                let pos = col as f64 * (n - 1) as f64 / (w - 1) as f64;
                let i0 = (pos.floor() as usize).min(n - 1);
                let i1 = (i0 + 1).min(n - 1);
                let t = (pos - i0 as f64).clamp(0.0, 1.0);
                let a = samples[i0];
                let b = samples[i1];
                if !a.is_finite() {
                    return b;
                }
                if !b.is_finite() {
                    return a;
                }
                a * (1.0 - t) + b * t
            }
        }
    }
}

/// One series in a multi-line / area chart.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChartSeries<'a> {
    /// Legend label.
    pub label: &'a str,
    /// Samples (non-finite = missing).
    pub samples: &'a [f64],
}

impl<'a> ChartSeries<'a> {
    /// Construct.
    #[must_use]
    pub const fn new(label: &'a str, samples: &'a [f64]) -> Self {
        Self { label, samples }
    }
}

/// Multi-series chart with axes, legend, selection, thresholds, optional area fill.
#[derive(Debug, Clone, Copy)]
pub struct Chart<'a> {
    series: &'a [ChartSeries<'a>],
    system: &'a DesignSystem,
    scale: ScaleMode,
    glyphs: VizGlyphSet,
    thresholds: &'a [f64],
    selected_series: Option<usize>,
    selected_index: Option<usize>,
    show_legend: bool,
    show_axes: bool,
    window: usize,
    title: Option<&'a str>,
    fill: ChartFill,
    interpolation: ChartInterpolation,
}

impl<'a> Chart<'a> {
    /// Series + system (line style by default).
    #[must_use]
    pub const fn new(series: &'a [ChartSeries<'a>], system: &'a DesignSystem) -> Self {
        Self {
            series,
            system,
            scale: ScaleMode::Auto,
            glyphs: VizGlyphSet::Auto,
            thresholds: &[],
            selected_series: None,
            selected_index: None,
            show_legend: true,
            show_axes: true,
            window: 0,
            title: None,
            fill: ChartFill::None,
            interpolation: ChartInterpolation::Nearest,
        }
    }

    /// Area chart: fill under each series from baseline (shadcn area default peer).
    #[must_use]
    pub const fn area(mut self) -> Self {
        self.fill = ChartFill::Area;
        self
    }

    /// Stacked area: series sum then fill under cumulative outline.
    #[must_use]
    pub const fn area_stacked(mut self) -> Self {
        self.fill = ChartFill::AreaStacked;
        self
    }

    /// Explicit fill mode.
    #[must_use]
    pub const fn fill(mut self, fill: ChartFill) -> Self {
        self.fill = fill;
        self
    }

    /// X-path interpolation (line linear / step peers).
    #[must_use]
    pub const fn interpolation(mut self, interp: ChartInterpolation) -> Self {
        self.interpolation = interp;
        self
    }

    /// Linear lerp between samples (shadcn chart-line-linear peer).
    #[must_use]
    pub const fn linear(mut self) -> Self {
        self.interpolation = ChartInterpolation::Linear;
        self
    }

    /// Step hold between samples (shadcn chart-line-step peer).
    #[must_use]
    pub const fn step(mut self) -> Self {
        self.interpolation = ChartInterpolation::Step;
        self
    }

    /// Scale.
    #[must_use]
    pub const fn scale(mut self, scale: ScaleMode) -> Self {
        self.scale = scale;
        self
    }

    /// Glyphs.
    #[must_use]
    pub const fn glyphs(mut self, glyphs: VizGlyphSet) -> Self {
        self.glyphs = glyphs;
        self
    }

    /// Thresholds in domain units.
    #[must_use]
    pub const fn thresholds(mut self, t: &'a [f64]) -> Self {
        self.thresholds = t;
        self
    }

    /// Selected series index.
    #[must_use]
    pub const fn selected_series(mut self, i: usize) -> Self {
        self.selected_series = Some(i);
        self
    }

    /// Selected sample index (into each series / window).
    #[must_use]
    pub const fn selected_index(mut self, i: usize) -> Self {
        self.selected_index = Some(i);
        self
    }

    /// Legend row.
    #[must_use]
    pub const fn show_legend(mut self, on: bool) -> Self {
        self.show_legend = on;
        self
    }

    /// Axis labels.
    #[must_use]
    pub const fn show_axes(mut self, on: bool) -> Self {
        self.show_axes = on;
        self
    }

    /// Streaming window.
    #[must_use]
    pub const fn window(mut self, n: usize) -> Self {
        self.window = n;
        self
    }

    /// Title.
    #[must_use]
    pub const fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }
}

impl Widget for &Chart<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() || self.series.is_empty() {
            return;
        }
        let gset = self.glyphs.resolve(self.system.glyphs);
        let mut y = area.y;
        let mut body_h = area.height;

        if let Some(title) = self.title {
            if body_h > 0 {
                buffer.set_stringn(
                    area.x,
                    y,
                    take_display_cols(title, usize::from(area.width)),
                    usize::from(area.width),
                    self.system.style(Role::TextStrong),
                );
                y = y.saturating_add(1);
                body_h = body_h.saturating_sub(1);
            }
        }

        if self.show_legend && body_h > 0 && area.width >= 12 {
            let mut legend = String::new();
            for (i, s) in self.series.iter().enumerate() {
                if i > 0 {
                    legend.push(' ');
                }
                let mark = gset.series_marker(i);
                legend.push(mark);
                legend.push(' ');
                legend.push_str(s.label);
            }
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&legend, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
            y = y.saturating_add(1);
            body_h = body_h.saturating_sub(1);
        }

        if body_h == 0 {
            return;
        }

        // Domain across all series (stacked: baseline 0 + full cumulative tops)
        let width_hint = usize::from(area.width).max(1);
        let mut all = Vec::new();
        match self.fill {
            ChartFill::AreaStacked => {
                // Always include baseline 0 so Auto min is not min(cum) > 0.
                all.push(0.0);
                let n = self
                    .series
                    .iter()
                    .map(|s| {
                        let win = if self.window == 0 {
                            s.samples
                        } else {
                            window_samples(s.samples, self.window.max(width_hint))
                        };
                        win.len()
                    })
                    .max()
                    .unwrap_or(0)
                    .max(1);
                for i in 0..n {
                    let mut sum = 0.0;
                    for s in self.series {
                        let win = if self.window == 0 {
                            s.samples
                        } else {
                            window_samples(s.samples, self.window.max(width_hint))
                        };
                        if let Some(v) = win.get(i).copied().filter(|v| v.is_finite()) {
                            sum += v.max(0.0);
                        }
                    }
                    all.push(sum);
                }
            }
            ChartFill::None | ChartFill::Area => {
                for s in self.series {
                    let win = if self.window == 0 {
                        s.samples
                    } else {
                        window_samples(s.samples, self.window)
                    };
                    all.extend(win.iter().copied());
                }
            }
        }
        for t in self.thresholds {
            all.push(*t);
        }
        let domain = resolve_domain(self.scale, all.into_iter());

        let axis_w = if self.show_axes && area.width >= 20 {
            6u16
        } else {
            0
        };
        let plot = Rect::new(
            area.x.saturating_add(axis_w),
            y,
            area.width.saturating_sub(axis_w),
            body_h,
        );
        if plot.is_empty() {
            return;
        }

        // Y-axis labels
        if axis_w > 0 {
            let hi = format_axis(domain.max);
            let lo = format_axis(domain.min);
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&hi, usize::from(axis_w)),
                usize::from(axis_w),
                self.system.style(Role::ChartAxis),
            );
            if body_h > 1 {
                buffer.set_stringn(
                    area.x,
                    y.saturating_add(body_h.saturating_sub(1)),
                    take_display_cols(&lo, usize::from(axis_w)),
                    usize::from(axis_w),
                    self.system.style(Role::ChartAxis),
                );
            }
        }

        // Paint thresholds first as horizontal lines of marks
        for thr in self.thresholds {
            if let Some(tf) = domain.normalize(*thr) {
                let row = fraction_to_row(tf, plot.height);
                let ch = gset.threshold_mark().to_string();
                let style = self.system.style(Role::ChartGrid);
                for col in 0..plot.width {
                    buffer.set_stringn(
                        plot.x.saturating_add(col),
                        plot.y.saturating_add(row),
                        &ch,
                        1,
                        style,
                    );
                }
            }
        }

        let width = usize::from(plot.width);
        let height = usize::from(plot.height.max(1));
        let ladder = gset.ladder();
        let miss = gset.missing_mark();
        let fill_ch = if matches!(gset, VizGlyphSet::Ascii) {
            ':'
        } else {
            ladder.get(ladder.len() / 3).copied().unwrap_or('·')
        };

        // Stacked: always paint series order 0..n-1 so lower bands survive.
        // Line/area: last writer wins; selected series last.
        let order: Vec<usize> = match self.fill {
            ChartFill::AreaStacked => (0..self.series.len()).collect(),
            ChartFill::None | ChartFill::Area => {
                let mut o: Vec<usize> = (0..self.series.len()).collect();
                if let Some(s) = self.selected_series {
                    if s < o.len() {
                        o.retain(|&i| i != s);
                        o.push(s);
                    }
                }
                o
            }
        };

        // Precompute per-column samples for each series (windowed).
        let win_n = if self.window == 0 {
            width.max(1)
        } else {
            self.window
        };
        let windows: Vec<&[f64]> = self
            .series
            .iter()
            .map(|s| window_samples(s.samples, win_n))
            .collect();

        // Pass 1: fills (stacked bands / area under curve) so outlines can win in pass 2.
        if height > 1 && !matches!(self.fill, ChartFill::None) {
            for si in 0..self.series.len() {
                let role = series_role(si);
                let fill_style = if matches!(
                    self.system.capability,
                    crate::style::ColorCapability::Monochrome
                ) {
                    self.system.style(Role::TextMuted)
                } else {
                    self.system.style(role)
                };
                let samples = windows[si];
                if samples.is_empty() {
                    continue;
                }
                for col in 0..width {
                    let (row_top, row_bot) = match self.fill {
                        ChartFill::Area => {
                            let sample = self.interpolation.sample_at(samples, col, width);
                            let Some(frac) = domain.normalize(sample) else {
                                continue;
                            };
                            (
                                fraction_to_row(frac, plot.height),
                                plot.height.saturating_sub(1),
                            )
                        }
                        ChartFill::AreaStacked => {
                            // Stack still uses nearest sample index for segment alignment.
                            let index = col * samples.len() / width.max(1);
                            let mut prev = 0.0;
                            for sj in 0..si {
                                if let Some(v) =
                                    windows[sj].get(index).copied().filter(|v| v.is_finite())
                                {
                                    prev += v.max(0.0);
                                }
                            }
                            let mut cum = prev;
                            if let Some(v) = samples.get(index).copied().filter(|v| v.is_finite()) {
                                cum += v.max(0.0);
                            }
                            let Some(frac) = domain.normalize(cum) else {
                                continue;
                            };
                            let top = fraction_to_row(frac, plot.height);
                            let bot = if si == 0 {
                                plot.height.saturating_sub(1)
                            } else if let Some(pf) = domain.normalize(prev) {
                                // exclusive of lower series outline row
                                fraction_to_row(pf, plot.height).saturating_sub(1)
                            } else {
                                plot.height.saturating_sub(1)
                            };
                            (top, bot)
                        }
                        ChartFill::None => continue,
                    };
                    // Fill rows strictly below outline down through row_bot (inclusive).
                    let mut r = row_top;
                    while r < row_bot {
                        r = r.saturating_add(1);
                        buffer.set_stringn(
                            plot.x.saturating_add(col as u16),
                            plot.y.saturating_add(r),
                            fill_ch.to_string(),
                            1,
                            fill_style,
                        );
                    }
                }
            }
        }

        // Pass 2: outlines / markers (always on top of fills).
        for si in order {
            let samples = windows[si];
            if samples.is_empty() {
                continue;
            }
            let mark = gset.series_marker(si);
            let role = series_role(si);
            let style = if matches!(
                self.system.capability,
                crate::style::ColorCapability::Monochrome
            ) {
                self.system.style(Role::Text)
            } else {
                self.system.style(role)
            };

            for col in 0..width {
                let index = col * samples.len() / width.max(1);
                let sample = match self.fill {
                    ChartFill::AreaStacked => {
                        let mut cum = 0.0;
                        for sj in 0..=si {
                            if let Some(v) =
                                windows[sj].get(index).copied().filter(|v| v.is_finite())
                            {
                                cum += v.max(0.0);
                            }
                        }
                        cum
                    }
                    ChartFill::None | ChartFill::Area => {
                        self.interpolation.sample_at(samples, col, width)
                    }
                };
                let missing = !sample.is_finite();
                let Some(frac) = domain.normalize(sample) else {
                    if missing && matches!(self.fill, ChartFill::None) {
                        buffer.set_stringn(
                            plot.x.saturating_add(col as u16),
                            plot.y.saturating_add(plot.height.saturating_sub(1)),
                            miss.to_string(),
                            1,
                            self.system.style(Role::TextMuted),
                        );
                    }
                    continue;
                };
                let row_top = fraction_to_row(frac, plot.height);
                // Selection: nearest sample index for interactive peer (not lerp t).
                let selected =
                    self.selected_series == Some(si) && self.selected_index == Some(index);
                let ch = if selected {
                    if matches!(gset, VizGlyphSet::Ascii) {
                        'X'
                    } else {
                        '◆'
                    }
                } else if height <= 1 {
                    glyph_for_fraction(frac, ladder, false, miss)
                } else {
                    mark
                };
                let st = if selected {
                    self.system
                        .style(Role::TextStrong)
                        .add_modifier(Modifier::BOLD)
                } else {
                    style
                };
                buffer.set_stringn(
                    plot.x.saturating_add(col as u16),
                    plot.y.saturating_add(row_top),
                    ch.to_string(),
                    1,
                    st,
                );
            }
        }
    }
}

impl Widget for Chart<'_> {
    #[expect(
        clippy::needless_borrows_for_generic_args,
        reason = "explicitly delegate the owned contract to the borrowed renderer"
    )]
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

fn fraction_to_row(frac: f64, height: u16) -> u16 {
    if height == 0 {
        return 0;
    }
    // 1.0 at top
    let h = f64::from(height.saturating_sub(1));
    let row = ((1.0 - frac.clamp(0.0, 1.0)) * h).round() as u16;
    row.min(height.saturating_sub(1))
}

fn format_axis(v: f64) -> String {
    if !v.is_finite() {
        return "nan".into();
    }
    let a = v.abs();
    if a >= 1000.0 || (a > 0.0 && a < 0.01) {
        format!("{v:.1e}")
    } else if a >= 10.0 {
        format!("{v:.0}")
    } else {
        format!("{v:.2}")
    }
}

// ── Gauge ───────────────────────────────────────────────────────────────────

/// Single-value gauge with thresholds and capability-aware fill.
#[derive(Debug, Clone, Copy)]
pub struct Gauge<'a> {
    value: f64,
    system: &'a DesignSystem,
    scale: ScaleMode,
    glyphs: VizGlyphSet,
    label: Option<&'a str>,
    unit: Option<&'a str>,
    thresholds: &'a [f64],
    /// Role for fill (overridden by threshold zone if any).
    role: Role,
}

impl<'a> Gauge<'a> {
    /// Value + system (auto domain 0..max(value,1) unless scaled).
    #[must_use]
    pub const fn new(value: f64, system: &'a DesignSystem) -> Self {
        Self {
            value,
            system,
            scale: ScaleMode::Fixed { min: 0.0, max: 1.0 },
            glyphs: VizGlyphSet::Auto,
            label: None,
            unit: None,
            thresholds: &[],
            role: Role::Accent,
        }
    }

    /// Percent convenience (0..=100 → fixed 0..=100).
    #[must_use]
    pub const fn percent(value: f64, system: &'a DesignSystem) -> Self {
        Self {
            value,
            system,
            scale: ScaleMode::Fixed {
                min: 0.0,
                max: 100.0,
            },
            glyphs: VizGlyphSet::Auto,
            label: None,
            unit: None,
            thresholds: &[],
            role: Role::Accent,
        }
    }

    /// Scale.
    #[must_use]
    pub const fn scale(mut self, scale: ScaleMode) -> Self {
        self.scale = scale;
        self
    }

    /// Glyphs.
    #[must_use]
    pub const fn glyphs(mut self, glyphs: VizGlyphSet) -> Self {
        self.glyphs = glyphs;
        self
    }

    /// Label.
    #[must_use]
    pub const fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    /// Unit suffix.
    #[must_use]
    pub const fn unit(mut self, unit: &'a str) -> Self {
        self.unit = Some(unit);
        self
    }

    /// Thresholds (ascending). Crossing raises warning/danger roles.
    #[must_use]
    pub const fn thresholds(mut self, t: &'a [f64]) -> Self {
        self.thresholds = t;
        self
    }

    /// Base role.
    #[must_use]
    pub const fn role(mut self, role: Role) -> Self {
        self.role = role;
        self
    }
}

impl Widget for &Gauge<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let domain = match self.scale {
            ScaleMode::Auto => resolve_domain(
                ScaleMode::Fixed {
                    min: 0.0,
                    max: self.value.abs().max(1.0),
                },
                std::iter::empty(),
            ),
            other => resolve_domain(other, std::iter::once(self.value)),
        };
        let frac = domain.normalize(self.value).unwrap_or(0.0);
        let gset = self.glyphs.resolve(self.system.glyphs);
        let ladder = gset.ladder();
        let fill_ch = *ladder.last().unwrap_or(&'#');
        let empty_ch = match gset {
            VizGlyphSet::Ascii => '-',
            _ => '░',
        };

        // Zone role from thresholds
        let mut role = self.role;
        let mut sorted: Vec<f64> = self
            .thresholds
            .iter()
            .copied()
            .filter(|t| t.is_finite())
            .collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        if sorted.len() >= 2 && self.value >= sorted[sorted.len() - 1] {
            role = Role::Danger;
        } else if !sorted.is_empty() && self.value >= sorted[0] {
            role = Role::Warning;
        }

        let mut x = area.x;
        let mut track_w = area.width;
        if let Some(label) = self.label {
            if area.width >= 10 {
                let lw = (area.width / 3).min(12);
                buffer.set_stringn(
                    x,
                    area.y,
                    take_display_cols(label, usize::from(lw)),
                    usize::from(lw),
                    self.system.style(Role::TextMuted),
                );
                x = x.saturating_add(lw);
                track_w = track_w.saturating_sub(lw);
            }
        }
        // value text on the right
        let val_s = {
            let mut s = format_axis(self.value);
            if let Some(u) = self.unit {
                s.push_str(u);
            }
            s
        };
        let vw = display_cols(&val_s) as u16 + 1;
        if track_w > vw + 2 {
            track_w = track_w.saturating_sub(vw);
        } else {
            // tiny: only value
            buffer.set_stringn(
                area.x,
                area.y,
                take_display_cols(&val_s, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(role),
            );
            return;
        }

        let filled = ((f64::from(track_w) * frac).round() as u16).min(track_w);
        let fill = fill_ch.to_string().repeat(usize::from(filled));
        let empty = empty_ch
            .to_string()
            .repeat(usize::from(track_w.saturating_sub(filled)));
        let style = if matches!(
            self.system.capability,
            crate::style::ColorCapability::Monochrome
        ) {
            self.system.style(Role::TextStrong)
        } else {
            self.system.style(role)
        };
        if filled > 0 {
            buffer.set_stringn(x, area.y, &fill, usize::from(filled), style);
        }
        if filled < track_w {
            buffer.set_stringn(
                x.saturating_add(filled),
                area.y,
                &empty,
                usize::from(track_w.saturating_sub(filled)),
                self.system.style(Role::TextDisabled),
            );
        }
        // threshold tick in track
        for thr in self.thresholds {
            if let Some(tf) = domain.normalize(*thr) {
                let col = ((f64::from(track_w.saturating_sub(1)) * tf).round() as u16)
                    .min(track_w.saturating_sub(1));
                buffer.set_stringn(
                    x.saturating_add(col),
                    area.y,
                    gset.threshold_mark().to_string(),
                    1,
                    self.system.style(Role::TextStrong),
                );
            }
        }
        buffer.set_stringn(
            x.saturating_add(track_w),
            area.y,
            take_display_cols(&format!(" {val_s}"), usize::from(vw)),
            usize::from(vw),
            self.system.style(Role::Text),
        );
    }
}

impl Widget for Gauge<'_> {
    #[expect(
        clippy::needless_borrows_for_generic_args,
        reason = "explicitly delegate the owned contract to the borrowed renderer"
    )]
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

// ── Histogram ───────────────────────────────────────────────────────────────

/// One histogram bucket.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HistBucket<'a> {
    /// Label (bin edge or name).
    pub label: &'a str,
    /// Count / weight (non-finite treated as 0).
    pub count: f64,
}

impl<'a> HistBucket<'a> {
    /// Construct.
    #[must_use]
    pub const fn new(label: &'a str, count: f64) -> Self {
        Self { label, count }
    }
}

/// Vertical or horizontal histogram of buckets.
#[derive(Debug, Clone, Copy)]
pub struct Histogram<'a> {
    buckets: &'a [HistBucket<'a>],
    system: &'a DesignSystem,
    scale: ScaleMode,
    glyphs: VizGlyphSet,
    vertical: bool,
    selected: Option<usize>,
    title: Option<&'a str>,
}

impl<'a> Histogram<'a> {
    /// Buckets + system.
    #[must_use]
    pub const fn new(buckets: &'a [HistBucket<'a>], system: &'a DesignSystem) -> Self {
        Self {
            buckets,
            system,
            scale: ScaleMode::Auto,
            glyphs: VizGlyphSet::Auto,
            vertical: true,
            selected: None,
            title: None,
        }
    }

    /// Scale for counts.
    #[must_use]
    pub const fn scale(mut self, scale: ScaleMode) -> Self {
        self.scale = scale;
        self
    }

    /// Glyphs.
    #[must_use]
    pub const fn glyphs(mut self, glyphs: VizGlyphSet) -> Self {
        self.glyphs = glyphs;
        self
    }

    /// Vertical columns (default) vs horizontal bars.
    #[must_use]
    pub const fn vertical(mut self, on: bool) -> Self {
        self.vertical = on;
        self
    }

    /// Selected bucket.
    #[must_use]
    pub const fn selected(mut self, i: usize) -> Self {
        self.selected = Some(i);
        self
    }

    /// Title.
    #[must_use]
    pub const fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }
}

impl Widget for &Histogram<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() || self.buckets.is_empty() {
            return;
        }
        let mut y = area.y;
        let mut h = area.height;
        if let Some(title) = self.title {
            if h > 0 {
                buffer.set_stringn(
                    area.x,
                    y,
                    take_display_cols(title, usize::from(area.width)),
                    usize::from(area.width),
                    self.system.style(Role::TextStrong),
                );
                y = y.saturating_add(1);
                h = h.saturating_sub(1);
            }
        }
        if h == 0 {
            return;
        }
        let domain = resolve_domain(
            self.scale,
            self.buckets.iter().map(|b| {
                if b.count.is_finite() && b.count > 0.0 {
                    b.count
                } else {
                    f64::NAN
                }
            }),
        );
        // ensure min at least 0 for counts
        let domain = if !domain.log && domain.min > 0.0 {
            ScaleDomain {
                min: 0.0,
                max: domain.max,
                log: false,
            }
        } else {
            domain
        };

        let gset = self.glyphs.resolve(self.system.glyphs);
        let ladder = gset.ladder();
        let fill = *ladder.last().unwrap_or(&'#');
        let miss = gset.missing_mark();

        if self.vertical {
            let n = self.buckets.len().min(usize::from(area.width));
            let col_w = usize::from(area.width) / n.max(1);
            for (i, bucket) in self.buckets.iter().take(n).enumerate() {
                let frac = domain.normalize(bucket.count).unwrap_or(0.0);
                let bar_h = ((f64::from(h.saturating_sub(1)) * frac).round() as u16)
                    .min(h.saturating_sub(1));
                let x = area.x.saturating_add((i * col_w.max(1)) as u16);
                let selected = self.selected == Some(i);
                let style = if selected {
                    self.system
                        .style(Role::TextStrong)
                        .add_modifier(Modifier::BOLD)
                } else if matches!(
                    self.system.capability,
                    crate::style::ColorCapability::Monochrome
                ) {
                    self.system.style(Role::Text)
                } else {
                    self.system.style(Role::ChartSeries1)
                };
                let ch = if selected {
                    if matches!(gset, VizGlyphSet::Ascii) {
                        'X'
                    } else {
                        '█'
                    }
                } else {
                    fill
                };
                // base label
                let label_y = y.saturating_add(h.saturating_sub(1));
                let lab = take_display_cols(bucket.label, col_w.max(1));
                buffer.set_stringn(
                    x,
                    label_y,
                    lab,
                    col_w.max(1),
                    self.system.style(Role::ChartAxis),
                );
                for row in 0..bar_h {
                    let py = label_y.saturating_sub(1).saturating_sub(row);
                    if py >= y {
                        buffer.set_stringn(x, py, ch.to_string(), 1, style);
                    }
                }
                if bar_h == 0 && !bucket.count.is_finite() {
                    buffer.set_stringn(
                        x,
                        label_y.saturating_sub(1).max(y),
                        miss.to_string(),
                        1,
                        self.system.style(Role::TextMuted),
                    );
                }
            }
        } else {
            // horizontal: reuse bar series style
            let rows = usize::from(h).min(self.buckets.len());
            let label_w = self
                .buckets
                .iter()
                .take(rows)
                .map(|b| display_cols(b.label))
                .max()
                .unwrap_or(0)
                .min(usize::from(area.width) / 3)
                .min(10);
            for (row, bucket) in self.buckets.iter().take(rows).enumerate() {
                let py = y.saturating_add(row as u16);
                let lab = take_display_cols(bucket.label, label_w);
                buffer.set_stringn(area.x, py, lab, label_w, self.system.style(Role::TextMuted));
                let track_x = area.x.saturating_add(label_w as u16);
                let track_w = area.width.saturating_sub(label_w as u16);
                if track_w == 0 {
                    continue;
                }
                let frac = domain.normalize(bucket.count).unwrap_or(0.0);
                let filled = ((f64::from(track_w) * frac).round() as u16).min(track_w);
                let selected = self.selected == Some(row);
                let style = if selected {
                    self.system
                        .style(Role::TextStrong)
                        .add_modifier(Modifier::BOLD)
                } else if matches!(
                    self.system.capability,
                    crate::style::ColorCapability::Monochrome
                ) {
                    self.system.style(Role::Text)
                } else {
                    self.system.style(Role::ChartSeries2)
                };
                let ch = fill.to_string();
                buffer.set_stringn(
                    track_x,
                    py,
                    &ch.repeat(usize::from(filled)),
                    usize::from(filled),
                    style,
                );
            }
        }
    }
}

impl Widget for Histogram<'_> {
    #[expect(
        clippy::needless_borrows_for_generic_args,
        reason = "explicitly delegate the owned contract to the borrowed renderer"
    )]
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

// ── BarSeries (migrated onto shared glyphs/scale) ───────────────────────────

/// One named bar in a horizontal bar series (shadcn bar-chart peer).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarDatum<'a> {
    /// Label shown to the left when width allows.
    pub label: &'a str,
    /// Fraction filled (`0.0..=1.0`) **or** raw value when used with scale on BarSeries.
    /// Ignored for layout when [`Self::segments`] is non-empty (stack uses segments).
    pub fraction: f64,
    /// Optional stacked segment values (non-negative). Empty → solid bar of `fraction`.
    pub segments: &'a [f64],
}

impl<'a> BarDatum<'a> {
    /// Normalized solid bar (0..=1).
    #[must_use]
    pub const fn new(label: &'a str, fraction: f64) -> Self {
        Self {
            label,
            fraction,
            segments: &[],
        }
    }

    /// Raw solid value (pair with [`BarSeries::scale`]).
    #[must_use]
    pub const fn value(label: &'a str, value: f64) -> Self {
        Self {
            label,
            fraction: value,
            segments: &[],
        }
    }

    /// Stacked bar: segment weights (shadcn `chart-bar-stacked` peer).
    ///
    /// Segments should be non-negative; domain uses sum (and baseline 0).
    #[must_use]
    pub const fn stacked(label: &'a str, segments: &'a [f64]) -> Self {
        Self {
            label,
            fraction: 0.0,
            segments,
        }
    }

    /// Whether this bar uses multi-segment stack paint.
    #[must_use]
    pub const fn is_stacked(self) -> bool {
        !self.segments.is_empty()
    }
}

/// Multi-row horizontal bar chart (solid, stacked, and bipolar/negative).
#[derive(Debug, Clone, Copy)]
pub struct BarSeries<'a> {
    bars: &'a [BarDatum<'a>],
    system: &'a DesignSystem,
    scale: ScaleMode,
    glyphs: VizGlyphSet,
    selected: Option<usize>,
    pre_normalized: bool,
}

impl<'a> BarSeries<'a> {
    /// Creates a bar series (default: values treated as fractions 0..=1).
    #[must_use]
    pub const fn new(bars: &'a [BarDatum<'a>], system: &'a DesignSystem) -> Self {
        Self {
            bars,
            system,
            scale: ScaleMode::Fixed { min: 0.0, max: 1.0 },
            glyphs: VizGlyphSet::Auto,
            selected: None,
            pre_normalized: true,
        }
    }

    /// Treat `fraction` / segments as raw values with scale.
    #[must_use]
    pub const fn scale(mut self, scale: ScaleMode) -> Self {
        self.scale = scale;
        self.pre_normalized = false;
        self
    }

    /// Glyphs.
    #[must_use]
    pub const fn glyphs(mut self, glyphs: VizGlyphSet) -> Self {
        self.glyphs = glyphs;
        self
    }

    /// Selected bar.
    #[must_use]
    pub const fn selected(mut self, i: usize) -> Self {
        self.selected = Some(i);
        self
    }
}

impl Widget for &BarSeries<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let rows = usize::from(area.height).min(self.bars.len());
        let label_width = self
            .bars
            .iter()
            .take(rows)
            .map(|bar| display_cols(bar.label))
            .max()
            .unwrap_or(0)
            .min(usize::from(area.width) / 3)
            .min(12);

        // Domain: solid values + stacked sums + baseline 0 (bipolar / stack)
        let domain = if self.pre_normalized {
            ScaleDomain::unit()
        } else {
            let mut vals: Vec<f64> = vec![0.0];
            for b in self.bars.iter().take(rows) {
                if b.is_stacked() {
                    let sum: f64 = b
                        .segments
                        .iter()
                        .filter(|v| v.is_finite())
                        .map(|v| v.max(0.0))
                        .sum();
                    vals.push(sum);
                } else if b.fraction.is_finite() {
                    vals.push(b.fraction);
                }
            }
            resolve_domain(self.scale, vals.into_iter())
        };

        let gset = self.glyphs.resolve(self.system.glyphs);
        let fill_ch = *gset.ladder().last().unwrap_or(&'#');
        let empty_ch = match gset {
            VizGlyphSet::Ascii => '-',
            _ => '░',
        };
        let neg_ch = if matches!(gset, VizGlyphSet::Ascii) {
            '='
        } else {
            '▒'
        };

        // Zero column for bipolar (fraction 0 of domain)
        let zero_frac = domain.normalize(0.0).unwrap_or(0.0);
        let bipolar = !self.pre_normalized
            && self
                .bars
                .iter()
                .take(rows)
                .any(|b| !b.is_stacked() && b.fraction.is_finite() && b.fraction < 0.0);

        for (row, bar) in self.bars.iter().take(rows).enumerate() {
            let y = area.y.saturating_add(row as u16);
            let label = take_display_cols(bar.label, label_width);
            if label_width > 0 {
                buffer.set_stringn(
                    area.x,
                    y,
                    label,
                    label_width,
                    self.system.style(Role::TextMuted),
                );
            }
            let track_x = area
                .x
                .saturating_add(u16::try_from(label_width).unwrap_or(0));
            let track_w = area
                .width
                .saturating_sub(u16::try_from(label_width).unwrap_or(0));
            if track_w == 0 {
                continue;
            }

            // Empty track first
            buffer.set_stringn(
                track_x,
                y,
                &empty_ch.to_string().repeat(usize::from(track_w)),
                usize::from(track_w),
                self.system.style(Role::TextDisabled),
            );

            let selected = self.selected == Some(row);

            if bar.is_stacked() {
                // Stacked non-negative segments left→right from domain min (0)
                let sum: f64 = bar
                    .segments
                    .iter()
                    .filter(|v| v.is_finite())
                    .map(|v| v.max(0.0))
                    .sum();
                if sum <= 0.0 || !sum.is_finite() {
                    continue;
                }
                let mut x = track_x;
                let mut painted = 0u16;
                for (si, seg) in bar.segments.iter().enumerate() {
                    let v = if seg.is_finite() { seg.max(0.0) } else { 0.0 };
                    if v <= 0.0 {
                        continue;
                    }
                    // Segment share of the full bar width (bar width ∝ sum/domain).
                    let frac = if self.pre_normalized {
                        (v / sum).clamp(0.0, 1.0)
                    } else {
                        let total_f = domain.normalize(sum).unwrap_or(1.0);
                        if total_f <= 0.0 {
                            v / sum
                        } else {
                            (v / sum) * total_f
                        }
                    };
                    let mut seg_w = ((f64::from(track_w) * frac).round() as u16).max(if v > 0.0 {
                        1
                    } else {
                        0
                    });
                    if painted + seg_w > track_w {
                        seg_w = track_w.saturating_sub(painted);
                    }
                    if seg_w == 0 {
                        continue;
                    }
                    let style = if selected {
                        self.system
                            .style(Role::TextStrong)
                            .add_modifier(Modifier::BOLD)
                    } else if matches!(
                        self.system.capability,
                        crate::style::ColorCapability::Monochrome
                    ) {
                        // Distinct no-color markers per segment
                        self.system.style(Role::Text)
                    } else {
                        self.system.style(series_role(si))
                    };
                    let ch = if matches!(
                        self.system.capability,
                        crate::style::ColorCapability::Monochrome
                    ) {
                        gset.series_marker(si)
                    } else {
                        fill_ch
                    };
                    buffer.set_stringn(
                        x,
                        y,
                        &ch.to_string().repeat(usize::from(seg_w)),
                        usize::from(seg_w),
                        style,
                    );
                    x = x.saturating_add(seg_w);
                    painted = painted.saturating_add(seg_w);
                    if painted >= track_w {
                        break;
                    }
                }
                continue;
            }

            // Solid bar (positive and/or negative)
            let value = bar.fraction;
            if !value.is_finite() {
                continue;
            }

            if self.pre_normalized || !bipolar {
                let fraction = if self.pre_normalized {
                    value.clamp(0.0, 1.0)
                } else {
                    domain.normalize(value).unwrap_or(0.0)
                };
                let filled = ((f64::from(track_w) * fraction).round() as u16).min(track_w);
                let style = if selected {
                    self.system
                        .style(Role::TextStrong)
                        .add_modifier(Modifier::BOLD)
                } else if matches!(
                    self.system.capability,
                    crate::style::ColorCapability::Monochrome
                ) {
                    self.system.style(Role::Text)
                } else {
                    self.system.style(Role::Accent)
                };
                if filled > 0 {
                    buffer.set_stringn(
                        track_x,
                        y,
                        &fill_ch.to_string().repeat(usize::from(filled)),
                        usize::from(filled),
                        style,
                    );
                }
            } else {
                // Bipolar: zero baseline; + to the right, − to the left
                let z = ((f64::from(track_w) * zero_frac).round() as u16).min(track_w);
                let vf = domain.normalize(value).unwrap_or(zero_frac);
                let vcol = ((f64::from(track_w) * vf).round() as u16).min(track_w);
                let pos_style = if selected {
                    self.system
                        .style(Role::TextStrong)
                        .add_modifier(Modifier::BOLD)
                } else if matches!(
                    self.system.capability,
                    crate::style::ColorCapability::Monochrome
                ) {
                    self.system.style(Role::Text)
                } else {
                    self.system.style(Role::Success)
                };
                let neg_style = if selected {
                    self.system
                        .style(Role::TextStrong)
                        .add_modifier(Modifier::BOLD)
                } else if matches!(
                    self.system.capability,
                    crate::style::ColorCapability::Monochrome
                ) {
                    self.system.style(Role::TextMuted)
                } else {
                    self.system.style(Role::Danger)
                };
                // zero tick
                if z < track_w {
                    buffer.set_stringn(
                        track_x.saturating_add(z),
                        y,
                        "|",
                        1,
                        self.system.style(Role::ChartAxis),
                    );
                }
                if value >= 0.0 {
                    let start = z.min(vcol);
                    let end = z.max(vcol);
                    let w = end
                        .saturating_sub(start)
                        .max(if value > 0.0 { 1 } else { 0 });
                    if w > 0 {
                        buffer.set_stringn(
                            track_x.saturating_add(start),
                            y,
                            &fill_ch.to_string().repeat(usize::from(w)),
                            usize::from(w),
                            pos_style,
                        );
                    }
                } else {
                    let start = vcol.min(z);
                    let end = vcol.max(z);
                    let w = end.saturating_sub(start).max(1);
                    buffer.set_stringn(
                        track_x.saturating_add(start),
                        y,
                        &neg_ch.to_string().repeat(usize::from(w)),
                        usize::from(w),
                        neg_style,
                    );
                }
            }
        }
    }
}

impl Widget for BarSeries<'_> {
    #[expect(
        clippy::needless_borrows_for_generic_args,
        reason = "explicitly delegate the owned contract to the borrowed renderer"
    )]
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

// ── SegmentedMeter (part-to-whole / pie peer) ───────────────────────────────

/// One segment in a segmented meter (shadcn pie chart peer).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeterSegment<'a> {
    /// Segment label for narrow fallback text / label row.
    pub label: &'a str,
    /// Relative weight (normalized across segments). Zero/NaN → no mass.
    pub weight: f64,
    /// Semantic role used for the fill.
    pub role: Role,
}

impl<'a> MeterSegment<'a> {
    /// Construct.
    #[must_use]
    pub const fn new(label: &'a str, weight: f64, role: Role) -> Self {
        Self {
            label,
            weight,
            role,
        }
    }

    /// Finite positive weight, else 0.
    #[must_use]
    pub fn effective_weight(self) -> f64 {
        if self.weight.is_finite() && self.weight > 0.0 {
            self.weight
        } else {
            0.0
        }
    }
}

/// Allocate integer column widths for segment weights that sum to `total_cols`.
///
/// Zero/NaN weights get **0** columns (no invented mass). Rounding remainder
/// goes to the last **positive** segment so the track is full when total > 0.
#[must_use]
pub fn allocate_segment_widths(weights: &[f64], total_cols: u16) -> Vec<u16> {
    let n = weights.len();
    let mut out = vec![0u16; n];
    if n == 0 || total_cols == 0 {
        return out;
    }
    let sum: f64 = weights
        .iter()
        .map(|w| if w.is_finite() && *w > 0.0 { *w } else { 0.0 })
        .sum();
    if sum <= 0.0 {
        return out;
    }
    let mut used = 0u16;
    let mut last_pos = None;
    for (i, w) in weights.iter().enumerate() {
        let ew = if w.is_finite() && *w > 0.0 { *w } else { 0.0 };
        if ew <= 0.0 {
            continue;
        }
        let width = ((f64::from(total_cols) * ew / sum).round() as u16).max(1);
        let width = width.min(total_cols.saturating_sub(used));
        out[i] = width;
        used = used.saturating_add(width);
        last_pos = Some(i);
        if used >= total_cols {
            break;
        }
    }
    // Give remaining columns to last positive segment (not a zero-weight tail).
    if let Some(i) = last_pos {
        if used < total_cols {
            out[i] = out[i].saturating_add(total_cols - used);
        }
    }
    out
}

/// Single-row segmented meter (part-to-whole proportions; pie chart TUI peer).
#[derive(Debug, Clone, Copy)]
pub struct SegmentedMeter<'a> {
    segments: &'a [MeterSegment<'a>],
    system: &'a DesignSystem,
    glyphs: VizGlyphSet,
    selected: Option<usize>,
    show_labels: bool,
    /// Insert 1-col gap between segments (default continuous = separator-none peer).
    separators: bool,
    /// Optional center caption (donut-text peer); paints on row below when height ≥ 2.
    center: Option<&'a str>,
}

impl<'a> SegmentedMeter<'a> {
    /// Creates a segmented meter (continuous segments, no separators).
    #[must_use]
    pub const fn new(segments: &'a [MeterSegment<'a>], system: &'a DesignSystem) -> Self {
        Self {
            segments,
            system,
            glyphs: VizGlyphSet::Auto,
            selected: None,
            show_labels: false,
            separators: false,
            center: None,
        }
    }

    /// Glyphs.
    #[must_use]
    pub const fn glyphs(mut self, glyphs: VizGlyphSet) -> Self {
        self.glyphs = glyphs;
        self
    }

    /// Selected / active segment (chart-pie-interactive / donut-active peer).
    #[must_use]
    pub const fn selected(mut self, i: usize) -> Self {
        self.selected = Some(i);
        self
    }

    /// Show per-segment labels on row under the bar when height ≥ 2.
    #[must_use]
    pub const fn show_labels(mut self, on: bool) -> Self {
        self.show_labels = on;
        self
    }

    /// 1-column gaps between segments (default off = separator-none continuous).
    #[must_use]
    pub const fn separators(mut self, on: bool) -> Self {
        self.separators = on;
        self
    }

    /// Center caption under the meter (donut-text peer; needs height ≥ 2).
    #[must_use]
    pub const fn center(mut self, text: &'a str) -> Self {
        self.center = Some(text);
        self
    }
}

impl Widget for &SegmentedMeter<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() || self.segments.is_empty() {
            return;
        }
        let gset = self.glyphs.resolve(self.system.glyphs);
        let fill_ch = *gset.ladder().last().unwrap_or(&'#');
        let sel_ch = if matches!(gset, VizGlyphSet::Ascii) {
            'X'
        } else {
            '█'
        };

        let weights: Vec<f64> = self.segments.iter().map(|s| s.effective_weight()).collect();
        let positive = weights.iter().filter(|w| **w > 0.0).count();
        let gap_cols = if self.separators && positive > 1 {
            (positive - 1) as u16
        } else {
            0
        };
        let track_w = area.width.saturating_sub(gap_cols);
        let widths = allocate_segment_widths(&weights, track_w);

        let label_h = u16::from(self.show_labels && area.height >= 2);
        let center_h = u16::from(self.center.is_some() && area.height >= 2 + label_h);
        let bar_y = area.y;
        let mut x = area.x;
        let mut painted_positive = 0usize;

        for (index, segment) in self.segments.iter().enumerate() {
            let width = widths.get(index).copied().unwrap_or(0);
            if width == 0 {
                continue;
            }
            if self.separators && painted_positive > 0 {
                // 1-col separator
                buffer.set_stringn(x, bar_y, " ", 1, self.system.style(Role::TextDisabled));
                x = x.saturating_add(1);
            }
            painted_positive += 1;

            let selected = self.selected == Some(index);
            let ch = if selected {
                sel_ch
            } else if matches!(
                self.system.capability,
                crate::style::ColorCapability::Monochrome
            ) {
                gset.series_marker(index)
            } else {
                fill_ch
            };
            let style = if selected {
                self.system
                    .style(Role::TextStrong)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else if matches!(
                self.system.capability,
                crate::style::ColorCapability::Monochrome
            ) {
                self.system.style(Role::Text)
            } else {
                self.system.style(segment.role)
            };
            let fill = ch.to_string().repeat(usize::from(width));
            buffer.set_stringn(x, bar_y, &fill, usize::from(width), style);
            if label_h > 0 {
                let lab = take_display_cols(segment.label, usize::from(width));
                buffer.set_stringn(
                    x,
                    bar_y.saturating_add(1),
                    lab,
                    usize::from(width),
                    self.system.style(Role::TextMuted),
                );
            }
            x = x.saturating_add(width);
        }

        // Center caption (donut-text) on last available row
        if let Some(text) = self.center {
            if center_h > 0 || (area.height >= 2 && !self.show_labels) {
                let cy = if self.show_labels && area.height >= 3 {
                    area.y.saturating_add(2)
                } else if area.height >= 2 {
                    area.y.saturating_add(1)
                } else {
                    area.y
                };
                let t = take_display_cols(text, usize::from(area.width));
                let tw = display_cols(&t) as u16;
                let cx = area.x.saturating_add(area.width.saturating_sub(tw) / 2);
                buffer.set_stringn(
                    cx,
                    cy,
                    &t,
                    usize::from(area.width),
                    self.system.style(Role::TextStrong),
                );
            }
        }
    }
}

impl Widget for SegmentedMeter<'_> {
    #[expect(
        clippy::needless_borrows_for_generic_args,
        reason = "explicitly delegate the owned contract to the borrowed renderer"
    )]
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

// ── MetricRadar (multi-axis comparison; shadcn radar TUI peer) ───────────────

/// One metric axis on a multi-metric comparison (radar substitute).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetricAxis<'a> {
    /// Short label (row header).
    pub label: &'a str,
}

impl<'a> MetricAxis<'a> {
    /// Construct.
    #[must_use]
    pub const fn new(label: &'a str) -> Self {
        Self { label }
    }
}

/// One series of values aligned with axes (len should match axis count).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetricSeries<'a> {
    /// Legend label.
    pub label: &'a str,
    /// Per-axis values (non-finite = missing).
    pub values: &'a [f64],
}

impl<'a> MetricSeries<'a> {
    /// Construct.
    #[must_use]
    pub const fn new(label: &'a str, values: &'a [f64]) -> Self {
        Self { label, values }
    }
}

/// Multi-metric / multi-axis comparison chart (shadcn **radar** job without polar theater).
///
/// Each axis is a labeled row; each series paints a horizontal value bar on that
/// row (grouped when multi-series). Shared scale/glyphs. Polar grid/SVG radar
/// remains N/A — this is the terminal-honest peer.
#[derive(Debug, Clone, Copy)]
pub struct MetricRadar<'a> {
    axes: &'a [MetricAxis<'a>],
    series: &'a [MetricSeries<'a>],
    system: &'a DesignSystem,
    scale: ScaleMode,
    glyphs: VizGlyphSet,
    selected_axis: Option<usize>,
    selected_series: Option<usize>,
    show_legend: bool,
    title: Option<&'a str>,
}

impl<'a> MetricRadar<'a> {
    /// Axes + series + design system.
    #[must_use]
    pub const fn new(
        axes: &'a [MetricAxis<'a>],
        series: &'a [MetricSeries<'a>],
        system: &'a DesignSystem,
    ) -> Self {
        Self {
            axes,
            series,
            system,
            scale: ScaleMode::Auto,
            glyphs: VizGlyphSet::Auto,
            selected_axis: None,
            selected_series: None,
            show_legend: true,
            title: None,
        }
    }

    /// Scale (Auto fits finite values across all series; Fixed for unit scores).
    #[must_use]
    pub const fn scale(mut self, scale: ScaleMode) -> Self {
        self.scale = scale;
        self
    }

    /// Glyphs.
    #[must_use]
    pub const fn glyphs(mut self, glyphs: VizGlyphSet) -> Self {
        self.glyphs = glyphs;
        self
    }

    /// Highlight axis row.
    #[must_use]
    pub const fn selected_axis(mut self, i: usize) -> Self {
        self.selected_axis = Some(i);
        self
    }

    /// Highlight series (emphasizes that series' bars).
    #[must_use]
    pub const fn selected_series(mut self, i: usize) -> Self {
        self.selected_series = Some(i);
        self
    }

    /// Legend row under title.
    #[must_use]
    pub const fn show_legend(mut self, on: bool) -> Self {
        self.show_legend = on;
        self
    }

    /// Title.
    #[must_use]
    pub const fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }
}

impl Widget for &MetricRadar<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() || self.axes.is_empty() || self.series.is_empty() {
            return;
        }
        let gset = self.glyphs.resolve(self.system.glyphs);
        let mut y = area.y;
        let mut h = area.height;

        if let Some(title) = self.title {
            if h > 0 {
                buffer.set_stringn(
                    area.x,
                    y,
                    take_display_cols(title, usize::from(area.width)),
                    usize::from(area.width),
                    self.system.style(Role::TextStrong),
                );
                y = y.saturating_add(1);
                h = h.saturating_sub(1);
            }
        }

        if self.show_legend && h > 0 && area.width >= 8 {
            let mut legend = String::new();
            for (i, s) in self.series.iter().enumerate() {
                if i > 0 {
                    legend.push(' ');
                }
                legend.push(gset.series_marker(i));
                legend.push(' ');
                legend.push_str(s.label);
            }
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&legend, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
            y = y.saturating_add(1);
            h = h.saturating_sub(1);
        }

        if h == 0 {
            return;
        }

        // Domain across all finite values (include 0 for bar baseline)
        let mut vals: Vec<f64> = vec![0.0];
        for s in self.series {
            for v in s.values {
                if v.is_finite() {
                    vals.push(*v);
                }
            }
        }
        let domain = resolve_domain(self.scale, vals.into_iter());

        let n_series = self.series.len().max(1);
        let label_w = self
            .axes
            .iter()
            .map(|a| display_cols(a.label))
            .max()
            .unwrap_or(0)
            .min(usize::from(area.width) / 3)
            .min(14);
        let track_w = area
            .width
            .saturating_sub(u16::try_from(label_w).unwrap_or(0));
        if track_w == 0 {
            return;
        }
        // Grouped bars: divide track among series with 1-col gap when multi
        let gap = u16::from(n_series > 1);
        let slots = n_series as u16;
        let gap_total = gap.saturating_mul(slots.saturating_sub(1));
        let bar_w = track_w.saturating_sub(gap_total) / slots.max(1);
        if bar_w == 0 {
            return;
        }

        let rows = usize::from(h).min(self.axes.len());
        let miss = gset.missing_mark();
        let fill_ch = *gset.ladder().last().unwrap_or(&'#');

        for (ai, axis) in self.axes.iter().take(rows).enumerate() {
            let py = y.saturating_add(ai as u16);
            let axis_selected = self.selected_axis == Some(ai);
            let lab = take_display_cols(axis.label, label_w);
            let lab_style = if axis_selected {
                self.system
                    .style(Role::TextStrong)
                    .add_modifier(Modifier::BOLD)
            } else {
                self.system.style(Role::TextMuted)
            };
            if label_w > 0 {
                buffer.set_stringn(area.x, py, lab, label_w, lab_style);
            }
            let track_x = area.x.saturating_add(u16::try_from(label_w).unwrap_or(0));

            for (si, series) in self.series.iter().enumerate() {
                let bx = track_x.saturating_add(si as u16 * (bar_w + gap));
                let val = series.values.get(ai).copied().unwrap_or(f64::NAN);
                if !val.is_finite() {
                    buffer.set_stringn(
                        bx,
                        py,
                        miss.to_string(),
                        1,
                        self.system.style(Role::TextMuted),
                    );
                    continue;
                }
                let frac = domain.normalize(val).unwrap_or(0.0);
                let filled = ((f64::from(bar_w) * frac).round() as u16)
                    .min(bar_w)
                    .max(if val > 0.0 && bar_w > 0 { 1 } else { 0 });
                let series_selected = self.selected_series == Some(si);
                let selected = series_selected || axis_selected;
                let ch = if selected && matches!(gset, VizGlyphSet::Ascii) {
                    'X'
                } else if matches!(
                    self.system.capability,
                    crate::style::ColorCapability::Monochrome
                ) {
                    gset.series_marker(si)
                } else if selected {
                    '█'
                } else {
                    fill_ch
                };
                let style = if selected {
                    self.system
                        .style(Role::TextStrong)
                        .add_modifier(Modifier::BOLD)
                } else if matches!(
                    self.system.capability,
                    crate::style::ColorCapability::Monochrome
                ) {
                    self.system.style(Role::Text)
                } else {
                    self.system.style(series_role(si))
                };
                // empty track
                let empty = match gset {
                    VizGlyphSet::Ascii => '-',
                    _ => '░',
                };
                buffer.set_stringn(
                    bx,
                    py,
                    &empty.to_string().repeat(usize::from(bar_w)),
                    usize::from(bar_w),
                    self.system.style(Role::TextDisabled),
                );
                if filled > 0 {
                    buffer.set_stringn(
                        bx,
                        py,
                        &ch.to_string().repeat(usize::from(filled)),
                        usize::from(filled),
                        style,
                    );
                }
            }
        }
    }
}

impl Widget for MetricRadar<'_> {
    #[expect(
        clippy::needless_borrows_for_generic_args,
        reason = "explicitly delegate the owned contract to the borrowed renderer"
    )]
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Streaming / tiny-dimension targets.
pub mod bench {
    /// Samples/sec append for sparkline/chart windows.
    pub const SAMPLES_PER_SEC: u32 = 60;
    /// Window width for streaming benchmarks.
    pub const STREAM_WINDOW: usize = 120;
    /// Tiny width.
    pub const TINY_WIDTH: u16 = 8;
    /// Tiny height.
    pub const TINY_HEIGHT: u16 = 1;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn system() -> DesignSystem {
        DesignSystem::from_palette(RolePalette::default())
    }

    fn system_ascii_nocolor() -> DesignSystem {
        DesignSystem::from_palette(RolePalette::default())
            .glyphs(GlyphSet::Ascii)
            .no_color()
    }

    #[test]
    fn scale_auto_fixed_log() {
        let d = resolve_domain(ScaleMode::Auto, [1.0, 2.0, 3.0].into_iter());
        assert!((d.min - 1.0).abs() < 1e-9);
        assert!((d.max - 3.0).abs() < 1e-9);
        assert_eq!(d.normalize(2.0), Some(0.5));

        let d = resolve_domain(
            ScaleMode::Fixed {
                min: 0.0,
                max: 100.0,
            },
            std::iter::empty(),
        );
        assert_eq!(d.normalize(50.0), Some(0.5));

        let d = resolve_domain(
            ScaleMode::Log { fixed: None },
            [1.0, 10.0, 100.0].into_iter(),
        );
        assert!(d.log);
        let mid = d.normalize(10.0).unwrap();
        assert!((mid - 0.5).abs() < 0.01);
    }

    #[test]
    fn missing_and_nan_samples() {
        let d = resolve_domain(ScaleMode::Auto, [1.0, f64::NAN, 3.0].into_iter());
        assert!(d.normalize(f64::NAN).is_none());
        assert!(d.normalize(2.0).is_some());
    }

    #[test]
    fn glyph_fallbacks_ascii() {
        let a = VizGlyphSet::Auto.resolve(GlyphSet::Ascii);
        assert_eq!(a, VizGlyphSet::Ascii);
        let g = glyph_for_fraction(1.0, a.ladder(), false, '?');
        assert_eq!(g, '@');
        let m = glyph_for_fraction(0.0, a.ladder(), true, '?');
        assert_eq!(m, '?');
    }

    #[test]
    fn sparkline_uses_block_glyphs() {
        let system = system();
        let samples = [0.0, 0.5, 1.0];
        let mut buffer = Buffer::empty(Rect::new(0, 0, 3, 1));
        Sparkline::new(&samples, &system)
            .pre_normalized(true)
            .render(Rect::new(0, 0, 3, 1), &mut buffer);
        assert_ne!(buffer[(0, 0)].symbol(), buffer[(2, 0)].symbol());
    }

    #[test]
    fn sparkline_autoscale_raw_and_selected() {
        let system = system_ascii_nocolor();
        // Values span domain; threshold equals a sample so band mark paints.
        let samples = [10.0, 20.0, 40.0, f64::NAN, 30.0];
        let area = Rect::new(0, 0, 10, 1);
        let mut plain = Buffer::empty(area);
        Sparkline::new(&samples, &system)
            .glyphs(VizGlyphSet::Ascii)
            .threshold(20.0)
            .render(area, &mut plain);
        let mut selected = Buffer::empty(area);
        Sparkline::new(&samples, &system)
            .glyphs(VizGlyphSet::Ascii)
            .selected(1)
            .threshold(20.0)
            .render(area, &mut selected);
        let plain_s: String = (0..10u16)
            .map(|x| plain[(x, 0)].symbol().to_string())
            .collect();
        let sel_s: String = (0..10u16)
            .map(|x| selected[(x, 0)].symbol().to_string())
            .collect();
        assert_ne!(
            plain_s, sel_s,
            "selection must change sparkline paint: plain={plain_s:?} sel={sel_s:?}"
        );
        // ASCII selected uses X
        assert!(
            sel_s.contains('X'),
            "selected sample highlight X: {sel_s:?}"
        );
        // Sample index 1 maps to cols — selection overrides threshold glyph there;
        // unselected path still paints threshold mark at samples near 20.
        let tick = VizGlyphSet::Ascii.threshold_mark();
        assert!(
            plain_s.contains(tick),
            "threshold mark on unselected sparkline: plain={plain_s:?}"
        );
    }

    #[test]
    fn chart_selection_changes_paint_vs_unselected() {
        let system = system_ascii_nocolor();
        let a = [1.0, 2.0, 5.0, 2.0, 1.0];
        let series = [ChartSeries::new("s", &a)];
        let area = Rect::new(0, 0, 28, 6);
        let mut plain = Buffer::empty(area);
        Chart::new(&series, &system)
            .glyphs(VizGlyphSet::Ascii)
            .show_legend(false)
            .show_axes(false)
            .render(area, &mut plain);
        let mut selected = Buffer::empty(area);
        Chart::new(&series, &system)
            .glyphs(VizGlyphSet::Ascii)
            .show_legend(false)
            .show_axes(false)
            .selected_series(0)
            .selected_index(2)
            .render(area, &mut selected);
        let cell = |buf: &Buffer| -> String {
            let mut s = String::new();
            for y in 0..6u16 {
                for x in 0..28u16 {
                    s.push_str(buf[(x, y)].symbol());
                }
                s.push('\n');
            }
            s
        };
        let plain_s = cell(&plain);
        let sel_s = cell(&selected);
        assert_ne!(
            plain_s, sel_s,
            "selected_index must change chart paint vs unselected"
        );
        assert!(sel_s.contains('X'), "ASCII selection highlight X: {sel_s}");
        assert!(
            !plain_s.contains('X'),
            "unselected chart must not paint X highlight: {plain_s}"
        );
    }

    #[test]
    fn chart_threshold_indicator_line_paints() {
        let system = system_ascii_nocolor();
        let a = [0.0, 10.0, 20.0, 30.0, 40.0];
        let series = [ChartSeries::new("s", &a)];
        let thr = [20.0];
        let area = Rect::new(0, 0, 30, 8);
        let mut buffer = Buffer::empty(area);
        Chart::new(&series, &system)
            .glyphs(VizGlyphSet::Ascii)
            .show_legend(false)
            .show_axes(false)
            .thresholds(&thr)
            .render(area, &mut buffer);
        let tick = VizGlyphSet::Ascii.threshold_mark();
        // Horizontal indicator: many threshold marks on one plot row
        let mut max_row_ticks = 0usize;
        for y in 0..8u16 {
            let n = (0..30u16)
                .filter(|&x| buffer[(x, y)].symbol().contains(tick))
                .count();
            max_row_ticks = max_row_ticks.max(n);
        }
        assert!(
            max_row_ticks >= 8,
            "threshold indicator line must span plot (got max_row_ticks={max_row_ticks})"
        );
    }

    #[test]
    fn sparkline_streaming_window() {
        let system = system();
        let mut samples = vec![0.0; 200];
        samples[199] = 100.0;
        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 1));
        for _ in 0..30 {
            Sparkline::new(&samples, &system)
                .window(20)
                .render(Rect::new(0, 0, 20, 1), &mut buffer);
            samples.push(samples.len() as f64);
        }
    }

    #[test]
    fn chart_multi_series_legend_axes() {
        let system = system_ascii_nocolor();
        // Distinct paths: a rises then falls; b falls then rises — both markers on plot
        let a = [1.0, 2.0, 4.0, 2.0, 1.0];
        let b = [4.0, 2.0, 1.0, 2.0, 4.0];
        let series = [ChartSeries::new("cpu", &a), ChartSeries::new("mem", &b)];
        let area = Rect::new(0, 0, 40, 8);
        let mut buffer = Buffer::empty(area);
        Chart::new(&series, &system)
            .title("Host")
            .glyphs(VizGlyphSet::Ascii)
            .show_legend(true)
            .show_axes(true)
            .selected_series(0)
            .selected_index(2)
            .render(area, &mut buffer);
        let text: String = buffer
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("cpu") && text.contains("mem"),
            "legend labels: {text}"
        );
        assert!(text.contains("Host"), "title: {text}");
        let m0 = VizGlyphSet::Ascii.series_marker(0);
        let m1 = VizGlyphSet::Ascii.series_marker(1);
        // Plot occupancy: both series markers (not legend-only — markers also on plot rows)
        let mut m0_plot = 0usize;
        let mut m1_plot = 0usize;
        let mut selected = 0usize;
        // Skip title+legend rows (~0-1); axes may use left cols
        for y in 2..8u16 {
            for x in 6..40u16 {
                let s = buffer[(x, y)].symbol();
                if s.contains(m0) {
                    m0_plot += 1;
                }
                if s.contains(m1) {
                    m1_plot += 1;
                }
                if s.contains('X') {
                    selected += 1;
                }
            }
        }
        assert!(m0_plot > 0, "series 0 marker on plot: {text}");
        assert!(m1_plot > 0, "series 1 marker on plot: {text}");
        assert!(
            selected > 0,
            "selected_index must paint highlight X: {text}"
        );
    }

    #[test]
    fn chart_line_step_differs_from_linear() {
        let system = system_ascii_nocolor();
        // Two samples: low then high — step holds low then jumps; linear ramps
        let s = [0.0, 10.0];
        let series = [ChartSeries::new("s", &s)];
        let area = Rect::new(0, 0, 20, 6);
        let mut step_buf = Buffer::empty(area);
        Chart::new(&series, &system)
            .step()
            .glyphs(VizGlyphSet::Ascii)
            .show_legend(false)
            .show_axes(false)
            .render(area, &mut step_buf);
        let mut lin_buf = Buffer::empty(area);
        Chart::new(&series, &system)
            .linear()
            .glyphs(VizGlyphSet::Ascii)
            .show_legend(false)
            .show_axes(false)
            .render(area, &mut lin_buf);
        // Collect outline rows per column for step vs linear
        let mark = VizGlyphSet::Ascii.series_marker(0);
        let row_of = |buf: &Buffer, x: u16| -> Option<u16> {
            for y in 0..6u16 {
                if buf[(x, y)].symbol().contains(mark) || buf[(x, y)].symbol().contains('X') {
                    return Some(y);
                }
            }
            None
        };
        let mut step_rows = Vec::new();
        let mut lin_rows = Vec::new();
        for x in 0..20u16 {
            if let Some(r) = row_of(&step_buf, x) {
                step_rows.push(r);
            }
            if let Some(r) = row_of(&lin_buf, x) {
                lin_rows.push(r);
            }
        }
        assert!(
            step_rows.len() >= 10 && lin_rows.len() >= 10,
            "need plot occupancy step={step_rows:?} lin={lin_rows:?}"
        );
        // Distinct paths: row sequences must differ for at least one column
        let differ = step_rows.iter().zip(lin_rows.iter()).any(|(a, b)| a != b);
        assert!(
            differ,
            "step and linear must differ in row occupancy: step={step_rows:?} lin={lin_rows:?}"
        );
        // Step should hold a constant row for a prefix of columns
        let first = step_rows[0];
        let hold = step_rows.iter().take_while(|&&r| r == first).count();
        assert!(
            hold >= 2,
            "step should hold first sample across ≥2 cols, hold={hold} rows={step_rows:?}"
        );
        assert_eq!(ChartInterpolation::Step.id(), "step");
        assert_eq!(ChartInterpolation::Linear.id(), "linear");
    }

    #[test]
    fn chart_line_missing_nan_does_not_collapse() {
        let system = system_ascii_nocolor();
        let s = [1.0, f64::NAN, 3.0, 2.0];
        let series = [ChartSeries::new("s", &s)];
        let area = Rect::new(0, 0, 24, 5);
        let mut buffer = Buffer::empty(area);
        Chart::new(&series, &system)
            .glyphs(VizGlyphSet::Ascii)
            .show_legend(false)
            .show_axes(false)
            .render(area, &mut buffer);
        let miss = VizGlyphSet::Ascii.missing_mark();
        let mark = VizGlyphSet::Ascii.series_marker(0);
        let text: String = buffer
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(text.contains(mark), "finite samples still paint: {text}");
        // Missing may paint miss glyph or skip; finite markers must remain
        let finite = text.chars().filter(|&c| c == mark).count();
        assert!(finite >= 2, "need multiple finite outline points: {text}");
        let _ = miss;
    }

    #[test]
    fn chart_area_fill_paints_below_outline() {
        let system = system();
        // Constant high value → outline near top; fill occupies lower rows
        let a = [10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0];
        let series = [ChartSeries::new("area", &a)];
        let area = Rect::new(0, 0, 24, 8);
        let mut line_buf = Buffer::empty(area);
        Chart::new(&series, &system)
            .show_legend(false)
            .show_axes(false)
            .render(area, &mut line_buf);
        let mut area_buf = Buffer::empty(area);
        Chart::new(&series, &system)
            .area()
            .show_legend(false)
            .show_axes(false)
            .render(area, &mut area_buf);
        // Area fill must paint more non-space cells than pure line
        let count = |buf: &Buffer| -> usize {
            buf.content()
                .iter()
                .filter(|c| {
                    let s = c.symbol();
                    !s.is_empty() && s != " "
                })
                .count()
        };
        let line_n = count(&line_buf);
        let area_n = count(&area_buf);
        assert!(
            area_n > line_n,
            "area fill should paint more cells than line: area={area_n} line={line_n}"
        );
        // Bottom row of plot should have fill (not empty)
        let bottom_filled = (0..24u16).any(|x| {
            let s = area_buf[(x, 7)].symbol();
            !s.is_empty() && s != " "
        });
        assert!(bottom_filled, "expected fill near baseline");
    }

    #[test]
    fn chart_area_stacked_uses_cumulative_domain() {
        let system = system();
        // Equal constant series: total=2 at every index → domain [0, 2]
        let a = [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0];
        let b = [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0];
        let series = [ChartSeries::new("a", &a), ChartSeries::new("b", &b)];
        // Domain for stacked must include baseline 0 and max sum 2
        let mut domain_vals = vec![0.0];
        for i in 0..a.len() {
            domain_vals.push(a[i] + b[i]);
        }
        let d = resolve_domain(ScaleMode::Auto, domain_vals.into_iter());
        assert!(
            (d.min - 0.0).abs() < 1e-9,
            "stacked domain min must be 0, got {}",
            d.min
        );
        assert!(
            (d.max - 2.0).abs() < 1e-9,
            "stacked domain max must be sum, got {}",
            d.max
        );
        // Mid stack (cum=1) maps to ~0.5, not clamped to floor
        let mid = d.normalize(1.0).unwrap();
        assert!(
            (mid - 0.5).abs() < 0.05,
            "cum=1 should be mid domain, got {mid}"
        );

        let area = Rect::new(0, 0, 24, 8);
        let mut buffer = Buffer::empty(area);
        Chart::new(&series, &system)
            .area_stacked()
            .show_legend(false)
            .show_axes(false)
            .glyphs(VizGlyphSet::Ascii)
            .render(area, &mut buffer);

        // Both series markers must appear (stack order 0 then 1 — distinct markers)
        let mark0 = VizGlyphSet::Ascii.series_marker(0);
        let mark1 = VizGlyphSet::Ascii.series_marker(1);
        let mut has0 = false;
        let mut has1 = false;
        let mut fill_cells = 0usize;
        for cell in buffer.content() {
            let s = cell.symbol();
            if s.contains(mark0) {
                has0 = true;
            }
            if s.contains(mark1) {
                has1 = true;
            }
            if s == ":" {
                fill_cells += 1;
            }
        }
        assert!(
            has0,
            "series 0 outline marker {mark0:?} missing after stacked paint"
        );
        assert!(
            has1,
            "series 1 outline marker {mark1:?} missing after stacked paint"
        );
        assert!(fill_cells > 0, "stacked area must paint band fill glyphs");
        // Series 0 band sits lower: count outline markers on bottom half vs top half
        let mut mark0_bottom = 0usize;
        let mut mark1_top = 0usize;
        let mid_y = 4u16;
        for y in 0..8u16 {
            for x in 0..24u16 {
                let s = buffer[(x, y)].symbol();
                if s.contains(mark0) && y >= mid_y {
                    mark0_bottom += 1;
                }
                if s.contains(mark1) && y < mid_y {
                    mark1_top += 1;
                }
            }
        }
        assert!(
            mark0_bottom > 0,
            "series 0 (lower band) outline should appear in lower half of plot"
        );
        assert!(
            mark1_top > 0,
            "series 1 (upper band) outline should appear in upper half of plot"
        );
        assert_eq!(ChartFill::AreaStacked.id(), "area-stacked");
        assert_eq!(ChartFill::Area.id(), "area");
    }

    #[test]
    fn chart_area_stacked_domain_includes_baseline() {
        // Varying series: max cum = 3+2=5 at peak; partial cum=3 must not clamp
        let a = [3.0, 1.0, 2.0];
        let b = [2.0, 1.0, 1.0];
        let mut vals = vec![0.0];
        for i in 0..3 {
            vals.push(a[i] + b[i]);
        }
        let d = resolve_domain(ScaleMode::Auto, vals.into_iter());
        assert!((d.min - 0.0).abs() < 1e-9);
        assert!((d.max - 5.0).abs() < 1e-9);
        let partial = d.normalize(3.0).unwrap();
        assert!(
            partial > 0.4 && partial < 0.8,
            "partial cumulative 3/5 should be interior, got {partial}"
        );
    }

    #[test]
    fn chart_nocolor_readable() {
        let system = system_ascii_nocolor();
        let a = [0.0, 0.5, 1.0, 0.5];
        let series = [ChartSeries::new("s", &a)];
        let mut buffer = Buffer::empty(Rect::new(0, 0, 24, 5));
        Chart::new(&series, &system)
            .glyphs(VizGlyphSet::Ascii)
            .render(Rect::new(0, 0, 24, 5), &mut buffer);
        let text: String = buffer
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains('*') || text.contains('X') || text.contains('.'),
            "{text}"
        );
    }

    #[test]
    fn gauge_fill_scales_with_percent() {
        let system = system_ascii_nocolor();
        let fill = *VizGlyphSet::Ascii.ladder().last().unwrap_or(&'#');
        let empty = '-';
        let count_fill = |buf: &Buffer, w: u16| -> usize {
            (0..w)
                .filter(|&x| buf[(x, 0)].symbol().contains(fill))
                .count()
        };
        let area = Rect::new(0, 0, 40, 1);
        let mut low = Buffer::empty(area);
        Gauge::percent(25.0, &system)
            .glyphs(VizGlyphSet::Ascii)
            .render(area, &mut low);
        let mut high = Buffer::empty(area);
        Gauge::percent(90.0, &system)
            .glyphs(VizGlyphSet::Ascii)
            .render(area, &mut high);
        let lo = count_fill(&low, 40);
        let hi = count_fill(&high, 40);
        assert!(
            hi > lo,
            "90% must fill more track cells than 25%: lo={lo} hi={hi}"
        );
        assert!(lo > 0, "25% must paint some fill cells");
        // Empty remainder present on partial fill
        let low_s: String = (0..40u16)
            .map(|x| low[(x, 0)].symbol().to_string())
            .collect();
        assert!(
            low_s.contains(empty),
            "partial gauge leaves empty track: {low_s:?}"
        );
        // Value text for 90 paints
        let high_s: String = (0..40u16)
            .map(|x| high[(x, 0)].symbol().to_string())
            .collect();
        assert!(
            high_s.contains('9') || high_s.contains('0'),
            "value text present: {high_s:?}"
        );
    }

    #[test]
    fn gauge_label_unit_and_threshold_ticks() {
        let system = system_ascii_nocolor();
        let thr = [50.0, 90.0];
        let area = Rect::new(0, 0, 48, 1);
        let mut buffer = Buffer::empty(area);
        Gauge::percent(75.0, &system)
            .glyphs(VizGlyphSet::Ascii)
            .label("cpu")
            .unit("%")
            .thresholds(&thr)
            .render(area, &mut buffer);
        let text: String = (0..48u16)
            .map(|x| buffer[(x, 0)].symbol().to_string())
            .collect();
        assert!(text.contains('c') && text.contains('p'), "label: {text:?}");
        assert!(text.contains('%'), "unit suffix: {text:?}");
        let tick = VizGlyphSet::Ascii.threshold_mark();
        assert!(
            text.contains(tick),
            "threshold tick {tick:?} must paint on track: {text:?}"
        );
        // 75 is between 50 and 90 — fill and empty both exist
        let fill = *VizGlyphSet::Ascii.ladder().last().unwrap_or(&'#');
        assert!(text.contains(fill), "fill glyph present: {text:?}");
    }

    #[test]
    fn gauge_tiny_width_paints_value() {
        let system = system_ascii_nocolor();
        let area = Rect::new(0, 0, 6, 1);
        let mut tiny = Buffer::empty(area);
        Gauge::percent(42.0, &system)
            .glyphs(VizGlyphSet::Ascii)
            .label("cpu")
            .unit("%")
            .render(area, &mut tiny);
        let text: String = (0..6u16)
            .map(|x| tiny[(x, 0)].symbol().to_string())
            .collect();
        // Tiny path prioritizes value text (may drop track/label)
        assert!(
            text.chars().any(|c| c.is_ascii_digit()),
            "tiny gauge must still show value digits: {text:?}"
        );
        // No panic / fully empty row
        assert!(
            text.chars().any(|c| !c.is_whitespace() && c != '\u{0}'),
            "tiny gauge paints something: {text:?}"
        );
    }

    #[test]
    fn gauge_zero_does_not_invent_fill_mass() {
        let system = system_ascii_nocolor();
        let fill = *VizGlyphSet::Ascii.ladder().last().unwrap_or(&'#');
        let area = Rect::new(0, 0, 32, 1);
        let mut buffer = Buffer::empty(area);
        Gauge::percent(0.0, &system)
            .glyphs(VizGlyphSet::Ascii)
            .render(area, &mut buffer);
        let text: String = (0..32u16)
            .map(|x| buffer[(x, 0)].symbol().to_string())
            .collect();
        let fill_n = text.chars().filter(|&c| c == fill).count();
        assert_eq!(fill_n, 0, "zero value must not invent fill mass: {text:?}");
    }

    #[test]
    fn histogram_vertical_and_horizontal() {
        let system = system();
        let buckets = [
            HistBucket::new("0", 1.0),
            HistBucket::new("1", 4.0),
            HistBucket::new("2", 2.0),
            HistBucket::new("3", f64::NAN),
        ];
        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 8));
        Histogram::new(&buckets, &system)
            .title("Hist")
            .selected(1)
            .render(Rect::new(0, 0, 20, 8), &mut buffer);

        let mut hbuf = Buffer::empty(Rect::new(0, 0, 24, 4));
        Histogram::new(&buckets, &system)
            .vertical(false)
            .render(Rect::new(0, 0, 24, 4), &mut hbuf);
    }

    #[test]
    fn segmented_meter_covers_full_width() {
        let system = system_ascii_nocolor();
        let segments = [
            MeterSegment::new("a", 1.0, Role::Success),
            MeterSegment::new("b", 1.0, Role::Danger),
        ];
        let area = Rect::new(0, 0, 10, 1);
        let mut buffer = Buffer::empty(area);
        SegmentedMeter::new(&segments, &system)
            .glyphs(VizGlyphSet::Ascii)
            .render(area, &mut buffer);
        for x in 0..10 {
            assert!(!buffer[(x, 0)].symbol().is_empty());
        }
        // Equal weights → equal half occupancy of distinct markers
        let m0 = VizGlyphSet::Ascii.series_marker(0);
        let m1 = VizGlyphSet::Ascii.series_marker(1);
        let row: String = (0..10u16)
            .map(|x| buffer[(x, 0)].symbol().to_string())
            .collect();
        let c0 = row.chars().filter(|&c| c == m0).count();
        let c1 = row.chars().filter(|&c| c == m1).count();
        assert_eq!(c0 + c1, 10, "full track painted: {row:?}");
        assert!((c0 as i32 - c1 as i32).abs() <= 1, "equal weights: {row:?}");
    }

    #[test]
    fn allocate_segment_widths_zero_weight_no_mass() {
        let w = allocate_segment_widths(&[1.0, 0.0, 1.0], 10);
        assert_eq!(w[1], 0, "zero weight must get 0 cols: {w:?}");
        assert_eq!(
            w[0] + w[1] + w[2],
            10,
            "positive segments fill track: {w:?}"
        );
        assert!(w[0] > 0 && w[2] > 0);
    }

    #[test]
    fn segmented_meter_selection_changes_glyph() {
        let system = system_ascii_nocolor();
        let segments = [
            MeterSegment::new("a", 2.0, Role::Success),
            MeterSegment::new("b", 1.0, Role::Danger),
            MeterSegment::new("c", 1.0, Role::Accent),
        ];
        let area = Rect::new(0, 0, 20, 1);
        let mut plain = Buffer::empty(area);
        SegmentedMeter::new(&segments, &system)
            .glyphs(VizGlyphSet::Ascii)
            .render(area, &mut plain);
        let mut selected = Buffer::empty(area);
        SegmentedMeter::new(&segments, &system)
            .glyphs(VizGlyphSet::Ascii)
            .selected(1)
            .render(area, &mut selected);
        let plain_s: String = (0..20u16)
            .map(|x| plain[(x, 0)].symbol().to_string())
            .collect();
        let sel_s: String = (0..20u16)
            .map(|x| selected[(x, 0)].symbol().to_string())
            .collect();
        assert!(
            sel_s.contains('X'),
            "selected segment must use X highlight: {sel_s:?}"
        );
        assert_ne!(
            plain_s, sel_s,
            "selection must change occupancy/glyphs vs unselected"
        );
        // Segment b is middle weight 1 of total 4 → about 5 cols of X
        let x_count = sel_s.chars().filter(|&c| c == 'X').count();
        assert!(
            x_count >= 3 && x_count <= 8,
            "selected mid segment width reasonable: {x_count} in {sel_s:?}"
        );
    }

    #[test]
    fn segmented_meter_center_caption_and_separators() {
        let system = system_ascii_nocolor();
        let segments = [
            MeterSegment::new("a", 1.0, Role::Success),
            MeterSegment::new("b", 1.0, Role::Danger),
        ];
        let area = Rect::new(0, 0, 16, 2);
        let mut buffer = Buffer::empty(area);
        SegmentedMeter::new(&segments, &system)
            .glyphs(VizGlyphSet::Ascii)
            .separators(true)
            .center("42%")
            .render(area, &mut buffer);
        let row0: String = (0..16u16)
            .map(|x| buffer[(x, 0)].symbol().to_string())
            .collect();
        let row1: String = (0..16u16)
            .map(|x| buffer[(x, 1)].symbol().to_string())
            .collect();
        assert!(row0.contains(' '), "separators insert gap: {row0:?}");
        assert!(
            row1.contains('4') && row1.contains('2'),
            "center caption on second row: {row1:?}"
        );
    }

    #[test]
    fn segmented_meter_zero_weight_segment_invisible() {
        let system = system_ascii_nocolor();
        let segments = [
            MeterSegment::new("a", 3.0, Role::Success),
            MeterSegment::new("empty", 0.0, Role::Danger),
            MeterSegment::new("c", 1.0, Role::Accent),
        ];
        let area = Rect::new(0, 0, 12, 1);
        let mut buffer = Buffer::empty(area);
        SegmentedMeter::new(&segments, &system)
            .glyphs(VizGlyphSet::Ascii)
            .render(area, &mut buffer);
        let m1 = VizGlyphSet::Ascii.series_marker(1);
        let row: String = (0..12u16)
            .map(|x| buffer[(x, 0)].symbol().to_string())
            .collect();
        // Zero-weight segment index 1 must not paint its marker (no invented mass)
        let m1_count = row.chars().filter(|&c| c == m1).count();
        assert_eq!(m1_count, 0, "zero-weight segment must not paint: {row:?}");
        let m0 = VizGlyphSet::Ascii.series_marker(0);
        let m2 = VizGlyphSet::Ascii.series_marker(2);
        assert!(
            row.contains(m0) && row.contains(m2),
            "positive segments paint: {row:?}"
        );
    }

    #[test]
    fn metric_radar_paints_axes_title_legend_and_bars() {
        let system = system_ascii_nocolor();
        let axes = [
            MetricAxis::new("cpu"),
            MetricAxis::new("mem"),
            MetricAxis::new("io"),
        ];
        let s0 = [10.0, 50.0, 90.0];
        let s1 = [80.0, 20.0, 40.0];
        let series = [
            MetricSeries::new("prod", &s0),
            MetricSeries::new("stage", &s1),
        ];
        let area = Rect::new(0, 0, 40, 6);
        let mut buffer = Buffer::empty(area);
        MetricRadar::new(&axes, &series, &system)
            .glyphs(VizGlyphSet::Ascii)
            .title("Metrics")
            .show_legend(true)
            .render(area, &mut buffer);

        let row0: String = (0..40u16)
            .map(|x| buffer[(x, 0)].symbol().to_string())
            .collect();
        assert!(
            row0.contains('M') && row0.contains('e'),
            "title on first row: {row0:?}"
        );
        let row1: String = (0..40u16)
            .map(|x| buffer[(x, 1)].symbol().to_string())
            .collect();
        // Monochrome uses series_marker glyphs for fill (not color)
        let m0 = VizGlyphSet::Ascii.series_marker(0);
        let m1 = VizGlyphSet::Ascii.series_marker(1);
        assert!(
            row1.contains(m0) && row1.contains(m1),
            "legend markers: {row1:?}"
        );
        assert!(
            row1.contains('p') || row1.contains('s'),
            "legend labels: {row1:?}"
        );

        // Data rows start at y=2 (title + legend). Higher magnitudes paint more marker cells.
        let count_mark = |row: u16| -> usize {
            (0..40u16)
                .filter(|&x| {
                    let s = buffer[(x, row)].symbol();
                    s.contains(m0) || s.contains(m1)
                })
                .count()
        };
        // mem (50+20) vs io (90+40) — io row must fill more marker cells
        let mem_row = 3u16;
        let io_row = 4u16;
        assert!(
            count_mark(io_row) > count_mark(mem_row),
            "io (90/40) should fill more than mem (50/20): mem={} io={}",
            count_mark(mem_row),
            count_mark(io_row)
        );
        let cpu_row_s: String = (0..40u16)
            .map(|x| buffer[(x, 2)].symbol().to_string())
            .collect();
        assert!(
            cpu_row_s.contains('c'),
            "axis label on data row: {cpu_row_s:?}"
        );
        // cpu prod=10 vs stage=80: series1 marker must outnumber series0 on that row
        let c0 = cpu_row_s.chars().filter(|&c| c == m0).count();
        let c1 = cpu_row_s.chars().filter(|&c| c == m1).count();
        assert!(
            c1 > c0,
            "stage(80) bar wider than prod(10) on cpu: c0={c0} c1={c1} row={cpu_row_s:?}"
        );
    }

    #[test]
    fn metric_radar_missing_and_selection() {
        let system = system_ascii_nocolor();
        let axes = [MetricAxis::new("a"), MetricAxis::new("b")];
        let s0 = [100.0, f64::NAN];
        let series = [MetricSeries::new("only", &s0)];
        let area = Rect::new(0, 0, 24, 4);
        let mut plain = Buffer::empty(area);
        MetricRadar::new(&axes, &series, &system)
            .glyphs(VizGlyphSet::Ascii)
            .show_legend(false)
            .render(area, &mut plain);
        let miss = VizGlyphSet::Ascii.missing_mark();
        let row_b: String = (0..24u16)
            .map(|x| plain[(x, 1)].symbol().to_string())
            .collect();
        assert!(
            row_b.contains(miss),
            "NaN value paints missing mark: {row_b:?}"
        );

        let mut selected = Buffer::empty(area);
        MetricRadar::new(&axes, &series, &system)
            .glyphs(VizGlyphSet::Ascii)
            .show_legend(false)
            .selected_axis(0)
            .selected_series(0)
            .render(area, &mut selected);
        let plain_a: String = (0..24u16)
            .map(|x| plain[(x, 0)].symbol().to_string())
            .collect();
        let sel_a: String = (0..24u16)
            .map(|x| selected[(x, 0)].symbol().to_string())
            .collect();
        assert!(
            sel_a.contains('X'),
            "selected series+axis uses X on ascii: {sel_a:?}"
        );
        assert_ne!(plain_a, sel_a, "selection must change paint vs unselected");
    }

    #[test]
    fn metric_radar_multi_series_grouped_slots() {
        let system = system_ascii_nocolor();
        let axes = [MetricAxis::new("m")];
        let s0 = [100.0];
        let s1 = [50.0];
        let series = [MetricSeries::new("a", &s0), MetricSeries::new("b", &s1)];
        // Wide enough for label + two bar slots
        let area = Rect::new(0, 0, 36, 3);
        let mut buffer = Buffer::empty(area);
        MetricRadar::new(&axes, &series, &system)
            .glyphs(VizGlyphSet::Ascii)
            .show_legend(true)
            .scale(ScaleMode::Fixed {
                min: 0.0,
                max: 100.0,
            })
            .render(area, &mut buffer);
        // Data row is y=1 (legend on y=0). Monochrome fill = series markers.
        let data: String = (0..36u16)
            .map(|x| buffer[(x, 1)].symbol().to_string())
            .collect();
        let m0 = VizGlyphSet::Ascii.series_marker(0);
        let m1 = VizGlyphSet::Ascii.series_marker(1);
        let empty = '-';
        assert!(
            data.contains(m0) && data.contains(m1),
            "both series markers paint: {data:?}"
        );
        assert!(
            data.contains(empty),
            "empty track remainder for half series: {data:?}"
        );
        let c0 = data.chars().filter(|&c| c == m0).count();
        let c1 = data.chars().filter(|&c| c == m1).count();
        assert!(
            c0 > c1,
            "full (100) wider than half (50): c0={c0} c1={c1} row={data:?}"
        );
    }

    #[test]
    fn bar_series_scale_and_ascii() {
        let system = system_ascii_nocolor();
        let bars = [BarDatum::value("a", 10.0), BarDatum::value("b", 20.0)];
        let mut buffer = Buffer::empty(Rect::new(0, 0, 24, 2));
        BarSeries::new(&bars, &system)
            .scale(ScaleMode::Auto)
            .glyphs(VizGlyphSet::Ascii)
            .selected(1)
            .render(Rect::new(0, 0, 24, 2), &mut buffer);
        // Larger bar (b=20) must paint more fill cells than a=10 on row 1 vs 0
        let fill = *VizGlyphSet::Ascii.ladder().last().unwrap_or(&'#');
        let count_fill = |row: u16| -> usize {
            (0..24u16)
                .filter(|&x| buffer[(x, row)].symbol().contains(fill))
                .count()
        };
        assert!(
            count_fill(1) > count_fill(0),
            "b should fill more than a: row0={} row1={}",
            count_fill(0),
            count_fill(1)
        );
    }

    #[test]
    fn bar_series_stacked_segments_distinct_markers() {
        let system = system_ascii_nocolor();
        let segs = [1.0, 2.0, 1.0];
        let bars = [
            BarDatum::stacked("q1", &segs),
            BarDatum::stacked("q2", &[2.0, 2.0]),
        ];
        let area = Rect::new(0, 0, 32, 2);
        let mut buffer = Buffer::empty(area);
        BarSeries::new(&bars, &system)
            .scale(ScaleMode::Auto)
            .glyphs(VizGlyphSet::Ascii)
            .render(area, &mut buffer);
        // No-color stack uses series_marker per segment — must see at least two markers
        let m0 = VizGlyphSet::Ascii.series_marker(0);
        let m1 = VizGlyphSet::Ascii.series_marker(1);
        let row0: String = (0..32u16)
            .map(|x| buffer[(x, 0)].symbol().to_string())
            .collect();
        assert!(
            row0.contains(m0) && row0.contains(m1),
            "stacked segments must paint distinct markers: {row0:?}"
        );
        // q2 sum=4 > q1 sum=4? equal - q2 has 2 segments both 2. Domain max 4 for both.
        // Segment 0 of q1 is 1/4 of bar; segment 1 is 2/4 — middle segment wider
        let c0 = row0.chars().filter(|&c| c == m0).count();
        let c1 = row0.chars().filter(|&c| c == m1).count();
        assert!(
            c1 > c0,
            "middle segment (2) should be wider than first (1): c0={c0} c1={c1} row={row0:?}"
        );
    }

    #[test]
    fn bar_series_negative_bipolar_domain() {
        let system = system_ascii_nocolor();
        let bars = [
            BarDatum::value("loss", -5.0),
            BarDatum::value("gain", 10.0),
            BarDatum::value("zero", 0.0),
        ];
        // Domain with 0 baseline
        let d = resolve_domain(ScaleMode::Auto, [0.0, -5.0, 10.0].into_iter());
        assert!(
            d.min < 0.0 && d.max > 0.0,
            "domain should be bipolar {:?}",
            d
        );
        let z = d.normalize(0.0).unwrap();
        assert!(z > 0.2 && z < 0.8, "zero should be interior, got {z}");

        let area = Rect::new(0, 0, 40, 3);
        let mut buffer = Buffer::empty(area);
        BarSeries::new(&bars, &system)
            .scale(ScaleMode::Auto)
            .glyphs(VizGlyphSet::Ascii)
            .render(area, &mut buffer);
        let fill = *VizGlyphSet::Ascii.ladder().last().unwrap_or(&'#');
        let neg = '=';
        // Skip label columns: "loss"/"gain"/"zero" + pad ≤12; track starts after labels
        let loss: String = (0..40u16)
            .map(|x| buffer[(x, 0)].symbol().to_string())
            .collect();
        let gain: String = (0..40u16)
            .map(|x| buffer[(x, 1)].symbol().to_string())
            .collect();
        // Must paint actual negative fill glyph — zero tick alone is not enough
        assert!(
            loss.contains(neg),
            "negative bar must paint neg glyph '=': {loss:?}"
        );
        assert!(
            gain.contains(fill),
            "positive bar should paint fill: {gain:?}"
        );
        let zero_col = loss
            .find('|')
            .or_else(|| gain.find('|'))
            .expect("zero tick | expected on bipolar track");
        // Sign occupancy: '=' only left of zero; no positive fill on the negative side
        let mut neg_left = 0usize;
        let mut fill_left_of_zero_on_loss = 0usize;
        let mut fill_right_of_zero_on_gain = 0usize;
        for (i, ch) in loss.chars().enumerate() {
            if i < zero_col && ch == neg {
                neg_left += 1;
            }
            if i < zero_col && ch == fill {
                fill_left_of_zero_on_loss += 1;
            }
        }
        for (i, ch) in gain.chars().enumerate() {
            if i > zero_col && ch == fill {
                fill_right_of_zero_on_gain += 1;
            }
        }
        assert!(
            neg_left > 0,
            "neg glyph must appear left of zero col {zero_col}: {loss:?}"
        );
        assert_eq!(
            fill_left_of_zero_on_loss, 0,
            "positive fill must not paint left of zero on loss row: {loss:?}"
        );
        assert!(
            fill_right_of_zero_on_gain > 0,
            "positive fill must appear right of zero on gain: {gain:?}"
        );
    }

    #[test]
    fn tiny_dimensions_bench_constants() {
        let system = system();
        let samples = [0.1, 0.2, 0.9];
        let mut buffer = Buffer::empty(Rect::new(0, 0, bench::TINY_WIDTH, bench::TINY_HEIGHT));
        Sparkline::new(&samples, &system).render(
            Rect::new(0, 0, bench::TINY_WIDTH, bench::TINY_HEIGHT),
            &mut buffer,
        );
        assert!(bench::STREAM_WINDOW >= 16);
        assert!(bench::SAMPLES_PER_SEC >= 1);
    }

    #[test]
    fn window_samples_helper() {
        let s: Vec<f64> = (0..100).map(|i| i as f64).collect();
        let w = window_samples(&s, 10);
        assert_eq!(w.len(), 10);
        assert_eq!(w[0], 90.0);
        assert_eq!(window_samples(&s, 200).len(), 100);
    }

    #[test]
    fn fuzz_normalize_clamps() {
        let d = ScaleDomain {
            min: 0.0,
            max: 10.0,
            log: false,
        };
        assert_eq!(d.normalize(-5.0), Some(0.0));
        assert_eq!(d.normalize(50.0), Some(1.0));
        for mode in [
            ScaleMode::Auto,
            ScaleMode::Fixed { min: 0.0, max: 1.0 },
            ScaleMode::Log { fixed: None },
        ] {
            let _ = mode.id();
        }
    }
}
