use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    text::Line,
    widgets::StatefulWidget,
};
use termrock::{
    Theme,
    input::{KeyCode, KeyEvent, KeyModifiers},
    style::Role,
    widgets::{Tree, TreeNode, TreeNodeStatus, TreeOutcome, TreeState},
};

fn nodes() -> Vec<TreeNode<'static, &'static str>> {
    vec![
        TreeNode {
            id: "root",
            label: Line::from("Workspace"),
            depth: 0,
            branch: true,
            expanded: true,
            enabled: true,
            status: TreeNodeStatus::Ready,
        },
        TreeNode {
            id: "loading",
            label: Line::from("Loading child"),
            depth: 1,
            branch: true,
            expanded: false,
            enabled: false,
            status: TreeNodeStatus::Loading,
        },
        TreeNode {
            id: "leaf",
            label: Line::from("Wide 🧪"),
            depth: 1,
            branch: false,
            expanded: false,
            enabled: true,
            status: TreeNodeStatus::Ready,
        },
    ]
}

#[test]
fn keyboard_navigation_skips_disabled_rows_and_requests_disclosure() {
    let rows = nodes();
    let mut state = TreeState::new(Some("root"));

    assert_eq!(
        state.handle_key(&rows, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
        TreeOutcome::SelectionChanged("leaf")
    );
    assert_eq!(state.selected(), Some(&"leaf"));
    assert_eq!(
        state.handle_key(&rows, KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)),
        TreeOutcome::SelectionChanged("root")
    );
    assert_eq!(
        state.handle_key(&rows, KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)),
        TreeOutcome::Toggle("root")
    );
}

#[test]
fn render_exposes_status_and_only_painted_enabled_rows_are_clickable() {
    let rows = nodes();
    let theme = Theme::default();
    let tree = Tree {
        nodes: &rows,
        theme: &theme,
    };
    let mut state = TreeState::new(Some("root"));
    let area = Rect::new(0, 0, 16, 3);
    let mut buffer = Buffer::empty(area);

    tree.render(area, &mut buffer, &mut state);

    let rendered = buffer
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(rendered.contains("Workspace"));
    assert!(rendered.contains("loading"));
    assert_eq!(
        buffer[(3, 0)].bg,
        theme
            .style(Role::Selection)
            .bg
            .expect("selection background"),
        "selected label must retain the row selection style"
    );
    assert_eq!(state.click(Position::new(0, 1)), TreeOutcome::Ignored);
    assert_eq!(
        state.click(Position::new(4, 2)),
        TreeOutcome::SelectionChanged("leaf")
    );
}

#[test]
fn empty_and_zero_sized_trees_are_safe() {
    let theme = Theme::default();
    let tree: Tree<'_, u8> = Tree {
        nodes: &[],
        theme: &theme,
    };
    let mut state = TreeState::default();
    let mut buffer = Buffer::empty(Rect::new(0, 0, 0, 0));

    tree.render(Rect::new(0, 0, 0, 0), &mut buffer, &mut state);

    assert!(state.regions().is_empty());

    let area = Rect::new(0, 0, 6, 2);
    let mut paintable = Buffer::empty(area);
    tree.render(area, &mut paintable, &mut state);
    assert!(paintable.content().iter().all(|cell| cell.symbol() == " "));
    assert_eq!(state.offset(), 0);
}

#[test]
fn painted_disclosure_and_selected_row_have_distinct_mouse_outcomes() {
    let rows = nodes();
    let theme = Theme::default();
    let tree = Tree {
        nodes: &rows,
        theme: &theme,
    };
    let mut state = TreeState::new(Some("leaf"));
    let area = Rect::new(3, 4, 20, 3);
    let mut buffer = Buffer::empty(Rect::new(0, 0, 24, 8));
    tree.render(area, &mut buffer, &mut state);

    assert_eq!(
        state.click(Position::new(3, 4)),
        TreeOutcome::Toggle("root")
    );
    assert_eq!(
        state.click(Position::new(8, 6)),
        TreeOutcome::Activated("leaf")
    );
    assert_eq!(state.hover(Position::new(8, 6)), Some(&"leaf"));
}

