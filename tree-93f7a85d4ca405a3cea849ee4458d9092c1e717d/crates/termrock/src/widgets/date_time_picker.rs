// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Date, time, and range selection for terminal forms that benefit from browse UX.
//!
//! **Mission.** Only where a calendar/time list helps (scheduling, filters with
//! bounds, relative “today”). Free-form rare values should use plain
//! [`TextInput`](super::TextInput) with ISO strings instead.
//!
//! **Storage.** Locale-independent civil types ([`CivilDate`], [`CivilTime`],
//! [`CivilDateTime`]). Presentation formats are explicit enums — never implied
//! by host locale. Timezone is a **display label** only (host owns real TZ).
//!
//! **vs TextInput.** Prefer TextInput for ISO paste, logs, and rare absolute
//! stamps with no browse need. Prefer DateTimePicker when users navigate months,
//! pick ranges, or choose from a stepped time list.
//!
//! Research: shadcn Calendar/DatePicker, Textual DateTimeInput patterns.

#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Modifier,
    widgets::{StatefulWidget, Widget},
};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    interaction::{
        CollectionItem, CollectionOutcome, CollectionState, OverlayId, OverlayOutcome, OverlaySize,
        OverlaySpec, OverlayStack, SemanticNode, SemanticRole, SemanticScene, SemanticState,
        UiIntent,
    },
    style::{DesignSystem, Role},
    text::{display_cols, take_display_cols},
};

use super::{Panel, PanelChrome, TextInput, TextInputOutcome, TextInputState, Validation};

/// Overlay id for modal datetime pickers.
pub const DATE_TIME_PICKER_OVERLAY_ID: &str = "termrock.date-time-picker";
/// Width under which grid becomes day-list fallback.
pub const DATE_TIME_PICKER_LIST_MAX_WIDTH: u16 = 36;
/// Height under which presentation prefers fullscreen / compact.
pub const DATE_TIME_PICKER_FULLSCREEN_MAX_HEIGHT: u16 = 12;

// ── Civil storage (locale-independent) ──────────────────────────────────────

/// Gregorian civil date without timezone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CivilDate {
    /// Year (…, -1, 0, 1, …) — proleptic Gregorian.
    pub year: i32,
    /// Month 1..=12.
    pub month: u8,
    /// Day 1..=31.
    pub day: u8,
}

impl CivilDate {
    /// Construct if components form a valid Gregorian date.
    #[must_use]
    pub fn new(year: i32, month: u8, day: u8) -> Option<Self> {
        if !(1..=12).contains(&month) {
            return None;
        }
        let dim = days_in_month(year, month)?;
        if day == 0 || day > dim {
            return None;
        }
        Some(Self { year, month, day })
    }

    /// ISO-8601 `YYYY-MM-DD` (negative years as `-YYYY-MM-DD`).
    #[must_use]
    pub fn to_iso(self) -> String {
        if self.year < 0 {
            format!("-{:04}-{:02}-{:02}", -self.year, self.month, self.day)
        } else {
            format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
        }
    }

    /// Parse `YYYY-MM-DD` or `-YYYY-MM-DD`.
    #[must_use]
    pub fn parse_iso(s: &str) -> Option<Self> {
        let s = s.trim();
        let (neg, rest) = if let Some(r) = s.strip_prefix('-') {
            (true, r)
        } else {
            (false, s)
        };
        let parts: Vec<&str> = rest.split('-').collect();
        if parts.len() != 3 {
            return None;
        }
        let y: i32 = parts[0].parse().ok()?;
        let m: u8 = parts[1].parse().ok()?;
        let d: u8 = parts[2].parse().ok()?;
        let year = if neg { -y } else { y };
        Self::new(year, m, d)
    }

    /// Day of week: 0 = Monday … 6 = Sunday (ISO).
    #[must_use]
    pub fn weekday_iso(self) -> u8 {
        // Sakamoto algorithm → 0=Sun …; convert to ISO Mon=0
        let mut y = self.year;
        let m = self.month as i32;
        let d = self.day as i32;
        let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
        if m < 3 {
            y -= 1;
        }
        let w = (y + y / 4 - y / 100 + y / 400 + t[(m - 1) as usize] + d).rem_euclid(7);
        // w: 0=Sun … 6=Sat → ISO Mon=0
        ((w + 6) % 7) as u8
    }

    /// Add `days` (can be negative).
    #[must_use]
    pub fn add_days(self, days: i32) -> Self {
        if days == 0 {
            return self;
        }
        let mut y = self.year;
        let mut m = self.month;
        let mut d = i32::from(self.day) + days;
        // Walk months (bounded loops; fine for UI navigation).
        for _ in 0..10_000 {
            let dim = i32::from(days_in_month(y, m).unwrap_or(28));
            if d > dim {
                d -= dim;
                m = m.saturating_add(1);
                if m > 12 {
                    m = 1;
                    y = y.saturating_add(1);
                }
            } else if d < 1 {
                m = m.saturating_sub(1);
                if m == 0 {
                    m = 12;
                    y = y.saturating_sub(1);
                }
                d += i32::from(days_in_month(y, m).unwrap_or(28));
            } else {
                break;
            }
        }
        Self::new(y, m, d as u8).unwrap_or(self)
    }

    /// Add months, clamping day into target month.
    #[must_use]
    pub fn add_months(self, months: i32) -> Self {
        let total = i32::from(self.month) - 1 + months;
        let y_add = total.div_euclid(12);
        let m = (total.rem_euclid(12) + 1) as u8;
        let y = self.year.saturating_add(y_add);
        let dim = days_in_month(y, m).unwrap_or(28);
        let d = self.day.min(dim);
        Self::new(y, m, d).unwrap_or(self)
    }
}

/// Time of day without timezone (0..=23:59:59).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CivilTime {
    /// Hour 0..=23.
    pub hour: u8,
    /// Minute 0..=59.
    pub minute: u8,
    /// Second 0..=59.
    pub second: u8,
}

impl CivilTime {
    /// Construct if valid.
    #[must_use]
    pub fn new(hour: u8, minute: u8, second: u8) -> Option<Self> {
        if hour > 23 || minute > 59 || second > 59 {
            return None;
        }
        Some(Self {
            hour,
            minute,
            second,
        })
    }

    /// Midnight.
    #[must_use]
    pub const fn midnight() -> Self {
        Self {
            hour: 0,
            minute: 0,
            second: 0,
        }
    }

    /// `HH:MM` or `HH:MM:SS`.
    #[must_use]
    pub fn to_iso(self, with_seconds: bool) -> String {
        if with_seconds {
            format!("{:02}:{:02}:{:02}", self.hour, self.minute, self.second)
        } else {
            format!("{:02}:{:02}", self.hour, self.minute)
        }
    }

    /// Parse `HH:MM` or `HH:MM:SS`.
    #[must_use]
    pub fn parse_iso(s: &str) -> Option<Self> {
        let s = s.trim();
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() == 2 {
            let h: u8 = parts[0].parse().ok()?;
            let m: u8 = parts[1].parse().ok()?;
            Self::new(h, m, 0)
        } else if parts.len() == 3 {
            let h: u8 = parts[0].parse().ok()?;
            let m: u8 = parts[1].parse().ok()?;
            let sec: u8 = parts[2].parse().ok()?;
            Self::new(h, m, sec)
        } else {
            None
        }
    }

    /// Minutes since midnight.
    #[must_use]
    pub fn minutes_since_midnight(self) -> u32 {
        u32::from(self.hour) * 60 + u32::from(self.minute)
    }

    /// From minutes since midnight (seconds 0).
    #[must_use]
    pub fn from_minutes(mins: u32) -> Option<Self> {
        if mins >= 24 * 60 {
            return None;
        }
        Self::new((mins / 60) as u8, (mins % 60) as u8, 0)
    }
}

/// Civil date + time (no timezone).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CivilDateTime {
    /// Date part.
    pub date: CivilDate,
    /// Time part.
    pub time: CivilTime,
}

impl CivilDateTime {
    /// Combine.
    #[must_use]
    pub const fn new(date: CivilDate, time: CivilTime) -> Self {
        Self { date, time }
    }

    /// `YYYY-MM-DDTHH:MM` or with seconds.
    #[must_use]
    pub fn to_iso(self, with_seconds: bool) -> String {
        format!("{}T{}", self.date.to_iso(), self.time.to_iso(with_seconds))
    }

    /// Parse `YYYY-MM-DDTHH:MM[:SS]` or space separator.
    #[must_use]
    pub fn parse_iso(s: &str) -> Option<Self> {
        let s = s.trim();
        let (d, t) = s.split_once('T').or_else(|| s.split_once(' '))?;
        Some(Self {
            date: CivilDate::parse_iso(d)?,
            time: CivilTime::parse_iso(t)?,
        })
    }
}

/// Inclusive civil date range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CivilDateRange {
    /// Start (≤ end when valid).
    pub start: CivilDate,
    /// End.
    pub end: CivilDate,
}

impl CivilDateRange {
    /// Ordered range (swaps if needed).
    #[must_use]
    pub fn new(a: CivilDate, b: CivilDate) -> Self {
        if a <= b {
            Self { start: a, end: b }
        } else {
            Self { start: b, end: a }
        }
    }

    /// Whether `d` is inside inclusive range.
    #[must_use]
    pub fn contains(self, d: CivilDate) -> bool {
        d >= self.start && d <= self.end
    }
}

fn is_leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i32, month: u8) -> Option<u8> {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => Some(31),
        4 | 6 | 9 | 11 => Some(30),
        2 => Some(if is_leap(year) { 29 } else { 28 }),
        _ => None,
    }
}

