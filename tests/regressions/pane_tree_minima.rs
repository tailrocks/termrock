use junie_tui::core::event::{MouseKind, Outcome};
use junie_tui::core::id::WidgetId;
use junie_tui::ui::layout::{Split, SplitDir};
use junie_tui::ui::pane_tree::{Direction, Frames, PaneId, PaneNode, PaneRefusal, PaneTree};
use ratatui::layout::{Position, Rect};

use crate::pane_rig::Rig;

const ID: WidgetId = WidgetId::of("t.pane.minima");

fn two(dir: SplitDir, min: (u16, u16)) -> PaneTree {
    let mut tree = PaneTree::new(ID, 1).min_size(min.0, min.1);
    tree.split(1, 2, dir, false);
    tree.focused = 1;
    tree
}

fn nested(min: (u16, u16)) -> PaneTree {
    let mut tree = PaneTree::new(ID, 1).min_size(min.0, min.1);
    tree.split(1, 2, SplitDir::Horizontal, false);
    tree.split(2, 3, SplitDir::Horizontal, false);
    tree.focused = 1;
    tree
}

fn node(dir: SplitDir, split: Split, first: PaneNode, second: PaneNode) -> PaneNode {
    PaneNode::Split {
        dir,
        split,
        first: Box::new(first),
        second: Box::new(second),
    }
}

fn root_split(tree: &PaneTree) -> Split {
    match &tree.root {
        PaneNode::Split { split, .. } => *split,
        PaneNode::Leaf(_) => panic!("the tree is split"),
    }
}

fn assert_all_at_least(rects: &[Rect], min: (u16, u16), why: &str) {
    for r in rects {
        assert!(
            r.width >= min.0 && r.height >= min.1,
            "{why}: pane {r:?} is smaller than {min:?}"
        );
    }
}

#[test]
fn pane_pointer_respects_outer_leaf_minimum() {
    let min = (12, 3);
    for dir in [SplitDir::Horizontal, SplitDir::Vertical] {
        let area = Rect::new(0, 0, 80, 20);
        let far = |rig: &Rig, end: bool| match (dir, end) {
            (SplitDir::Horizontal, false) => Position::new(0, rig.handle_origin(0).y),
            (SplitDir::Horizontal, true) => {
                Position::new(rig.area.right() - 1, rig.handle_origin(0).y)
            }
            (SplitDir::Vertical, false) => Position::new(rig.handle_origin(0).x, 0),
            (SplitDir::Vertical, true) => {
                Position::new(rig.handle_origin(0).x, rig.area.bottom() - 1)
            }
        };
        for end in [false, true] {
            let mut pointer = Rig::new(two(dir, min), area);
            let to = far(&pointer, end);
            pointer.drag_seam(0, to);
            assert_all_at_least(
                &pointer.rects(),
                min,
                &format!(
                    "{dir:?} pointer drag to the {} extreme",
                    if end { "far" } else { "near" }
                ),
            );

            let mut keys = Rig::new(two(dir, min), area);
            let toward = match (dir, end) {
                (SplitDir::Horizontal, false) => Direction::Left,
                (SplitDir::Horizontal, true) => Direction::Right,
                (SplitDir::Vertical, false) => Direction::Up,
                (SplitDir::Vertical, true) => Direction::Down,
            };
            assert_eq!(keys.tree.resize(toward, 500), Ok(Outcome::Changed));
            keys.render();
            assert_all_at_least(&keys.rects(), min, "keyboard resize");
            assert_eq!(
                pointer.rects(),
                keys.rects(),
                "{dir:?}: the pointer and the keyboard stop at the same boundary"
            );
        }
    }
}

#[test]
fn pane_pointer_drag_at_the_minimum_reports_changed_once_then_consumed() {
    let min = (12, 3);
    for dir in [SplitDir::Horizontal, SplitDir::Vertical] {
        let mut rig = Rig::new(two(dir, min), Rect::new(0, 0, 80, 20));
        let from = rig.handle_origin(0);
        let far = match dir {
            SplitDir::Horizontal => Position::new(0, from.y),
            SplitDir::Vertical => Position::new(from.x, 0),
        };
        rig.mouse(MouseKind::Down, from);

        assert_eq!(
            rig.mouse(MouseKind::Drag, far),
            Outcome::Changed,
            "{dir:?}: the first drag to the extreme moves the seam to the minimum"
        );
        rig.render();
        let at_minimum = rig.tree.root.clone();
        assert_all_at_least(&rig.rects(), min, "the first drag stops at the minimum");

        for again in [far, far] {
            assert_eq!(
                rig.mouse(MouseKind::Drag, again),
                Outcome::Consumed,
                "{dir:?}: a repeated drag at the minimum is consumed, not a change"
            );
            rig.render();
            assert_eq!(rig.tree.root, at_minimum, "{dir:?}: nothing moved");
        }
        rig.mouse(MouseKind::Up, far);
    }
}

