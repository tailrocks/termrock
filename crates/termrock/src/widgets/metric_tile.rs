//! **MetricTile** — one measured number, stated well.
//!
//! A dashboard is a wall of numbers, and the number is not the interesting
//! part: the interesting part is how it compares and whether it is healthy.
//! The tile fixes that reading order — value loud, unit quiet, delta marked by
//! a glyph before it is marked by a color, health confined to one letter — so
//! a grid of twelve tiles scans instead of shouting.
//!
//! The host owns the samples and the formatting; the tile owns the chrome.
use ratatui_core::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::style::{DesignSystem, Role};
use crate::text::{display_cols, take_display_cols};
use crate::widgets::charts::{Gauge, ScaleMode, Sparkline};
use crate::widgets::tiered_row::TieredRow;
use crate::widgets::{SemanticStatus, StatusIndicator};

/// How a metric tile paints its body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum MetricViz {
    /// Value + sparkline trend (default).
    #[default]
    Sparkline,
    /// Single gauge fill.
    Gauge,
    /// Value only (no spark/gauge).
    ValueOnly,
}

impl MetricViz {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Sparkline => "sparkline",
            Self::Gauge => "gauge",
            Self::ValueOnly => "value",
        }
    }
}

/// Health of one tile (partial failure support).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum MetricTileHealth {
    /// Ok.
    #[default]
    Ok,
    /// Crossing warning threshold.
    Warning,
    /// Crossing danger threshold / error.
    Danger,
    /// Loading.
    Loading,
    /// Failed to load this metric (others may succeed).
    Failed,
    /// Stale data.
    Stale,
}

impl MetricTileHealth {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warning => "warning",
            Self::Danger => "danger",
            Self::Loading => "loading",
            Self::Failed => "failed",
            Self::Stale => "stale",
        }
    }

    /// Letter (never color alone).
    #[must_use]
    pub const fn letter(self) -> char {
        match self {
            Self::Ok => '·',
            Self::Warning => '!',
            Self::Danger => '‼',
            Self::Loading => '…',
            Self::Failed => 'x',
            Self::Stale => '~',
        }
    }

    /// ASCII letter.
    #[must_use]
    pub const fn letter_ascii(self) -> char {
        match self {
            Self::Ok => '.',
            Self::Warning => '!',
            Self::Danger => 'X',
            Self::Loading => '.',
            Self::Failed => 'x',
            Self::Stale => '~',
        }
    }

    /// Shared health projection for recipe-owned status paint.
    #[must_use]
    pub const fn semantic(self) -> SemanticStatus {
        match self {
            Self::Ok => SemanticStatus::Success,
            Self::Warning | Self::Stale => SemanticStatus::Warning,
            Self::Danger | Self::Failed => SemanticStatus::Failed,
            Self::Loading => SemanticStatus::Running,
        }
    }
}

/// One metric card projection (host-owned samples).
#[derive(Debug, Clone, PartialEq)]
pub struct MetricTile<'a> {
    /// Stable id (drill-down / commands).
    pub id: &'a str,
    /// Title.
    pub title: &'a str,
    /// Formatted primary value (`42.1%`, `1.2k rps`).
    pub value: &'a str,
    /// Unit / subtitle.
    pub unit: &'a str,
    /// Comparison delta text (`+3.2%`, `−12`). Empty = hide.
    pub delta: &'a str,
    /// True when delta is “bad” direction (host policy).
    pub delta_bad: bool,
    /// Samples for sparkline / gauge domain (NaN = missing).
    pub samples: &'a [f64],
    /// Current numeric value for gauge (optional).
    pub gauge_value: Option<f64>,
    /// Thresholds in domain units (warning/danger).
    pub thresholds: &'a [f64],
    /// Visualization.
    pub viz: MetricViz,
    /// Health.
    pub health: MetricTileHealth,
    /// Error message when Failed.
    pub error: Option<&'a str>,
}

impl<'a> MetricTile<'a> {
    /// Construct value-only tile.
    #[must_use]
    pub const fn new(id: &'a str, title: &'a str, value: &'a str) -> Self {
        Self {
            id,
            title,
            value,
            unit: "",
            delta: "",
            delta_bad: false,
            samples: &[],
            gauge_value: None,
            thresholds: &[],
            viz: MetricViz::Sparkline,
            health: MetricTileHealth::Ok,
            error: None,
        }
    }