// ── Presentation formats (explicit; not locale) ─────────────────────────────

/// How dates are shown / parsed in the field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum DateDisplayFormat {
    /// `YYYY-MM-DD` (storage default).
    #[default]
    Iso,
    /// `YYYY/MM/DD`.
    YmdSlash,
    /// `MM/DD/YYYY`.
    MdySlash,
    /// `DD/MM/YYYY`.
    DmySlash,
}

impl DateDisplayFormat {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Iso => "iso",
            Self::YmdSlash => "ymd-slash",
            Self::MdySlash => "mdy-slash",
            Self::DmySlash => "dmy-slash",
        }
    }

    /// Format date.
    #[must_use]
    pub fn format(self, d: CivilDate) -> String {
        match self {
            Self::Iso => d.to_iso(),
            Self::YmdSlash => format!("{:04}/{:02}/{:02}", d.year, d.month, d.day),
            Self::MdySlash => format!("{:02}/{:02}/{:04}", d.month, d.day, d.year),
            Self::DmySlash => format!("{:02}/{:02}/{:04}", d.day, d.month, d.year),
        }
    }

    /// Parse with this format (also accepts ISO as fallback).
    #[must_use]
    pub fn parse(self, s: &str) -> Option<CivilDate> {
        let s = s.trim();
        if let Some(d) = CivilDate::parse_iso(s) {
            return Some(d);
        }
        match self {
            Self::Iso => None,
            Self::YmdSlash => {
                let p: Vec<_> = s.split('/').collect();
                if p.len() != 3 {
                    return None;
                }
                CivilDate::new(p[0].parse().ok()?, p[1].parse().ok()?, p[2].parse().ok()?)
            }
            Self::MdySlash => {
                let p: Vec<_> = s.split('/').collect();
                if p.len() != 3 {
                    return None;
                }
                CivilDate::new(p[2].parse().ok()?, p[0].parse().ok()?, p[1].parse().ok()?)
            }
            Self::DmySlash => {
                let p: Vec<_> = s.split('/').collect();
                if p.len() != 3 {
                    return None;
                }
                CivilDate::new(p[2].parse().ok()?, p[1].parse().ok()?, p[0].parse().ok()?)
            }
        }
    }
}

/// How times are shown / parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TimeDisplayFormat {
    /// `HH:MM` 24h.
    #[default]
    Hm24,
    /// `HH:MM:SS` 24h.
    Hms24,
    /// `h:MM AM/PM` (presentation only; storage still 24h civil).
    Hm12,
}

impl TimeDisplayFormat {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Hm24 => "hm24",
            Self::Hms24 => "hms24",
            Self::Hm12 => "hm12",
        }
    }

    /// Format time.
    #[must_use]
    pub fn format(self, t: CivilTime) -> String {
        match self {
            Self::Hm24 => t.to_iso(false),
            Self::Hms24 => t.to_iso(true),
            Self::Hm12 => {
                let (h12, am) = to_12h(t.hour);
                format!("{}:{:02} {}", h12, t.minute, if am { "AM" } else { "PM" })
            }
        }
    }

    /// Parse.
    #[must_use]
    pub fn parse(self, s: &str) -> Option<CivilTime> {
        let s = s.trim();
        if let Some(t) = CivilTime::parse_iso(s) {
            return Some(t);
        }
        if matches!(self, Self::Hm12)
            || s.to_ascii_uppercase().contains("AM")
            || s.to_ascii_uppercase().contains("PM")
        {
            return parse_12h(s);
        }
        None
    }
}

fn to_12h(hour: u8) -> (u8, bool) {
    let am = hour < 12;
    let h = match hour {
        0 => 12,
        1..=12 => hour,
        _ => hour - 12,
    };
    (h, am)
}

fn parse_12h(s: &str) -> Option<CivilTime> {
    let u = s.trim().to_ascii_uppercase();
    let (body, am) = if let Some(b) = u.strip_suffix("AM") {
        (b.trim(), true)
    } else if let Some(b) = u.strip_suffix("PM") {
        (b.trim(), false)
    } else {
        return None;
    };
    let parts: Vec<_> = body.split(':').collect();
    if parts.len() < 2 {
        return None;
    }
    let mut h: u8 = parts[0].parse().ok()?;
    let m: u8 = parts[1].parse().ok()?;
    let sec: u8 = if parts.len() > 2 {
        parts[2].parse().ok()?
    } else {
        0
    };
    if !(1..=12).contains(&h) {
        return None;
    }
    h = match (h, am) {
        (12, true) => 0,
        (12, false) => 12,
        (h, true) => h,
        (h, false) => h + 12,
    };
    CivilTime::new(h, m, sec)
}

// ── Mode / view / presentation ──────────────────────────────────────────────

/// What the control edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum DateTimePickerKind {
    /// Single date.
    #[default]
    Date,
    /// Time only.
    Time,
    /// Date + time.
    DateTime,
    /// Inclusive date range (start/end).
    DateRange,
}

impl DateTimePickerKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Date => "date",
            Self::Time => "time",
            Self::DateTime => "date-time",
            Self::DateRange => "date-range",
        }
    }

    /// Needs calendar / day list.
    #[must_use]
    pub const fn has_date(self) -> bool {
        !matches!(self, Self::Time)
    }

    /// Needs time list / time field.
    #[must_use]
    pub const fn has_time(self) -> bool {
        matches!(self, Self::Time | Self::DateTime)
    }

    /// Range selection.
    #[must_use]
    pub const fn is_range(self) -> bool {
        matches!(self, Self::DateRange)
    }
}

/// Active interaction surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum DateTimePickerView {
    /// Text field (always available).
    #[default]
    Field,
    /// Month grid calendar.
    Calendar,
    /// Stepped time list.
    TimeList,
    /// Day list (tiny-terminal fallback).
    DayList,
}

impl DateTimePickerView {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Field => "field",
            Self::Calendar => "calendar",
            Self::TimeList => "time-list",
            Self::DayList => "day-list",
        }
    }
}

/// Host layout presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum DateTimePickerPresentation {
    /// In-place expanded panel.
    #[default]
    Embedded,
    /// Modal / popover overlay.
    Modal,
    /// Fullscreen on tiny terminals.
    Fullscreen,
}

impl DateTimePickerPresentation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Embedded => "embedded",
            Self::Modal => "modal",
            Self::Fullscreen => "fullscreen",
        }
    }
}

/// First day of week for grid headers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum WeekStart {
    /// Monday (ISO).
    #[default]
    Monday,
    /// Sunday.
    Sunday,
}

impl WeekStart {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Monday => "monday",
            Self::Sunday => "sunday",
        }
    }

    /// Header labels (ASCII).
    #[must_use]
    pub const fn headers(self) -> [&'static str; 7] {
        match self {
            Self::Monday => ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"],
            Self::Sunday => ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"],
        }
    }

    /// Column index 0..7 for ISO weekday (Mon=0).
    #[must_use]
    pub const fn column(self, iso_weekday: u8) -> u8 {
        match self {
            Self::Monday => iso_weekday % 7,
            Self::Sunday => (iso_weekday + 1) % 7,
        }
    }
}

/// Validation / commit status for the field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum DateTimeValidity {
    /// Empty allowed / no value.
    #[default]
    Empty,
    /// Draft parses and is in range.
    Valid,
    /// Intermediate typing (not yet parseable).
    Intermediate,
    /// Parsed but outside min/max or bad range order.
    OutOfRange,
    /// Unparseable.
    Invalid,
}

impl DateTimeValidity {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Valid => "valid",
            Self::Intermediate => "intermediate",
            Self::OutOfRange => "out-of-range",
            Self::Invalid => "invalid",
        }
    }
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// DateTimePicker outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DateTimePickerOutcome {
    /// No effect.
    Ignored,
    /// Focus / view / cursor chrome.
    Changed,
    /// Committed single date.
    DateChanged {
        /// Value.
        date: CivilDate,
    },
    /// Committed time.
    TimeChanged {
        /// Value.
        time: CivilTime,
    },
    /// Committed date-time.
    DateTimeChanged {
        /// Value.
        value: CivilDateTime,
    },
    /// Committed range.
    RangeChanged {
        /// Value.
        range: CivilDateRange,
    },
    /// Value cleared.
    Cleared,
    /// Validation failed on commit.
    ValidationFailed {
        /// Reason.
        reason: DateTimeValidity,
    },
    /// Expanded / view opened (host may place overlay).
    Opened {
        /// View.
        view: DateTimePickerView,
        /// Presentation.
        presentation: DateTimePickerPresentation,
    },
    /// Collapsed to field.
    Closed,
    /// Cancelled (Esc from field / modal).
    Cancelled,
    /// Presentation hint.
    PresentationChanged {
        /// Presentation.
        presentation: DateTimePickerPresentation,
    },
}

// ── State ───────────────────────────────────────────────────────────────────