#[test]
fn pane_nested_minimum_includes_descendants_and_seams() {
    let min = (12, 3);
    let mut rig = Rig::new(nested(min), Rect::new(0, 0, 80, 10));
    let y = rig.handle_origin(0).y;
    rig.drag_seam(0, Position::new(79, y));

    let rects: Vec<Rect> = rig.rects();
    assert_all_at_least(&rects, min, "pointer drag across a nested subtree");
    let parent = rig.tree.seams()[0].handle;
    assert_eq!(
        79 - parent.x,
        2 * min.0 + 1,
        "the parent stops at 2 * leaf minimum + the nested seam"
    );

    let mut keys = Rig::new(nested(min), Rect::new(0, 0, 80, 10));
    assert_eq!(
        keys.tree.resize(Direction::Right, 500),
        Ok(Outcome::Changed)
    );
    keys.render();
    assert_all_at_least(
        &keys.rects(),
        min,
        "keyboard resize across a nested subtree",
    );
    assert_eq!(rects, keys.rects());
    assert_eq!(
        keys.tree.resize(Direction::Right, 1),
        Err(PaneRefusal::AtMinimum),
        "one more cell would shrink a descendant below its minimum"
    );
}

#[test]
fn pane_frames_do_not_change_minimum_definition() {
    let min = (12, 5);
    let area = Rect::new(0, 0, 80, 12);
    let mut always = Rig::new(two(SplitDir::Horizontal, min).frames(Frames::Always), area);
    let mut never = Rig::new(two(SplitDir::Horizontal, min).frames(Frames::Never), area);
    for rig in [&mut always, &mut never] {
        let y = rig.handle_origin(0).y;
        rig.drag_seam(0, Position::new(0, y));
    }

    assert_eq!(
        always.rects(),
        never.rects(),
        "the minimum is the outer rectangle with and without frames"
    );
    assert_all_at_least(&always.rects(), min, "framed");
    assert_ne!(
        always.inner, never.inner,
        "the returned content rectangles still differ by the frame"
    );
    let (framed, bare) = (always.inner[0].1, never.inner[0].1);
    assert_eq!(framed.width, bare.width.saturating_sub(2));
    assert_eq!(framed.height, bare.height.saturating_sub(2));
}

#[test]
fn pane_infeasible_terminal_freezes_resize_and_recovers() {
    let min = (12, 3);
    let mut rig = Rig::new(nested(min), Rect::new(0, 0, 80, 10));
    rig.area = Rect::new(0, 0, 30, 10);
    rig.render();
    let frozen = rig.tree.root.clone();

    assert_eq!(
        rig.tree.resize(Direction::Right, 2),
        Err(PaneRefusal::AtMinimum)
    );
    assert_eq!(
        rig.tree.resize(Direction::Left, 2),
        Err(PaneRefusal::AtMinimum)
    );
    let y = rig.handle_origin(0).y;
    rig.drag_seam(0, Position::new(20, y));
    rig.double_click_seam(0);
    assert_eq!(
        rig.tree.root, frozen,
        "an impossible terminal freezes every split"
    );

    rig.area = Rect::new(0, 0, 80, 10);
    rig.render();
    assert_eq!(rig.tree.resize(Direction::Right, 2), Ok(Outcome::Changed));
    rig.render();
    assert_all_at_least(&rig.rects(), min, "after the terminal grew");
}

#[test]
fn pane_extreme_minimum_values_never_panic_or_invent_space() {
    for min in [(0, 0), (1, 1), (12, 3), (40, 9), (u16::MAX, u16::MAX)] {
        for dir in [SplitDir::Horizontal, SplitDir::Vertical] {
            let mut rig = Rig::new(two(dir, min), Rect::new(0, 0, 80, 20));
            let before = rig.tree.root.clone();
            let feasible = min.0 <= 39 && min.1 <= 9;
            let check = |rig: &Rig, step: &str| {
                if min == (u16::MAX, u16::MAX) {
                    assert_eq!(rig.tree.root, before, "{step}: MAX minima freeze the split");
                } else if feasible {
                    assert_all_at_least(&rig.rects(), min, step);
                }
            };
            rig.drag_seam(0, Position::new(0, 0));
            check(&rig, "drag to the near corner");
            rig.drag_seam(0, Position::new(79, 19));
            check(&rig, "drag to the far corner");
            let _ = rig.tree.resize(Direction::Left, u16::MAX);
            let _ = rig.tree.resize(Direction::Up, u16::MAX);
            rig.render();
            check(&rig, "keyboard with the largest step");
            rig.double_click_seam(0);
            check(&rig, "double-click reset");
        }
    }
}

