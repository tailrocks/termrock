use junie_tui::core::event::MouseKind;
use junie_tui::core::id::WidgetId;
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::ui::layout::{Split, SplitDir};
use junie_tui::ui::pane_tree::{PaneNode, PaneTree};
use ratatui::layout::{Position, Rect};

use crate::pane_rig::Rig;

const ID: WidgetId = WidgetId::of("t.pane.seam");
const AREA: Rect = Rect::new(0, 0, 60, 12);
const Y: u16 = 5;
const OUTSIDE: Position = Position::new(200, 200);

fn rig() -> Rig {
    let mut tree = PaneTree::new(ID, 1);
    tree.split(1, 2, SplitDir::Horizontal, false);
    Rig::new(tree, AREA)
}

impl Rig {
    pub fn seam_x(&self) -> u16 {
        self.tree.seams()[0].handle.x
    }

    pub fn root_percent(&self) -> u16 {
        match &self.tree.root {
            PaneNode::Split { split, .. } => split.percent,
            PaneNode::Leaf(_) => panic!("the tree is split"),
        }
    }

    pub fn seam_symbol(&mut self) -> String {
        let buf = self.render();
        let handle = self.tree.seams()[0].handle;
        buf[(handle.x, handle.y)].symbol().to_owned()
    }
}

fn rest_glyph() -> String {
    Theme::for_level(ColorLevel::TrueColor)
        .glyphs
        .divider
        .to_owned()
}

#[test]
fn releasing_outside_the_tree_returns_the_seam_to_rest() {
    let mut rig = rig();
    let x = rig.seam_x();
    rig.mouse(MouseKind::Down, Position::new(x, Y));
    rig.mouse(MouseKind::Drag, Position::new(x, Y));
    rig.mouse(MouseKind::Drag, OUTSIDE);
    rig.mouse(MouseKind::Up, OUTSIDE);

    assert_eq!(
        rig.seam_symbol(),
        rest_glyph(),
        "a drag that ends outside the tree leaves the seam at rest"
    );
}

#[test]
fn releasing_over_another_pane_returns_the_seam_to_rest() {
    let mut rig = rig();
    let x = rig.seam_x();
    rig.mouse(MouseKind::Down, Position::new(x, Y));
    rig.mouse(MouseKind::Drag, Position::new(x - 5, Y));
    rig.mouse(MouseKind::Up, Position::new(x - 5, Y));

    assert_eq!(
        rig.seam_symbol(),
        rest_glyph(),
        "a release over a pane (Release after DragEnd) returns the seam to rest"
    );
}

#[test]
fn the_terminal_phases_after_a_drag_do_not_move_the_split_again() {
    let mut rig = rig();
    let x = rig.seam_x();
    rig.mouse(MouseKind::Down, Position::new(x, Y));
    rig.mouse(MouseKind::Drag, Position::new(x + 3, Y));
    rig.render();
    let dragged = rig.root_percent();

    let x = rig.seam_x();
    rig.mouse(MouseKind::Up, Position::new(x, Y));

    assert_eq!(
        rig.root_percent(),
        dragged,
        "DragEnd and the release that follows it keep the dragged ratio"
    );
    assert_eq!(rig.seam_symbol(), rest_glyph());
}

#[test]
fn splitting_the_tree_during_a_drag_cancels_the_grab() {
    let mut rig = rig();
    let x = rig.seam_x();
    rig.mouse(MouseKind::Down, Position::new(x, Y));
    rig.mouse(MouseKind::Drag, Position::new(x + 2, Y));

    rig.tree.split(2, 3, SplitDir::Vertical, false);
    rig.render();
    let before = rig.root_percent();
    let handle = rig.tree.seams()[0].handle;

    rig.mouse(MouseKind::Drag, Position::new(handle.x + 4, Y));

    assert_eq!(
        rig.root_percent(),
        before,
        "a drag still held after a split does not resize the seam at the same index"
    );
    assert_eq!(
        rig.seam_symbol(),
        rest_glyph(),
        "the seam at the reused index is not drawn pressed"
    );
}

#[test]
fn replacing_the_root_during_a_drag_does_not_retarget_the_grab() {
    let mut rig = rig();
    let x = rig.seam_x();
    rig.mouse(MouseKind::Down, Position::new(x, Y));

    rig.tree.root = PaneNode::Split {
        dir: SplitDir::Horizontal,
        split: Split::new(50, 1, 1),
        first: Box::new(PaneNode::Leaf(7)),
        second: Box::new(PaneNode::Leaf(8)),
    };
    rig.render();
    let before = rig.root_percent();

    rig.mouse(MouseKind::Drag, Position::new(x + 5, Y));
    rig.mouse(MouseKind::Up, Position::new(x + 5, Y));

    assert_eq!(
        rig.root_percent(),
        before,
        "a held seam does not resize a replacement tree with different panes"
    );
}

#[test]
fn a_gesture_after_a_cancelled_release_starts_with_its_own_grab() {
    let mut rig = rig();
    let x = rig.seam_x();
    rig.mouse(MouseKind::Down, Position::new(x, Y));
    rig.mouse(MouseKind::Drag, Position::new(x + 2, Y));
    rig.mouse(MouseKind::Up, OUTSIDE);
    rig.render();

    let x = rig.seam_x();
    rig.mouse(MouseKind::Down, Position::new(x, Y));
    rig.mouse(MouseKind::Drag, Position::new(x + 2, Y));
    rig.mouse(MouseKind::Up, Position::new(x + 2, Y));
    rig.render();

    assert_eq!(
        rig.seam_x(),
        x + 2,
        "the second drag moves the seam by its own distance"
    );
}

#[test]
fn a_real_double_click_resets_the_split_but_a_cancelled_release_does_not() {
    let mut rig = rig();
    let base = rig.root_percent();
    let x = rig.seam_x();
    rig.mouse(MouseKind::Down, Position::new(x, Y));
    rig.mouse(MouseKind::Drag, Position::new(x + 4, Y));
    rig.mouse(MouseKind::Up, OUTSIDE);
    rig.render();
    let moved = rig.root_percent();
    assert_ne!(moved, base, "the drag moved the split");

    assert_eq!(
        rig.root_percent(),
        moved,
        "the cancelled release does not reset the split"
    );

    let x = rig.seam_x();
    for _ in 0..2 {
        rig.mouse(MouseKind::Down, Position::new(x, Y));
        rig.mouse(MouseKind::Up, Position::new(x, Y));
    }
    assert_eq!(
        rig.root_percent(),
        base,
        "a real double click resets the split to its default"
    );
}

#[test]
fn a_seam_still_under_the_pointer_after_release_shows_hover() {
    let mut rig = rig();
    let x = rig.seam_x();
    rig.mouse(MouseKind::Down, Position::new(x, Y));
    rig.mouse(MouseKind::Drag, Position::new(x, Y));
    rig.mouse(MouseKind::Up, Position::new(x, Y));

    let hover = Theme::for_level(ColorLevel::TrueColor)
        .glyphs
        .scroll_thumb
        .to_owned();
    assert_eq!(
        rig.seam_symbol(),
        hover,
        "after release the seam under the pointer is drawn as hovered, not pressed or at rest"
    );
}
