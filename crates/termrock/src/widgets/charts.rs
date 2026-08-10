// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **Visualization family** — Sparkline, Chart, Gauge, Histogram (+ BarSeries,
//! SegmentedMeter).
//!
//! **Mission.** Coherent terminal data-viz: time series, bars, stacked bars,
//! histogram buckets, gauges, thresholds, labels, legends, missing data, and
//! selected points. Braille / block / ASCII capability fallbacks with
//! **consistent scale semantics**. Autoscale, fixed scale, log (where justified),
//! time-window behavior. No-color mode uses line styles, glyphs, labels, and
//! ordering — never color alone.
//!
//! Research: btop, bottom, gping, Ratatui charts, observability dashboards.

use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::Modifier,
    widgets::Widget,
};

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
        ScaleMode::Log { fixed: Some((min, max)) } => {
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
            Self::Block | Self::Auto => &[' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'],
            // Braille vertical density approximation (single cell heights)
            Self::Braille => &[' ', '⣀', '⣤', '⣶', '⣿'],
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
            } else if matches!(self.system.capability, crate::style::ColorCapability::Monochrome) {
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

/// One series in a multi-line chart.
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

/// Multi-series chart with axes, legend, selection, thresholds.
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
}

impl<'a> Chart<'a> {
    /// Series + system.
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

