use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    widgets::StatefulWidget,
};
use termrock::{
    Theme, max_offset,
    widgets::{DetailCapability, DetailRow, DetailTable, DetailTableOutcome, DetailTableState},
};

fn rows() -> Vec<DetailRow<'static, &'static str>> {
    vec![
        DetailRow {
            id: "copy",
            label: "Run ID",
            value: "abc-123",
            href: None,
            capability: DetailCapability::Copy,
            emphasis: false,
            style: None,
        },
        DetailRow {
            id: "link",
            label: "Docs",
            value: "documentation",
            href: Some("https://example.invalid/docs"),
            capability: DetailCapability::Link,
            emphasis: false,
            style: None,
        },
        DetailRow {
            id: "plain",
            label: "Role",
            value: "operator",
            href: None,
            capability: DetailCapability::None,
            emphasis: true,
            style: None,
        },
    ]
}

fn render<'a, Id: Clone + PartialEq>(
    rows: &'a [DetailRow<'a, Id>],
    theme: &'a Theme,
    state: &mut DetailTableState<Id>,
    area: Rect,
    wrap: bool,
) -> Buffer {
    let table = DetailTable {
        rows,
        label_width: 0,
        wrap,
        theme,
    };
    let mut buffer = Buffer::empty(area);
    (&table).render(area, &mut buffer, state);
    buffer
}

#[test]
fn select_next_previous_traverse_and_wrap() {
    let rows = rows();
    let mut state = DetailTableState::default();

    assert_eq!(
        state.select_next(&rows),
        DetailTableOutcome::Selected("copy")
    );
    assert_eq!(
        state.select_previous(&rows),
        DetailTableOutcome::Selected("plain")
    );
    assert_eq!(
        state.select_next(&rows),
        DetailTableOutcome::Selected("copy")
    );
}

#[test]
fn selection_includes_rows_without_capability() {
    let rows = rows();
    let mut state = DetailTableState {
        selected: Some("link"),
        ..DetailTableState::default()
    };

    assert_eq!(
        state.select_next(&rows),
        DetailTableOutcome::Selected("plain")
    );
}

#[test]
fn activate_at_on_copyable_row_returns_copy_and_affordance_changes() {
    let rows = rows();
    let theme = Theme::default();
    let area = Rect::new(0, 0, 40, 3);
    let mut state = DetailTableState::default();
    let before = render(&rows, &theme, &mut state, area, false);
    let copy = state
        .regions
        .iter()
        .find(|region| region.id == "copy")
        .unwrap()
        .clone();

    assert_eq!(
        state.activate_at(copy.action_area.as_position()),
        DetailTableOutcome::Copy("copy")
    );
    assert!(before.content().iter().any(|cell| cell.symbol() == "⧉"));
    state.mark_copied(Some("copy"));
    let after = render(&rows, &theme, &mut state, area, false);
    assert!(after.content().iter().any(|cell| cell.symbol() == "✓"));
}

#[test]
fn activate_link_at_returns_activate_link() {
    let rows = rows();
    let theme = Theme::default();
    let mut state = DetailTableState::default();
    render(&rows, &theme, &mut state, Rect::new(0, 0, 40, 3), false);
    let link = state
        .regions
        .iter()
        .find(|region| region.id == "link")
        .unwrap()
        .value_area;

    assert_eq!(
        state.activate_link_at(link.as_position()),
        DetailTableOutcome::ActivateLink("link")
    );
}

#[test]
fn hover_tracks_row_id() {
    let rows = rows();
    let theme = Theme::default();
    let mut state = DetailTableState::default();
    render(&rows, &theme, &mut state, Rect::new(3, 2, 40, 3), false);
    let copy = state
        .regions
        .iter()
        .find(|region| region.id == "copy")
        .unwrap()
        .action_area;

    assert_eq!(state.hover_at(copy.as_position()), Some(&"copy"));
    assert_eq!(state.hover_at(Position::new(0, 0)), None);
}

#[test]
fn clamp_scroll_after_rows_shrink() {
    let many = (0..20)
        .map(|id| DetailRow {
            id,
            label: "Item",
            value: "long detail value",
            href: None,
            capability: DetailCapability::None,
            emphasis: false,
            style: None,
        })
        .collect::<Vec<_>>();
    let one = [DetailRow {
        id: 0,
        label: "Item",
        value: "short",
        href: None,
        capability: DetailCapability::None,
        emphasis: false,
        style: None,
    }];
    let theme = Theme::default();
    let mut state = DetailTableState::default();
    render(&many, &theme, &mut state, Rect::new(0, 0, 12, 3), false);
    state.scroll.scroll_x = u16::MAX;
    state.scroll.scroll_y = u16::MAX;
    render(&one, &theme, &mut state, Rect::new(0, 0, 12, 3), false);
    state.clamp_scroll();

    assert_eq!(state.scroll.scroll_y, 0);
    assert_eq!(
        usize::from(state.scroll.scroll_x),
        max_offset(state.content_width, usize::from(state.viewport.width))
    );
}

#[test]
fn activate_selected_routes_by_capability() {
    let rows = rows();
    let theme = Theme::default();
    let table = DetailTable {
        rows: &rows,
        label_width: 0,
        wrap: false,
        theme: &theme,
    };
    let mut state = DetailTableState::default();

    assert_eq!(table.activate_selected(&state), DetailTableOutcome::Ignored);
    state.selected = Some("copy");
    assert_eq!(
        table.activate_selected(&state),
        DetailTableOutcome::Copy("copy")
    );
    state.selected = Some("link");
    assert_eq!(
        table.activate_selected(&state),
        DetailTableOutcome::ActivateLink("link")
    );
    state.selected = Some("plain");
    assert_eq!(
        table.activate_selected(&state),
        DetailTableOutcome::Selected("plain")
    );
}

#[test]
fn wrap_mode_regions_cover_continuation_rows() {
    let rows = [DetailRow {
        id: "long",
        label: "Value",
        value: "a long value that needs several continuation rows",
        href: Some("https://example.invalid"),
        capability: DetailCapability::Link,
        emphasis: false,
        style: None,
    }];
    let theme = Theme::default();
    let mut state = DetailTableState::default();
    render(&rows, &theme, &mut state, Rect::new(0, 0, 18, 6), true);
    let regions = state
        .regions
        .iter()
        .filter(|region| region.id == "long")
        .collect::<Vec<_>>();

    assert!(regions.len() > 1);
    assert!(
        regions
            .windows(2)
            .all(|pair| pair[0].row_area.y < pair[1].row_area.y)
    );
}
