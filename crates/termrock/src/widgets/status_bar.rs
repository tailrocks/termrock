use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Style,
    widgets::StatefulWidget,
};

use crate::{
    interaction::{HitRegion, Outcome},
    style::{Role, Theme, faded},
};

#[derive(Debug, Clone)]
pub struct StatusSlot<'a, Id> {
    pub id: Id,
    pub content: &'a str,
    /// Higher-priority slots receive width before lower-priority slots.
    pub priority: u8,
    /// Minimum display columns required to keep the slot. Zero means all-or-nothing.
    pub min_width: u16,
    pub enabled: bool,
    pub style: Style,
    pub hover_style: Option<Style>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusBarState<Id> {
    pub hovered: Option<Id>,
    pub regions: Vec<HitRegion<Id>>,
}

impl<Id> Default for StatusBarState<Id> {
    fn default() -> Self {
        Self {
            hovered: None,
            regions: Vec::new(),
        }
    }
}

impl<Id: Clone> StatusBarState<Id> {
    pub fn hover(&mut self, position: Position) -> Option<&Id> {
        self.hovered = self
            .regions
            .iter()
            .find(|region| region.area.contains(position))
            .map(|region| region.id.clone());
        self.hovered.as_ref()
    }

    #[must_use]
    pub fn click(&mut self, position: Position) -> Outcome<Id> {
        self.regions
            .iter()
            .find(|region| region.area.contains(position))
            .map_or(Outcome::Ignored, |region| {
                Outcome::Activated(region.id.clone())
            })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StatusBar<'a, Id> {
    left: &'a [StatusSlot<'a, Id>],
    right: &'a [StatusSlot<'a, Id>],
    theme: &'a Theme,
    alpha: f32,
}

impl<'a, Id> StatusBar<'a, Id> {
    #[must_use]
    pub const fn new(
        left: &'a [StatusSlot<'a, Id>],
        right: &'a [StatusSlot<'a, Id>],
        theme: &'a Theme,
    ) -> Self {
        Self {
            left,
            right,
            theme,
            alpha: 1.0,
        }
    }

    #[must_use]
    pub const fn alpha(mut self, alpha: f32) -> Self {
        self.alpha = alpha;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Left,
    Right,
}

#[derive(Debug, Clone)]
struct Allocation<Id> {
    id: Id,
    side: Side,
    index: usize,
    width: u16,
    full_width: u16,
    priority: u8,
}

#[derive(Debug, Clone)]
struct Placement<Id> {
    id: Id,
    side: Side,
    index: usize,
    area: Rect,
}

impl<Id: Clone> StatusBar<'_, Id> {
    #[must_use]
    pub fn regions(&self, area: Rect) -> Vec<HitRegion<Id>> {
        self.placements(area)
            .into_iter()
            .map(|placement| HitRegion {
                id: placement.id,
                area: placement.area,
            })
            .collect()
    }

    fn placements(&self, area: Rect) -> Vec<Placement<Id>> {
        if area.is_empty() {
            return Vec::new();
        }
        let mut candidates = self
            .left
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| allocation(slot, Side::Left, index))
            .chain(
                self.right
                    .iter()
                    .enumerate()
                    .filter_map(|(index, slot)| allocation(slot, Side::Right, index)),
            )
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            right
                .priority
                .cmp(&left.priority)
                .then_with(|| side_rank(left.side).cmp(&side_rank(right.side)))
                .then_with(|| left.index.cmp(&right.index))
        });

        let mut remaining = area.width;
        let mut included = Vec::new();
        for mut candidate in candidates {
            let minimum = if candidate.width == 0 {
                candidate.full_width
            } else {
                candidate.width
            };
            if minimum == 0 || minimum > remaining {
                continue;
            }
            candidate.width = minimum;
            remaining = remaining.saturating_sub(minimum);
            included.push(candidate);
        }
        for allocation in &mut included {
            let growth = allocation
                .full_width
                .saturating_sub(allocation.width)
                .min(remaining);
            allocation.width = allocation.width.saturating_add(growth);
            remaining = remaining.saturating_sub(growth);
        }

        let mut placements = Vec::with_capacity(included.len());
        let mut left_x = area.x;
        let mut left = included
            .iter()
            .filter(|allocation| allocation.side == Side::Left)
            .collect::<Vec<_>>();
        left.sort_by_key(|allocation| allocation.index);
        for allocation in left {
            let width = allocation.width.min(area.right().saturating_sub(left_x));
            if width == 0 {
                continue;
            }
            placements.push(Placement {
                id: allocation.id.clone(),
                side: Side::Left,
                index: allocation.index,
                area: Rect::new(left_x, area.y, width, 1),
            });
            left_x = left_x.saturating_add(width);
        }

        let mut right_x = area.right();
        let mut right = included
            .iter()
            .filter(|allocation| allocation.side == Side::Right)
            .collect::<Vec<_>>();
        right.sort_by_key(|allocation| std::cmp::Reverse(allocation.index));
        for allocation in right {
            let start = right_x.saturating_sub(allocation.width).max(left_x);
            if start >= right_x {
                continue;
            }
            placements.push(Placement {
                id: allocation.id.clone(),
                side: Side::Right,
                index: allocation.index,
                area: Rect::new(start, area.y, right_x.saturating_sub(start), 1),
            });
            right_x = start;
        }
        placements
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &StatusBar<'_, Id> {
    type State = StatusBarState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() {
            state.regions.clear();
            return;
        }
        buffer.set_style(
            area,
            fade_style(self.theme.style(Role::StatusBar), self.alpha),
        );
        state.regions.clear();
        for placement in self.placements(area) {
            let slot = match placement.side {
                Side::Left => &self.left[placement.index],
                Side::Right => &self.right[placement.index],
            };
            let hovered = state.hovered.as_ref() == Some(&slot.id);
            let style = if hovered {
                slot.hover_style.unwrap_or(slot.style)
            } else {
                slot.style
            };
            let content =
                crate::text::display_cols_slice(slot.content, 0, usize::from(placement.area.width));
            buffer.set_stringn(
                placement.area.x,
                placement.area.y,
                content,
                usize::from(placement.area.width),
                fade_style(style, self.alpha),
            );
            state.regions.push(HitRegion {
                id: placement.id,
                area: placement.area,
            });
        }
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for StatusBar<'_, Id> {
    type State = StatusBarState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        StatefulWidget::render(&self, area, buffer, state);
    }
}