#[test]
fn selected_node_is_scrolled_into_a_bounded_viewport() {
    let rows = vec![
        TreeNode {
            id: 0,
            label: Line::from("zero"),
            depth: 0,
            branch: false,
            expanded: false,
            enabled: true,
            status: TreeNodeStatus::Ready,
        },
        TreeNode {
            id: 1,
            label: Line::from("one"),
            depth: 0,
            branch: false,
            expanded: false,
            enabled: true,
            status: TreeNodeStatus::Error,
        },
        TreeNode {
            id: 2,
            label: Line::from("two"),
            depth: 0,
            branch: false,
            expanded: false,
            enabled: true,
            status: TreeNodeStatus::Ready,
        },
    ];
    let theme = Theme::default();
    let tree = Tree {
        nodes: &rows,
        theme: &theme,
    };
    let mut state = TreeState::new(Some(2));
    let area = Rect::new(0, 0, 10, 1);
    let mut buffer = Buffer::empty(area);

    tree.render(area, &mut buffer, &mut state);

    assert_eq!(state.offset(), 2);
    assert_eq!(state.regions().len(), 1);
    let rendered = buffer
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(rendered.contains("two"));
}

#[test]
fn page_keys_and_scroll_delta_use_the_painted_viewport() {
    let rows = (0..8)
        .map(|id| TreeNode {
            id,
            label: Line::from(format!("node {id}")),
            depth: 0,
            branch: false,
            expanded: false,
            enabled: true,
            status: TreeNodeStatus::Ready,
        })
        .collect::<Vec<_>>();
    let theme = Theme::default();
    let tree = Tree {
        nodes: &rows,
        theme: &theme,
    };
    let mut state = TreeState::new(Some(0));
    let area = Rect::new(0, 0, 12, 3);
    let mut buffer = Buffer::empty(area);
    tree.render(area, &mut buffer, &mut state);
    assert_eq!(buffer[(11, 0)].symbol(), "█");
    assert_eq!(buffer[(11, 2)].symbol(), "│");

    assert_eq!(
        state.handle_key(&rows, KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE)),
        TreeOutcome::SelectionChanged(3)
    );
    assert!(state.scroll_by(2, rows.len()));
    assert_eq!(state.offset(), 2);
    assert!(
        state.scroll_to_position(Position::new(11, 2), rows.len()),
        "painted scrollbar track supports drag/page positioning"
    );
    assert_eq!(state.offset(), 5);
    state.select(Some(7));
    assert_eq!(state.selected(), Some(&7));
}

