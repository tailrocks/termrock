use ratatui::buffer::Buffer;
use ratatui::layout::{Rect, Size};
use ratatui::style::Color;

use crate::ui::ctx::RenderCtx;
use crate::ui::text::width;

pub const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn spinner_frame(tick: u64) -> &'static str {
    SPINNER[(tick % SPINNER.len() as u64) as usize]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressStatus {
    Active,
    Done,
    Error,
    Paused,
}

pub const VALUE_W: u16 = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BarLayout {
    pub label_width: Option<u16>,

    pub reserve_value: bool,
}

impl Default for BarLayout {
    fn default() -> Self {
        Self {
            label_width: None,
            reserve_value: true,
        }
    }
}

impl BarLayout {
    pub fn label_width(mut self, w: u16) -> Self {
        self.label_width = Some(w);
        self
    }
    pub fn reserve_value(mut self, on: bool) -> Self {
        self.reserve_value = on;
        self
    }

    fn label(&self, area: Rect, buf: &mut Buffer, ctx: &RenderCtx, label: &str, bg: Color) -> u16 {
        let label_w = self.label_width.unwrap_or(width(label) as u16);
        let has_label = label_w > 0 && !label.is_empty() && area.width > label_w + 8;
        if !has_label {
            return area.x;
        }
        let text = crate::ui::text::truncate(label, label_w as usize);
        buf.set_string(area.x, area.y, text, ctx.theme.primary().bg(bg));
        area.x + label_w + 2
    }
}

pub fn render_bar(
    area: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx,
    label: &str,
    ratio: f64,
    status: ProgressStatus,
    bg: Color,
) {
    render_bar_with(
        area,
        buf,
        ctx,
        label,
        ratio,
        status,
        bg,
        BarLayout::default(),
    );
}

#[allow(clippy::too_many_arguments)]
pub fn render_bar_with(
    area: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx,
    label: &str,
    ratio: f64,
    status: ProgressStatus,
    bg: Color,
    layout: BarLayout,
) {
    let area = area.intersection(*buf.area());
    if area.is_empty() {
        return;
    }
    let t = ctx.theme;
    let ratio = ratio.clamp(0.0, 1.0);
    let pct = format!("{:>4}", format!("{}%", (ratio * 100.0).round() as u32));
    let mut x = layout.label(area, buf, ctx, label, bg);
    let pct_w = 5u16;

    let suffix = match status {
        ProgressStatus::Done => format!(" {}", t.glyphs.done),
        ProgressStatus::Error => " !".to_owned(),
        ProgressStatus::Paused => format!(" {}", t.glyphs.paused),
        ProgressStatus::Active => "  ".to_owned(),
    };
    let track_w = area.right().saturating_sub(x).saturating_sub(VALUE_W);
    if track_w < 6 {
        buf.set_string(x, area.y, pct.trim_start(), t.secondary().bg(bg));
        return;
    }
    let filled = ((track_w as f64) * ratio).round() as u16;

    let fill_color = match status {
        ProgressStatus::Active => t.tone(crate::theme::Tone::Active),
        ProgressStatus::Done => t.success,
        ProgressStatus::Error => t.error,
        ProgressStatus::Paused => t.text_muted,
    };
    for i in 0..track_w {
        let (sym, st) = if i < filled {
            (
                t.glyphs.bar_fill,
                ratatui::style::Style::new().fg(fill_color).bg(bg),
            )
        } else {
            (
                t.glyphs.bar_empty,
                ratatui::style::Style::new().fg(t.border_subtle).bg(bg),
            )
        };
        buf.set_string(x + i, area.y, sym, st);
    }
    x += track_w;
    buf.set_string(x, area.y, format!(" {pct}"), t.secondary().bg(bg));
    let st = ratatui::style::Style::new().fg(fill_color).bg(bg);
    buf.set_string(x + pct_w, area.y, &suffix, st);
}

pub const METER_LOW_MAX: u8 = 59;
pub const METER_MEDIUM_MAX: u8 = 84;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeterLevel {
    Low,
    Medium,
    High,
}

impl MeterLevel {
    pub fn of(pct: u8) -> Self {
        Self::with_thresholds(pct, METER_LOW_MAX + 1, METER_MEDIUM_MAX + 1)
    }