    /// Unit.
    #[must_use]
    pub const fn unit(mut self, u: &'a str) -> Self {
        self.unit = u;
        self
    }

    /// Delta.
    #[must_use]
    pub const fn delta(mut self, d: &'a str, bad: bool) -> Self {
        self.delta = d;
        self.delta_bad = bad;
        self
    }

    /// Samples.
    #[must_use]
    pub const fn samples(mut self, s: &'a [f64]) -> Self {
        self.samples = s;
        self
    }

    /// Gauge value.
    #[must_use]
    pub const fn gauge(mut self, v: f64) -> Self {
        self.gauge_value = Some(v);
        self.viz = MetricViz::Gauge;
        self
    }

    /// Thresholds.
    #[must_use]
    pub const fn thresholds(mut self, t: &'a [f64]) -> Self {
        self.thresholds = t;
        self
    }

    /// Viz.
    #[must_use]
    pub const fn viz(mut self, v: MetricViz) -> Self {
        self.viz = v;
        self
    }

    /// Health.
    #[must_use]
    pub const fn health(mut self, h: MetricTileHealth) -> Self {
        self.health = h;
        self
    }

    /// Failed with message.
    #[must_use]
    pub const fn failed(mut self, msg: &'a str) -> Self {
        self.health = MetricTileHealth::Failed;
        self.error = Some(msg);
        self
    }

    /// Binds the tile to a design system for painting.
    #[must_use]
    pub const fn view(&'a self, system: &'a DesignSystem) -> MetricTileView<'a> {
        MetricTileView {
            tile: self,
            system,
            presentation: MetricTilePresentation::Card,
            focused: false,
        }
    }
}

/// How much room the tile gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum MetricTilePresentation {
    /// A card: rule, title row, value row, visualization body.
    #[default]
    Card,
    /// One row in a list of metrics.
    Row,
}

/// A [`MetricTile`] bound to a design system, ready to paint.
#[derive(Debug, Clone, Copy)]
pub struct MetricTileView<'a> {
    tile: &'a MetricTile<'a>,
    system: &'a DesignSystem,
    presentation: MetricTilePresentation,
    focused: bool,
}

impl<'a> MetricTileView<'a> {
    /// Card or single row.
    #[must_use]
    pub const fn presentation(mut self, presentation: MetricTilePresentation) -> Self {
        self.presentation = presentation;
        self
    }

    /// Marks the tile as the one the operator is on.
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Forces the ASCII glyph profile.
    #[must_use]
    /// The health letter under the active glyph profile.
    pub fn health_letter(&self) -> char {
        self.tile.health.letter()
    }

