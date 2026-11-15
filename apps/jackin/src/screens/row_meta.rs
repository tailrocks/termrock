//! Row text that changes with the clock without changing the row.
//!
//! A cached row holds its structure. The part of its meta text that depends
//! on the time (a spinner frame, an age, a running duration) is stored as a
//! [`Moment`] and turned into text at draw, so the clock never invalidates a
//! projection.

use junie_tui::ui::text::{DurationStyle, duration};
use junie_tui::widgets::progress::spinner_frame;

use crate::world::World;

pub const SPINNER_MS: u64 = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moment {
    /// `"{frame} {what}"`, a new frame every [`SPINNER_MS`].
    Spinner(&'static str),
    /// Spoken time since the epoch second, such as `3 minutes`.
    Running(i64),
    /// `"3 minutes ago"`.
    Ago(i64),
    /// `"3 minutes"`, the age without its `ago`.
    AgeBare(i64),
    /// The age without its `ago`, or `fallback` when no time is known.
    AgeOr(Option<i64>, &'static str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowMeta {
    before: String,
    moment: Option<Moment>,
    after: String,
}

impl RowMeta {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            before: text.into(),
            moment: None,
            after: String::new(),
        }
    }

    pub fn timed(before: impl Into<String>, moment: Moment, after: impl Into<String>) -> Self {
        Self {
            before: before.into(),
            moment: Some(moment),
            after: after.into(),
        }
    }

    pub fn spinner(before: impl Into<String>, what: &'static str) -> Self {
        Self::timed(before, Moment::Spinner(what), "")
    }

    pub fn moment(&self) -> Option<Moment> {
        self.moment
    }

    pub fn text(&self, w: &World) -> String {
        let Some(moment) = self.moment else {
            return self.before.clone();
        };
        let mid = match moment {
            Moment::Spinner(what) => format!("{} {what}", spinner_frame(frame(w))),
            Moment::Running(since) => duration(
                std::time::Duration::from_secs((w.now_secs() - since).max(0) as u64),
                DurationStyle::Spoken,
            ),
            Moment::Ago(secs) => w.clock.ago(secs),
            Moment::AgeBare(secs) => w.clock.ago(secs).replace(" ago", ""),
            Moment::AgeOr(Some(secs), _) => w.clock.ago(secs).replace(" ago", ""),
            Moment::AgeOr(None, fallback) => fallback.to_owned(),
        };
        format!("{}{mid}{}", self.before, self.after)
    }
}

fn frame(w: &World) -> u64 {
    w.clock.now_ms.max(0) as u64 / SPINNER_MS
}

/// What a page last painted of the clock. `due` says whether painting now
/// would show something different from the last paint.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClockStamp {
    second: Option<i64>,
    frame: Option<u64>,
}

impl ClockStamp {
    pub fn painted(&mut self, w: &World) {
        self.second = Some(w.now_secs());
        self.frame = Some(frame(w));
    }

    /// `seconds`: some visible text shows an age or a duration. `spinning`:
    /// some visible text shows a spinner.
    pub fn due(&self, w: &World, seconds: bool, spinning: bool) -> bool {
        (seconds && self.second != Some(w.now_secs())) || (spinning && self.frame != Some(frame(w)))
    }
}

/// Whether the visible metas hold a clock text of each kind.
pub fn clocks<'a>(metas: impl Iterator<Item = &'a RowMeta>) -> (bool, bool) {
    let (mut seconds, mut spinning) = (false, false);
    for meta in metas {
        match meta.moment() {
            None => {}
            Some(Moment::Spinner(_)) => spinning = true,
            Some(Moment::AgeOr(None, _)) => {}
            Some(_) => seconds = true,
        }
    }
    (seconds, spinning)
}
