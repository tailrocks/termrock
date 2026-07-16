use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::Line,
    widgets::{StatefulWidget, Widget},
};

use super::*;
use crate::style::{Role, Theme};

#[cfg(feature = "serde")]
#[test]
fn persistable_states_implement_serde_contracts() {
    fn assert_serde<T: serde::Serialize + serde::de::DeserializeOwned>() {}

    assert_serde::<DiffState>();
    assert_serde::<SplitRatio>();
    assert_serde::<TextInputState>();
}

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
    let panel = Panel::new(&theme)
        .title("Title")
        .emphasis(PanelEmphasis::Focused);
    let hints = [Hint {
        chord: "Enter",
        label: "choose",
        priority: 1,
        visible: true,
    }];
    let hint_bar = HintBar::new(&hints, &theme).separator(" · ");
    let toast = Toast::new(&theme, "Updated", Severity::Success).anchor(Anchor::TopRight);
    let backdrop = Backdrop::new().symbol(' ').style(Style::new().dim());
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
    let panel = Panel::new(&theme).emphasis(PanelEmphasis::Focused);
    (&panel).render(area, &mut buffer);
    assert_panel_border(&buffer, area, theme.style(Role::BorderFocused));
}

#[test]
fn inactive_panel_preserves_plain_gray_border() {
    let theme = Theme::default();
    let area = Rect::new(0, 0, 10, 3);
    let mut buffer = Buffer::empty(area);
    Panel::new(&theme).render(area, &mut buffer);
    assert_panel_border(&buffer, area, theme.style(Role::Border));
}

fn assert_panel_border(buffer: &Buffer, area: Rect, expected: Style) {
    assert_eq!(buffer[(area.left(), area.top())].symbol(), "┌");
    assert_eq!(buffer[(area.right() - 1, area.top())].symbol(), "┐");
    assert_eq!(buffer[(area.left(), area.bottom() - 1)].symbol(), "└");
    assert_eq!(buffer[(area.right() - 1, area.bottom() - 1)].symbol(), "┘");
    for x in area.left() + 1..area.right() - 1 {
        assert_eq!(buffer[(x, area.top())].symbol(), "─");
        assert_eq!(buffer[(x, area.bottom() - 1)].symbol(), "─");
    }
    for y in area.top() + 1..area.bottom() - 1 {
        assert_eq!(buffer[(area.left(), y)].symbol(), "│");
        assert_eq!(buffer[(area.right() - 1, y)].symbol(), "│");
    }
    for x in area.left()..area.right() {
        assert_eq!(buffer[(x, area.top())].fg, expected.fg.unwrap());
        assert_eq!(buffer[(x, area.bottom() - 1)].fg, expected.fg.unwrap());
    }
    for y in area.top() + 1..area.bottom() - 1 {
        assert_eq!(buffer[(area.left(), y)].fg, expected.fg.unwrap());
        assert_eq!(buffer[(area.right() - 1, y)].fg, expected.fg.unwrap());
    }
}

#[test]
fn stable_ids_survive_reordering() {
    let first = [
        ListRow {
            id: "a",
            label: Line::from("Alpha"),
            trailing: None,
            role: RowRole::Item,
            enabled: true,
        },
        ListRow {
            id: "b",
            label: Line::from("Beta"),
            trailing: None,
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
    StatefulWidget::render(&List::new(&first, &theme), area, &mut buffer, &mut state);
    StatefulWidget::render(&List::new(&second, &theme), area, &mut buffer, &mut state);
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
            trailing: None,
            role: RowRole::Item,
            enabled: false,
        },
        ListRow {
            id: 2,
            label: Line::from("Section"),
            trailing: None,
            role: RowRole::Separator,
            enabled: true,
        },
        ListRow {
            id: 3,
            label: Line::from("Enabled"),
            trailing: None,
            role: RowRole::Item,
            enabled: true,
        },
    ];
    let mut state = ListState::default();
    let area = Rect::new(4, 3, 20, 3);
    let mut buffer = Buffer::empty(Rect::new(0, 0, 30, 10));
    let theme = Theme::default();
    StatefulWidget::render(&List::new(&rows, &theme), area, &mut buffer, &mut state);
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
    let theme = Theme::default();
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
        &ActionBar::new(&actions, &theme).gap(" "),
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
    let status = StatusBar::new(&left, &right, &theme).alpha(1.0);
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
    let theme = Theme::default();
    let viewport = Viewport::new(&lines, &theme).title(" Log ");
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

#[test]
fn theme_override_reaches_active_tab_cells() {
    use ratatui_core::style::Color;

    let theme = Theme::default().with_role(Role::TabActive, Style::new().bg(Color::Blue));
    let tabs = [Tab {
        id: "active",
        label: "Active",
        glyph: None,
        active: true,
        enabled: true,
    }];
    let widget = Tabs::new(&tabs, &theme).gap(1);
    let area = Rect::new(0, 0, 12, 2);
    let mut buffer = Buffer::empty(area);
    let mut state = TabsState::default();

    (&widget).render(area, &mut buffer, &mut state);

    assert_eq!(buffer[(0, 0)].bg, Color::Blue);
}

#[test]
fn owned_panel_render_matches_borrowed_render() {
    let theme = Theme::default();
    let area = Rect::new(0, 0, 12, 3);
    let mut owned = Buffer::empty(area);
    let mut borrowed = Buffer::empty(area);

    Widget::render(Panel::new(&theme).title("Panel"), area, &mut owned);
    let panel = Panel::new(&theme).title("Panel");
    Widget::render(&panel, area, &mut borrowed);

    assert_eq!(owned, borrowed);
}
