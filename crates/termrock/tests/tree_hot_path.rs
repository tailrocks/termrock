#![allow(missing_docs)]

use std::{
    alloc::System,
    hint::black_box,
    time::{Duration, Instant},
};

use ratatui_core::{buffer::Buffer, layout::Rect, text::Line, widgets::StatefulWidget};
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use termrock::{
    Theme,
    widgets::{Tree, TreeNode, TreeNodeStatus, TreeState},
};

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

#[test]
fn warmed_large_tree_viewport_render_is_bounded_and_allocation_free() {
    const NODE_COUNT: usize = 10_000;
    const VIEWPORT_HEIGHT: u16 = 40;
    const SAMPLES: usize = 100;

    let nodes = (0..NODE_COUNT)
        .map(|id| TreeNode {
            id,
            label: Line::from("resident node"),
            trailing: None,
            depth: u16::try_from(id % 4).unwrap(),
            branch: id % 7 == 0,
            expanded: id % 14 == 0,
            enabled: true,
            status: TreeNodeStatus::Ready,
        })
        .collect::<Vec<_>>();
    let theme = Theme::default();
    let tree = Tree::new(&nodes, &theme);
    let area = Rect::new(0, 0, 120, VIEWPORT_HEIGHT);
    let mut buffer = Buffer::empty(area);
    let mut state = TreeState::new(Some(NODE_COUNT - 1));

    tree.render(area, &mut buffer, &mut state);
    assert_eq!(state.regions().len(), usize::from(VIEWPORT_HEIGHT));

    let allocations = Region::new(GLOBAL);
    let started = Instant::now();
    for _ in 0..SAMPLES {
        tree.render(area, black_box(&mut buffer), black_box(&mut state));
    }
    let elapsed = started.elapsed();
    let change = allocations.change();

    assert_eq!(
        change.allocations, 0,
        "warmed renders allocated: {change:?}"
    );
    assert_eq!(
        change.reallocations, 0,
        "warmed renders reallocated: {change:?}"
    );
    assert_eq!(state.regions().len(), usize::from(VIEWPORT_HEIGHT));
    assert!(
        elapsed <= Duration::from_millis(250),
        "100 warmed renders exceeded the 250 ms debug-profile budget: {elapsed:?}"
    );
    eprintln!(
        "tree hot path: {SAMPLES} renders, {NODE_COUNT} nodes, {VIEWPORT_HEIGHT} visible, {elapsed:?}, {change:?}"
    );
}
