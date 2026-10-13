//! **StatusStrip** — a row of short facts, in one voice with one exception.
//!
//! Header strips are where color discipline dies: eight segments, five hues,
//! and the one segment that is actually actionable buried among them. The
//! strip enforces the rule instead of asking every host to remember it — at
//! most one status hue and at most one accent survive; everything else reads
//! as metadata — and it drops by stated priority rather than by whatever
//! happened to be last in the vector.

use ratatui_core::{buffer::Buffer, layout::Rect, style::Style};

use crate::style::{DesignSystem, Role};
use crate::text::display_cols;
use crate::widgets::tiered_row::TieredRow;

/// One fact in a status strip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusSegment<'a> {
    /// Segment text, already worded by the host.
    pub text: &'a str,
    /// Semantic role the segment asks for.
    pub role: Role,
    /// Survival priority: higher survives longer under width pressure.
    pub priority: u8,
}

impl<'a> StatusSegment<'a> {
    /// A metadata segment (priority 50).
    #[must_use]
    pub const fn new(text: &'a str) -> Self {
        Self {
            text,
            role: Role::TextMuted,
            priority: 50,
        }
    }

    /// States the role the segment asks for.
    #[must_use]
    pub const fn role(mut self, role: Role) -> Self {
        self.role = role;
        self
    }

    /// States survival priority (higher survives longer).
    #[must_use]
    pub const fn priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }
}

/// Whether a role counts against the strip's colour budget.
fn is_status_hue(role: Role) -> bool {
    matches!(
        role,
        Role::Success | Role::Warning | Role::Danger | Role::Info
    )
}

/// A row of segments, budgeted and priority-dropped.
#[derive(Debug, Clone, Copy)]
pub struct StatusStrip<'a> {
    segments: &'a [StatusSegment<'a>],
    system: &'a DesignSystem,
    colorless: bool,
    overflow_hint: Option<&'a str>,
}

impl<'a> StatusStrip<'a> {
    /// Binds segments to a design system.
    #[must_use]
    pub const fn new(segments: &'a [StatusSegment<'a>], system: &'a DesignSystem) -> Self {
        Self {
            segments,
            system,
            colorless: false,
            overflow_hint: None,
        }
    }

    /// Drops every hue (colorless terminals, `NO_COLOR`).
    #[must_use]
    pub const fn colorless(mut self, colorless: bool) -> Self {
        self.colorless = colorless;
        self
    }

    /// Trailing hint painted when segments had to drop (`i +3`).
    #[must_use]
    pub const fn overflow_hint(mut self, hint: &'a str) -> Self {
        self.overflow_hint = Some(hint);
        self
    }

    /// The segments that survive `width`, in paint order.
    ///
    /// Contraction is by stated priority, not by position: the connection
    /// that is failing outlives the cost counter even when it was added after
    /// it. The first segment is the actionable one and is never dropped — it
    /// clips instead.
    #[must_use]
    pub fn visible(&self, width: u16) -> Vec<usize> {
        let kept = self.fit(width, 0);
        if kept.len() == self.segments.len() {
            return kept;
        }
        // Something dropped, so the hint will be painted: pay for it and fit
        // again, rather than letting it clip the last segment it announces.
        let separator_cols = display_cols(self.system.glyphs.meta_join());
        let reserve = self
            .overflow_hint
            .map(|hint| display_cols(hint) + separator_cols)
            .unwrap_or(0);
        if reserve == 0 {
            return kept;
        }
        self.fit(width, reserve)
    }

    fn fit(&self, width: u16, reserve: usize) -> Vec<usize> {
        if self.segments.is_empty() || width == 0 {
            return Vec::new();
        }
        let separator_cols = display_cols(self.system.glyphs.meta_join());
        let budget = usize::from(width).saturating_sub(reserve);

        let mut order: Vec<usize> = (0..self.segments.len()).collect();
        order.sort_by_key(|&i| (std::cmp::Reverse(self.segments[i].priority), i));

        let mut kept: Vec<usize> = Vec::new();
        let mut used = 0usize;
        for i in order {
            let cols = display_cols(self.segments[i].text);
            let with_separator = if kept.is_empty() {
                cols
            } else {
                cols + separator_cols
            };
            // The first segment is the actionable one: it survives at any
            // width, clipped rather than dropped.
            if kept.is_empty() || used + with_separator <= budget {
                used += with_separator;
                kept.push(i);
            }
        }
        kept.sort_unstable();
        kept
    }

    /// Paints the strip into a single row.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let row = Rect::new(area.x, area.y, area.width, 1);
        let kept = self.visible(area.width);
        let dropped = self.segments.len().saturating_sub(kept.len());

        let mut tiers = TieredRow::with_separator(self.system.glyphs.meta_join());
        let mut spent_hue = false;
        let mut spent_accent = false;
        for (position, &index) in kept.iter().enumerate() {
            let segment = &self.segments[index];
            let mut role = segment.role;
            // One status hue, one accent. A strip that spends more is a
            // dashboard pretending to be a status line.
            if self.colorless {
                role = if position == 0 {
                    Role::Text
                } else {
                    role_quiet(role)
                };
            } else if is_status_hue(role) {
                if spent_hue {
                    role = role_quiet(role);
                } else {
                    spent_hue = true;
                }
            } else if matches!(role, Role::Accent) {
                if spent_accent {
                    role = role_quiet(role);
                } else {
                    spent_accent = true;
                }
            }
            tiers.push(segment.text, self.system.style(role));
        }
        if let Some(hint) = self.overflow_hint
            && dropped > 0
        {
            tiers.push(hint, self.system.style(Role::TextFaint));
        }

        let line = tiers.text().to_string();
        let base: Style = self.system.style(Role::TextMuted);
        buffer.set_stringn(
            row.x,
            row.y,
            crate::text::take_display_cols(&line, usize::from(row.width)),
            usize::from(row.width),
            base,
        );
        tiers.paint_tiers(buffer, row, 0);
    }
}