/// Runtime state for [`DateTimePicker`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateTimePickerState {
    kind: DateTimePickerKind,
    date_fmt: DateDisplayFormat,
    time_fmt: TimeDisplayFormat,
    week_start: WeekStart,
    /// Host-injected “today” for markers (required for non-color today cue).
    today: Option<CivilDate>,
    /// Inclusive min date.
    min_date: Option<CivilDate>,
    /// Inclusive max date.
    max_date: Option<CivilDate>,
    /// Time step minutes for list (1..=60 recommended).
    time_step_minutes: u32,
    /// Timezone display only (e.g. `"UTC"`, `"America/New_York"`).
    timezone_label: Option<String>,
    /// Committed date (Date / DateTime / range start).
    value_date: Option<CivilDate>,
    /// Committed time.
    value_time: Option<CivilTime>,
    /// Range end when `DateRange`.
    value_end: Option<CivilDate>,
    /// Range pick phase: false = next click is start/reset, true = picking end.
    range_picking_end: bool,
    draft: TextInputState,
    view: DateTimePickerView,
    open: bool,
    presentation: DateTimePickerPresentation,
    /// Calendar cursor month.
    view_year: i32,
    view_month: u8,
    /// Focused day in calendar / list.
    focus_date: Option<CivilDate>,
    /// Time list collection (ids = minutes since midnight as string).
    time_collection: CollectionState<String>,
    /// Day list collection (ids = ISO date).
    day_collection: CollectionState<String>,
    focused: bool,
    enabled: bool,
    allow_empty: bool,
    validity: DateTimeValidity,
    // hit geometry
    cell_hits: Vec<(CivilDate, Rect)>,
    time_hits: Vec<(CivilTime, Rect)>,
    field_area: Rect,
    grid_area: Rect,
    root: Rect,
}

impl Default for DateTimePickerState {
    fn default() -> Self {
        Self::new(DateTimePickerKind::Date)
    }
}

impl DateTimePickerState {
    /// New picker of `kind`.
    #[must_use]
    pub fn new(kind: DateTimePickerKind) -> Self {
        let mut draft = TextInputState::new("").with_allow_empty(true);
        draft.set_focused(false);
        let today = CivilDate::new(2026, 8, 10); // neutral seed; host should set_today
        Self {
            kind,
            date_fmt: DateDisplayFormat::Iso,
            time_fmt: TimeDisplayFormat::Hm24,
            week_start: WeekStart::Monday,
            today,
            min_date: None,
            max_date: None,
            time_step_minutes: 15,
            timezone_label: None,
            value_date: None,
            value_time: None,
            value_end: None,
            range_picking_end: false,
            draft,
            view: DateTimePickerView::Field,
            open: false,
            presentation: DateTimePickerPresentation::Embedded,
            view_year: 2026,
            view_month: 8,
            focus_date: today,
            time_collection: CollectionState::new().wrap(true),
            day_collection: CollectionState::new().wrap(true),
            focused: false,
            enabled: true,
            allow_empty: true,
            validity: DateTimeValidity::Empty,
            cell_hits: Vec::new(),
            time_hits: Vec::new(),
            field_area: Rect::default(),
            grid_area: Rect::default(),
            root: Rect::default(),
        }
    }

    /// Kind.
    #[must_use]
    pub const fn with_kind(mut self, kind: DateTimePickerKind) -> Self {
        self.kind = kind;
        self
    }

    /// Date display format.
    #[must_use]
    pub const fn with_date_format(mut self, fmt: DateDisplayFormat) -> Self {
        self.date_fmt = fmt;
        self
    }

    /// Time display format.
    #[must_use]
    pub const fn with_time_format(mut self, fmt: TimeDisplayFormat) -> Self {
        self.time_fmt = fmt;
        self
    }

    /// Week start.
    #[must_use]
    pub const fn with_week_start(mut self, w: WeekStart) -> Self {
        self.week_start = w;
        self
    }

    /// Time list step.
    #[must_use]
    pub fn with_time_step_minutes(mut self, step: u32) -> Self {
        self.time_step_minutes = step.clamp(1, 60);
        self
    }

    /// Timezone label (display only).
    #[must_use]
    pub fn with_timezone_label(mut self, label: impl Into<String>) -> Self {
        self.timezone_label = Some(label.into());
        self
    }

    /// Allow empty commit.
    #[must_use]
    pub const fn with_allow_empty(mut self, on: bool) -> Self {
        self.allow_empty = on;
        self
    }

    /// Min date inclusive.
    #[must_use]
    pub const fn with_min_date(mut self, d: CivilDate) -> Self {
        self.min_date = Some(d);
        self
    }

    /// Max date inclusive.
    #[must_use]
    pub const fn with_max_date(mut self, d: CivilDate) -> Self {
        self.max_date = Some(d);
        self
    }

    /// Presentation.
    #[must_use]
    pub const fn with_presentation(mut self, p: DateTimePickerPresentation) -> Self {
        self.presentation = p;
        self
    }

    /// Initial date value.
    #[must_use]
    pub fn with_date(mut self, d: CivilDate) -> Self {
        self.set_date(Some(d));
        self
    }

    /// Initial time.
    #[must_use]
    pub fn with_time(mut self, t: CivilTime) -> Self {
        self.set_time(Some(t));
        self
    }

    /// Initial range.
    #[must_use]
    pub fn with_range(mut self, range: CivilDateRange) -> Self {
        self.set_range(Some(range));
        self
    }

    // ── accessors ───────────────────────────────────────────────────────────

    /// Kind.
    #[must_use]
    pub const fn kind(&self) -> DateTimePickerKind {
        self.kind
    }

    /// View.
    #[must_use]
    pub const fn view(&self) -> DateTimePickerView {
        self.view
    }