    /// The delta's direction glyph, so a delta reads without color.
    #[must_use]
    pub fn delta_glyph(&self) -> &'static str {
        match (self.tile.delta_bad, false) {
            (true, true) => "v",
            (true, false) => "▼",
            (false, true) => "^",
            (false, false) => "▲",
        }
    }

    /// Paints the tile into `area`.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        match self.presentation {
            MetricTilePresentation::Card => self.paint_card(area, buffer),
            MetricTilePresentation::Row => self.paint_row(area, buffer),
        }
    }

    fn paint_row(&self, area: Rect, buffer: &mut Buffer) {
        let tile = self.tile;
        let mark = if self.focused {
            self.system.glyphs.selection_gutter()
        } else {
            " "
        };
        let mut row = TieredRow::with_separator(" ");
        row.push_joined(mark, self.focused.then(|| self.system.style(Role::Accent)));
        let status =
            StatusIndicator::new(tile.health.semantic(), self.system).label(tile.health.id());
        row.push_plain(&status.text(None));
        row.push_plain(tile.title);
        if let Some(err) = tile.error {
            row.push_joined(":", None);
            row.push(err, self.system.style(Role::Danger));
        } else {
            row.push(tile.value, self.system.style(Role::TextStrong));
            if !tile.unit.is_empty() {
                row.push(tile.unit, self.system.style(Role::TextMuted));
            }
            if !tile.delta.is_empty() {
                let tone = self.system.style(if tile.delta_bad {
                    Role::Danger
                } else {
                    Role::TextStrong
                });
                row.push(self.delta_glyph(), tone);
                row.push(tile.delta, tone);
            }
        }
        let line = row.text().to_string();
        buffer.set_stringn(
            area.x,
            area.y,
            take_display_cols(&line, usize::from(area.width)),
            usize::from(area.width),
            self.system.style(Role::Text),
        );
        row.paint_tiers(buffer, Rect::new(area.x, area.y, area.width, 1), 0);
        if area.width > 2 {
            status.paint(
                Rect::new(
                    area.x.saturating_add(2),
                    area.y,
                    area.width.saturating_sub(2),
                    1,
                ),
                buffer,
                None,
            );
        }
    }

    fn paint_card(&self, area: Rect, buffer: &mut Buffer) {
        let tile = self.tile;
        let system = self.system;
        let border = if self.focused {
            system.style(Role::BorderFocused)
        } else {
            system.style(Role::Border)
        };
        let rule = system.glyphs.rule();
        let top: String = std::iter::repeat_n(rule, usize::from(area.width)).collect();
        buffer.set_stringn(area.x, area.y, &top, usize::from(area.width), border);

        let inner_x = area.x.saturating_add(1);
        let inner_w = area.width.saturating_sub(2);
        if inner_w == 0 {
            return;
        }

        let mut title = TieredRow::with_separator(" ");
        let status = StatusIndicator::new(tile.health.semantic(), system).label(tile.health.id());
        title.push_plain(&status.text(None));
        title.push_plain(tile.title);
        let title_line = title.text().to_string();
        buffer.set_stringn(
            inner_x,
            area.y,
            take_display_cols(&title_line, usize::from(inner_w)),
            usize::from(inner_w),
            system.style(Role::TextStrong),
        );
        title.paint_tiers(buffer, Rect::new(inner_x, area.y, inner_w, 1), 0);
        status.paint(Rect::new(inner_x, area.y, inner_w, 1), buffer, None);

        let mut y = area.y.saturating_add(1);
        if y < area.bottom() {
            if let Some(err) = tile.error {
                buffer.set_stringn(
                    inner_x,
                    y,
                    take_display_cols(err, usize::from(inner_w)),
                    usize::from(inner_w),
                    system.style(Role::Danger),
                );
            } else {
                let mut value = TieredRow::with_separator(" ");
                value.push_plain(tile.value);
                if !tile.unit.is_empty() {
                    value.push(tile.unit, system.style(Role::TextMuted));
                }
                let value_line = value.text().to_string();
                buffer.set_stringn(
                    inner_x,
                    y,
                    take_display_cols(&value_line, usize::from(inner_w)),
                    usize::from(inner_w),
                    system.style(Role::TextStrong),
                );
                value.paint_tiers(buffer, Rect::new(inner_x, y, inner_w, 1), 0);

                if !tile.delta.is_empty() {
                    let delta = format!("{} {}", self.delta_glyph(), tile.delta);
                    let dw = u16::try_from(display_cols(&delta)).unwrap_or(u16::MAX);
                    if dw + 2 < area.width {
                        buffer.set_stringn(
                            area.right().saturating_sub(dw.saturating_add(1)),
                            y,
                            &delta,
                            usize::from(dw),
                            system.style(if tile.delta_bad {
                                Role::Danger
                            } else {
                                Role::TextStrong
                            }),
                        );
                    }
                }
            }
            y = y.saturating_add(1);
        }

        let body = Rect {
            x: inner_x,
            y,
            width: inner_w,
            height: area.bottom().saturating_sub(y),
        };
        if body.height == 0 || body.width == 0 {
            return;
        }
        if matches!(
            tile.health,
            MetricTileHealth::Failed | MetricTileHealth::Loading
        ) {
            let msg = match tile.health {
                MetricTileHealth::Loading => "loading…",
                _ => tile.error.unwrap_or("failed"),
            };
            buffer.set_stringn(
                body.x,
                body.y,
                take_display_cols(msg, usize::from(body.width)),
                usize::from(body.width),
                system.style(Role::TextMuted),
            );
            return;
        }

        match tile.viz {
            MetricViz::Sparkline if !tile.samples.is_empty() => {
                // A trend is data, so it reads as a series; the health is
                // already stated by the letter in the title row.
                let mut spark = Sparkline::new(tile.samples, system).role(Role::ChartSeries1);
                // A threshold is the second question, asked after "what is the
                // number": it draws a line across every tile on a dashboard
                // whether or not the operator is looking at that one. Focus is
                // the keypress that asks it (plans/017 §B2).
                if let Some(&threshold) = tile.thresholds.first().filter(|_| self.focused) {
                    spark = spark.threshold(threshold);
                }

                Widget::render(&spark, body, buffer);
            }
            MetricViz::Gauge => {
                let value = tile.gauge_value.unwrap_or(0.0);
                let thresholds: &[f64] = if self.focused { tile.thresholds } else { &[] };
                let mut gauge = Gauge::percent(value, system)
                    .label(tile.title)
                    .thresholds(thresholds)
                    .role(tile.health.semantic().role());

                if value > 100.0 {
                    let max = tile
                        .samples
                        .iter()
                        .copied()
                        .filter(|x| x.is_finite())
                        .fold(value, f64::max)
                        .max(1.0);
                    gauge = Gauge::new(value, system)
                        .scale(ScaleMode::Fixed { min: 0.0, max })
                        .thresholds(thresholds)
                        .role(tile.health.semantic().role());
                }
                Widget::render(&gauge, body, buffer);
            }
            MetricViz::ValueOnly | MetricViz::Sparkline => {
                if matches!(tile.health, MetricTileHealth::Stale) {
                    buffer.set_stringn(
                        body.x,
                        body.y,
                        take_display_cols("stale", usize::from(body.width)),
                        usize::from(body.width),
                        system.style(Role::Warning),
                    );
                }
            }
        }
    }
}

