use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Modifier,
    widgets::StatefulWidget,
};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind},
    style::{Role, Theme},
};

const RATIO_SCALE: u16 = 10_000;
const KEYBOARD_STEP: u16 = 250;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitSide {
    First,
    Second,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SplitRatio(u16);

impl SplitRatio {
    #[must_use]
    pub const fn from_basis_points(basis_points: u16) -> Self {
        Self(if basis_points > RATIO_SCALE {
            RATIO_SCALE
        } else {
            basis_points
        })
    }

    #[must_use]
    pub const fn from_percent(percent: u8) -> Self {
        Self::from_basis_points(if percent > 100 {
            RATIO_SCALE
        } else {
            percent as u16 * 100
        })
    }

    #[must_use]
    pub const fn basis_points(self) -> u16 {
        self.0
    }
}

impl Default for SplitRatio {
    fn default() -> Self {
        Self::from_percent(50)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SplitPaneLayout {
    pub first: Rect,
    pub divider: Rect,
    pub second: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitPaneOutcome {
    Ignored,
    Focused,
    RatioChanged(SplitRatio),
    Collapsed(SplitSide),
    Expanded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitPaneState {
    ratio: SplitRatio,
    focused: bool,
    hovered: bool,
    dragging: bool,
    collapsed: Option<SplitSide>,
    layout: SplitPaneLayout,
    painted: Option<PaintedSplit>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PaintedSplit {
    direction: SplitDirection,
    layout: SplitPaneLayout,
}

impl Default for SplitPaneState {
    fn default() -> Self {
        Self::new(SplitRatio::default())
    }
}

impl SplitPaneState {
    #[must_use]
    pub const fn new(ratio: SplitRatio) -> Self {
        Self {
            ratio,
            focused: false,
            hovered: false,
            dragging: false,
            collapsed: None,
            layout: SplitPaneLayout {
                first: Rect::ZERO,
                divider: Rect::ZERO,
                second: Rect::ZERO,
            },
            painted: None,
        }
    }

    #[must_use]
    pub const fn ratio(&self) -> SplitRatio {
        self.ratio
    }

    pub const fn set_ratio(&mut self, ratio: SplitRatio) {
        self.ratio = ratio;
        self.collapsed = None;
    }

    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.focused
    }

    pub const fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
        if !focused {
            self.dragging = false;
        }
    }

    #[must_use]
    pub const fn is_hovered(&self) -> bool {
        self.hovered
    }

    #[must_use]
    pub const fn is_dragging(&self) -> bool {
        self.dragging
    }

    #[must_use]
    pub const fn collapsed(&self) -> Option<SplitSide> {
        self.collapsed
    }

    #[must_use]
    pub const fn layout(&self) -> SplitPaneLayout {
        self.layout
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SplitPane<'a> {
    pub direction: SplitDirection,
    pub first_min: u16,
    pub second_min: u16,
    pub theme: &'a Theme,
}

impl<'a> SplitPane<'a> {
    #[must_use]
    pub const fn new(
        direction: SplitDirection,
        first_min: u16,
        second_min: u16,
        theme: &'a Theme,
    ) -> Self {
        Self {
            direction,
            first_min,
            second_min,
            theme,
        }
    }

    pub fn layout(&self, area: Rect, state: &mut SplitPaneState) -> SplitPaneLayout {
        let total = match self.direction {
            SplitDirection::Horizontal => area.width,
            SplitDirection::Vertical => area.height,
        };
        if total == 0 {
            state.layout = SplitPaneLayout {
                first: empty_rect(area, self.direction),
                divider: empty_rect(area, self.direction),
                second: empty_rect(area, self.direction),
            };
            return state.layout;
        }

        let available = total.saturating_sub(1);
        let first = match state.collapsed {
            Some(SplitSide::First) => 0,
            Some(SplitSide::Second) => available,
            None => constrained_first(available, state.ratio, self.first_min, self.second_min),
        };
        state.layout = split_rects(area, self.direction, first, available - first);
        state.layout
    }

    pub fn handle_key(&self, state: &mut SplitPaneState, key: KeyEvent) -> SplitPaneOutcome {
        if !state.focused || key.kind == KeyEventKind::Release {
            return SplitPaneOutcome::Ignored;
        }
        let delta = match (self.direction, key.code) {
            (SplitDirection::Horizontal, KeyCode::Left)
            | (SplitDirection::Vertical, KeyCode::Up) => Some(-i32::from(KEYBOARD_STEP)),
            (SplitDirection::Horizontal, KeyCode::Right)
            | (SplitDirection::Vertical, KeyCode::Down) => Some(i32::from(KEYBOARD_STEP)),
            _ => None,
        };
        if let Some(delta) = delta {
            state.collapsed = None;
            let current = i32::from(state.ratio.basis_points());
            let next = current
                .saturating_add(delta)
                .clamp(0, i32::from(RATIO_SCALE));
            state.ratio = SplitRatio::from_basis_points(next as u16);
            return SplitPaneOutcome::RatioChanged(state.ratio);
        }
        SplitPaneOutcome::Ignored
    }

    pub fn collapse(&self, state: &mut SplitPaneState, side: SplitSide) -> SplitPaneOutcome {
        if state.collapsed == Some(side) {
            SplitPaneOutcome::Ignored
        } else {
            state.collapsed = Some(side);
            state.dragging = false;
            SplitPaneOutcome::Collapsed(side)
        }
    }

    pub fn expand(&self, state: &mut SplitPaneState) -> SplitPaneOutcome {
        if state.collapsed.take().is_some() {
            SplitPaneOutcome::Expanded
        } else {
            SplitPaneOutcome::Ignored
        }
    }

    pub fn pointer_down(&self, state: &mut SplitPaneState, position: Position) -> SplitPaneOutcome {
        let Some(painted) = state
            .painted
            .filter(|painted| painted.direction == self.direction)
        else {
            return SplitPaneOutcome::Ignored;
        };
        if painted.layout.divider.is_empty() || !painted.layout.divider.contains(position) {
            return SplitPaneOutcome::Ignored;
        }
        state.focused = true;
        state.hovered = true;
        state.dragging = true;
        SplitPaneOutcome::Focused
    }

    pub fn pointer_move(&self, state: &mut SplitPaneState, position: Position) -> bool {
        let hovered = state
            .painted
            .filter(|painted| painted.direction == self.direction)
            .is_some_and(|painted| {
                !painted.layout.divider.is_empty() && painted.layout.divider.contains(position)
            });
        let changed = state.hovered != hovered;
        state.hovered = hovered;
        changed
    }

    pub fn pointer_drag(&self, state: &mut SplitPaneState, position: Position) -> SplitPaneOutcome {
        if !state.dragging {
            return SplitPaneOutcome::Ignored;
        }
        let Some(painted) = state
            .painted
            .filter(|painted| painted.direction == self.direction)
        else {
            return SplitPaneOutcome::Ignored;
        };
        let area = painted_area(painted.layout, self.direction);
        let available = match self.direction {
            SplitDirection::Horizontal => painted
                .layout
                .first
                .width
                .saturating_add(painted.layout.second.width),
            SplitDirection::Vertical => painted
                .layout
                .first
                .height
                .saturating_add(painted.layout.second.height),
        };
        if available == 0 {
            return SplitPaneOutcome::Ignored;
        }
        let origin = match self.direction {
            SplitDirection::Horizontal => area.x,
            SplitDirection::Vertical => area.y,
        };
        let coordinate = match self.direction {
            SplitDirection::Horizontal => position.x,
            SplitDirection::Vertical => position.y,
        };
        let first = coordinate.saturating_sub(origin).min(available);
        let basis_points = (u32::from(first) * u32::from(RATIO_SCALE) + u32::from(available) / 2)
            / u32::from(available);
        state.ratio = SplitRatio::from_basis_points(basis_points as u16);
        state.collapsed = None;
        self.layout(area, state);
        SplitPaneOutcome::RatioChanged(state.ratio)
    }

    pub const fn pointer_up(&self, state: &mut SplitPaneState) {
        state.dragging = false;
    }
}

impl StatefulWidget for &SplitPane<'_> {
    type State = SplitPaneState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        let layout = self.layout(area, state);
        if layout.divider.is_empty() {
            state.painted = Some(PaintedSplit {
                direction: self.direction,
                layout,
            });
            return;
        }
        let (glyph, role) = match (self.direction, state.collapsed, state.focused) {
            (SplitDirection::Horizontal, Some(SplitSide::First), _) => ("›", Role::Accent),
            (SplitDirection::Horizontal, Some(SplitSide::Second), _) => ("‹", Role::Accent),
            (SplitDirection::Vertical, Some(SplitSide::First), _) => ("⌄", Role::Accent),
            (SplitDirection::Vertical, Some(SplitSide::Second), _) => ("⌃", Role::Accent),
            (SplitDirection::Horizontal, None, true) => ("┃", Role::Focus),
            (SplitDirection::Horizontal, None, false) if state.hovered => ("┋", Role::Focus),
            (SplitDirection::Horizontal, None, false) => ("│", Role::Border),
            (SplitDirection::Vertical, None, true) => ("━", Role::Focus),
            (SplitDirection::Vertical, None, false) if state.hovered => ("┅", Role::Focus),
            (SplitDirection::Vertical, None, false) => ("─", Role::Border),
        };
        let mut style = self.theme.style(role);
        if state.focused {
            style = style.add_modifier(Modifier::BOLD);
        }
        for y in layout.divider.top()..layout.divider.bottom() {
            for x in layout.divider.left()..layout.divider.right() {
                buffer.set_string(x, y, glyph, style);
            }
        }
        state.painted = Some(PaintedSplit {
            direction: self.direction,
            layout,
        });
    }
}

fn constrained_first(available: u16, ratio: SplitRatio, first_min: u16, second_min: u16) -> u16 {
    let desired = ((u32::from(available) * u32::from(ratio.basis_points())
        + u32::from(RATIO_SCALE) / 2)
        / u32::from(RATIO_SCALE)) as u16;
    let minimum_sum = u32::from(first_min) + u32::from(second_min);
    if u32::from(available) >= minimum_sum {
        desired.clamp(first_min, available.saturating_sub(second_min))
    } else {
        let proportional =
            (u32::from(available) * u32::from(first_min) + minimum_sum / 2) / minimum_sum;
        u16::try_from(proportional)
            .unwrap_or(available)
            .min(available)
    }
}

fn split_rects(area: Rect, direction: SplitDirection, first: u16, second: u16) -> SplitPaneLayout {
    match direction {
        SplitDirection::Horizontal => SplitPaneLayout {
            first: Rect::new(area.x, area.y, first, area.height),
            divider: Rect::new(area.x.saturating_add(first), area.y, 1, area.height),
            second: Rect::new(
                area.x.saturating_add(first).saturating_add(1),
                area.y,
                second,
                area.height,
            ),
        },
        SplitDirection::Vertical => SplitPaneLayout {
            first: Rect::new(area.x, area.y, area.width, first),
            divider: Rect::new(area.x, area.y.saturating_add(first), area.width, 1),
            second: Rect::new(
                area.x,
                area.y.saturating_add(first).saturating_add(1),
                area.width,
                second,
            ),
        },
    }
}

fn empty_rect(area: Rect, direction: SplitDirection) -> Rect {
    match direction {
        SplitDirection::Horizontal => Rect::new(area.x, area.y, 0, area.height),
        SplitDirection::Vertical => Rect::new(area.x, area.y, area.width, 0),
    }
}

fn painted_area(layout: SplitPaneLayout, direction: SplitDirection) -> Rect {
    match direction {
        SplitDirection::Horizontal => Rect::new(
            layout.first.x,
            layout.divider.y,
            layout
                .first
                .width
                .saturating_add(layout.divider.width)
                .saturating_add(layout.second.width),
            layout.divider.height,
        ),
        SplitDirection::Vertical => Rect::new(
            layout.divider.x,
            layout.first.y,
            layout.divider.width,
            layout
                .first
                .height
                .saturating_add(layout.divider.height)
                .saturating_add(layout.second.height),
        ),
    }
}