#[test]
fn focus_gates_input_and_preserves_non_color_selection_cues() {
    let rows = nodes();
    let theme = Theme::default();
    let tree = Tree {
        nodes: &rows,
        theme: &theme,
    };
    let mut state = TreeState::new(Some("root"));
    state.set_focused(false);
    assert_eq!(
        state.handle_key(&rows, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
        TreeOutcome::Ignored
    );
    let area = Rect::new(0, 0, 18, 3);
    let mut buffer = Buffer::empty(area);
    tree.render(area, &mut buffer, &mut state);
    assert!(
        buffer[(3, 0)]
            .modifier
            .contains(ratatui_core::style::Modifier::UNDERLINED),
        "unfocused selection remains visible without color"
    );

    state.set_focused(true);
    assert!(state.is_focused());
    state.hover(Position::new(4, 2));
    tree.render(area, &mut buffer, &mut state);
    assert!(
        buffer[(3, 0)]
            .modifier
            .contains(ratatui_core::style::Modifier::BOLD),
        "focused selection remains visible without color"
    );
    assert!(
        buffer[(4, 2)]
            .modifier
            .contains(ratatui_core::style::Modifier::UNDERLINED),
        "hover is visible without color"
    );
}

#[test]
fn disabled_loading_and_error_rows_have_explicit_semantic_styles() {
    let rows = vec![
        TreeNode {
            id: 0,
            label: Line::from("disabled"),
            depth: 0,
            branch: false,
            expanded: false,
            enabled: false,
            status: TreeNodeStatus::Ready,
        },
        TreeNode {
            id: 1,
            label: Line::from("pending"),
            depth: 0,
            branch: false,
            expanded: false,
            enabled: false,
            status: TreeNodeStatus::Loading,
        },
        TreeNode {
            id: 2,
            label: Line::from("failed"),
            depth: 0,
            branch: false,
            expanded: false,
            enabled: true,
            status: TreeNodeStatus::Error,
        },
    ];
    let theme = Theme::default();
    let tree = Tree {
        nodes: &rows,
        theme: &theme,
    };
    let mut state = TreeState::default();
    let area = Rect::new(0, 0, 20, 3);
    let mut buffer = Buffer::empty(area);
    tree.render(area, &mut buffer, &mut state);

    assert_eq!(
        buffer[(2, 0)].fg,
        theme.style(Role::TextDisabled).fg.unwrap()
    );
    assert!(
        buffer[(2, 0)]
            .modifier
            .contains(ratatui_core::style::Modifier::DIM),
        "disabled rows remain distinct without color"
    );
    assert_eq!(buffer[(2, 1)].fg, theme.style(Role::TextMuted).fg.unwrap());
    assert_eq!(buffer[(2, 2)].fg, theme.style(Role::Danger).fg.unwrap());
    let rendered = buffer
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(rendered.contains("loading"));
    assert!(rendered.contains("error"));
}

#[test]
fn narrow_clipping_never_splits_a_wide_grapheme() {
    let rows = vec![TreeNode {
        id: 0,
        label: Line::from("🧪e\u{301}Z"),
        depth: 0,
        branch: false,
        expanded: false,
        enabled: true,
        status: TreeNodeStatus::Ready,
    }];
    let theme = Theme::default();
    let tree = Tree {
        nodes: &rows,
        theme: &theme,
    };
    let mut state = TreeState::new(Some(0));
    let mut one_cell = Buffer::empty(Rect::new(0, 0, 1, 1));
    tree.render(Rect::new(0, 0, 1, 1), &mut one_cell, &mut state);
    assert_eq!(one_cell[(0, 0)].symbol(), " ");

    let mut four_cells = Buffer::empty(Rect::new(0, 0, 4, 1));
    tree.render(Rect::new(0, 0, 4, 1), &mut four_cells, &mut state);
    assert_eq!(four_cells[(2, 0)].symbol(), "🧪");
    assert_eq!(four_cells[(3, 0)].symbol(), " ");

    let deeply_nested = vec![TreeNode {
        depth: u16::MAX,
        ..rows[0].clone()
    }];
    let deep_tree = Tree {
        nodes: &deeply_nested,
        theme: &theme,
    };
    deep_tree.render(Rect::new(0, 0, 1, 1), &mut one_cell, &mut state);
}

#[test]
fn status_suffix_reserves_space_before_clipping_wide_labels() {
    let rows = vec![TreeNode {
        id: 0,
        label: Line::from("🧪🧪"),
        depth: 0,
        branch: false,
        expanded: false,
        enabled: false,
        status: TreeNodeStatus::Loading,
    }];
    let theme = Theme::default();
    let tree = Tree {
        nodes: &rows,
        theme: &theme,
    };
    let mut state = TreeState::default();
    let area = Rect::new(0, 0, 11, 1);
    let mut buffer = Buffer::empty(area);

    tree.render(area, &mut buffer, &mut state);

    assert_eq!(buffer[(2, 0)].symbol(), " ");
    let rendered = buffer
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(rendered.ends_with(" loading"));
}
