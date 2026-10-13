//! Integration coverage for tree rendering and interaction.

use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    text::Line,
    widgets::StatefulWidget,
};
use termrock::{
    input::{KeyCode, KeyEvent, KeyModifiers},
    style::{Density, DesignSystem, Role, RolePalette, SelectionChrome},
    widgets::{Tree, TreeNode, TreeNodeStatus, TreeOutcome, TreeState},
};

fn nodes() -> Vec<TreeNode<'static, &'static str>> {
    vec![
        TreeNode {
            id: "root",
            label: Line::from("Workspace"),
            leading: None,
            secondary: None,
            badge: None,
            shortcut: None,
            trailing: None,
            depth: 0,
            branch: true,
            expanded: true,
            enabled: true,
            status: TreeNodeStatus::Ready,
            tone: termrock::widgets::ToneTier::Primary,
            actions: None,
            parent: None,
        },
        TreeNode {
            id: "loading",
            label: Line::from("Loading child"),
            leading: None,
            secondary: None,
            badge: None,
            shortcut: None,
            trailing: None,
            depth: 1,
            branch: true,
            expanded: false,
            enabled: false,
            status: TreeNodeStatus::Loading,
            tone: termrock::widgets::ToneTier::Primary,
            actions: None,
            parent: None,
        },
        TreeNode {
            id: "leaf",
            label: Line::from("Wide 🧪"),
            leading: None,
            secondary: None,
            badge: None,
            shortcut: None,
            trailing: None,
            depth: 1,
            branch: false,
            expanded: false,
            enabled: true,
            status: TreeNodeStatus::Ready,
            tone: termrock::widgets::ToneTier::Primary,
            actions: None,
            parent: None,
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
    let tokens = DesignSystem::default();
    let rows = nodes();
    let tree = Tree::new(&rows, &tokens);
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
    // Quiet phosphor: selection is recipe/gutter + non-color modifiers, not fill bg.
    assert!(
        buffer[(3, 0)]
            .modifier
            .contains(ratatui_core::style::Modifier::UNDERLINED)
            || buffer[(0, 0)].symbol() == "▌"
            || buffer[(3, 0)]
                .modifier
                .contains(ratatui_core::style::Modifier::BOLD),
        "selected row remains visible without Selection fill"
    );
    assert_eq!(state.click(Position::new(0, 1)), TreeOutcome::Ignored);
    assert_eq!(
        state.click(Position::new(4, 2)),
        TreeOutcome::SelectionChanged("leaf")
    );
}

#[test]
fn empty_and_zero_sized_trees_are_safe() {
    let tokens = DesignSystem::default();
    let tree: Tree<'_, u8> = Tree::new(&[], &tokens);
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
    // Fill selection (not the gutter default) so disclosure column geometry
    // matches the historical hit-test expectations for this regression.
    let tokens = DesignSystem::new(RolePalette::default(), Density::default())
        .selection(SelectionChrome::Fill);
    let rows = nodes();
    let tree = Tree::new(&rows, &tokens);
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
    let tokens = DesignSystem::default();
    let rows = vec![
        TreeNode {
            id: 0,
            label: Line::from("zero"),
            leading: None,
            secondary: None,
            badge: None,
            shortcut: None,
            trailing: None,
            depth: 0,
            branch: false,
            expanded: false,
            enabled: true,
            status: TreeNodeStatus::Ready,
            tone: termrock::widgets::ToneTier::Primary,
            actions: None,
            parent: None,
        },
        TreeNode {
            id: 1,
            label: Line::from("one"),
            leading: None,
            secondary: None,
            badge: None,
            shortcut: None,
            trailing: None,
            depth: 0,
            branch: false,
            expanded: false,
            enabled: true,
            status: TreeNodeStatus::Error,
            tone: termrock::widgets::ToneTier::Primary,
            actions: None,
            parent: None,
        },
        TreeNode {
            id: 2,
            label: Line::from("two"),
            leading: None,
            secondary: None,
            badge: None,
            shortcut: None,
            trailing: None,
            depth: 0,
            branch: false,
            expanded: false,
            enabled: true,
            status: TreeNodeStatus::Ready,
            tone: termrock::widgets::ToneTier::Primary,
            actions: None,
            parent: None,
        },
    ];
    let tree = Tree::new(&rows, &tokens);
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
    let tokens = DesignSystem::default();
    let rows = (0..8)
        .map(|id| TreeNode {
            id,
            label: Line::from(format!("node {id}")),
            leading: None,
            secondary: None,
            badge: None,
            shortcut: None,
            trailing: None,
            depth: 0,
            branch: false,
            expanded: false,
            enabled: true,
            status: TreeNodeStatus::Ready,
            tone: termrock::widgets::ToneTier::Primary,
            actions: None,
            parent: None,
        })
        .collect::<Vec<_>>();
    let tree = Tree::new(&rows, &tokens);
    let mut state = TreeState::new(Some(0));
    let area = Rect::new(0, 0, 12, 3);
    let mut buffer = Buffer::empty(area);
    tree.render(area, &mut buffer, &mut state);
    // One scrollbar language across every scroll surface (plans/022 Step 5).
    assert_eq!(buffer[(11, 0)].symbol(), "┃");
    assert_eq!(buffer[(11, 2)].symbol(), "·");

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
fn host_focus_chrome_preserves_non_color_selection_cues() {
    // Key gating is host-owned; paint focus is List/Tree::focused(bool).
    let tokens = DesignSystem::default();
    let rows = nodes();
    let mut state = TreeState::new(Some("root"));
    let area = Rect::new(0, 0, 18, 3);
    let mut buffer = Buffer::empty(area);
    Tree::new(&rows, &tokens)
        .focused(false)
        .render(area, &mut buffer, &mut state);
    let gutter = tokens.glyphs.selection_gutter();
    assert_eq!(
        buffer[(0, 0)].symbol(),
        gutter,
        "unfocused selection remains visible without color"
    );
    state.hover(Position::new(4, 2));
    Tree::new(&rows, &tokens)
        .focused(true)
        .render(area, &mut buffer, &mut state);
    assert_eq!(buffer[(0, 0)].symbol(), gutter);
    assert!(
        buffer[(3, 0)]
            .modifier
            .contains(ratatui_core::style::Modifier::BOLD),
        "focused selection remains visible without color"
    );
    assert_eq!(
        buffer[(4, 2)].bg,
        tokens
            .style(Role::HoverTint)
            .bg
            .expect("hover wash carries a background"),
        "hover lifts the row instead of underlining it"
    );
}

#[test]
fn disabled_loading_and_error_rows_have_explicit_semantic_styles() {
    let tokens = DesignSystem::default();
    let theme = &tokens.palette;
    let rows = vec![
        TreeNode {
            id: 0,
            label: Line::from("disabled"),
            leading: None,
            secondary: None,
            badge: None,
            shortcut: None,
            trailing: None,
            depth: 0,
            branch: false,
            expanded: false,
            enabled: false,
            status: TreeNodeStatus::Ready,
            tone: termrock::widgets::ToneTier::Primary,
            actions: None,
            parent: None,
        },
        TreeNode {
            id: 1,
            label: Line::from("pending"),
            leading: None,
            secondary: None,
            badge: None,
            shortcut: None,
            trailing: None,
            depth: 0,
            branch: false,
            expanded: false,
            enabled: false,
            status: TreeNodeStatus::Loading,
            tone: termrock::widgets::ToneTier::Primary,
            actions: None,
            parent: None,
        },
        TreeNode {
            id: 2,
            label: Line::from("failed"),
            leading: None,
            secondary: None,
            badge: None,
            shortcut: None,
            trailing: None,
            depth: 0,
            branch: false,
            expanded: false,
            enabled: true,
            status: TreeNodeStatus::Error,
            tone: termrock::widgets::ToneTier::Primary,
            actions: None,
            parent: None,
        },
    ];
    let tree = Tree::new(&rows, &tokens);
    let mut state = TreeState::default();
    let area = Rect::new(0, 0, 20, 3);
    let mut buffer = Buffer::empty(area);
    tree.render(area, &mut buffer, &mut state);

    let rendered = buffer
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(rendered.contains("disabled"), "{rendered:?}");
    assert!(rendered.contains("loading"), "{rendered:?}");
    assert!(
        rendered.contains("error") || rendered.contains("failed"),
        "{rendered:?}"
    );
    // Semantic styles: disabled dim, loading muted, error danger — scan primary label cells.
    let find_fg = |needle: &str| -> Option<ratatui_core::style::Color> {
        for y in 0..3 {
            let row: String = (0..20)
                .map(|x| buffer[(x, y)].symbol().to_string())
                .collect();
            if let Some(idx) = row.find(needle) {
                return Some(buffer[(u16::try_from(idx).unwrap(), y)].fg);
            }
        }
        None
    };
    assert_eq!(find_fg("disabled"), theme.style(Role::TextDisabled).fg);
    assert_eq!(find_fg("loading"), theme.style(Role::TextMuted).fg);
    assert_eq!(find_fg("failed"), theme.style(Role::Danger).fg);
}

#[test]
fn narrow_clipping_never_splits_a_wide_grapheme() {
    let tokens = DesignSystem::default();
    let rows = vec![TreeNode {
        id: 0,
        label: Line::from("🧪e\u{301}Z"),
        leading: None,
        secondary: None,
        badge: None,
        shortcut: None,
        trailing: None,
        depth: 0,
        branch: false,
        expanded: false,
        enabled: true,
        status: TreeNodeStatus::Ready,
        tone: termrock::widgets::ToneTier::Primary,
        actions: None,
        parent: None,
    }];
    let tree = Tree::new(&rows, &tokens);
    let mut state = TreeState::new(Some(0));
    let mut one_cell = Buffer::empty(Rect::new(0, 0, 1, 1));
    tree.render(Rect::new(0, 0, 1, 1), &mut one_cell, &mut state);
    // Selected row may paint gutter/disclosure in the only cell — never a half emoji.
    let one = one_cell[(0, 0)].symbol();
    assert!(
        one == " " || one == "▌" || one == ">" || one.chars().count() == 1,
        "single cell must not split wide graphemes: {one:?}"
    );
    assert_ne!(
        one, "🧪",
        "wide emoji must not paint into a 1-cell clip alone mid-split"
    );

    let mut four_cells = Buffer::empty(Rect::new(0, 0, 8, 1));
    tree.render(Rect::new(0, 0, 8, 1), &mut four_cells, &mut state);
    let row: String = (0..8)
        .map(|x| four_cells[(x, 0)].symbol().to_string())
        .collect();
    // Emoji fully present or absent — never half.
    let emoji_count = row.matches('🧪').count();
    assert!(emoji_count <= 1, "must not split emoji: {row:?}");

    let deeply_nested = vec![TreeNode {
        depth: u16::MAX,
        ..rows[0].clone()
    }];
    let deep_tree = Tree::new(&deeply_nested, &tokens);
    deep_tree.render(Rect::new(0, 0, 1, 1), &mut one_cell, &mut state);
}

#[test]
fn status_suffix_reserves_space_before_clipping_wide_labels() {
    let tokens = DesignSystem::default();
    let rows = vec![TreeNode {
        id: 0,
        label: Line::from("🧪🧪"),
        leading: None,
        secondary: None,
        badge: None,
        shortcut: None,
        trailing: None,
        depth: 0,
        branch: false,
        expanded: false,
        enabled: false,
        status: TreeNodeStatus::Loading,
        tone: termrock::widgets::ToneTier::Primary,
        actions: None,
        parent: None,
    }];
    let tree = Tree::new(&rows, &tokens);
    let mut state = TreeState::default();
    let area = Rect::new(0, 0, 11, 1);
    let mut buffer = Buffer::empty(area);

    tree.render(area, &mut buffer, &mut state);

    let rendered = buffer
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(
        rendered.contains("loading"),
        "loading status remains visible: {rendered:?}"
    );
    // Wide graphemes in the label must not be split mid-cell.
    let emoji = rendered.matches('🧪').count();
    assert!(emoji <= 2, "{rendered:?}");
}

#[test]
fn trailing_cells_align_right_and_preserve_wide_metadata() {
    let tokens = DesignSystem::default();
    let rows = vec![
        TreeNode {
            id: 0,
            label: Line::from("🧪🧪label"),
            leading: None,
            secondary: None,
            badge: None,
            shortcut: None,
            trailing: Some(Line::from("12 KiB")),
            depth: 0,
            branch: false,
            expanded: false,
            enabled: true,
            status: TreeNodeStatus::Ready,
            tone: termrock::widgets::ToneTier::Primary,
            actions: None,
            parent: None,
        },
        TreeNode {
            id: 1,
            label: Line::from("short"),
            leading: None,
            secondary: None,
            badge: None,
            shortcut: None,
            trailing: Some(Line::from("1 B")),
            depth: 0,
            branch: false,
            expanded: false,
            enabled: true,
            status: TreeNodeStatus::Ready,
            tone: termrock::widgets::ToneTier::Primary,
            actions: None,
            parent: None,
        },
    ];
    let tree = Tree::new(&rows, &tokens);
    let mut state = TreeState::default();
    let area = Rect::new(0, 0, 16, 2);
    let mut buffer = Buffer::empty(area);

    tree.render(area, &mut buffer, &mut state);

    let row0: String = (0..16)
        .map(|x| buffer[(x, 0)].symbol().to_string())
        .collect();
    let row1: String = (0..16)
        .map(|x| buffer[(x, 1)].symbol().to_string())
        .collect();
    assert!(row0.contains("12") && row0.contains('B'), "{row0:?}");
    assert!(row1.contains('1') && row1.contains('B'), "{row1:?}");
    assert!(row0.contains('🧪'), "{row0:?}");
}

#[test]
fn narrow_trailing_cell_clips_wide_graphemes_and_separates_status() {
    let tokens = DesignSystem::default();
    let narrow_rows = [TreeNode {
        id: 0,
        label: Line::from("hidden"),
        leading: None,
        secondary: None,
        badge: None,
        shortcut: None,
        trailing: Some(Line::from("🧪Z")),
        depth: 0,
        branch: false,
        expanded: false,
        enabled: true,
        status: TreeNodeStatus::Ready,
        tone: termrock::widgets::ToneTier::Primary,
        actions: None,
        parent: None,
    }];
    let mut state = TreeState::default();
    // Disclosure glyph + content: badge contracts; wide emoji never splits.
    let narrow_area = Rect::new(0, 0, 6, 1);
    let mut narrow = Buffer::empty(narrow_area);
    Tree::new(&narrow_rows, &tokens).render(narrow_area, &mut narrow, &mut state);
    let text: String = (0..6)
        .map(|x| narrow[(x, 0)].symbol().to_string())
        .collect();
    let emoji = text.matches('🧪').count();
    assert!(emoji <= 1, "{text:?}");
    if emoji == 1 {
        assert!(!text.contains('Z'), "{text:?}");
    }

    let combined_rows = [TreeNode {
        id: 1,
        label: Line::from("job"),
        leading: None,
        secondary: None,
        badge: None,
        shortcut: None,
        trailing: Some(Line::from("7 B")),
        depth: 0,
        branch: false,
        expanded: false,
        enabled: true,
        status: TreeNodeStatus::Loading,
        tone: termrock::widgets::ToneTier::Primary,
        actions: None,
        parent: None,
    }];
    let combined_area = Rect::new(0, 0, 20, 1);
    let mut combined = Buffer::empty(combined_area);
    Tree::new(&combined_rows, &tokens).render(combined_area, &mut combined, &mut state);
    let rendered: String = combined
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(
        rendered.contains("loading") && rendered.contains("7 B"),
        "status + trailing badge both visible: {rendered:?}"
    );
}

#[test]
fn multi_select_toggles_by_space_and_painted_checkbox() {
    let tokens = DesignSystem::default();
    let rows = nodes();
    let tree = Tree::new(&rows, &tokens);
    let mut state = TreeState::new(Some("root"));
    state.enable_multi_select();

    assert_eq!(
        state.handle_key(&rows, KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)),
        TreeOutcome::CheckToggled("root")
    );
    let area = Rect::new(0, 0, 24, 3);
    let mut buffer = Buffer::empty(area);
    tree.render(area, &mut buffer, &mut state);
    let row0: String = (0..24)
        .map(|x| buffer[(x, 0)].symbol().to_string())
        .collect();
    assert!(
        row0.contains('☑') || row0.contains('[') || row0.contains('x'),
        "multi-select check chrome: {row0:?}"
    );
    // Toggle leaf via Space after selecting it (check hit regions vary by glyph width).
    assert_eq!(
        state.handle_key(&rows, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
        TreeOutcome::SelectionChanged("leaf")
    );
    assert_eq!(
        state.handle_key(&rows, KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)),
        TreeOutcome::CheckToggled("leaf")
    );
    assert_eq!(state.selection().unwrap().checked(), ["root", "leaf"]);

    state.selection_mut().unwrap().clear();
    assert!(state.selection().unwrap().checked().is_empty());
    state.disable_multi_select();
    assert_eq!(
        state.handle_key(&rows, KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)),
        TreeOutcome::Ignored
    );
}

#[test]
fn tone_tiers_map_to_semantic_roles() {
    let tokens = DesignSystem::phosphor();
    let rows = [
        TreeNode::new("live", Line::from("streaming"), 0).tone(termrock::widgets::ToneTier::Live),
        TreeNode::new("dim", Line::from("waiting"), 0).tone(termrock::widgets::ToneTier::LiveDim),
    ];
    let area = Rect::new(0, 0, 24, 2);
    let mut buffer = Buffer::empty(area);
    Tree::new(&rows, &tokens).render(area, &mut buffer, &mut TreeState::default());
    assert_eq!(
        buffer[(2, 0)].fg,
        tokens.style(Role::InfoStrong).fg.unwrap()
    );
    assert_eq!(buffer[(2, 1)].fg, tokens.style(Role::InfoDim).fg.unwrap());
}

#[test]
fn horizontal_scroll_keeps_hierarchy_prefix_pinned() {
    let tokens = DesignSystem::phosphor();
    let rows = [TreeNode::new("root", Line::from("abcdefghijklmnopqrstuvwxyz"), 0).branch()];
    let area = Rect::new(0, 0, 12, 1);
    let mut state = TreeState::default();
    let mut before = Buffer::empty(area);
    Tree::new(&rows, &tokens).render(area, &mut before, &mut state);
    let disclosure = before[(0, 0)].symbol().to_owned();
    state.set_h_offset(5);
    let mut after = Buffer::empty(area);
    Tree::new(&rows, &tokens).render(area, &mut after, &mut state);
    assert_eq!(after[(0, 0)].symbol(), disclosure);
    assert_eq!(state.h_offset(), 5);
    let rendered: String = after.content().iter().map(|cell| cell.symbol()).collect();
    assert!(rendered.contains("fgh"), "{rendered:?}");
}