    /// Expanded.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.open
    }

    /// Validity of draft / value.
    #[must_use]
    pub const fn validity(&self) -> DateTimeValidity {
        self.validity
    }

    /// Committed date.
    #[must_use]
    pub const fn date(&self) -> Option<CivilDate> {
        self.value_date
    }

    /// Committed time.
    #[must_use]
    pub const fn time(&self) -> Option<CivilTime> {
        self.value_time
    }

    /// Committed datetime.
    #[must_use]
    pub fn datetime(&self) -> Option<CivilDateTime> {
        Some(CivilDateTime::new(self.value_date?, self.value_time?))
    }

    /// Committed range.
    #[must_use]
    pub fn range(&self) -> Option<CivilDateRange> {
        Some(CivilDateRange {
            start: self.value_date?,
            end: self.value_end?,
        })
    }

    /// Draft text.
    #[must_use]
    pub fn draft(&self) -> &str {
        self.draft.value()
    }

    fn set_draft_text(&mut self, text: impl Into<String>) {
        let mut draft = TextInputState::new(text).with_allow_empty(true);
        draft.set_enabled(self.enabled);
        draft.set_focused(self.focused && matches!(self.view, DateTimePickerView::Field));
        self.draft = draft;
    }

    /// Today marker.
    #[must_use]
    pub const fn today(&self) -> Option<CivilDate> {
        self.today
    }

    /// Timezone label.
    #[must_use]
    pub fn timezone_label(&self) -> Option<&str> {
        self.timezone_label.as_deref()
    }

    /// Focus date in calendar.
    #[must_use]
    pub const fn focus_date(&self) -> Option<CivilDate> {
        self.focus_date
    }

    /// Presentation.
    #[must_use]
    pub const fn presentation(&self) -> DateTimePickerPresentation {
        self.presentation
    }

    /// Whether date is available (min/max).
    #[must_use]
    pub fn is_available(&self, d: CivilDate) -> bool {
        if self.min_date.is_some_and(|m| d < m) {
            return false;
        }
        if self.max_date.is_some_and(|m| d > m) {
            return false;
        }
        true
    }

    /// Host injects civil “today”.
    pub fn set_today(&mut self, d: CivilDate) {
        self.today = Some(d);
        if self.focus_date.is_none() {
            self.focus_date = Some(d);
        }
        if self.value_date.is_none() {
            self.view_year = d.year;
            self.view_month = d.month;
        }
    }

    /// Min date.
    pub fn set_min_date(&mut self, d: Option<CivilDate>) {
        self.min_date = d;
        self.refresh_validity();
    }

    /// Max date.
    pub fn set_max_date(&mut self, d: Option<CivilDate>) {
        self.max_date = d;
        self.refresh_validity();
    }

    /// Timezone label.
    pub fn set_timezone_label(&mut self, label: Option<String>) {
        self.timezone_label = label;
    }

    /// Set committed date and sync draft.
    pub fn set_date(&mut self, d: Option<CivilDate>) {
        self.value_date = d;
        if let Some(d) = d {
            self.view_year = d.year;
            self.view_month = d.month;
            self.focus_date = Some(d);
        }
        self.sync_draft_from_value();
        self.refresh_validity();
    }

    /// Set committed time.
    pub fn set_time(&mut self, t: Option<CivilTime>) {
        self.value_time = t;
        self.sync_draft_from_value();
        self.refresh_validity();
    }

    /// Set range.
    pub fn set_range(&mut self, range: Option<CivilDateRange>) {
        if let Some(r) = range {
            self.value_date = Some(r.start);
            self.value_end = Some(r.end);
            self.view_year = r.start.year;
            self.view_month = r.start.month;
            self.focus_date = Some(r.start);
        } else {
            self.value_date = None;
            self.value_end = None;
        }
        self.range_picking_end = false;
        self.sync_draft_from_value();
        self.refresh_validity();
    }

    /// Focus.
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on;
        self.draft
            .set_focused(on && matches!(self.view, DateTimePickerView::Field));
    }

    /// Enabled.
    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
        self.draft.set_enabled(on);
    }

    /// Auto presentation from bounds.
    #[must_use]
    pub fn presentation_for_bounds(bounds: Rect) -> DateTimePickerPresentation {
        if bounds.width < DATE_TIME_PICKER_LIST_MAX_WIDTH
            || bounds.height < DATE_TIME_PICKER_FULLSCREEN_MAX_HEIGHT
        {
            DateTimePickerPresentation::Fullscreen
        } else {
            DateTimePickerPresentation::Embedded
        }
    }

    /// Prefer list vs grid from width.
    #[must_use]
    pub fn preferred_date_view(bounds: Rect) -> DateTimePickerView {
        if bounds.width < DATE_TIME_PICKER_LIST_MAX_WIDTH {
            DateTimePickerView::DayList
        } else {
            DateTimePickerView::Calendar
        }
    }

    fn sync_draft_from_value(&mut self) {
        let text = match self.kind {
            DateTimePickerKind::Date => self
                .value_date
                .map(|d| self.date_fmt.format(d))
                .unwrap_or_default(),
            DateTimePickerKind::Time => self
                .value_time
                .map(|t| self.time_fmt.format(t))
                .unwrap_or_default(),
            DateTimePickerKind::DateTime => match (self.value_date, self.value_time) {
                (Some(d), Some(t)) => {
                    format!("{} {}", self.date_fmt.format(d), self.time_fmt.format(t))
                }
                (Some(d), None) => self.date_fmt.format(d),
                _ => String::new(),
            },
            DateTimePickerKind::DateRange => match (self.value_date, self.value_end) {
                (Some(a), Some(b)) => {
                    format!("{}..{}", self.date_fmt.format(a), self.date_fmt.format(b))
                }
                (Some(a), None) => self.date_fmt.format(a),
                _ => String::new(),
            },
        };
        self.set_draft_text(text);
    }

    fn refresh_validity(&mut self) {
        let text = self.draft.value().trim();
        if text.is_empty() {
            self.validity = if self.allow_empty {
                DateTimeValidity::Empty
            } else {
                DateTimeValidity::Invalid
            };
            return;
        }
        self.validity = match self.try_parse_draft() {
            Ok(_) => DateTimeValidity::Valid,
            Err(DateTimeValidity::Intermediate) => DateTimeValidity::Intermediate,
            Err(v) => v,
        };
    }

    fn try_parse_draft(&self) -> Result<ParsedValue, DateTimeValidity> {
        let text = self.draft.value().trim();
        if text.is_empty() {
            return if self.allow_empty {
                Err(DateTimeValidity::Empty)
            } else {
                Err(DateTimeValidity::Invalid)
            };
        }
        match self.kind {
            DateTimePickerKind::Date => {
                let d = self
                    .date_fmt
                    .parse(text)
                    .ok_or(if looks_partial_date(text) {
                        DateTimeValidity::Intermediate
                    } else {
                        DateTimeValidity::Invalid
                    })?;
                if !self.is_available(d) {
                    return Err(DateTimeValidity::OutOfRange);
                }
                Ok(ParsedValue::Date(d))
            }
            DateTimePickerKind::Time => {
                let t = self
                    .time_fmt
                    .parse(text)
                    .ok_or(if looks_partial_time(text) {
                        DateTimeValidity::Intermediate
                    } else {
                        DateTimeValidity::Invalid
                    })?;
                Ok(ParsedValue::Time(t))
            }
            DateTimePickerKind::DateTime => {
                let (ds, ts) = split_datetime_text(text).ok_or(DateTimeValidity::Intermediate)?;
                let d = self
                    .date_fmt
                    .parse(ds)
                    .or_else(|| CivilDate::parse_iso(ds))
                    .ok_or(DateTimeValidity::Intermediate)?;
                let t = if ts.is_empty() {
                    return Err(DateTimeValidity::Intermediate);
                } else {
                    self.time_fmt
                        .parse(ts)
                        .ok_or(DateTimeValidity::Intermediate)?
                };
                if !self.is_available(d) {
                    return Err(DateTimeValidity::OutOfRange);
                }
                Ok(ParsedValue::DateTime(CivilDateTime::new(d, t)))
            }
            DateTimePickerKind::DateRange => {
                let (a, b) = split_range_text(text).ok_or(DateTimeValidity::Intermediate)?;
                let start = self
                    .date_fmt
                    .parse(a)
                    .ok_or(DateTimeValidity::Intermediate)?;
                let end = self
                    .date_fmt
                    .parse(b)
                    .ok_or(DateTimeValidity::Intermediate)?;
                if !self.is_available(start) || !self.is_available(end) {
                    return Err(DateTimeValidity::OutOfRange);
                }
                Ok(ParsedValue::Range(CivilDateRange::new(start, end)))
            }
        }
    }

    /// Open expanded view.
    pub fn open(&mut self, bounds: Rect) -> DateTimePickerOutcome {
        if !self.enabled {
            return DateTimePickerOutcome::Ignored;
        }
        self.open = true;
        self.presentation = Self::presentation_for_bounds(bounds);
        self.view = if self.kind.has_time() && !self.kind.has_date() {
            DateTimePickerView::TimeList
        } else if self.kind.has_date() {
            Self::preferred_date_view(bounds)
        } else {
            DateTimePickerView::Field
        };
        self.draft.set_focused(false);
        if let Some(d) = self.value_date.or(self.today) {
            self.view_year = d.year;
            self.view_month = d.month;
            self.focus_date = Some(d);
        }
        self.rebuild_collections();
        DateTimePickerOutcome::Opened {
            view: self.view,
            presentation: self.presentation,
        }
    }

    /// Close to field.
    pub fn close(&mut self) -> DateTimePickerOutcome {
        self.open = false;
        self.view = DateTimePickerView::Field;
        self.draft.set_focused(self.focused);
        DateTimePickerOutcome::Closed
    }

    fn rebuild_collections(&mut self) {
        // time list
        let step = self.time_step_minutes.max(1);
        let mut times = Vec::new();
        let mut m = 0u32;
        while m < 24 * 60 {
            if let Some(t) = CivilTime::from_minutes(m) {
                let id = m.to_string();
                times.push(CollectionItem::new(id, self.time_fmt.format(t)));
            }
            m = m.saturating_add(step);
            if step == 0 {
                break;
            }
        }
        let _ = self.time_collection.reconcile(&times);
        if let Some(t) = self.value_time {
            let id = t.minutes_since_midnight().to_string();
            self.time_collection.set_active(Some(id));
        }

        // day list for current month
        let mut days = Vec::new();
        if let Some(dim) = days_in_month(self.view_year, self.view_month) {
            for day in 1..=dim {
                if let Some(d) = CivilDate::new(self.view_year, self.view_month, day) {
                    let label = format!("{:02} {}", day, weekday_short(d));
                    let enabled = self.is_available(d);
                    days.push(CollectionItem::new(d.to_iso(), label).enabled(enabled));
                }
            }
        }
        let _ = self.day_collection.reconcile(&days);
        if let Some(f) = self.focus_date {
            self.day_collection.set_active(Some(f.to_iso()));
        }
    }

    /// Commit draft text if valid.
    pub fn commit_draft(&mut self) -> DateTimePickerOutcome {
        match self.try_parse_draft() {
            Ok(ParsedValue::Date(d)) => {
                self.value_date = Some(d);
                self.focus_date = Some(d);
                self.sync_draft_from_value();
                self.validity = DateTimeValidity::Valid;
                DateTimePickerOutcome::DateChanged { date: d }
            }
            Ok(ParsedValue::Time(t)) => {
                self.value_time = Some(t);
                self.sync_draft_from_value();
                self.validity = DateTimeValidity::Valid;
                DateTimePickerOutcome::TimeChanged { time: t }
            }
            Ok(ParsedValue::DateTime(v)) => {
                self.value_date = Some(v.date);
                self.value_time = Some(v.time);
                self.focus_date = Some(v.date);
                self.sync_draft_from_value();
                self.validity = DateTimeValidity::Valid;
                DateTimePickerOutcome::DateTimeChanged { value: v }
            }
            Ok(ParsedValue::Range(r)) => {
                self.value_date = Some(r.start);
                self.value_end = Some(r.end);
                self.sync_draft_from_value();
                self.validity = DateTimeValidity::Valid;
                DateTimePickerOutcome::RangeChanged { range: r }
            }
            Err(DateTimeValidity::Empty) if self.allow_empty => {
                self.value_date = None;
                self.value_time = None;
                self.value_end = None;
                self.validity = DateTimeValidity::Empty;
                DateTimePickerOutcome::Cleared
            }
            Err(reason) => {
                self.validity = reason;
                DateTimePickerOutcome::ValidationFailed { reason }
            }
        }
    }

    /// Select a calendar day.
    pub fn select_date(&mut self, d: CivilDate) -> DateTimePickerOutcome {
        if !self.is_available(d) {
            return DateTimePickerOutcome::ValidationFailed {
                reason: DateTimeValidity::OutOfRange,
            };
        }
        self.focus_date = Some(d);
        match self.kind {
            DateTimePickerKind::DateRange => {
                if !self.range_picking_end || self.value_date.is_none() {
                    self.value_date = Some(d);
                    self.value_end = None;
                    self.range_picking_end = true;
                    self.sync_draft_from_value();
                    self.validity = DateTimeValidity::Intermediate;
                    DateTimePickerOutcome::Changed
                } else {
                    let start = self.value_date.unwrap_or(d);
                    let range = CivilDateRange::new(start, d);
                    self.value_date = Some(range.start);
                    self.value_end = Some(range.end);
                    self.range_picking_end = false;
                    self.sync_draft_from_value();
                    self.validity = DateTimeValidity::Valid;
                    DateTimePickerOutcome::RangeChanged { range }
                }
            }
            DateTimePickerKind::Date => {
                self.value_date = Some(d);
                self.sync_draft_from_value();
                self.validity = DateTimeValidity::Valid;
                DateTimePickerOutcome::DateChanged { date: d }
            }
            DateTimePickerKind::DateTime => {
                self.value_date = Some(d);
                if self.value_time.is_none() {
                    self.value_time = Some(CivilTime::midnight());
                }
                self.sync_draft_from_value();
                self.validity = DateTimeValidity::Valid;
                if let Some(t) = self.value_time {
                    DateTimePickerOutcome::DateTimeChanged {
                        value: CivilDateTime::new(d, t),
                    }
                } else {
                    DateTimePickerOutcome::DateChanged { date: d }
                }
            }
            DateTimePickerKind::Time => DateTimePickerOutcome::Ignored,
        }
    }

    /// Select time.
    pub fn select_time(&mut self, t: CivilTime) -> DateTimePickerOutcome {
        self.value_time = Some(t);
        self.sync_draft_from_value();
        self.validity = DateTimeValidity::Valid;
        if self.kind == DateTimePickerKind::DateTime {
            if let Some(d) = self.value_date {
                return DateTimePickerOutcome::DateTimeChanged {
                    value: CivilDateTime::new(d, t),
                };
            }
        }
        DateTimePickerOutcome::TimeChanged { time: t }
    }

    /// Key adapter.
    pub fn handle_key(&mut self, key: KeyEvent) -> DateTimePickerOutcome {
        if key.kind == KeyEventKind::Release || !self.enabled {
            return DateTimePickerOutcome::Ignored;
        }
        if !self.focused {
            return DateTimePickerOutcome::Ignored;
        }

        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

        // Alt+Down opens expanded view (neutral KeyCode has no F-keys).
        if key.code == KeyCode::Down && key.modifiers.contains(KeyModifiers::ALT) {
            return self.open(self.root);
        }

        if !self.open {
            return self.handle_field_key(key);
        }

        // Esc closes expanded first
        if key.code == KeyCode::Esc && key.modifiers.is_empty() {
            return self.close();
        }

        // Tab cycles Field → Calendar/DayList → TimeList
        if matches!(key.code, KeyCode::Tab) && !ctrl {
            self.view = match self.view {
                DateTimePickerView::Field => {
                    if self.kind.has_date() {
                        if self.root.width > 0 && self.root.width < DATE_TIME_PICKER_LIST_MAX_WIDTH
                        {
                            DateTimePickerView::DayList
                        } else {
                            DateTimePickerView::Calendar
                        }
                    } else {
                        DateTimePickerView::TimeList
                    }
                }
                DateTimePickerView::Calendar | DateTimePickerView::DayList => {
                    if self.kind.has_time() {
                        DateTimePickerView::TimeList
                    } else {
                        DateTimePickerView::Field
                    }
                }
                DateTimePickerView::TimeList => DateTimePickerView::Field,
            };
            self.draft
                .set_focused(matches!(self.view, DateTimePickerView::Field));
            self.rebuild_collections();
            return DateTimePickerOutcome::Changed;
        }

        match self.view {
            DateTimePickerView::Field => self.handle_field_key(key),
            DateTimePickerView::Calendar => self.handle_calendar_key(key),
            DateTimePickerView::DayList => self.handle_day_list_key(key),
            DateTimePickerView::TimeList => self.handle_time_list_key(key),
        }
    }

    fn handle_field_key(&mut self, key: KeyEvent) -> DateTimePickerOutcome {
        // Space / Down open when empty binding not typing? Down alone opens when not editing mid-text
        if matches!(key.code, KeyCode::Down | KeyCode::Char(' '))
            && key.modifiers.is_empty()
            && self.draft.value().is_empty()
        {
            return self.open(self.root);
        }

        if key.code == KeyCode::Enter && key.modifiers.is_empty() {
            let out = self.commit_draft();
            if matches!(
                out,
                DateTimePickerOutcome::DateChanged { .. }
                    | DateTimePickerOutcome::TimeChanged { .. }
                    | DateTimePickerOutcome::DateTimeChanged { .. }
                    | DateTimePickerOutcome::RangeChanged { .. }
                    | DateTimePickerOutcome::Cleared
            ) && self.open
            {
                let _ = self.close();
            }
            return out;
        }

        if key.code == KeyCode::Esc && key.modifiers.is_empty() {
            if self.open {
                return self.close();
            }
            return DateTimePickerOutcome::Cancelled;
        }

        match self.draft.handle_key(key) {
            TextInputOutcome::Changed | TextInputOutcome::Cleared => {
                self.refresh_validity();
                DateTimePickerOutcome::Changed
            }
            TextInputOutcome::Submitted(_) => self.commit_draft(),
            TextInputOutcome::Cancelled => {
                if self.open {
                    self.close()
                } else {
                    DateTimePickerOutcome::Cancelled
                }
            }
            _ => DateTimePickerOutcome::Ignored,
        }
    }

    fn handle_calendar_key(&mut self, key: KeyEvent) -> DateTimePickerOutcome {
        let focus = self
            .focus_date
            .or(self.value_date)
            .or(self.today)
            .unwrap_or_else(|| CivilDate::new(2026, 1, 1).unwrap());

        match key.code {
            KeyCode::Left => {
                let d = focus.add_days(-1);
                self.focus_date = Some(d);
                self.ensure_view_shows(d);
                DateTimePickerOutcome::Changed
            }
            KeyCode::Right => {
                let d = focus.add_days(1);
                self.focus_date = Some(d);
                self.ensure_view_shows(d);
                DateTimePickerOutcome::Changed
            }
            KeyCode::Up => {
                let d = focus.add_days(-7);
                self.focus_date = Some(d);
                self.ensure_view_shows(d);
                DateTimePickerOutcome::Changed
            }
            KeyCode::Down => {
                let d = focus.add_days(7);
                self.focus_date = Some(d);
                self.ensure_view_shows(d);
                DateTimePickerOutcome::Changed
            }
            KeyCode::PageUp => {
                self.view_month_delta(-1);
                if let Some(f) = self.focus_date {
                    let d = f.add_months(-1);
                    self.focus_date = Some(d);
                }
                self.rebuild_collections();
                DateTimePickerOutcome::Changed
            }
            KeyCode::PageDown => {
                self.view_month_delta(1);
                if let Some(f) = self.focus_date {
                    let d = f.add_months(1);
                    self.focus_date = Some(d);
                }
                self.rebuild_collections();
                DateTimePickerOutcome::Changed
            }
            KeyCode::Home => {
                if let Some(t) = self.today {
                    self.focus_date = Some(t);
                    self.ensure_view_shows(t);
                }
                DateTimePickerOutcome::Changed
            }
            KeyCode::Enter | KeyCode::Char(' ') => self.select_date(focus),
            KeyCode::Char(c) if c.is_ascii_digit() => {
                // typeahead: jump to day number in month (1–9, then 10–31).
                let digit = c.to_digit(10).unwrap_or(0) as u8;
                if let Some(f) = self.focus_date {
                    if f.year == self.view_year && f.month == self.view_month && f.day <= 3 {
                        let day = f.day.saturating_mul(10).saturating_add(digit);
                        if let Some(d) = CivilDate::new(self.view_year, self.view_month, day) {
                            self.focus_date = Some(d);
                            return DateTimePickerOutcome::Changed;
                        }
                    }
                }
                if digit >= 1 {
                    if let Some(d) = CivilDate::new(self.view_year, self.view_month, digit) {
                        self.focus_date = Some(d);
                    }
                }
                DateTimePickerOutcome::Changed
            }
            _ => DateTimePickerOutcome::Ignored,
        }
    }

    fn handle_day_list_key(&mut self, key: KeyEvent) -> DateTimePickerOutcome {
        if matches!(key.code, KeyCode::PageUp) {
            self.view_month_delta(-1);
            self.rebuild_collections();
            return DateTimePickerOutcome::Changed;
        }
        if matches!(key.code, KeyCode::PageDown) {
            self.view_month_delta(1);
            self.rebuild_collections();
            return DateTimePickerOutcome::Changed;
        }
        let items = self.day_list_items();
        if key.code == KeyCode::Enter && key.modifiers.is_empty() {
            if let Some(id) = self.day_collection.active() {
                if let Some(d) = CivilDate::parse_iso(id) {
                    return self.select_date(d);
                }
            }
        }
        match self.day_collection.handle_key(key, &items) {
            CollectionOutcome::ActiveChanged { to, .. } => {
                if let Some(id) = to {
                    self.focus_date = CivilDate::parse_iso(&id);
                }
                DateTimePickerOutcome::Changed
            }
            CollectionOutcome::Scrolled => DateTimePickerOutcome::Changed,
            CollectionOutcome::Ignored => DateTimePickerOutcome::Ignored,
        }
    }

    fn handle_time_list_key(&mut self, key: KeyEvent) -> DateTimePickerOutcome {
        let items = self.time_list_items();
        if key.code == KeyCode::Enter && key.modifiers.is_empty() {
            if let Some(id) = self.time_collection.active() {
                if let Ok(mins) = id.parse::<u32>() {
                    if let Some(t) = CivilTime::from_minutes(mins) {
                        return self.select_time(t);
                    }
                }
            }
        }
        // digit typeahead: accumulate minutes roughly via hour jump
        if let KeyCode::Char(c) = key.code {
            if c.is_ascii_digit() {
                let d = c.to_digit(10).unwrap_or(0);
                let hour = d.min(9);
                let mins = hour * 60;
                if let Some(t) = CivilTime::from_minutes(mins) {
                    let id = mins.to_string();
                    self.time_collection.set_active(Some(id));
                    let _ = t;
                    return DateTimePickerOutcome::Changed;
                }
            }
        }
        match self.time_collection.handle_key(key, &items) {
            CollectionOutcome::ActiveChanged { .. } | CollectionOutcome::Scrolled => {
                DateTimePickerOutcome::Changed
            }
            CollectionOutcome::Ignored => DateTimePickerOutcome::Ignored,
        }
    }

    fn day_list_items(&self) -> Vec<CollectionItem<String>> {
        let mut days = Vec::new();
        if let Some(dim) = days_in_month(self.view_year, self.view_month) {
            for day in 1..=dim {
                if let Some(d) = CivilDate::new(self.view_year, self.view_month, day) {
                    days.push(
                        CollectionItem::new(d.to_iso(), format!("{day:02}"))
                            .enabled(self.is_available(d)),
                    );
                }
            }
        }
        days
    }

    fn time_list_items(&self) -> Vec<CollectionItem<String>> {
        let step = self.time_step_minutes.max(1);
        let mut times = Vec::new();
        let mut m = 0u32;
        while m < 24 * 60 {
            if let Some(t) = CivilTime::from_minutes(m) {
                times.push(CollectionItem::new(m.to_string(), self.time_fmt.format(t)));
            }
            m = m.saturating_add(step);
        }
        times
    }

    fn ensure_view_shows(&mut self, d: CivilDate) {
        self.view_year = d.year;
        self.view_month = d.month;
        self.rebuild_collections();
    }

    fn view_month_delta(&mut self, delta: i32) {
        let d = CivilDate::new(self.view_year, self.view_month, 1)
            .unwrap_or_else(|| CivilDate::new(2026, 1, 1).unwrap())
            .add_months(delta);
        self.view_year = d.year;
        self.view_month = d.month;
    }

    /// Intent path.
    pub fn handle_intent(&mut self, intent: UiIntent) -> DateTimePickerOutcome {
        if !self.enabled || !self.focused {
            return DateTimePickerOutcome::Ignored;
        }
        match intent {
            UiIntent::Cancel | UiIntent::Close => {
                if self.open {
                    self.close()
                } else {
                    DateTimePickerOutcome::Cancelled
                }
            }
            UiIntent::Submit | UiIntent::Activate => {
                if self.open && matches!(self.view, DateTimePickerView::Calendar) {
                    if let Some(d) = self.focus_date {
                        return self.select_date(d);
                    }
                }
                self.commit_draft()
            }
            UiIntent::Fullscreen => {
                self.presentation = DateTimePickerPresentation::Fullscreen;
                DateTimePickerOutcome::PresentationChanged {
                    presentation: DateTimePickerPresentation::Fullscreen,
                }
            }
            _ => DateTimePickerOutcome::Ignored,
        }
    }

    /// Mouse.
    pub fn handle_mouse(&mut self, event: MouseEvent) -> DateTimePickerOutcome {
        if !self.enabled {
            return DateTimePickerOutcome::Ignored;
        }
        if !matches!(event.kind, MouseEventKind::Down(MouseButton::Left)) {
            return DateTimePickerOutcome::Ignored;
        }
        self.focused = true;

        if self.field_area.contains(event.position) {
            self.view = DateTimePickerView::Field;
            self.draft.set_focused(true);
            if !self.open {
                return self.open(self.root);
            }
            return DateTimePickerOutcome::Changed;
        }

        for (d, rect) in &self.cell_hits {
            if rect.contains(event.position) {
                let d = *d;
                return self.select_date(d);
            }
        }
        for (t, rect) in &self.time_hits {
            if rect.contains(event.position) {
                let t = *t;
                return self.select_time(t);
            }
        }
        DateTimePickerOutcome::Ignored
    }

    /// Overlay open helper.
    pub fn open_overlay<FocusId: Clone>(
        stack: &mut OverlayStack<FocusId>,
        bounds: Rect,
        size: OverlaySize,
        opener: Option<FocusId>,
    ) -> OverlayOutcome<FocusId> {
        stack.open(
            bounds,
            OverlaySpec::dialog(DATE_TIME_PICKER_OVERLAY_ID, size, opener),
        )
    }

    /// Dismiss overlay.
    pub fn dismiss_overlay<FocusId: Clone>(
        stack: &mut OverlayStack<FocusId>,
    ) -> OverlayOutcome<FocusId> {
        stack.dismiss(&OverlayId::from_static(DATE_TIME_PICKER_OVERLAY_ID))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParsedValue {
    Date(CivilDate),
    Time(CivilTime),
    DateTime(CivilDateTime),
    Range(CivilDateRange),
}

fn looks_partial_date(s: &str) -> bool {
    s.len() < 10
        && s.chars()
            .all(|c| c.is_ascii_digit() || c == '-' || c == '/')
}

fn looks_partial_time(s: &str) -> bool {
    s.len() < 5 && s.chars().all(|c| c.is_ascii_digit() || c == ':')
}

fn split_datetime_text(s: &str) -> Option<(&str, &str)> {
    if let Some((d, t)) = s.split_once('T') {
        return Some((d.trim(), t.trim()));
    }
    if let Some((d, t)) = s.split_once(' ') {
        return Some((d.trim(), t.trim()));
    }
    Some((s, ""))
}

fn split_range_text(s: &str) -> Option<(&str, &str)> {
    for sep in ["..", " - ", "—", "–", " to "] {
        if let Some((a, b)) = s.split_once(sep) {
            return Some((a.trim(), b.trim()));
        }
    }
    None
}

fn weekday_short(d: CivilDate) -> &'static str {
    ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"][d.weekday_iso() as usize]
}

