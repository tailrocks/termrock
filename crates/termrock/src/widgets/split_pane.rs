use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Modifier,
    widgets::StatefulWidget,
};

use crate::{
    input::{KeyCode, KeyEvent},
    style::{DesignSystem, Role},
};

const RATIO_SCALE: u16 = 10_000;
const MIN_RATIO_BASIS_POINTS: u16 = 500;
const MAX_RATIO_BASIS_POINTS: u16 = 9_500;
const KEYBOARD_STEP_CELLS: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// The axis along which a split pane divides its area.
pub enum SplitDirection {
    /// The horizontal terminal axis.
    Horizontal,
    /// The vertical terminal axis.
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// One side of a split pane.
pub enum SplitSide {
    /// The first pane.
    First,
    /// The second pane.
    Second,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
/// A split proportion stored as bounded basis points.
pub struct SplitRatio(u16);

impl SplitRatio {
    #[must_use]
    /// Creates a ratio clamped to the inclusive 500–9,500 basis-point range.
    pub const fn from_basis_points(basis_points: u16) -> Self {
        Self(normalize_basis_points(basis_points))
    }

    #[must_use]
    /// Creates a ratio from a percentage clamped to the inclusive 5–95 range.
    pub const fn from_percent(percent: u8) -> Self {
        Self::from_basis_points((percent as u16).saturating_mul(100))
    }

