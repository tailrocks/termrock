// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Content-measured vertical panel allocation.
use ratatui_core::layout::Rect;

/// Measured height request for one panel-like block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelStackBlock {
    /// Measured content height without chrome.
    pub content_rows: u16,
    /// Border/title/footer rows added around content.
    pub chrome_rows: u16,
    /// Minimum preferred allocation before overflow contraction.
    pub min: u16,
    /// Maximum allocation.
    pub max: u16,
    /// Hidden blocks are omitted and consume no gap.
    pub visible: bool,
}

impl PanelStackBlock {
    fn preferred(self) -> u16 {
        self.content_rows
            .saturating_add(self.chrome_rows)
            .clamp(self.min, self.max.max(self.min))
    }
}

/// How a shortfall is distributed when the preferred block heights exceed
/// the stack area.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShrinkPolicy {
    /// Shrink visible blocks from the end of the stack first (the
    /// historical `panel_stack` behavior).
    #[default]
    Tail,
    /// Distribute the shortfall with ratatui `Length`-solver semantics
    /// (equal-strength relaxation) — byte-identical to a hand-rolled
    /// ratatui `Layout` stack of `Constraint::Length` blocks, so consumers
    /// migrating such a layout onto `panel_stack` keep their rendered
    /// geometry under overflow.
    Equal,
}

/// Allocates measured vertical blocks, shrinking overflow from the end.
///
/// The returned vector preserves source indices. Invisible blocks are `None`;
/// visible blocks always return a rectangle, which may have zero height under
/// extreme contraction.
#[must_use]
pub fn panel_stack(area: Rect, blocks: &[PanelStackBlock], gap: u16) -> Vec<Option<Rect>> {
    panel_stack_with_policy(area, blocks, gap, ShrinkPolicy::Tail)
}

/// `panel_stack` with an explicit shortfall distribution [`ShrinkPolicy`].
#[must_use]
pub fn panel_stack_with_policy(
    area: Rect,
    blocks: &[PanelStackBlock],
    gap: u16,
    policy: ShrinkPolicy,
) -> Vec<Option<Rect>> {
    match policy {
        ShrinkPolicy::Tail => panel_stack_tail(area, blocks, gap),
        ShrinkPolicy::Equal => panel_stack_equal(area, blocks, gap),
    }
}

fn panel_stack_equal(area: Rect, blocks: &[PanelStackBlock], gap: u16) -> Vec<Option<Rect>> {
    use ratatui_core::layout::{Constraint, Direction, Layout};

    let constraints: Vec<Constraint> = blocks
        .iter()
        .filter(|block| block.visible)
        .map(|block| Constraint::Length(block.preferred()))
        .collect();
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .spacing(gap)
        .constraints(constraints)
        .split(area);
    let mut rows = rows.iter().copied();
    blocks
        .iter()
        .map(|block| block.visible.then(|| rows.next().unwrap_or_default()))
        .collect()
}

fn panel_stack_tail(area: Rect, blocks: &[PanelStackBlock], gap: u16) -> Vec<Option<Rect>> {
    let visible = blocks.iter().filter(|block| block.visible).count();
    let gaps = u16::try_from(visible.saturating_sub(1))
        .unwrap_or(u16::MAX)
        .saturating_mul(gap)
        .min(area.height);
    let mut heights: Vec<u16> = blocks
        .iter()
        .map(|block| if block.visible { block.preferred() } else { 0 })
        .collect();
    let desired = heights.iter().copied().fold(gaps, u16::saturating_add);
    let mut overflow = desired.saturating_sub(area.height);
    for (block, height) in blocks.iter().zip(&mut heights).rev() {
        if !block.visible || overflow == 0 {
            continue;
        }
        let shrink = (*height).min(overflow);
        *height = height.saturating_sub(shrink);
        overflow = overflow.saturating_sub(shrink);
    }

    let mut y = area.y;
    let mut seen = 0usize;
    blocks
        .iter()
        .enumerate()
        .map(|(index, block)| {
            if !block.visible {
                return None;
            }
            if seen > 0 {
                y = y.saturating_add(gap.min(area.bottom().saturating_sub(y)));
            }
            seen += 1;
            let height = heights[index].min(area.bottom().saturating_sub(y));
            let rect = Rect::new(area.x, y, area.width, height);
            y = y.saturating_add(height);
            Some(rect)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn block(content_rows: u16) -> PanelStackBlock {
        PanelStackBlock {
            content_rows,
            chrome_rows: 2,
            min: 3,
            max: 8,
            visible: true,
        }
    }

    #[test]
    fn invisible_blocks_consume_neither_height_nor_gap() {
        let mut hidden = block(4);
        hidden.visible = false;
        let result = panel_stack(Rect::new(0, 0, 20, 12), &[block(2), hidden, block(2)], 1);
        assert_eq!(result[1], None);
        assert_eq!(result[0].unwrap(), Rect::new(0, 0, 20, 4));
        assert_eq!(result[2].unwrap(), Rect::new(0, 5, 20, 4));
    }

    #[test]
    fn content_plus_chrome_respects_caps() {
        let result = panel_stack(Rect::new(0, 0, 20, 20), &[block(0), block(20)], 1);
        assert_eq!(result[0].unwrap().height, 3);
        assert_eq!(result[1].unwrap().height, 8);
    }

    #[test]
    fn equal_policy_matches_solver_when_nothing_overflows() {
        let blocks = [block(2), block(4), block(1)];
        assert_eq!(
            panel_stack_with_policy(Rect::new(0, 0, 20, 30), &blocks, 1, ShrinkPolicy::Tail),
            panel_stack_with_policy(Rect::new(0, 0, 20, 30), &blocks, 1, ShrinkPolicy::Equal),
        );
    }

    #[test]
    fn equal_policy_distributes_shortfall_like_length_layout() {
        // Equal-strength `Length` relaxation equalizes block heights under
        // overflow instead of zeroing the tail.
        let blocks = [block(10), block(10), block(10)]; // preferred 12 each, area 20
        let result =
            panel_stack_with_policy(Rect::new(0, 0, 20, 20), &blocks, 0, ShrinkPolicy::Equal);
        let heights: Vec<u16> = result.iter().flatten().map(|rect| rect.height).collect();
        assert_eq!(heights, vec![7, 6, 7]);
        assert!(result.iter().flatten().all(|rect| rect.bottom() <= 20));
    }

    #[test]
    fn equal_policy_skips_invisible_blocks() {
        let mut hidden = block(4);
        hidden.visible = false;
        let result = panel_stack_with_policy(
            Rect::new(0, 0, 20, 20),
            &[block(2), hidden, block(2)],
            1,
            ShrinkPolicy::Equal,
        );
        assert_eq!(result[1], None);
        assert_eq!(result[0].unwrap(), Rect::new(0, 0, 20, 4));
        assert_eq!(result[2].unwrap(), Rect::new(0, 5, 20, 4));
    }

    #[test]
    fn overflow_shrinks_last_block_first() {
        let result = panel_stack(Rect::new(0, 0, 20, 10), &[block(4), block(4), block(4)], 1);
        assert_eq!(result[0].unwrap().height, 6);
        assert_eq!(result[1].unwrap().height, 2);
        assert_eq!(result[2].unwrap().height, 0);
        assert!(result.iter().flatten().all(|rect| rect.bottom() <= 10));
    }
}