fn month_name(m: u8) -> &'static str {
    match m {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "????",
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// DateTimePicker chrome.
#[derive(Debug, Clone, Copy)]
pub struct DateTimePicker<'a> {
    system: &'a DesignSystem,
    label: &'a str,
    ascii: bool,
    show_timezone: bool,
}

impl<'a> DateTimePicker<'a> {
    /// Create.
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self {
            system,
            label: "",
            ascii: false,
            show_timezone: true,
        }
    }

    /// Label above field.
    #[must_use]
    pub const fn label(mut self, label: &'a str) -> Self {
        self.label = label;
        self
    }

    /// ASCII glyphs for states.
    #[must_use]
    pub const fn ascii(mut self, on: bool) -> Self {
        self.ascii = on;
        self
    }

    /// Show timezone label when present.
    #[must_use]
    pub const fn show_timezone(mut self, on: bool) -> Self {
        self.show_timezone = on;
        self
    }

    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut DateTimePickerState) {
        state.cell_hits.clear();
        state.time_hits.clear();
        state.root = area;
        if area.is_empty() {
            return;
        }

        if area.width < DATE_TIME_PICKER_LIST_MAX_WIDTH
            || area.height < DATE_TIME_PICKER_FULLSCREEN_MAX_HEIGHT
        {
            state.presentation = DateTimePickerPresentation::Fullscreen;
        }

        let mut y = area.y;
        if !self.label.is_empty() && area.height >= 1 {
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(self.label, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
            y = y.saturating_add(1);
        }

        // Field row
        if y < area.bottom() {
            let field_h = 1u16;
            let field = Rect::new(area.x, y, area.width, field_h);
            state.field_area = field;
            let validation = match state.validity {
                DateTimeValidity::Invalid | DateTimeValidity::OutOfRange => {
                    Validation::Invalid(state.validity.id())
                }
                _ => Validation::Valid,
            };
            let ph = match state.kind {
                DateTimePickerKind::Date => "YYYY-MM-DD",
                DateTimePickerKind::Time => "HH:MM",
                DateTimePickerKind::DateTime => "YYYY-MM-DD HH:MM",
                DateTimePickerKind::DateRange => "YYYY-MM-DD..YYYY-MM-DD",
            };
            let _ = TextInput::new("", self.system)
                .placeholder(ph)
                .validation(validation)
                .paint(field, buffer, &mut state.draft);
            // open marker
            if area.width > 4 {
                let mark = if state.open {
                    if self.ascii { "^" } else { "▴" }
                } else if self.ascii {
                    "v"
                } else {
                    "▾"
                };
                buffer.set_stringn(
                    area.right().saturating_sub(2),
                    y,
                    mark,
                    1,
                    self.system.style(Role::TextMuted),
                );
            }
            y = y.saturating_add(1);
        }

        // TZ + validity caption
        if y < area.bottom()
            && (self.show_timezone && state.timezone_label.is_some()
                || !matches!(
                    state.validity,
                    DateTimeValidity::Valid | DateTimeValidity::Empty
                ))
        {
            let mut parts = Vec::new();
            if self.show_timezone {
                if let Some(tz) = &state.timezone_label {
                    parts.push(format!("tz:{tz}"));
                }
            }
            if !matches!(
                state.validity,
                DateTimeValidity::Valid | DateTimeValidity::Empty
            ) {
                parts.push(state.validity.id().to_owned());
            }
            let cap = parts.join(" · ");
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&cap, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(
                    if matches!(
                        state.validity,
                        DateTimeValidity::Invalid | DateTimeValidity::OutOfRange
                    ) {
                        Role::Danger
                    } else {
                        Role::TextMuted
                    },
                ),
            );
            y = y.saturating_add(1);
        }

        if !state.open {
            return;
        }

        let body = Rect::new(area.x, y, area.width, area.bottom().saturating_sub(y));
        if body.is_empty() {
            return;
        }

        // Auto switch calendar → day list on narrow
        if matches!(state.view, DateTimePickerView::Calendar)
            && body.width < DATE_TIME_PICKER_LIST_MAX_WIDTH
        {
            state.view = DateTimePickerView::DayList;
            state.rebuild_collections();
        }

        let panel = Panel::new(self.system).emphasis(if state.focused {
            PanelChrome::Focused
        } else {
            PanelChrome::Normal
        });
        let inner = panel.inner(body);
        Widget::render(&panel, body, buffer);
        if inner.is_empty() {
            return;
        }
        state.grid_area = inner;

        match state.view {
            DateTimePickerView::Calendar => self.paint_calendar(inner, buffer, state),
            DateTimePickerView::DayList => self.paint_day_list(inner, buffer, state),
            DateTimePickerView::TimeList => self.paint_time_list(inner, buffer, state),
            DateTimePickerView::Field => {}
        }
    }

    fn paint_calendar(&self, area: Rect, buffer: &mut Buffer, state: &mut DateTimePickerState) {
        if area.height < 3 || area.width < 20 {
            self.paint_day_list(area, buffer, state);
            return;
        }
        let title = format!(
            "{} {:04}  [< > Pg]",
            month_name(state.view_month),
            state.view_year
        );
        buffer.set_stringn(
            area.x,
            area.y,
            take_display_cols(&title, usize::from(area.width)),
            usize::from(area.width),
            self.system
                .style(Role::TextStrong)
                .add_modifier(Modifier::BOLD),
        );

        let headers = state.week_start.headers();
        let col_w = 4u16;
        let header_y = area.y.saturating_add(1);
        for (i, h) in headers.iter().enumerate() {
            let x = area.x.saturating_add((i as u16).saturating_mul(col_w));
            if x >= area.right() {
                break;
            }
            buffer.set_stringn(x, header_y, h, 2, self.system.style(Role::TextMuted));
        }

        // Build grid: find first cell date
        let first = CivilDate::new(state.view_year, state.view_month, 1)
            .unwrap_or_else(|| CivilDate::new(2026, 1, 1).unwrap());
        let start_col = state.week_start.column(first.weekday_iso());
        let mut day_ordinal = first.add_days(-(start_col as i32));

        let grid_top = area.y.saturating_add(2);
        let rows = area.bottom().saturating_sub(grid_top).min(6);
        for row in 0..rows {
            for col in 0..7u16 {
                let d = day_ordinal;
                day_ordinal = day_ordinal.add_days(1);
                let x = area.x.saturating_add(col.saturating_mul(col_w));
                let y = grid_top.saturating_add(row);
                if x >= area.right() || y >= area.bottom() {
                    continue;
                }
                let in_month = d.month == state.view_month && d.year == state.view_year;
                let available = state.is_available(d);
                let is_focus = state.focus_date == Some(d);
                let is_today = state.today == Some(d);
                let is_selected = match state.kind {
                    DateTimePickerKind::DateRange => {
                        if let (Some(a), Some(b)) = (state.value_date, state.value_end) {
                            CivilDateRange::new(a, b).contains(d)
                        } else {
                            state.value_date == Some(d)
                        }
                    }
                    _ => state.value_date == Some(d),
                };

                // Non-color distinct marks:
                // selected: [dd], today: *dd*, focus: >dd<, unavailable: .dd., other month:  dd
                let num = format!("{:2}", d.day);
                let cell = if !available {
                    format!(".{}.", num.trim())
                } else if is_focus && is_selected {
                    format!("[{}]", num.trim())
                } else if is_focus {
                    format!(">{}<", num.trim())
                } else if is_selected {
                    format!("[{}]", num.trim())
                } else if is_today {
                    format!("*{}*", num.trim())
                } else if in_month {
                    format!(" {} ", num.trim())
                } else {
                    format!("·{}·", num.trim())
                };

                let style = if !available {
                    self.system.style(Role::TextMuted)
                } else if is_focus {
                    self.system
                        .style(Role::Focus)
                        .add_modifier(Modifier::REVERSED)
                } else if is_selected {
                    self.system
                        .style(Role::TextStrong)
                        .add_modifier(Modifier::BOLD)
                } else if is_today {
                    self.system
                        .style(Role::Text)
                        .add_modifier(Modifier::UNDERLINED)
                } else if in_month {
                    self.system.style(Role::Text)
                } else {
                    self.system.style(Role::TextMuted)
                };

                let rect = Rect::new(x, y, col_w.min(area.right().saturating_sub(x)), 1);
                buffer.set_stringn(
                    rect.x,
                    rect.y,
                    take_display_cols(&cell, usize::from(rect.width)),
                    usize::from(rect.width),
                    style,
                );
                if available && in_month {
                    state.cell_hits.push((d, rect));
                }
            }
        }

        // legend
        if area.height >= 9 {
            let legend = if self.ascii {
                "[sel] *today* >focus< .unavail."
            } else {
                "[sel] *today* >focus< .unavail."
            };
            buffer.set_stringn(
                area.x,
                area.bottom().saturating_sub(1),
                take_display_cols(legend, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
        }
    }

    fn paint_day_list(&self, area: Rect, buffer: &mut Buffer, state: &mut DateTimePickerState) {
        let title = format!("{} {:04}", month_name(state.view_month), state.view_year);
        buffer.set_stringn(
            area.x,
            area.y,
            take_display_cols(&title, usize::from(area.width)),
            usize::from(area.width),
            self.system.style(Role::TextStrong),
        );
        let list = Rect::new(
            area.x,
            area.y.saturating_add(1),
            area.width,
            area.height.saturating_sub(1),
        );
        let items = state.day_list_items();
        let vp = usize::from(list.height).max(1);
        state
            .day_collection
            .set_viewport(state.day_collection.offset(), vp, items.len());
        let _ = state.day_collection.reconcile(&items);
        let offset = state.day_collection.offset();
        for (row, item) in items.iter().skip(offset).enumerate() {
            if row >= vp {
                break;
            }
            let y = list.y.saturating_add(row as u16);
            let d = CivilDate::parse_iso(item.id.as_str());
            let is_hi = state.day_collection.active() == Some(&item.id);
            let is_sel = d.is_some_and(|d| state.value_date == Some(d));
            let is_today = d.is_some_and(|d| state.today == Some(d));
            let mark = if is_sel {
                "*"
            } else if is_today {
                "."
            } else {
                " "
            };
            let line = format!("{mark}{}", item.label);
            let style = if is_hi {
                self.system
                    .style(Role::Focus)
                    .add_modifier(Modifier::REVERSED)
            } else if is_sel {
                self.system.style(Role::TextStrong)
            } else {
                self.system.style(Role::Text)
            };
            let rect = Rect::new(list.x, y, list.width, 1);
            buffer.set_stringn(
                rect.x,
                rect.y,
                take_display_cols(&line, usize::from(rect.width)),
                usize::from(rect.width),
                style,
            );
            if let Some(d) = d {
                state.cell_hits.push((d, rect));
            }
        }
    }

    fn paint_time_list(&self, area: Rect, buffer: &mut Buffer, state: &mut DateTimePickerState) {
        let title = match &state.timezone_label {
            Some(tz) if self.show_timezone => format!("Time ({tz})"),
            _ => "Time".into(),
        };
        buffer.set_stringn(
            area.x,
            area.y,
            take_display_cols(&title, usize::from(area.width)),
            usize::from(area.width),
            self.system.style(Role::TextStrong),
        );
        let list = Rect::new(
            area.x,
            area.y.saturating_add(1),
            area.width,
            area.height.saturating_sub(1),
        );
        let items = state.time_list_items();
        let vp = usize::from(list.height).max(1);
        state
            .time_collection
            .set_viewport(state.time_collection.offset(), vp, items.len());
        let _ = state.time_collection.reconcile(&items);
        let offset = state.time_collection.offset();
        for (row, item) in items.iter().skip(offset).enumerate() {
            if row >= vp {
                break;
            }
            let y = list.y.saturating_add(row as u16);
            let is_hi = state.time_collection.active() == Some(&item.id);
            let mins: u32 = item.id.parse().unwrap_or(0);
            let t = CivilTime::from_minutes(mins);
            let is_sel = t.is_some_and(|t| state.value_time == Some(t));
            let mark = if is_sel { "*" } else { " " };
            let line = format!("{mark}{}", item.label);
            let style = if is_hi {
                self.system
                    .style(Role::Focus)
                    .add_modifier(Modifier::REVERSED)
            } else if is_sel {
                self.system.style(Role::TextStrong)
            } else {
                self.system.style(Role::Text)
            };
            let rect = Rect::new(list.x, y, list.width, 1);
            buffer.set_stringn(
                rect.x,
                rect.y,
                take_display_cols(&line, usize::from(rect.width)),
                usize::from(rect.width),
                style,
            );
            if let Some(t) = t {
                state.time_hits.push((t, rect));
            }
        }
    }

    /// Semantic registration.
    pub fn register_semantic<Id, Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        id: Id,
        area: Rect,
        state: &DateTimePickerState,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if area.is_empty() {
            return;
        }
        let desc = format!(
            "date-time-picker {} view={} validity={}",
            state.kind.id(),
            state.view.id(),
            state.validity.id()
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Input)
                .label(if self.label.is_empty() {
                    "date-time"
                } else {
                    self.label
                })
                .description(desc)
                .focusable(state.enabled)
                .disabled(!state.enabled)
                .state(SemanticState {
                    selected: state.focused,
                    busy: false,
                    invalid: matches!(
                        state.validity,
                        DateTimeValidity::Invalid | DateTimeValidity::OutOfRange
                    ),
                    expanded: state.open,
                    ..Default::default()
                }),
        );
    }
}