/// The quiet tier a segment falls back to when the budget is spent.
const fn role_quiet(role: Role) -> Role {
    match role {
        Role::TextFaint => Role::TextFaint,
        _ => Role::TextMuted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segments() -> Vec<StatusSegment<'static>> {
        vec![
            StatusSegment::new("running")
                .role(Role::Success)
                .priority(100),
            StatusSegment::new("offline")
                .role(Role::Danger)
                .priority(90),
            StatusSegment::new("opus-5").priority(60),
            StatusSegment::new("q:3").role(Role::Warning).priority(80),
            StatusSegment::new("$0.42").priority(10),
        ]
    }

    #[test]
    fn one_status_hue_survives_the_budget() {
        let system = DesignSystem::default();
        let segments = segments();
        let area = Rect::new(0, 0, 60, 1);
        let mut buffer = Buffer::empty(area);
        StatusStrip::new(&segments, &system).paint(area, &mut buffer);

        let hues = [Role::Success, Role::Danger, Role::Warning, Role::Info];
        let spent = hues
            .iter()
            .filter(|role| {
                let fg = system.style(**role).fg;
                (0..area.width).any(|x| {
                    let cell = &buffer[(x, 0)];
                    !cell.symbol().trim().is_empty() && Some(cell.fg) == fg
                })
            })
            .count();
        assert!(
            spent <= 1,
            "a status strip must spend at most one status hue, spent {spent}"
        );
    }

    #[test]
    fn narrow_strips_drop_by_priority_not_by_position() {
        let system = DesignSystem::default();
        let segments = segments();
        let strip = StatusStrip::new(&segments, &system);
        let kept = strip.visible(24);
        assert!(kept.contains(&0), "the actionable segment always survives");
        assert!(
            !kept.contains(&4),
            "the cheapest segment drops first: {kept:?}"
        );
        assert!(
            kept.len() < segments.len(),
            "a narrow strip must actually drop something"
        );
    }

    #[test]
    fn the_actionable_segment_survives_any_width() {
        let system = DesignSystem::default();
        let segments = segments();
        let strip = StatusStrip::new(&segments, &system);
        assert_eq!(strip.visible(3), vec![0]);
        assert_eq!(strip.visible(1), vec![0]);
    }

    #[test]
    fn an_overflow_hint_says_how_much_is_hidden() {
        let system = DesignSystem::default();
        let segments = segments();
        let area = Rect::new(0, 0, 26, 1);
        let mut buffer = Buffer::empty(area);
        StatusStrip::new(&segments, &system)
            .overflow_hint("i +2")
            .paint(area, &mut buffer);
        let row: String = (0..area.width).map(|x| buffer[(x, 0)].symbol()).collect();
        assert!(row.contains("i +2"), "{row:?}");
    }

    #[test]
    fn colorless_strips_paint_no_hue_at_all() {
        let system = DesignSystem::default();
        let segments = segments();
        let area = Rect::new(0, 0, 60, 1);
        let mut buffer = Buffer::empty(area);
        StatusStrip::new(&segments, &system)
            .colorless(true)
            .paint(area, &mut buffer);
        for role in [Role::Success, Role::Danger, Role::Warning, Role::Info] {
            let fg = system.style(role).fg;
            assert!(
                !(0..area.width).any(|x| {
                    let cell = &buffer[(x, 0)];
                    !cell.symbol().trim().is_empty() && Some(cell.fg) == fg
                }),
                "colorless strips must not paint {role:?}"
            );
        }
    }
}