    pub fn with_thresholds(pct: u8, warn: u8, crit: u8) -> Self {
        if pct >= crit {
            MeterLevel::High
        } else if pct >= warn {
            MeterLevel::Medium
        } else {
            MeterLevel::Low
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MeterVisual {
    #[default]
    Line,

    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MeterTone {
    #[default]
    Normal,

    Level {
        warn: u8,
        crit: u8,
    },

    Fixed(MeterLevel),

    Warning,

    Exhausted,

    Stale,

    Refreshing,

    Error,

    Unknown,
}

impl MeterTone {
    pub fn level(self, used_pct: Option<u8>) -> Option<MeterLevel> {
        match self {
            MeterTone::Normal | MeterTone::Stale => used_pct.map(MeterLevel::of),
            MeterTone::Level { warn, crit } => {
                used_pct.map(|p| MeterLevel::with_thresholds(p, warn, crit))
            }
            MeterTone::Fixed(l) => Some(l),
            MeterTone::Warning => Some(MeterLevel::Medium),
            MeterTone::Exhausted => Some(MeterLevel::High),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StaleStyle {
    #[default]
    Dashed,

    Faint,
}

#[derive(Debug, Clone)]
pub struct Meter {
    pub used_pct: Option<u8>,
    pub value: String,
    pub tone: MeterTone,
    pub visual: MeterVisual,

    pub stale: bool,

    pub stale_style: StaleStyle,
}

struct MeterPaint {
    run: Color,
    text: ratatui::style::Style,
    suffix: String,
    has_run: bool,

    fill: &'static str,

    faint: bool,

    dashed: bool,
}

impl Meter {
    pub fn new(used_pct: Option<u8>) -> Self {
        Self {
            used_pct,
            value: used_pct.map(|p| format!("{p}%")).unwrap_or_default(),
            tone: MeterTone::Normal,
            visual: MeterVisual::Line,
            stale: false,
            stale_style: StaleStyle::Dashed,
        }
    }
    pub fn value(mut self, v: impl Into<String>) -> Self {
        self.value = v.into();
        self
    }
    pub fn tone(mut self, tone: MeterTone) -> Self {
        self.tone = tone;
        self
    }
    pub fn visual(mut self, visual: MeterVisual) -> Self {
        self.visual = visual;
        self
    }

    pub fn stale(mut self, on: bool) -> Self {
        self.stale = on;
        self
    }

    pub fn stale_style(mut self, style: StaleStyle) -> Self {
        self.stale_style = style;
        self
    }

    pub fn is_stale(&self) -> bool {
        (self.stale || self.tone == MeterTone::Stale)
            && !matches!(
                self.tone,
                MeterTone::Refreshing | MeterTone::Error | MeterTone::Unknown
            )
    }

    fn paint(&self, t: &crate::theme::Theme, bg: Color) -> MeterPaint {
        use ratatui::style::{Modifier, Style};
        let level_color = |l: MeterLevel| match l {
            MeterLevel::Low => t.info,
            MeterLevel::Medium => t.warning,
            MeterLevel::High => t.error,
        };
        let (run, text, suffix, has_run) = match self.tone {
            MeterTone::Normal
            | MeterTone::Stale
            | MeterTone::Level { .. }
            | MeterTone::Fixed(_) => {
                let l = self.tone.level(self.used_pct).unwrap_or(MeterLevel::Low);
                let c = level_color(l);
                let (text, suffix) = match l {
                    MeterLevel::Low => (t.text_primary, "  ".into()),
                    MeterLevel::Medium => (c, format!(" {}", t.glyphs.warning)),
                    MeterLevel::High => (c, " !".into()),
                };
                (c, text, suffix, self.used_pct.is_some())
            }
            MeterTone::Warning => (
                t.warning,
                t.warning,
                format!(" {}", t.glyphs.warning),
                self.used_pct.is_some(),
            ),
            MeterTone::Exhausted => (t.error, t.error, " !".into(), true),
            MeterTone::Refreshing => (
                t.text_muted,
                t.text_muted,
                "  ".into(),
                self.used_pct.is_some(),
            ),
            MeterTone::Error => (t.error, t.error, " !".into(), false),
            MeterTone::Unknown => (t.text_faint, t.text_faint, "  ".into(), false),
        };
        let mut paint = MeterPaint {
            run,
            text: Style::new().fg(text).bg(bg),
            suffix,
            has_run,
            fill: t.glyphs.bar_fill,
            faint: false,
            dashed: false,
        };
        if self.is_stale() {
            match self.stale_style {
                StaleStyle::Dashed => {
                    paint.fill = t.glyphs.bar_stale;
                    paint.text = Style::new()
                        .fg(t.legible(t.text_muted, bg))
                        .bg(bg)
                        .add_modifier(Modifier::ITALIC);
                    paint.dashed = true;
                }
                StaleStyle::Faint => {
                    paint.run = t.text_faint;
                    paint.text = Style::new().fg(t.text_muted).bg(bg);
                    paint.suffix = "  ".into();
                    paint.faint = true;
                }
            }
        }
        paint
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, bg: Color) {
        let area = area.intersection(*buf.area());
        if area.is_empty() {
            return;
        }
        let t = ctx.theme;
        let paint = self.paint(t, bg);
        let (fill_color, suffix, has_run) = (paint.run, &paint.suffix, paint.has_run);
        let value = match self.tone {
            MeterTone::Refreshing => {
                format!(
                    "{} refreshing",
                    t.glyphs.spinner_frame(ctx.interaction.tick)
                )
            }
            MeterTone::Unknown if self.value.is_empty() => t.glyphs.none.to_owned(),
            _ => self.value.clone(),
        };
        let ratio = (self.used_pct.unwrap_or(0).min(100) as f64) / 100.0;
        let vw = width(&value) as u16;
        let text_style = paint.text;
        let mut suffix_style = ratatui::style::Style::new().fg(fill_color).bg(bg);
        if self.tone == MeterTone::Exhausted {
            suffix_style = suffix_style.add_modifier(ratatui::style::Modifier::BOLD);
        }
        if !has_run {
            let text = crate::ui::text::truncate(&value, area.width.saturating_sub(2) as usize);
            buf.set_string(area.x, area.y, &text, text_style);
            let sx = area.x + width(&text) as u16;
            if sx + 2 <= area.right() {
                buf.set_string(sx, area.y, suffix, suffix_style);
            }
            return;
        }
        match self.visual {
            MeterVisual::Line => {
                let track_w = area.width.saturating_sub(vw + 3);
                if track_w < MIN_TRACK {
                    buf.set_string(
                        area.x,
                        area.y,
                        crate::ui::text::truncate(&value, area.width as usize),
                        text_style,
                    );
                    return;
                }
                let filled = ((track_w as f64) * ratio).round() as u16;
                for i in 0..track_w {
                    let (sym, st) = if i < filled {
                        (
                            paint.fill,
                            ratatui::style::Style::new().fg(fill_color).bg(bg),
                        )
                    } else {
                        (
                            t.glyphs.bar_empty,
                            ratatui::style::Style::new().fg(t.border_subtle).bg(bg),
                        )
                    };
                    buf.set_string(area.x + i, area.y, sym, st);
                }
                let x = area.x + track_w + 1;
                buf.set_string(x, area.y, &value, text_style);
                buf.set_string(x + vw, area.y, suffix, suffix_style);
            }
            MeterVisual::Block => {
                let bar_w = area.width.saturating_sub(2);
                if bar_w < 4 {
                    buf.set_string(
                        area.x,
                        area.y,
                        crate::ui::text::truncate(&value, area.width as usize),
                        text_style,
                    );
                    return;
                }
                let filled = ((bar_w as f64) * ratio).round() as u16;
                let rest_bg = t.lift(bg);
                let on_fill = if paint.faint {
                    t.text_secondary
                } else {
                    t.canvas
                };
                let on_rest = if paint.faint || paint.dashed || self.tone == MeterTone::Refreshing {
                    t.legible(t.text_muted, rest_bg)
                } else {
                    t.text_primary
                };
                let cell_style = |i: u16| {
                    let in_fill = i < filled;
                    let (fg, cell_bg) = if in_fill {
                        (on_fill, fill_color)
                    } else {
                        (on_rest, rest_bg)
                    };
                    let mut st = ratatui::style::Style::new().fg(fg).bg(cell_bg);
                    if in_fill {
                        st = st.add_modifier(ratatui::style::Modifier::BOLD);
                    }
                    if paint.dashed {
                        st = st.add_modifier(ratatui::style::Modifier::ITALIC);
                    }
                    st
                };
                for i in 0..bar_w {
                    buf.set_string(area.x + i, area.y, " ", cell_style(i));
                }
                let text_area = Rect::new(area.x + 1, area.y, bar_w - 2, 1);
                crate::ui::text::paint_graphemes(
                    buf,
                    text_area,
                    (0, 0),
                    &value,
                    crate::ui::text::TextClip::Ellipsis,
                    |_, col| cell_style(1 + col as u16),
                );
                buf.set_string(area.x + bar_w, area.y, suffix, suffix_style);
            }
        }
    }
}

const MIN_TRACK: u16 = 6;

impl crate::ui::measure::Measure for Meter {
    fn measure(&self, _avail: Size, ctx: &RenderCtx) -> Size {
        let t = ctx.theme;
        let value = match self.tone {
            MeterTone::Refreshing => format!("{} refreshing", t.glyphs.spinner_frame(0)),
            MeterTone::Unknown if self.value.is_empty() => t.glyphs.none.to_owned(),
            _ => self.value.clone(),
        };
        let vw = width(&value) as u16;
        let w = if !self.paint(t, t.canvas).has_run {
            vw + 2
        } else {
            match self.visual {
                MeterVisual::Line => vw + 3 + MIN_TRACK,
                MeterVisual::Block => vw.max(4) + 2,
            }
        };
        Size::new(w, 1)
    }
}

pub fn render_meter(
    area: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx,
    used_pct: u8,
    value: &str,
    tone: MeterTone,
    bg: Color,
) {
    Meter::new(Some(used_pct))
        .value(value)
        .tone(tone)
        .render(area, buf, ctx, bg);
}

pub fn render_indeterminate(
    area: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx,
    label: &str,
    bg: Color,
) {
    render_indeterminate_with(area, buf, ctx, label, bg, BarLayout::default());
}

pub fn render_indeterminate_with(
    area: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx,
    label: &str,
    bg: Color,
    layout: BarLayout,
) {
    let area = area.intersection(*buf.area());
    if area.is_empty() {
        return;
    }
    let t = ctx.theme;
    let x = layout.label(area, buf, ctx, label, bg);
    let reserve = if layout.reserve_value { VALUE_W } else { 0 };
    let room = area.right().saturating_sub(x);

    let reserve = if room >= reserve + 6 { reserve } else { 0 };
    let track_w = room.saturating_sub(reserve).max(1) as i64;
    let seg = (track_w / 5).clamp(2, 8);
    let period = track_w + seg;
    let pos = (ctx.interaction.tick as i64 % period) - seg;
    for i in 0..track_w {
        let in_seg = i >= pos && i < pos + seg;
        let (sym, st) = if in_seg {
            (
                t.glyphs.bar_fill,
                ratatui::style::Style::new().fg(t.accent).bg(bg),
            )
        } else {
            (
                t.glyphs.bar_empty,
                ratatui::style::Style::new().fg(t.border_subtle).bg(bg),
            )
        };
        buf.set_string(x + i as u16, area.y, sym, st);
    }
}

pub fn render_spinner(area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, label: &str, bg: Color) {
    let area = area.intersection(*buf.area());
    if area.is_empty() {
        return;
    }
    let t = ctx.theme;
    let glyph = crate::ui::text::paint_clipped(
        buf,
        area,
        0,
        t.glyphs.spinner_frame(ctx.interaction.tick),
        ratatui::style::Style::new()
            .fg(t.legible_mark(t.accent, bg))
            .bg(bg),
    );
    if glyph > 0 {
        crate::ui::text::paint_truncated(
            buf,
            area,
            glyph + 1,
            label,
            ratatui::style::Style::new()
                .fg(t.legible(t.text_secondary, bg))
                .bg(bg),
        );
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProgressBar {
    pub label: String,

    pub ratio: Option<f64>,
    pub status: ProgressStatus,
    pub layout: BarLayout,
}

impl ProgressBar {
    pub fn new(label: &str, ratio: f64) -> Self {
        Self {
            label: label.to_owned(),
            ratio: Some(ratio),
            status: ProgressStatus::Active,
            layout: BarLayout::default(),
        }
    }

    pub fn indeterminate(label: &str) -> Self {
        Self {
            ratio: None,
            ..Self::new(label, 0.0)
        }
    }
    pub fn status(mut self, status: ProgressStatus) -> Self {
        self.status = status;
        self
    }
    pub fn layout(mut self, layout: BarLayout) -> Self {
        self.layout = layout;
        self
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, bg: Color) {
        match self.ratio {
            Some(r) => {
                render_bar_with(area, buf, ctx, &self.label, r, self.status, bg, self.layout)
            }
            None => render_indeterminate_with(area, buf, ctx, &self.label, bg, self.layout),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Tally {
    pub done: usize,
    pub total: usize,
    pub running: usize,
    pub paused: usize,
    pub failed: usize,
}

impl Tally {
    pub fn of(statuses: impl IntoIterator<Item = ProgressStatus>) -> Self {
        let mut t = Self::default();
        for s in statuses {
            t.total += 1;
            match s {
                ProgressStatus::Done => t.done += 1,
                ProgressStatus::Active => t.running += 1,
                ProgressStatus::Paused => t.paused += 1,
                ProgressStatus::Error => t.failed += 1,
            }
        }
        t
    }

    fn clauses(&self) -> Vec<(String, bool)> {
        let mut out = vec![(format!("{} of {} done", self.done, self.total), false)];
        for (n, what, failed) in [
            (self.running, "running", false),
            (self.paused, "paused", false),
            (self.failed, "failed", true),
        ] {
            if n > 0 {
                out.push((format!("{n} {what}"), failed));
            }
        }
        out
    }

    pub fn text(&self) -> String {
        self.clauses()
            .into_iter()
            .map(|(c, _)| c)
            .collect::<Vec<_>>()
            .join(" · ")
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, ctx: &RenderCtx, bg: Color) -> u16 {
        let area = area.intersection(*buf.area());
        if area.is_empty() {
            return 0;
        }
        let t = ctx.theme;
        let mut x = area.x;
        for (i, (clause, failed)) in self.clauses().into_iter().enumerate() {
            let sep = if i == 0 { "" } else { " · " };
            let tone = if failed { t.error_fg() } else { t.secondary() };
            for (s, style) in [(sep.to_owned(), t.faint()), (clause, tone)] {
                let room = area.right().saturating_sub(x) as usize;
                if room == 0 {
                    return x - area.x;
                }
                let s = crate::ui::text::truncate(&s, room);
                buf.set_string(x, area.y, &s, style.bg(bg));
                x += width(&s) as u16;
            }
        }
        x - area.x
    }
}

const SAMPLE_ID: crate::core::id::WidgetId = crate::core::id::WidgetId::of("progress-sample");

impl crate::ui::specimen::Specimen for ProgressBar {
    fn specimen_id(&self) -> crate::core::id::WidgetId {
        SAMPLE_ID
    }

    fn supports(&self, state: crate::ui::specimen::SpecimenState) -> bool {
        use crate::ui::specimen::SpecimenState::*;
        match state {
            Rest | Empty | Loading | Error | Narrow => true,

            Overflow => !self.label.is_empty(),
            _ => false,
        }
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
        let mut b = self.clone();
        match state {
            Empty => b.ratio = b.ratio.map(|_| 0.0),
            Loading => b.ratio = None,
            Error => b.status = ProgressStatus::Error,
            Overflow => b.label = format!("{} and a long label", b.label),
            _ => {}
        }
        b.render(area, buf, ctx, bg);
    }
}

impl crate::ui::specimen::Specimen for Meter {
    fn specimen_id(&self) -> crate::core::id::WidgetId {
        SAMPLE_ID
    }

    fn supports(&self, state: crate::ui::specimen::SpecimenState) -> bool {
        use crate::ui::specimen::SpecimenState::*;
        matches!(state, Rest | Empty | Loading | Error | Overflow | Narrow)
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
        let mut m = self.clone();
        match state {
            Empty => {
                m.tone = MeterTone::Unknown;
                m.used_pct = None;
                m.value.clear();
            }
            Loading => m.tone = MeterTone::Refreshing,
            Error => {
                m.tone = MeterTone::Error;
                m.value = "read failed".into();
            }
            Overflow => m.value = format!("{} of a long quota name", m.value),
            _ => {}
        }
        m.render(area, buf, ctx, bg);
    }
}
