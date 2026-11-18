use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use crate::core::id::WidgetId;
use crate::theme::Tone;
use crate::ui::ctx::RenderCtx;
use crate::ui::measure::terminal_extent;
use crate::ui::text::{fit, truncate, width};
use crate::widgets::progress::{Meter, MeterTone};

#[derive(Debug, Clone)]
pub struct MeterRow {
    pub label: String,
    pub meter: Meter,

    pub detail: Option<String>,
}

impl MeterRow {
    pub fn new(label: impl Into<String>, meter: Meter) -> Self {
        Self {
            label: label.into(),
            meter,
            detail: None,
        }
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

#[derive(Debug, Clone)]
pub struct MeterList {
    pub rows: Vec<MeterRow>,

    pub label_width: Option<u16>,

    pub label_tone: Tone,

    pub label_gap: u16,

    pub pad_label: bool,

    pub meter_width: Option<u16>,

    pub reserve: u16,

    pub min_meter: u16,

    pub detail_gap: u16,

    pub detail_tone: Tone,
}

impl Default for MeterList {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl MeterList {
    pub fn new(rows: Vec<MeterRow>) -> Self {
        Self {
            rows,
            label_width: None,
            label_tone: Tone::Normal,
            label_gap: 1,
            pad_label: false,
            meter_width: None,
            reserve: 0,
            min_meter: 0,
            detail_gap: 2,
            detail_tone: Tone::Muted,
        }
    }

    pub fn label_width(mut self, w: u16) -> Self {
        self.label_width = Some(w);
        self
    }
    pub fn label_tone(mut self, tone: Tone) -> Self {
        self.label_tone = tone;
        self
    }

    pub fn pad_label(mut self, on: bool) -> Self {
        self.pad_label = on;
        self
    }
    pub fn label_gap(mut self, gap: u16) -> Self {
        self.label_gap = gap;
        self
    }

    pub fn meter_width(mut self, w: u16) -> Self {
        self.meter_width = Some(w);
        self
    }
    pub fn reserve(mut self, cells: u16) -> Self {
        self.reserve = cells;
        self
    }
    pub fn min_meter(mut self, w: u16) -> Self {
        self.min_meter = w;
        self
    }
    pub fn detail_gap(mut self, gap: u16) -> Self {
        self.detail_gap = gap;
        self
    }
    pub fn detail_tone(mut self, tone: Tone) -> Self {
        self.detail_tone = tone;
        self
    }

    /// Row count, saturating at `u16::MAX`.
    pub fn height(&self) -> u16 {
        terminal_extent(self.rows.len())
    }

    fn intrinsic_label_width(&self) -> usize {
        self.label_width.map_or_else(
            || self.rows.iter().map(|r| width(&r.label)).max().unwrap_or(0),
            usize::from,
        )
    }

    pub fn label_column(&self, area: Rect) -> u16 {
        terminal_extent(self.intrinsic_label_width().min(usize::from(area.width)))
    }

    pub fn meter_column(&self, area: Rect) -> (u16, u16) {
        let used = usize::from(self.label_column(area)).saturating_add(usize::from(self.label_gap));
        let mx = area
            .x
            .saturating_add(terminal_extent(used))
            .min(area.right());
        let room =
            usize::from(area.right().saturating_sub(mx)).saturating_sub(usize::from(self.reserve));
        let mw = terminal_extent(self.meter_width.map_or(room, |w| usize::from(w).min(room)));
        if mw == 0 || mw < self.min_meter {
            (mx, 0)
        } else {
            (mx, mw)
        }
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, bg: Color) -> u16 {
        let area = area.intersection(*buf.area());
        if area.is_empty() {
            return 0;
        }
        let t = ctx.theme;
        let lw = self.label_column(area);
        let (mx, mw) = self.meter_column(area);
        let label_style = Style::new().fg(t.tone(self.label_tone)).bg(bg);
        let detail_style = Style::new().fg(t.tone(self.detail_tone)).bg(bg);
        let mut drawn = 0;
        for (i, row) in self.rows.iter().enumerate() {
            let y = area.y.saturating_add(terminal_extent(i));
            if y >= area.bottom() {
                break;
            }
            let label = if self.pad_label {
                fit(&row.label, lw as usize)
            } else {
                truncate(&row.label, lw as usize)
            };
            buf.set_string(area.x, y, label, label_style);
            drawn += 1;
            if mw == 0 {
                continue;
            }
            row.meter.render(Rect::new(mx, y, mw, 1), buf, ctx, bg);
            if let Some(d) = &row.detail {
                let dx = usize::from(mx) + usize::from(mw) + usize::from(self.detail_gap);
                if dx < usize::from(area.right()) {
                    buf.set_string(
                        terminal_extent(dx),
                        y,
                        truncate(d, usize::from(area.right()) - dx),
                        detail_style,
                    );
                }
            }
        }
        drawn
    }
}

impl crate::ui::measure::Measure for MeterList {
    fn measure(&self, avail: ratatui::layout::Size, ctx: &RenderCtx) -> ratatui::layout::Size {
        let label = self.intrinsic_label_width();
        let meter = self.meter_width.map_or_else(
            || {
                self.rows
                    .iter()
                    .map(|r| usize::from(r.meter.measure(avail, ctx).width))
                    .max()
                    .unwrap_or(0)
            },
            usize::from,
        );

        let detail = match self.meter_width {
            None => 0,
            Some(_) => self
                .rows
                .iter()
                .filter_map(|r| r.detail.as_deref())
                .map(|d| usize::from(self.detail_gap).saturating_add(width(d)))
                .max()
                .unwrap_or(0),
        };
        let total = label
            .saturating_add(usize::from(self.label_gap))
            .saturating_add(meter.max(usize::from(self.min_meter)))
            .saturating_add(detail.max(usize::from(self.reserve)));
        ratatui::layout::Size::new(terminal_extent(total), self.height())
    }
}

const SAMPLE_ID: WidgetId = WidgetId::of("meter-list-sample");

impl crate::ui::specimen::Specimen for MeterList {
    fn specimen_id(&self) -> WidgetId {
        SAMPLE_ID
    }

    fn supports(&self, state: crate::ui::specimen::SpecimenState) -> bool {
        use crate::ui::specimen::SpecimenState::*;
        matches!(state, Rest | Loading | Error | Overflow | Narrow)
    }

    fn specimen_height(&self, _state: crate::ui::specimen::SpecimenState) -> u16 {
        self.height().max(1)
    }

    fn specimen(
        &self,
        state: crate::ui::specimen::SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) {
        use crate::ui::specimen::SpecimenState::*;
        let mut l = self.clone();
        for r in &mut l.rows {
            match state {
                Loading => r.meter.tone = MeterTone::Refreshing,
                Error => {
                    r.meter.tone = MeterTone::Error;
                    r.meter.value = "read failed".into();
                }
                Overflow => {
                    r.label = format!("{} and a longer name", r.label);
                    r.detail = Some("a detail longer than its column".into());
                }
                _ => {}
            }
        }
        l.render(area, buf, ctx, bg);
    }
}