    #[must_use]
    /// Returns the split proportion in basis points.
    pub const fn basis_points(self) -> u16 {
        self.0
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for SplitRatio {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let basis_points = <u16 as serde::Deserialize>::deserialize(deserializer)?;
        Ok(Self::from_basis_points(basis_points))
    }
}

impl Default for SplitRatio {
    fn default() -> Self {
        Self::from_percent(50)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
/// Resolved rectangles for both panes and their divider.
pub struct SplitPaneLayout {
    /// Resolved rectangle for the first pane.
    pub first: Rect,
    /// Resolved one-cell divider rectangle.
    pub divider: Rect,
    /// Resolved rectangle for the second pane.
    pub second: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
/// Semantic results produced by split-pane interaction.
pub enum SplitPaneOutcome {
    /// The gesture did not apply to the divider.
    Ignored,
    /// Pointer interaction moved focus to the divider.
    Focused,
    /// The divider moved to this new bounded ratio.
    RatioChanged(SplitRatio),
    /// The identified pane side became collapsed.
    Collapsed(SplitSide),
    /// A collapsed pane returned to the remembered ratio.
    Expanded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Runtime state for `SplitPane`.
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
    /// Creates split state at the supplied ratio with both panes expanded.
    pub const fn new(ratio: SplitRatio) -> Self {
        Self {
            ratio: SplitRatio::from_basis_points(ratio.basis_points()),
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
    /// Returns the current split proportion.
    pub const fn ratio(&self) -> SplitRatio {
        self.ratio
    }

    /// The side currently collapsed, if any.
    #[must_use]
    pub const fn collapsed(&self) -> Option<SplitSide> {
        self.collapsed
    }

    /// Replaces the expanded ratio and clears any collapsed side.
    pub const fn set_ratio(&mut self, ratio: SplitRatio) {
        self.ratio = SplitRatio::from_basis_points(ratio.basis_points());
        self.collapsed = None;
    }

    /// Updates divider focus, cancelling an active drag when focus leaves.
    pub const fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
        if !focused {
            self.dragging = false;
        }
    }

    #[must_use]
    /// Returns whether the pointer is over the painted divider.
    pub const fn is_hovered(&self) -> bool {
        self.hovered
    }

    #[must_use]
    /// Returns whether a divider drag is currently active.
    pub const fn is_dragging(&self) -> bool {
        self.dragging
    }

    #[must_use]
    /// Resolves both panes and the divider inside the supplied rectangle.
    pub const fn layout(&self) -> SplitPaneLayout {
        self.layout
    }

    /// Moves the focused divider along its layout axis with arrow keys.
    pub fn handle_key(&mut self, spec: &SplitPane<'_>, key: KeyEvent) -> SplitPaneOutcome {
        if !self.focused
            || key.is_release()
            || self.collapsed.is_some()
            || self.layout.divider.is_empty()
        {
            return SplitPaneOutcome::Ignored;
        }
        let delta = match (spec.direction, key.code) {
            (SplitDirection::Horizontal, KeyCode::Left)
            | (SplitDirection::Vertical, KeyCode::Up) => Some(-i32::from(KEYBOARD_STEP_CELLS)),
            (SplitDirection::Horizontal, KeyCode::Right)
            | (SplitDirection::Vertical, KeyCode::Down) => Some(i32::from(KEYBOARD_STEP_CELLS)),
            _ => None,
        };
        let Some(delta) = delta else {
            return SplitPaneOutcome::Ignored;
        };
        let available = layout_available(self.layout, spec.direction);
        let Some(feasible) = resolve_feasible_split(available, spec.first_min, spec.second_min)
        else {
            return SplitPaneOutcome::Ignored;
        };
        let current = feasible.first_for_ratio(self.ratio);
        let target = i32::from(current)
            .saturating_add(delta)
            .clamp(0, i32::from(available)) as u16;
        let target = feasible.clamp_first(target);
        if target == current {
            return SplitPaneOutcome::Ignored;
        }
        let next_ratio = feasible.ratio_for_keyboard(target);
        if next_ratio == self.ratio {
            return SplitPaneOutcome::Ignored;
        }
        if feasible.first_for_ratio(next_ratio) == current {
            return SplitPaneOutcome::Ignored;
        }
        self.ratio = next_ratio;
        self.collapsed = None;
        SplitPaneOutcome::RatioChanged(self.ratio)
    }

    /// Collapses one pane while preserving the configured split ratio.
    pub fn collapse(&mut self, side: SplitSide) -> SplitPaneOutcome {
        if self.collapsed == Some(side) {
            SplitPaneOutcome::Ignored
        } else {
            self.collapsed = Some(side);
            self.dragging = false;
            SplitPaneOutcome::Collapsed(side)
        }
    }

    /// Restores both panes to the configured split ratio.
    pub fn expand(&mut self) -> SplitPaneOutcome {
        if self.collapsed.take().is_some() {
            SplitPaneOutcome::Expanded
        } else {
            SplitPaneOutcome::Ignored
        }
    }

    /// Begins divider dragging only when the pointer hits painted divider geometry.
    pub fn drag_start(&mut self, spec: &SplitPane<'_>, position: Position) -> SplitPaneOutcome {
        let Some(painted) = self
            .painted
            .filter(|painted| painted.direction == spec.direction)
        else {
            return SplitPaneOutcome::Ignored;
        };
        if painted.layout.divider.is_empty() || !painted.layout.divider.contains(position) {
            return SplitPaneOutcome::Ignored;
        }
        self.focused = true;
        self.hovered = true;
        self.dragging = true;
        SplitPaneOutcome::Focused
    }

    /// Updates hover state from the current pointer position and painted hit regions.
    pub fn hover(&mut self, spec: &SplitPane<'_>, position: Position) -> bool {
        let hovered = self
            .painted
            .filter(|painted| painted.direction == spec.direction)
            .is_some_and(|painted| {
                !painted.layout.divider.is_empty() && painted.layout.divider.contains(position)
            });
        let changed = self.hovered != hovered;
        self.hovered = hovered;
        changed
    }

    /// Updates the split ratio from an active divider drag.
    pub fn drag_move(&mut self, spec: &SplitPane<'_>, position: Position) -> SplitPaneOutcome {
        if !self.dragging {
            return SplitPaneOutcome::Ignored;
        }
        let Some(painted) = self
            .painted
            .filter(|painted| painted.direction == spec.direction)
        else {
            return SplitPaneOutcome::Ignored;
        };
        let area = painted_area(painted.layout, spec.direction);
        let available = layout_available(painted.layout, spec.direction);
        let Some(feasible) = resolve_feasible_split(available, spec.first_min, spec.second_min)
        else {
            return SplitPaneOutcome::Ignored;
        };
        if available == 0 {
            return SplitPaneOutcome::Ignored;
        }
        let origin = match spec.direction {
            SplitDirection::Horizontal => area.x,
            SplitDirection::Vertical => area.y,
        };
        let coordinate = match spec.direction {
            SplitDirection::Horizontal => position.x,
            SplitDirection::Vertical => position.y,
        };
        let requested = coordinate.saturating_sub(origin).min(available);
        let next_ratio = feasible.ratio_for_drag(requested);
        if next_ratio == self.ratio {
            return SplitPaneOutcome::Ignored;
        }
        let painted_first = match spec.direction {
            SplitDirection::Horizontal => painted.layout.first.width,
            SplitDirection::Vertical => painted.layout.first.height,
        };
        if feasible.first_for_ratio(next_ratio) == painted_first {
            return SplitPaneOutcome::Ignored;
        }
        self.ratio = next_ratio;
        self.collapsed = None;
        spec.layout(area, self);
        SplitPaneOutcome::RatioChanged(self.ratio)
    }

    /// Ends an active divider drag.
    pub const fn drag_end(&mut self) {
        self.dragging = false;
    }
}

#[derive(Debug, Clone, Copy)]
/// A resizable two-pane layout with collapse support.
pub struct SplitPane<'a> {
    direction: SplitDirection,
    first_min: u16,
    second_min: u16,
    system: &'a DesignSystem,
}

impl<'a> SplitPane<'a> {
    #[must_use]
    /// Creates a split pane with the supplied direction, state, and theme.
    pub const fn new(
        direction: SplitDirection,
        first_min: u16,
        second_min: u16,
        system: &'a DesignSystem,
    ) -> Self {
        Self {
            direction,
            first_min,
            second_min,
            system,
        }
    }

    /// Resolves both panes and the divider inside the supplied rectangle.
    pub fn layout(&self, area: Rect, state: &mut SplitPaneState) -> SplitPaneLayout {
        if let Some(side) = state.collapsed {
            state.layout = collapsed_layout(area, self.direction, side);
            return state.layout;
        }

        let total = match self.direction {
            SplitDirection::Horizontal => area.width,
            SplitDirection::Vertical => area.height,
        };
        let available = total.saturating_sub(1);
        let Some(feasible) = resolve_feasible_split(available, self.first_min, self.second_min)
        else {
            state.layout = impossible_layout(area, self.direction);
            return state.layout;
        };
        let first = feasible.first_for_ratio(state.ratio);
        state.layout = split_rects(area, self.direction, first, available - first);
        state.layout
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
        let (glyph, role) = match (self.direction, state.collapsed, state.focused, false) {
            (SplitDirection::Horizontal, Some(SplitSide::First), _, true) => (">", Role::Accent),
            (SplitDirection::Horizontal, Some(SplitSide::Second), _, true) => ("<", Role::Accent),
            (SplitDirection::Vertical, Some(SplitSide::First), _, true) => ("v", Role::Accent),
            (SplitDirection::Vertical, Some(SplitSide::Second), _, true) => ("^", Role::Accent),
            (SplitDirection::Horizontal, Some(SplitSide::First), _, false) => ("›", Role::Accent),
            (SplitDirection::Horizontal, Some(SplitSide::Second), _, false) => ("‹", Role::Accent),
            (SplitDirection::Vertical, Some(SplitSide::First), _, false) => ("⌄", Role::Accent),
            (SplitDirection::Vertical, Some(SplitSide::Second), _, false) => ("⌃", Role::Accent),
            (SplitDirection::Horizontal, None, true, _) => {
                (self.system.glyphs.rule_v(), Role::BorderFocused)
            }
            (SplitDirection::Horizontal, None, false, _) if state.hovered => {
                (self.system.glyphs.rule_v(), Role::Focus)
            }
            (SplitDirection::Horizontal, None, false, _) => (" ", Role::Border),
            (SplitDirection::Vertical, None, true, _) => {
                (self.system.glyphs.rule(), Role::BorderFocused)
            }
            (SplitDirection::Vertical, None, false, _) if state.hovered => {
                (self.system.glyphs.rule(), Role::Focus)
            }
            (SplitDirection::Vertical, None, false, _) => (" ", Role::Border),
        };
        let mut style = self.system.style(role);
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

impl StatefulWidget for SplitPane<'_> {
    type State = SplitPaneState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FeasibleSplit {
    available: u16,
    first_min: u16,
    second_min: u16,
}

impl FeasibleSplit {
    fn clamp_first(self, first: u16) -> u16 {
        first.clamp(
            self.first_min,
            self.available.saturating_sub(self.second_min),
        )
    }

    fn first_for_ratio(self, ratio: SplitRatio) -> u16 {
        let desired = (u32::from(self.available) * u32::from(ratio.basis_points())
            / u32::from(RATIO_SCALE)) as u16;
        self.clamp_first(desired)
    }

    /// Converts a physical seam to the canonical nearest whole-percent ratio.
    fn ratio_for_drag(self, first: u16) -> SplitRatio {
        self.ratio_for_cell(first)
    }

    /// Converts a keyboard target cell to the smallest whole-percent ratio that reaches it.
    fn ratio_for_keyboard(self, first: u16) -> SplitRatio {
        if self.available == 0 {
            return SplitRatio::default();
        }
        let first = self.clamp_first(first);
        let percent = ((u32::from(first) * 100 + u32::from(self.available) - 1)
            / u32::from(self.available)) as u8;
        SplitRatio::from_percent(percent)
    }

    fn ratio_for_cell(self, first: u16) -> SplitRatio {
        if self.available == 0 {
            return SplitRatio::default();
        }
        let first = self.clamp_first(first);
        let percent = ((u32::from(first) * 100 + u32::from(self.available) / 2)
            / u32::from(self.available)) as u8;
        SplitRatio::from_percent(percent)
    }
}

fn resolve_feasible_split(
    available: u16,
    first_min: u16,
    second_min: u16,
) -> Option<FeasibleSplit> {
    if u32::from(first_min) + u32::from(second_min) > u32::from(available) {
        None
    } else {
        Some(FeasibleSplit {
            available,
            first_min,
            second_min,
        })
    }
}

const fn normalize_basis_points(basis_points: u16) -> u16 {
    if basis_points < MIN_RATIO_BASIS_POINTS {
        MIN_RATIO_BASIS_POINTS
    } else if basis_points > MAX_RATIO_BASIS_POINTS {
        MAX_RATIO_BASIS_POINTS
    } else {
        basis_points
    }
}

fn split_rects(area: Rect, direction: SplitDirection, first: u16, second: u16) -> SplitPaneLayout {
    if first == 0 && second == 0 {
        return SplitPaneLayout {
            first: empty_rect(area, direction),
            divider: Rect::ZERO,
            second: empty_second_rect(area, direction),
        };
    }

    let divider = if first == 0 || second == 0 {
        Rect::ZERO
    } else {
        match direction {
            SplitDirection::Horizontal => {
                Rect::new(area.x.saturating_add(first), area.y, 1, area.height)
            }
            SplitDirection::Vertical => {
                Rect::new(area.x, area.y.saturating_add(first), area.width, 1)
            }
        }
    };

    match direction {
        SplitDirection::Horizontal => SplitPaneLayout {
            first: Rect::new(area.x, area.y, first, area.height),
            divider,
            second: Rect::new(
                area.x.saturating_add(first).saturating_add(1),
                area.y,
                second,
                area.height,
            ),
        },
        SplitDirection::Vertical => SplitPaneLayout {
            first: Rect::new(area.x, area.y, area.width, first),
            divider,
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

fn empty_second_rect(area: Rect, direction: SplitDirection) -> Rect {
    match direction {
        SplitDirection::Horizontal => Rect::new(area.x.saturating_add(1), area.y, 0, area.height),
        SplitDirection::Vertical => Rect::new(area.x, area.y.saturating_add(1), area.width, 0),
    }
}

fn collapsed_layout(area: Rect, direction: SplitDirection, side: SplitSide) -> SplitPaneLayout {
    match (direction, side) {
        (SplitDirection::Horizontal, SplitSide::First)
        | (SplitDirection::Vertical, SplitSide::First) => SplitPaneLayout {
            first: Rect::ZERO,
            divider: Rect::ZERO,
            second: area,
        },
        (SplitDirection::Horizontal, SplitSide::Second)
        | (SplitDirection::Vertical, SplitSide::Second) => SplitPaneLayout {
            first: area,
            divider: Rect::ZERO,
            second: Rect::ZERO,
        },
    }
}

fn impossible_layout(area: Rect, direction: SplitDirection) -> SplitPaneLayout {
    match direction {
        SplitDirection::Horizontal => SplitPaneLayout {
            first: Rect::ZERO,
            divider: Rect::ZERO,
            second: area,
        },
        SplitDirection::Vertical => SplitPaneLayout {
            first: area,
            divider: Rect::ZERO,
            second: Rect::ZERO,
        },
    }
}

fn layout_available(layout: SplitPaneLayout, direction: SplitDirection) -> u16 {
    let total = match direction {
        SplitDirection::Horizontal => {
            u32::from(layout.first.width)
                + u32::from(layout.divider.width)
                + u32::from(layout.second.width)
        }
        SplitDirection::Vertical => {
            u32::from(layout.first.height)
                + u32::from(layout.divider.height)
                + u32::from(layout.second.height)
        }
    };
    total.saturating_sub(1).min(u32::from(u16::MAX)) as u16
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expanded_divider_is_invisible_until_interaction() {
        let system = DesignSystem::default();
        let split = SplitPane::new(SplitDirection::Horizontal, 1, 1, &system);
        let area = Rect::new(0, 0, 9, 3);
        let mut state = SplitPaneState::default();
        let mut buffer = Buffer::empty(area);
        StatefulWidget::render(&split, area, &mut buffer, &mut state);
        let x = state.layout().divider.x;
        assert_eq!(buffer[(x, 1)].symbol(), " ");

        state.set_focused(true);
        StatefulWidget::render(&split, area, &mut buffer, &mut state);
        assert_eq!(buffer[(x, 1)].symbol(), system.glyphs.rule_v());
        assert!(buffer[(x, 1)].modifier.contains(Modifier::BOLD));
    }
}