#[test]
fn pane_rejected_candidate_restores_precise_state() {
    let min = (12, 3);
    let mut rig = Rig::new(two(SplitDir::Horizontal, min), Rect::new(7, 2, 80, 10));
    let y = rig.handle_origin(0).y;
    rig.drag_seam(0, Position::new(40, y));
    let precise = rig.tree.root.clone();
    assert_ne!(
        root_split(&rig.tree),
        Split::new(50, 1, 1),
        "the drag left a precise basis-point state"
    );

    rig.drag_seam(0, Position::new(7, y));
    let at_minimum = rig.tree.root.clone();
    assert_all_at_least(&rig.rects(), min, "dragged to the minimum");

    rig.drag_seam(0, Position::new(0, y));
    assert_eq!(
        rig.tree.root, at_minimum,
        "a drag that cannot move the seam leaves the whole Split untouched"
    );
    assert_eq!(
        rig.tree.resize(Direction::Left, 3),
        Err(PaneRefusal::AtMinimum)
    );
    assert_eq!(
        rig.tree.root, at_minimum,
        "a refused key press leaves the whole Split untouched"
    );
    assert_ne!(precise, at_minimum);
}

#[test]
fn pane_minimum_boundary_not_representable_refuses_atomically() {
    let min = (2, 1);
    let chain = |from: PaneId, leaves: PaneId| {
        let mut node = PaneNode::Leaf(from + leaves - 1);
        for id in (from..from + leaves - 1).rev() {
            node = self::node(
                SplitDir::Horizontal,
                Split::new(1, 2, 2),
                PaneNode::Leaf(id),
                node,
            );
        }
        node
    };
    let first = chain(1, 33);
    let mut tree = PaneTree::new(ID, 1).min_size(min.0, min.1);
    tree.root = node(
        SplitDir::Horizontal,
        Split::new(98, 1, 1),
        first,
        PaneNode::Leaf(100),
    );
    let mut rig = Rig::new(tree, Rect::new(0, 0, 101, 3));
    assert_all_at_least(
        &rig.rects(),
        min,
        "the 98/2 requirement is feasible as stored",
    );
    let before = rig.tree.root.clone();
    let y = rig.handle_origin(0).y;

    rig.drag_seam(0, Position::new(100, y));
    assert_eq!(
        rig.tree.root, before,
        "the 5-95% ratio cannot express 98/2, so the drag is refused whole"
    );
    assert_all_at_least(&rig.rects(), min, "no undersized leaf on the next render");

    rig.tree.focused = 1;
    assert_eq!(
        rig.tree.resize(Direction::Right, 1),
        Err(PaneRefusal::AtMinimum)
    );
    assert_eq!(rig.tree.root, before);
}

#[test]
fn pane_double_click_reset_obeys_minimum() {
    let min = (24, 3);
    let area = Rect::new(0, 0, 80, 10);
    let low_default = || {
        let mut tree = PaneTree::new(ID, 1).min_size(min.0, min.1);
        tree.root = node(
            SplitDir::Horizontal,
            Split::new(20, 1, 1),
            PaneNode::Leaf(1),
            PaneNode::Leaf(2),
        );
        tree
    };
    let mut rig = Rig::new(low_default(), area);
    rig.drag_seam(0, Position::new(40, rig.handle_origin(0).y));
    let dragged = rig.tree.root.clone();
    assert_all_at_least(&rig.rects(), min, "the drag stays legal");

    rig.double_click_seam(0);
    assert_eq!(
        rig.tree.root, dragged,
        "the 20% default would make the first pane 15 columns, below the 24 minimum"
    );
    assert_all_at_least(&rig.rects(), min, "after the refused reset");

    let mut valid = Rig::new(two(SplitDir::Horizontal, (12, 3)), area);
    let base = valid.tree.root.clone();
    valid.drag_seam(0, Position::new(20, valid.handle_origin(0).y));
    assert_ne!(valid.tree.root, base);
    valid.double_click_seam(0);
    assert_eq!(
        root_split(&valid.tree).percent,
        50,
        "a reset that respects the minimum still works"
    );
    assert_all_at_least(&valid.rects(), (12, 3), "after the valid reset");
}

#[test]
fn pane_min_size_change_cannot_bypass_the_invariant() {
    let mut rig = Rig::new(two(SplitDir::Horizontal, (12, 3)), Rect::new(0, 0, 80, 10));
    rig.tree.min_size = (30, 3);
    let y = rig.handle_origin(0).y;
    rig.drag_seam(0, Position::new(0, y));
    assert_all_at_least(
        &rig.rects(),
        (30, 3),
        "a later, larger min_size applies at once",
    );
}