impl StatefulWidget for &DateTimePicker<'_> {
    type State = DateTimePickerState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl StatefulWidget for DateTimePicker<'_> {
    type State = DateTimePickerState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

// ── Guidance: when TextInput is better ──────────────────────────────────────

/// When to prefer plain [`TextInput`] over [`DateTimePicker`].
///
/// Use **TextInput** when:
/// - Values are rare free-form ISO stamps (logs, machine paste).
/// - No calendar browse / range / “today” affordance is needed.
/// - Host already normalizes strings server-side.
///
/// Use **DateTimePicker** when:
/// - Users navigate months or pick from stepped times.
/// - Min/max and unavailable days must be visible.
/// - Range selection or timezone label is part of the form UX.
pub mod guidance {
    /// Short doc string for handbooks / Studio.
    pub const WHEN_TEXT_INPUT: &str = "Prefer TextInput for rare ISO paste and machine-oriented stamps; \
         DateTimePicker when browse, range, or stepped time helps.";
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::RolePalette;

    #[test]
    fn civil_date_roundtrip_iso() {
        let d = CivilDate::new(2026, 8, 10).unwrap();
        assert_eq!(d.to_iso(), "2026-08-10");
        assert_eq!(CivilDate::parse_iso("2026-08-10"), Some(d));
        assert_eq!(d.weekday_iso(), 0); // Monday
    }

