use ratatui_core::{buffer::Buffer, layout::Rect, text::Line, widgets::StatefulWidget};
use termrock::Theme;
use termrock::widgets::{List, ListRow, ListState, RowRole};

pub fn render() {
    let rows = [
        ListRow {
            id: "first",
            label: Line::from("First"),
            trailing: None,
            role: RowRole::Item,
            enabled: true,
        },
        ListRow {
            id: "second",
            label: Line::from("Second"),
            trailing: None,
            role: RowRole::Item,
            enabled: true,
        },
    ];
    let mut state = ListState {
        selected: Some("first"),
        ..ListState::default()
    };
    let area = Rect::new(0, 0, 24, 4);
    let mut buffer = Buffer::empty(area);
    let theme = Theme::default();
    StatefulWidget::render(&List::new(&rows, &theme), area, &mut buffer, &mut state);
    assert_eq!(state.selected, Some("first"));
}