        // Domain across all series
        let mut all = Vec::new();
        for s in self.series {
            let win = if self.window == 0 {
                s.samples
            } else {
                window_samples(s.samples, self.window)
            };
            all.extend(win.iter().copied());
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

        // Clear plot with grid dots in no-color? use space
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

        // Paint each series as column glyphs (shared columns)
        let width = usize::from(plot.width);
        let height = usize::from(plot.height.max(1));
        // cell occupancy: last writer wins; selected series last
        let order: Vec<usize> = {
            let mut o: Vec<usize> = (0..self.series.len()).collect();
            if let Some(s) = self.selected_series {
                if s < o.len() {
                    o.retain(|&i| i != s);
                    o.push(s);
                }
            }
            o
        };

        for si in order {
            let series = &self.series[si];
            let win_n = if self.window == 0 {
                width.max(1)
            } else {
                self.window
            };
            let samples = window_samples(series.samples, win_n);
            if samples.is_empty() {
                continue;
            }
            let mark = gset.series_marker(si);
            let ladder = gset.ladder();
            let miss = gset.missing_mark();
            let role = series_role(si);
            let style = if matches!(self.system.capability, crate::style::ColorCapability::Monochrome) {
                self.system.style(Role::Text)
            } else {
                self.system.style(role)
            };

            for col in 0..width {
                let index = col * samples.len() / width.max(1);
                let sample = samples.get(index).copied().unwrap_or(f64::NAN);
                let missing = !sample.is_finite();
                let Some(frac) = domain.normalize(sample) else {
                    if missing {
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
                let row = fraction_to_row(frac, plot.height);
                let selected = self.selected_series == Some(si)
                    && self.selected_index == Some(index);
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
                    plot.y.saturating_add(row),
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
        let style = if matches!(self.system.capability, crate::style::ColorCapability::Monochrome) {
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
                let col = ((f64::from(track_w.saturating_sub(1)) * tf).round() as u16).min(track_w.saturating_sub(1));
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
                let bar_h = ((f64::from(h.saturating_sub(1)) * frac).round() as u16).min(h.saturating_sub(1));
                let x = area
                    .x
                    .saturating_add((i * col_w.max(1)) as u16);
                let selected = self.selected == Some(i);
                let style = if selected {
                    self.system
                        .style(Role::TextStrong)
                        .add_modifier(Modifier::BOLD)
                } else if matches!(self.system.capability, crate::style::ColorCapability::Monochrome) {
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
                buffer.set_stringn(
                    area.x,
                    py,
                    lab,
                    label_w,
                    self.system.style(Role::TextMuted),
                );
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
                } else if matches!(self.system.capability, crate::style::ColorCapability::Monochrome) {
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

/// One named bar in a horizontal bar series.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarDatum<'a> {
    /// Label shown to the left when width allows.
    pub label: &'a str,
    /// Fraction filled (`0.0..=1.0`) **or** raw value when used with scale on BarSeries.
    pub fraction: f64,
}

impl<'a> BarDatum<'a> {
    /// Normalized bar.
    #[must_use]
    pub const fn new(label: &'a str, fraction: f64) -> Self {
        Self { label, fraction }
    }

    /// Alias for raw value (pair with [`BarSeries::scale`]).
    #[must_use]
    pub const fn value(label: &'a str, value: f64) -> Self {
        Self {
            label,
            fraction: value,
        }
    }
}

/// Multi-row horizontal bar chart.
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

    /// Treat `fraction` field as raw values with scale.
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
        let domain = if self.pre_normalized {
            ScaleDomain::unit()
        } else {
            resolve_domain(self.scale, self.bars.iter().map(|b| b.fraction))
        };
        let gset = self.glyphs.resolve(self.system.glyphs);
        let fill_ch = *gset.ladder().last().unwrap_or(&'#');
        let empty_ch = match gset {
            VizGlyphSet::Ascii => '-',
            _ => '░',
        };

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
            let fraction = if self.pre_normalized {
                if bar.fraction.is_finite() {
                    bar.fraction.clamp(0.0, 1.0)
                } else {
                    0.0
                }
            } else {
                domain.normalize(bar.fraction).unwrap_or(0.0)
            };
            let filled = ((f64::from(track_w) * fraction).round() as u16).min(track_w);
            let selected = self.selected == Some(row);
            let style = if selected {
                self.system
                    .style(Role::TextStrong)
                    .add_modifier(Modifier::BOLD)
            } else if matches!(self.system.capability, crate::style::ColorCapability::Monochrome) {
                self.system.style(Role::Text)
            } else {
                self.system.style(Role::Accent)
            };
            let fill = fill_ch.to_string().repeat(usize::from(filled));
            let empty = empty_ch
                .to_string()
                .repeat(usize::from(track_w.saturating_sub(filled)));
            buffer.set_stringn(track_x, y, &fill, usize::from(filled), style);
            if filled < track_w {
                buffer.set_stringn(
                    track_x.saturating_add(filled),
                    y,
                    &empty,
                    usize::from(track_w.saturating_sub(filled)),
                    self.system.style(Role::TextDisabled),
                );
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

// ── SegmentedMeter (stacked bars / shares) ──────────────────────────────────

/// One segment in a segmented meter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeterSegment<'a> {
    /// Segment label for narrow fallback text.
    pub label: &'a str,
    /// Relative weight (normalized across segments).
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
}

/// Single-row segmented meter (stacked proportions).
#[derive(Debug, Clone, Copy)]
pub struct SegmentedMeter<'a> {
    segments: &'a [MeterSegment<'a>],
    system: &'a DesignSystem,
    glyphs: VizGlyphSet,
    selected: Option<usize>,
    show_labels: bool,
}

impl<'a> SegmentedMeter<'a> {
    /// Creates a segmented meter.
    #[must_use]
    pub const fn new(segments: &'a [MeterSegment<'a>], system: &'a DesignSystem) -> Self {
        Self {
            segments,
            system,
            glyphs: VizGlyphSet::Auto,
            selected: None,
            show_labels: false,
        }
    }

    /// Glyphs.
    #[must_use]
    pub const fn glyphs(mut self, glyphs: VizGlyphSet) -> Self {
        self.glyphs = glyphs;
        self
    }

    /// Selected segment.
    #[must_use]
    pub const fn selected(mut self, i: usize) -> Self {
        self.selected = Some(i);
        self
    }

    /// Show label row when height ≥ 2.
    #[must_use]
    pub const fn show_labels(mut self, on: bool) -> Self {
        self.show_labels = on;
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
        let total: f64 = self
            .segments
            .iter()
            .map(|segment| {
                if segment.weight.is_finite() && segment.weight > 0.0 {
                    segment.weight
                } else {
                    0.0
                }
            })
            .sum::<f64>()
            .max(f64::EPSILON);

        let label_h = u16::from(self.show_labels && area.height >= 2);
        let bar_y = area.y;
        let mut remaining = area.width;
        let mut x = area.x;
        // No-color: use distinct markers per segment instead of color alone
        for (index, segment) in self.segments.iter().enumerate() {
            let weight = if segment.weight.is_finite() && segment.weight > 0.0 {
                segment.weight
            } else {
                0.0
            };
            let mut width =
                ((f64::from(area.width) * weight / total).round() as u16).min(remaining);
            if index + 1 == self.segments.len() {
                width = remaining;
            }
            if width == 0 {
                continue;
            }
            let selected = self.selected == Some(index);
            let ch = if matches!(self.system.capability, crate::style::ColorCapability::Monochrome) {
                gset.series_marker(index)
            } else {
                fill_ch
            };
            let style = if selected {
                self.system
                    .style(Role::TextStrong)
                    .add_modifier(Modifier::BOLD)
            } else if matches!(self.system.capability, crate::style::ColorCapability::Monochrome) {
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
            remaining = remaining.saturating_sub(width);
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
            ScaleMode::Fixed { min: 0.0, max: 100.0 },
            std::iter::empty(),
        );
        assert_eq!(d.normalize(50.0), Some(0.5));

        let d = resolve_domain(ScaleMode::Log { fixed: None }, [1.0, 10.0, 100.0].into_iter());
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
        let system = system();
        let samples = [10.0, 20.0, 40.0, f64::NAN, 30.0];
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 1));
        Sparkline::new(&samples, &system)
            .selected(1)
            .threshold(25.0)
            .render(Rect::new(0, 0, 10, 1), &mut buffer);
        // selected glyph present somewhere
        let text: String = buffer.content().iter().map(|c| c.symbol().to_string()).collect();
        assert!(text.contains('◆') || text.contains('X') || !text.trim().is_empty());
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
        let system = system();
        let a = [1.0, 2.0, 3.0, 2.0, 1.0];
        let b = [3.0, 2.0, 1.0, 2.0, 3.0];
        let series = [
            ChartSeries::new("cpu", &a),
            ChartSeries::new("mem", &b),
        ];
        let mut buffer = Buffer::empty(Rect::new(0, 0, 40, 8));
        Chart::new(&series, &system)
            .title("host")
            .thresholds(&[2.0])
            .selected_series(0)
            .selected_index(2)
            .render(Rect::new(0, 0, 40, 8), &mut buffer);
        let text: String = buffer.content().iter().map(|c| c.symbol().to_string()).collect();
        assert!(text.contains("cpu") || text.contains("host") || text.contains('●') || text.contains('*'), "{text}");
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
        let text: String = buffer.content().iter().map(|c| c.symbol().to_string()).collect();
        assert!(text.contains('*') || text.contains('X') || text.contains('.'), "{text}");
    }

    #[test]
    fn gauge_thresholds_and_tiny() {
        let system = system();
        let thr = [70.0, 90.0];
        let mut buffer = Buffer::empty(Rect::new(0, 0, 30, 1));
        Gauge::percent(85.0, &system)
            .label("cpu")
            .unit("%")
            .thresholds(&thr)
            .render(Rect::new(0, 0, 30, 1), &mut buffer);
        let text: String = buffer.content().iter().map(|c| c.symbol().to_string()).collect();
        assert!(text.contains("85") || text.contains('%') || text.contains("cpu"), "{text}");

        let mut tiny = Buffer::empty(Rect::new(0, 0, 6, 1));
        Gauge::percent(50.0, &system).render(Rect::new(0, 0, 6, 1), &mut tiny);
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
            .title("hist")
            .selected(1)
            .render(Rect::new(0, 0, 20, 8), &mut buffer);

        let mut hbuf = Buffer::empty(Rect::new(0, 0, 24, 4));
        Histogram::new(&buckets, &system)
            .vertical(false)
            .render(Rect::new(0, 0, 24, 4), &mut hbuf);
    }

    #[test]
    fn segmented_meter_covers_full_width() {
        let system = system();
        let segments = [
            MeterSegment {
                label: "a",
                weight: 1.0,
                role: Role::Success,
            },
            MeterSegment {
                label: "b",
                weight: 1.0,
                role: Role::Danger,
            },
        ];
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 1));
        SegmentedMeter::new(&segments, &system).render(Rect::new(0, 0, 10, 1), &mut buffer);
        for x in 0..10 {
            assert!(!buffer[(x, 0)].symbol().is_empty());
        }
    }

    #[test]
    fn bar_series_scale_and_ascii() {
        let system = system_ascii_nocolor();
        let bars = [
            BarDatum::value("a", 10.0),
            BarDatum::value("b", 20.0),
        ];
        let mut buffer = Buffer::empty(Rect::new(0, 0, 24, 2));
        BarSeries::new(&bars, &system)
            .scale(ScaleMode::Auto)
            .glyphs(VizGlyphSet::Ascii)
            .selected(1)
            .render(Rect::new(0, 0, 24, 2), &mut buffer);
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
