use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::Line,
    widgets::{StatefulWidget, Widget},
};

use super::*;
use crate::style::Theme;

fn areas() -> [Rect; 5] {
    [
        Rect::new(0, 0, 0, 0),
        Rect::new(0, 0, 1, 1),
        Rect::new(3, 2, 8, 3),
        Rect::new(0, 0, 40, 8),
        Rect::new(7, 4, 80, 12),
    ]
}

#[test]
fn leaf_widgets_render_at_tiny_and_off_origin_areas() {
    let theme = Theme::default();
    let panel = Panel {
        title: Some("Title"),
        emphasis: PanelEmphasis::Focused,
        style: None,
        theme: &theme,
    };
    let hints = [Hint {
        chord: "Enter",
        label: "choose",
        priority: 1,
        visible: true,
    }];
    let hint_bar = HintBar {
        hints: &hints,
        separator: " · ",
    };
    let toast = Toast {
        message: "Updated",
        severity: Severity::Success,
        anchor: Anchor::TopRight,
        style: Style::new(),
    };
    let backdrop = Backdrop {
        symbol: ' ',
        style: Style::new().dim(),
    };
    for area in areas() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 100, 30));
        (&panel).render(area, &mut buffer);
        (&hint_bar).render(area, &mut buffer);
        (&toast).render(area, &mut buffer);
        (&backdrop).render(area, &mut buffer);
    }
}

#[test]
fn focused_panel_preserves_plain_border_glyphs() {
    let theme = Theme::default();
    let area = Rect::new(0, 0, 10, 3);
    let mut buffer = Buffer::empty(area);
    let panel = Panel {
        title: None,
        emphasis: PanelEmphasis::Focused,
        style: None,
        theme: &theme,
    };
    (&panel).render(area, &mut buffer);
    assert_eq!(buffer[(0, 0)].symbol(), "┌");
}

#[test]
fn stable_ids_survive_reordering() {
    let first = [
        ListRow {
            id: "a",
            label: Line::from("Alpha"),
            role: RowRole::Item,
            enabled: true,
        },
        ListRow {
            id: "b",
            label: Line::from("Beta"),
            role: RowRole::Item,
            enabled: true,
        },
    ];
    let second = [first[1].clone(), first[0].clone()];
    let mut state = ListState {
        selected: Some("b"),
        ..ListState::default()
    };
    let area = Rect::new(0, 0, 20, 2);
    let mut buffer = Buffer::empty(area);
    let theme = Theme::default();
    StatefulWidget::render(
        &List {
            rows: &first,
            theme: &theme,
        },
        area,
        &mut buffer,
        &mut state,
    );
    StatefulWidget::render(
        &List {
            rows: &second,
            theme: &theme,
        },
        area,
        &mut buffer,
        &mut state,
    );
    assert_eq!(state.selected, Some("b"));
    assert_eq!(
        state
            .regions
            .iter()
            .find(|region| region.id == "b")
            .unwrap()
            .area
            .y,
        0
    );
}

#[test]
fn disabled_and_separator_rows_have_no_hit_regions() {
    let rows = [
        ListRow {
            id: 1,
            label: Line::from("Disabled"),
            role: RowRole::Item,
            enabled: false,
        },
        ListRow {
            id: 2,
            label: Line::from("Section"),
            role: RowRole::Separator,
            enabled: true,
        },
        ListRow {
            id: 3,
            label: Line::from("Enabled"),
            role: RowRole::Item,
            enabled: true,
        },
    ];
    let mut state = ListState::default();
    let area = Rect::new(4, 3, 20, 3);
    let mut buffer = Buffer::empty(Rect::new(0, 0, 30, 10));
    let theme = Theme::default();
    StatefulWidget::render(
        &List {
            rows: &rows,
            theme: &theme,
        },
        area,
        &mut buffer,
        &mut state,
    );
    assert_eq!(state.regions.len(), 1);
    assert_eq!(state.regions[0].id, 3);
    assert_eq!(state.regions[0].area, Rect::new(4, 5, 20, 1));
}

#[test]
fn text_input_edits_extended_graphemes_atomically() {
    for value in ["e\u{301}", "👩‍💻", "👍🏽", "🌐", "🧪", "\u{200b}"] {
        let mut state = TextInputState::new(value);
        state.apply(EditAction::MoveLeft);
        assert_eq!(state.cursor_byte(), 0, "{value:?}");
        state.apply(EditAction::MoveRight);
        assert_eq!(state.cursor_byte(), value.len(), "{value:?}");
        state.apply(EditAction::Backspace);
        assert_eq!(state.value(), "", "{value:?}");
    }
}

#[test]
fn action_and_status_regions_match_painted_geometry() {
    let actions = [
        Action {
            id: "save",
            label: "Save",
            enabled: true,
            style: None,
        },
        Action {
            id: "cancel",
            label: "Cancel",
            enabled: true,
            style: None,
        },
    ];
    let mut action_state = ActionBarState::default();
    let area = Rect::new(5, 2, 30, 1);
    let mut buffer = Buffer::empty(Rect::new(0, 0, 40, 5));
    StatefulWidget::render(
        &ActionBar {
            actions: &actions,
            gap: " ",
        },
        area,
        &mut buffer,
        &mut action_state,
    );
    assert_eq!(action_state.regions[0].id, "save");
    assert_eq!(action_state.regions[0].area.x, area.x);

    let left = [StatusSlot {
        id: "left",
        content: "Ready",
        priority: 1,
        min_width: 0,
        enabled: true,
        style: Style::new(),
        hover_style: None,
    }];
    let right = [StatusSlot {
        id: "right",
        content: "42%",
        priority: 1,
        min_width: 0,
        enabled: true,
        style: Style::new(),
        hover_style: None,
    }];
    let status = StatusBar {
        left: &left,
        right: &right,
        style: Style::new(),
        alpha: 1.0,
    };
    let regions = status.regions(area);
    assert_eq!(regions[1].area.right(), area.right());
    (&status).render(area, &mut buffer, &mut StatusBarState::default());
}

#[test]
fn viewport_clamps_scroll_and_paints_a_full_cell_thumb() {
    let lines = [
        Line::from("zero"),
        Line::from("one"),
        Line::from("two"),
        Line::from("three"),
    ];
    let viewport = Viewport {
        lines: &lines,
        title: Some(" Log "),
        content_style: Style::new(),
        border_style: Style::new(),
        title_style: Style::new(),
        scroll_track_style: Style::new(),
        scroll_thumb_style: Style::new(),
    };
    let area = Rect::new(0, 0, 12, 4);
    let mut buffer = Buffer::empty(area);
    let mut state = crate::scroll::DialogScroll {
        scroll_x: 0,
        scroll_y: 1,
    };

    StatefulWidget::render(&viewport, area, &mut buffer, &mut state);

    assert_eq!(state.scroll_y, 1);
    assert_eq!(buffer[(1, 1)].symbol(), "o");
    assert_eq!(buffer[(11, 1)].symbol(), "·");
    assert_eq!(buffer[(11, 2)].symbol(), "┃");
}