fn allocation<Id: Clone>(
    slot: &StatusSlot<'_, Id>,
    side: Side,
    index: usize,
) -> Option<Allocation<Id>> {
    if !slot.enabled {
        return None;
    }
    let full_width = u16::try_from(crate::text::display_cols(slot.content)).unwrap_or(u16::MAX);
    if full_width == 0 {
        return None;
    }
    Some(Allocation {
        id: slot.id.clone(),
        side,
        index,
        width: slot.min_width.min(full_width),
        full_width,
        priority: slot.priority,
    })
}

const fn side_rank(side: Side) -> u8 {
    match side {
        Side::Left => 0,
        Side::Right => 1,
    }
}

fn fade_style(mut style: Style, alpha: f32) -> Style {
    if let Some(foreground) = style.fg {
        style = style.fg(faded(foreground, alpha));
    }
    if let Some(background) = style.bg {
        style = style.bg(faded(background, alpha));
    }
    if let Some(underline) = style.underline_color {
        style = style.underline_color(faded(underline, alpha));
    }
    style
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_core::style::Color;

    fn slot(
        id: &'static str,
        content: &'static str,
        priority: u8,
        min_width: u16,
    ) -> StatusSlot<'static, &'static str> {
        StatusSlot {
            id,
            content,
            priority,
            min_width,
            enabled: true,
            style: Style::new().fg(Color::Rgb(100, 50, 20)),
            hover_style: Some(Style::new().fg(Color::Rgb(200, 100, 40))),
        }
    }

    #[test]
    fn priority_and_minimum_width_control_narrow_layout() {
        let theme = Theme::default();
        let left = [slot("activity", " activity ", 10, 4)];
        let right = [
            slot("usage", " usage-long ", 1, 0),
            slot("run", " run ", 20, 0),
        ];
        let bar = StatusBar::new(&left, &right, &theme);
        let regions = bar.regions(Rect::new(3, 2, 10, 1));
        assert!(regions.iter().any(|region| region.id == "run"));
        assert!(regions.iter().any(|region| region.id == "activity"));
        assert!(!regions.iter().any(|region| region.id == "usage"));
        assert!(regions.iter().all(|region| region.area.width > 0));
    }

    #[test]
    fn hover_and_activation_follow_only_painted_regions() {
        let left = [slot("activity", " activity ", 1, 4)];
        let theme =
            Theme::default().with_role(Role::StatusBar, Style::new().bg(Color::Rgb(80, 80, 80)));
        let bar = StatusBar::new(&left, &[], &theme).alpha(0.5);
        let area = Rect::new(4, 3, 6, 1);
        let mut state = StatusBarState::default();
        let mut buffer = Buffer::empty(area);
        (&bar).render(area, &mut buffer, &mut state);
        assert_eq!(state.regions.len(), 1);
        let position = Position::new(area.x, area.y);
        assert_eq!(state.hover(position), Some(&"activity"));
        (&bar).render(area, &mut buffer, &mut state);
        assert_eq!(state.click(position), Outcome::Activated("activity"));
        assert_eq!(buffer[(area.x, area.y)].bg, Color::Rgb(40, 40, 40));
        assert_eq!(buffer[(area.x, area.y)].fg, Color::Rgb(100, 50, 20));
    }

    #[test]
    fn unicode_truncation_never_paints_half_a_wide_grapheme() {
        let left = [slot("wide", " 🧪🔬🧭 ", 1, 3)];
        let theme = Theme::default();
        let bar = StatusBar::new(&left, &[], &theme);
        let area = Rect::new(0, 0, 3, 1);
        let mut state = StatusBarState::default();
        let mut buffer = Buffer::empty(area);
        (&bar).render(area, &mut buffer, &mut state);
        assert_eq!(state.regions[0].area.width, 3);
        assert_ne!(buffer[(2, 0)].symbol(), "\0");
    }
}