    #[test]
    fn civil_time_and_datetime() {
        let t = CivilTime::new(14, 30, 0).unwrap();
        assert_eq!(t.to_iso(false), "14:30");
        let dt = CivilDateTime::new(CivilDate::new(2026, 1, 2).unwrap(), t);
        assert!(dt.to_iso(false).starts_with("2026-01-02T14:30"));
        assert_eq!(CivilDateTime::parse_iso("2026-01-02T14:30"), Some(dt));
    }

    #[test]
    fn leap_and_add_months() {
        let d = CivilDate::new(2024, 1, 31).unwrap();
        let f = d.add_months(1);
        assert_eq!(f, CivilDate::new(2024, 2, 29).unwrap());
        assert!(CivilDate::new(2023, 2, 29).is_none());
    }

    #[test]
    fn display_formats() {
        let d = CivilDate::new(2026, 8, 10).unwrap();
        assert_eq!(DateDisplayFormat::MdySlash.format(d), "08/10/2026");
        assert_eq!(DateDisplayFormat::MdySlash.parse("08/10/2026"), Some(d));
        let t = CivilTime::new(0, 5, 0).unwrap();
        assert_eq!(TimeDisplayFormat::Hm12.format(t), "12:05 AM");
        assert_eq!(TimeDisplayFormat::Hm12.parse("12:05 AM"), Some(t));
    }