impl Widget for &MetricTileView<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        self.paint(area, buffer);
    }
}

impl Widget for MetricTileView<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        self.paint(area, buffer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tile_states_value_unit_and_delta_in_three_voices() {
        let system = DesignSystem::default();
        let samples = [1.0, 4.0, 2.0, 8.0];
        let tile = MetricTile::new("rps", "Requests", "1.2k")
            .unit("rps")
            .delta("+3.2%", false)
            .samples(&samples);
        let area = Rect::new(0, 0, 30, 5);
        let mut buffer = Buffer::empty(area);
        tile.view(&system).paint(area, &mut buffer);

        let row: String = (0..area.width).map(|x| buffer[(x, 1)].symbol()).collect();
        assert!(row.contains("1.2k rps"), "{row:?}");
        let at = |y: u16, needle: char| {
            let x = (0..area.width)
                .find(|x| buffer[(*x, y)].symbol().starts_with(needle))
                .unwrap_or_else(|| panic!("{needle:?} must be painted"));
            buffer[(x, y)].style().fg
        };
        assert_eq!(
            at(1, '1'),
            system.style(Role::TextStrong).fg,
            "value is loud"
        );
        assert_eq!(
            at(1, 'r'),
            system.style(Role::TextMuted).fg,
            "unit is quiet"
        );
        assert_eq!(
            at(1, '▲'),
            system.style(Role::TextStrong).fg,
            "a good delta is a good delta"
        );
    }

    #[test]
    fn health_is_confined_to_status_rail_and_glyph() {
        let system = DesignSystem::default();
        let tile = MetricTile::new("cpu", "CPU", "97%")
            .health(MetricTileHealth::Danger)
            .viz(MetricViz::ValueOnly);
        let area = Rect::new(0, 0, 24, 3);
        let mut buffer = Buffer::empty(area);
        tile.view(&system).paint(area, &mut buffer);
        let danger = system.style(Role::Danger).fg;
        let danger_cells = (0..area.width)
            .filter(|x| {
                let cell = &buffer[(*x, 0)];
                !cell.symbol().trim().is_empty() && Some(cell.fg) == danger
            })
            .count();
        assert_eq!(
            danger_cells, 2,
            "danger belongs to the status rail and glyph, not to the title"
        );
    }

    #[test]
    fn a_row_presentation_fits_one_line() {
        let system = DesignSystem::default();
        let tile = MetricTile::new("err", "Errors", "12")
            .unit("/min")
            .delta("+4", true)
            .health(MetricTileHealth::Warning);
        let area = Rect::new(0, 0, 40, 1);
        let mut buffer = Buffer::empty(area);
        tile.view(&system)
            .presentation(MetricTilePresentation::Row)
            .focused(true)
            .paint(area, &mut buffer);
        let row: String = (0..area.width).map(|x| buffer[(x, 0)].symbol()).collect();
        assert!(row.contains("Errors"), "{row:?}");
        assert!(
            row.contains('▼'),
            "a bad delta says so without color: {row:?}"
        );
    }
}
