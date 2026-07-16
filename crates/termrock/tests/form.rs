use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Modifier,
    text::Line,
    widgets::StatefulWidget,
};
use termrock::theme::Role;
use termrock::{
    Theme,
    input::{KeyCode, KeyEvent, KeyModifiers},
    widgets::{Form, FormField, FormOutcome, FormSection, FormState},
};

fn fields() -> Vec<FormField<'static, &'static str>> {
    vec![
        FormField {
            id: "host",
            label: Line::from("Host"),
            value: Line::from("localhost"),
            help: Some(Line::from("Server name or address")),
            error: None,
            required: true,
            enabled: true,
        },
        FormField {
            id: "database",
            label: Line::from("Database"),
            value: Line::from("app"),
            help: None,
            error: None,
            required: false,
            enabled: true,
        },
        FormField {
            id: "port",
            label: Line::from("Port"),
            value: Line::from("5432"),
            help: None,
            error: Some(Line::from("Port must be numeric")),
            required: false,
            enabled: false,
        },
    ]
}

#[test]
fn traversal_skips_disabled_fields_and_activation_is_semantic() {
    let fields = fields();
    let sections = [FormSection {
        title: Line::from("General"),
        fields: &fields,
    }];
    let mut state = FormState::new(Some("host"));

    assert_eq!(
        state.handle_key(&sections, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
        FormOutcome::FocusChanged("database")
    );
    assert_eq!(
        state.handle_key(
            &sections,
            KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE)
        ),
        FormOutcome::FocusChanged("host")
    );
    assert_eq!(
        state.handle_key(&sections, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        FormOutcome::Activated("host")
    );
    state.set_active(false);
    assert_eq!(
        state.handle_key(&sections, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
        FormOutcome::Ignored
    );
}

#[test]
fn rendering_exposes_sections_required_help_error_and_non_color_states() {
    let fields = fields();
    let sections = [FormSection {
        title: Line::from("General"),
        fields: &fields,
    }];
    let theme = Theme::default();
    let form = Form::new(&sections, &theme);
    let mut state = FormState::new(Some("host"));
    let area = Rect::new(0, 0, 36, 14);
    let mut buffer = Buffer::empty(area);

    form.render(area, &mut buffer, &mut state);

    let rendered = buffer
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(rendered.contains("General"));
    assert!(rendered.contains("Host"));
    assert!(rendered.contains('*'));
    assert!(rendered.contains("Server name or address"));
    assert!(rendered.contains("Port must be numeric"));
    assert!(rendered.contains('⊘'));
    assert_eq!(state.column_count(), 1);
    assert_eq!(
        state.regions().len(),
        2,
        "disabled fields are not actionable"
    );
    let host = state
        .field_regions()
        .iter()
        .find(|region| region.id == "host")
        .expect("focused host painted");
    assert_eq!(
        buffer[host.value.expect("host value visible").as_position()].fg,
        theme.style(Role::Focus).fg.expect("focus foreground")
    );
}

#[test]
fn wide_forms_use_two_columns_and_clicks_follow_painted_geometry() {
    let fields = fields();
    let sections = [FormSection {
        title: Line::from("General"),
        fields: &fields,
    }];
    let theme = Theme::default();
    let form = Form::new(&sections, &theme);
    let mut state = FormState::new(Some("host"));
    let area = Rect::new(4, 2, 80, 10);
    let mut buffer = Buffer::empty(Rect::new(0, 0, 90, 14));

    form.render(area, &mut buffer, &mut state);

    assert_eq!(state.column_count(), 2);
    let database_layout = state
        .field_regions()
        .iter()
        .find(|region| region.id == "database")
        .expect("database layout painted")
        .clone();
    let database_label = database_layout.label.expect("database label visible");
    let database_value = database_layout.value.expect("database value visible");
    assert_eq!(database_value.height, 1);
    assert_eq!(database_value.y, database_label.y + 1);
    state.hover(Position::new(database_label.x, database_label.y));
    form.render(area, &mut buffer, &mut state);
    assert!(
        buffer[(database_label.x, database_label.y)]
            .modifier
            .contains(Modifier::UNDERLINED)
    );
    let database = state
        .regions()
        .iter()
        .find(|region| region.id == "database")
        .expect("database field painted")
        .area;
    assert!(database.x > area.x);
    assert_eq!(
        state.click(Position::new(database.x, database.y)),
        FormOutcome::FocusChanged("database")
    );
    assert_eq!(
        state.click(Position::new(database.x, database.y)),
        FormOutcome::Activated("database")
    );
}

#[test]
fn focused_field_is_revealed_and_manual_scroll_is_bounded() {
    let fields = (0..8)
        .map(|id| FormField {
            id,
            label: Line::from(format!("Field {id}")),
            value: Line::from(format!("value {id}")),
            help: None,
            error: None,
            required: false,
            enabled: true,
        })
        .collect::<Vec<_>>();
    let sections = [FormSection {
        title: Line::from("Long form"),
        fields: &fields,
    }];
    let theme = Theme::default();
    let form = Form::new(&sections, &theme);
    let mut state = FormState::new(Some(7));
    let area = Rect::new(0, 0, 30, 5);
    let mut buffer = Buffer::empty(area);

    form.render(area, &mut buffer, &mut state);

    assert!(state.offset() > 0);
    assert!(state.regions().iter().any(|region| region.id == 7));
    assert!(state.scroll_to_position(Position::new(area.right() - 1, area.y)));
    assert_eq!(state.offset(), 0);
    let content_len = state.content_height();
    state.scroll_by(3, content_len);
    form.render(area, &mut buffer, &mut state);
    let first = state
        .field_regions()
        .iter()
        .find(|region| region.id == 0)
        .expect("partially painted first field retains geometry");
    assert!(first.label.is_none());
    assert!(first.value.is_some());
    assert!(state.scroll_by(isize::MAX, content_len));
    assert!(!state.scroll_by(1, content_len));
    assert!(state.scroll_by(isize::MIN, content_len));
    assert_eq!(state.offset(), 0);
}

#[test]
fn empty_and_tiny_forms_are_safe() {
    let theme = Theme::default();
    let form: Form<'_, u8> = Form::new(&[], &theme);
    let mut state = FormState::default();
    let mut zero = Buffer::empty(Rect::new(0, 0, 0, 0));
    form.render(Rect::new(0, 0, 0, 0), &mut zero, &mut state);
    assert!(state.regions().is_empty());
    assert_eq!(state.offset(), 0);

    let empty_area = Rect::new(0, 0, 4, 2);
    let mut empty = Buffer::empty(empty_area);
    form.render(empty_area, &mut empty, &mut state);
    assert!(empty.content().iter().all(|cell| cell.symbol() == " "));

    let fields = [FormField {
        id: 1,
        label: Line::from("🧪A"),
        value: Line::from("Value 🧪"),
        help: None,
        error: None,
        required: true,
        enabled: true,
    }];
    let sections = [FormSection {
        title: Line::from("Settings"),
        fields: &fields,
    }];
    let form = Form::new(&sections, &theme);
    let area = Rect::new(0, 0, 2, 6);
    let mut tiny = Buffer::empty(area);
    form.render(area, &mut tiny, &mut state);
    assert_eq!(tiny[(0, 2)].symbol(), " ", "wide glyph is not split");
    assert_eq!(tiny[(1, 2)].symbol(), "*", "required marker is reserved");
}

#[test]
fn traversal_is_stable_across_sections_and_responsive_reflow() {
    let first = [
        FormField {
            id: 1,
            label: Line::from("One"),
            value: Line::from("1"),
            help: None,
            error: None,
            required: false,
            enabled: true,
        },
        FormField {
            id: 2,
            label: Line::from("Two"),
            value: Line::from("2"),
            help: None,
            error: None,
            required: false,
            enabled: false,
        },
    ];
    let second = [FormField {
        id: 3,
        label: Line::from("Three"),
        value: Line::from("3"),
        help: None,
        error: None,
        required: false,
        enabled: true,
    }];
    let sections = [
        FormSection {
            title: Line::from("First"),
            fields: &first,
        },
        FormSection {
            title: Line::from("Second"),
            fields: &second,
        },
    ];
    let theme = Theme::default();
    let form = Form::new(&sections, &theme);
    let mut state = FormState::new(Some(1));

    assert_eq!(
        state.handle_key(&sections, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
        FormOutcome::FocusChanged(3)
    );
    assert_eq!(
        state.handle_key(&sections, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
        FormOutcome::FocusChanged(1)
    );
    assert_eq!(
        state.handle_key(&sections, KeyEvent::new(KeyCode::End, KeyModifiers::NONE)),
        FormOutcome::FocusChanged(3)
    );
    assert_eq!(
        state.handle_key(&sections, KeyEvent::new(KeyCode::Home, KeyModifiers::NONE)),
        FormOutcome::FocusChanged(1)
    );
    assert_eq!(
        state.handle_key(
            &sections,
            KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE)
        ),
        FormOutcome::FocusChanged(3)
    );

    let mut narrow = Buffer::empty(Rect::new(0, 0, 30, 20));
    form.render(Rect::new(0, 0, 30, 20), &mut narrow, &mut state);
    assert_eq!(state.focused(), Some(&3));
    let mut wide = Buffer::empty(Rect::new(0, 0, 80, 20));
    form.render(Rect::new(0, 0, 80, 20), &mut wide, &mut state);
    assert_eq!(state.focused(), Some(&3));
}

#[test]
fn tab_cycles_focus_across_sections_and_skips_disabled() {
    let fields = fields();
    let sections = [FormSection {
        title: Line::from("General"),
        fields: &fields,
    }];
    let mut state = FormState::new(Some("database"));

    assert_eq!(
        state.handle_key(&sections, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
        FormOutcome::FocusChanged("host")
    );
    assert_ne!(state.focused(), Some(&"port"));
}

#[test]
fn enter_on_focused_field_activates() {
    let fields = fields();
    let sections = [FormSection {
        title: Line::from("General"),
        fields: &fields,
    }];
    let mut state = FormState::new(Some("database"));

    assert_eq!(
        state.handle_key(&sections, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        FormOutcome::Activated("database")
    );
}

#[test]
fn inactive_form_ignores_keys() {
    let fields = fields();
    let sections = [FormSection {
        title: Line::from("General"),
        fields: &fields,
    }];
    let mut state = FormState::new(Some("host"));
    state.set_active(false);

    for code in [KeyCode::Tab, KeyCode::Down, KeyCode::Home, KeyCode::Enter] {
        assert_eq!(
            state.handle_key(&sections, KeyEvent::new(code, KeyModifiers::NONE)),
            FormOutcome::Ignored
        );
    }
}

#[test]
fn arrow_navigation_matches_tab_order_in_each_column_layout() {
    let fields = fields();
    let sections = [FormSection {
        title: Line::from("General"),
        fields: &fields,
    }];
    let theme = Theme::default();
    let form = Form::new(&sections, &theme);
    let mut state = FormState::new(Some("host"));

    let narrow_area = Rect::new(0, 0, 40, 14);
    let mut narrow = Buffer::empty(narrow_area);
    form.render(narrow_area, &mut narrow, &mut state);
    assert_eq!(state.column_count(), 1);
    assert_eq!(
        state.handle_key(&sections, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
        FormOutcome::FocusChanged("database")
    );

    state.focus(Some("host"));
    let wide_area = Rect::new(0, 0, 100, 14);
    let mut wide = Buffer::empty(wide_area);
    form.render(wide_area, &mut wide, &mut state);
    assert_eq!(state.column_count(), 2);
    assert_eq!(
        state.handle_key(&sections, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
        FormOutcome::FocusChanged("database")
    );
}

#[test]
fn scroll_by_clamps_at_bounds() {
    let fields = (0..8)
        .map(|id| FormField {
            id,
            label: Line::from(format!("Field {id}")),
            value: Line::from("value"),
            help: None,
            error: None,
            required: false,
            enabled: true,
        })
        .collect::<Vec<_>>();
    let sections = [FormSection {
        title: Line::from("Long"),
        fields: &fields,
    }];
    let theme = Theme::default();
    let form = Form::new(&sections, &theme);
    let area = Rect::new(0, 0, 30, 5);
    let mut buffer = Buffer::empty(area);
    let mut state = FormState::new(None);
    form.render(area, &mut buffer, &mut state);

    let content_len = state.content_height();
    assert!(!state.scroll_by(-1, content_len));
    assert!(state.scroll_by(isize::MAX, content_len));
    let maximum = state.offset();
    assert!(maximum > 0);
    assert!(!state.scroll_by(1, content_len));
    assert!(state.scroll_by(isize::MIN, content_len));
    assert_eq!(state.offset(), 0);
}

#[test]
fn click_on_field_focuses_and_reports() {
    let fields = fields();
    let sections = [FormSection {
        title: Line::from("General"),
        fields: &fields,
    }];
    let theme = Theme::default();
    let form = Form::new(&sections, &theme);
    let area = Rect::new(0, 0, 40, 14);
    let mut buffer = Buffer::empty(area);
    let mut state = FormState::new(Some("host"));
    form.render(area, &mut buffer, &mut state);
    let database = state
        .field_regions()
        .iter()
        .find(|region| region.id == "database")
        .unwrap()
        .area;
    let position = database.as_position();

    assert_eq!(state.hover(position), Some(&"database"));
    assert_eq!(state.click(position), FormOutcome::FocusChanged("database"));
}

#[test]
fn partially_clipped_field_retains_union_hit_region() {
    let fields = fields();
    let sections = [FormSection {
        title: Line::from("General"),
        fields: &fields,
    }];
    let theme = Theme::default();
    let form = Form::new(&sections, &theme);
    let area = Rect::new(0, 0, 30, 5);
    let mut buffer = Buffer::empty(area);
    let mut state = FormState::new(None);
    form.render(area, &mut buffer, &mut state);
    let content_len = state.content_height();
    state.scroll_by(3, content_len);
    form.render(area, &mut buffer, &mut state);
    let host = state
        .field_regions()
        .iter()
        .find(|region| region.id == "host")
        .unwrap();

    assert!(!host.area.is_empty());
    assert!(host.label.is_none());
    assert!(host.value.is_some());
}

#[test]
fn click_outside_any_region_is_ignored() {
    let fields = fields();
    let sections = [FormSection {
        title: Line::from("General"),
        fields: &fields,
    }];
    let theme = Theme::default();
    let form = Form::new(&sections, &theme);
    let area = Rect::new(4, 3, 40, 14);
    let mut buffer = Buffer::empty(Rect::new(0, 0, 50, 20));
    let mut state = FormState::new(Some("host"));
    form.render(area, &mut buffer, &mut state);

    assert_eq!(state.click(Position::new(0, 0)), FormOutcome::Ignored);
}