    #[test]
    fn commit_date_and_min_max() {
        let mut state = DateTimePickerState::new(DateTimePickerKind::Date)
            .with_min_date(CivilDate::new(2026, 8, 1).unwrap())
            .with_max_date(CivilDate::new(2026, 8, 31).unwrap());
        state.set_focused(true);
        state.set_today(CivilDate::new(2026, 8, 10).unwrap());
        state.set_draft_text("2026-08-15");
        assert!(matches!(
            state.commit_draft(),
            DateTimePickerOutcome::DateChanged {
                date
            } if date.day == 15
        ));
        state.set_draft_text("2026-09-01");
        assert!(matches!(
            state.commit_draft(),
            DateTimePickerOutcome::ValidationFailed {
                reason: DateTimeValidity::OutOfRange
            }
        ));
    }

    #[test]
    fn calendar_select_and_range() {
        let mut state = DateTimePickerState::new(DateTimePickerKind::DateRange);
        state.set_focused(true);
        state.set_today(CivilDate::new(2026, 8, 10).unwrap());
        let a = CivilDate::new(2026, 8, 5).unwrap();
        let b = CivilDate::new(2026, 8, 12).unwrap();
        assert!(matches!(
            state.select_date(a),
            DateTimePickerOutcome::Changed
        ));
        assert!(matches!(
            state.select_date(b),
            DateTimePickerOutcome::RangeChanged { range }
                if range.start == a && range.end == b
        ));
    }

    #[test]
    fn open_calendar_nav_keys() {
        let mut state = DateTimePickerState::new(DateTimePickerKind::Date)
            .with_date(CivilDate::new(2026, 8, 10).unwrap());
        state.set_focused(true);
        state.set_today(CivilDate::new(2026, 8, 10).unwrap());
        assert!(matches!(
            state.open(Rect::new(0, 0, 40, 16)),
            DateTimePickerOutcome::Opened {
                view: DateTimePickerView::Calendar,
                ..
            }
        ));
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
            DateTimePickerOutcome::Changed
        ));
        assert_eq!(state.focus_date().map(|d| d.day), Some(11));
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            DateTimePickerOutcome::DateChanged { date } if date.day == 11
        ));
    }

    #[test]
    fn tiny_terminal_day_list() {
        let mut state = DateTimePickerState::new(DateTimePickerKind::Date);
        state.set_focused(true);
        state.set_today(CivilDate::new(2026, 8, 10).unwrap());
        let tiny = Rect::new(0, 0, 28, 10);
        assert!(matches!(
            state.open(tiny),
            DateTimePickerOutcome::Opened {
                view: DateTimePickerView::DayList,
                presentation: DateTimePickerPresentation::Fullscreen,
            }
        ));
    }

    #[test]
    fn time_list_select() {
        let mut state =
            DateTimePickerState::new(DateTimePickerKind::Time).with_time_step_minutes(30);
        state.set_focused(true);
        let _ = state.open(Rect::new(0, 0, 40, 16));
        assert_eq!(state.view(), DateTimePickerView::TimeList);
        let t = CivilTime::new(9, 30, 0).unwrap();
        assert!(matches!(
            state.select_time(t),
            DateTimePickerOutcome::TimeChanged { time } if time == t
        ));
    }

    #[test]
    fn unavailable_non_color_paint() {
        let system = DesignSystem::from_palette(RolePalette::default());
        let mut state = DateTimePickerState::new(DateTimePickerKind::Date)
            .with_min_date(CivilDate::new(2026, 8, 10).unwrap())
            .with_max_date(CivilDate::new(2026, 8, 20).unwrap())
            .with_date(CivilDate::new(2026, 8, 15).unwrap())
            .with_timezone_label("UTC");
        state.set_focused(true);
        state.set_today(CivilDate::new(2026, 8, 10).unwrap());
        let _ = state.open(Rect::new(0, 0, 48, 18));
        let area = Rect::new(0, 0, 48, 18);
        let mut buf = Buffer::empty(area);
        DateTimePicker::new(&system)
            .label("Due")
            .ascii(true)
            .paint(area, &mut buf, &mut state);
        assert!(!state.cell_hits.is_empty());
        // selected day hit present
        assert!(state.cell_hits.iter().any(|(d, _)| d.day == 15));
    }

    #[test]
    fn esc_closes_then_cancels() {
        let mut state = DateTimePickerState::new(DateTimePickerKind::Date);
        state.set_focused(true);
        let _ = state.open(Rect::new(0, 0, 40, 16));
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            DateTimePickerOutcome::Closed
        ));
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            DateTimePickerOutcome::Cancelled
        ));
    }

    #[test]
    fn fuzz_keys() {
        let mut state =
            DateTimePickerState::new(DateTimePickerKind::DateTime).with_timezone_label("UTC");
        state.set_focused(true);
        state.set_today(CivilDate::new(2026, 8, 10).unwrap());
        let _ = state.open(Rect::new(0, 0, 50, 18));
        let keys = [
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Home, KeyModifiers::NONE),
        ];
        for key in keys.iter().cycle().take(40) {
            let _ = state.handle_key(*key);
        }
    }

    #[test]
    fn paint_hot_path() {
        let system = DesignSystem::default();
        let mut state = DateTimePickerState::new(DateTimePickerKind::Date)
            .with_date(CivilDate::new(2026, 8, 10).unwrap());
        state.set_focused(true);
        state.set_today(CivilDate::new(2026, 8, 10).unwrap());
        let _ = state.open(Rect::new(0, 0, 40, 16));
        let area = Rect::new(0, 0, 40, 16);
        let mut buf = Buffer::empty(area);
        let w = DateTimePicker::new(&system).ascii(true);
        for _ in 0..50 {
            w.paint(area, &mut buf, &mut state);
        }
    }

    #[test]
    fn semantic() {
        let system = DesignSystem::default();
        let state = DateTimePickerState::new(DateTimePickerKind::Date);
        let mut scene = SemanticScene::<&str, ()>::default();
        DateTimePicker::new(&system).register_semantic(
            &mut scene,
            "dt",
            Rect::new(0, 0, 30, 10),
            &state,
        );
        assert!(scene.get(&"dt").is_some());
    }

    #[test]
    fn overlay_helpers() {
        let mut stack = OverlayStack::<&str>::default();
        let bounds = Rect::new(0, 0, 80, 24);
        let _ = DateTimePickerState::open_overlay(
            &mut stack,
            bounds,
            OverlaySize::dialog(40, 16),
            Some("opener"),
        );
        let _ = DateTimePickerState::dismiss_overlay(&mut stack);
    }

    #[test]
    fn guidance_constant() {
        assert!(guidance::WHEN_TEXT_INPUT.contains("TextInput"));
    }

    #[test]
    fn mouse_select_day() {
        let system = DesignSystem::default();
        let mut state = DateTimePickerState::new(DateTimePickerKind::Date);
        state.set_focused(true);
        state.set_today(CivilDate::new(2026, 8, 10).unwrap());
        let area = Rect::new(0, 0, 48, 18);
        let _ = state.open(area);
        let mut buf = Buffer::empty(area);
        DateTimePicker::new(&system)
            .ascii(true)
            .paint(area, &mut buf, &mut state);
        let (d, rect) = state
            .cell_hits
            .iter()
            .find(|(d, _)| d.day == 12)
            .cloned()
            .expect("day 12 cell");
        assert!(matches!(
            state.handle_mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: Position::new(rect.x, rect.y),
                modifiers: KeyModifiers::NONE,
            }),
            DateTimePickerOutcome::DateChanged { date } if date == d
        ));
    }
}
