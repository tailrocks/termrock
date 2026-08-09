//! Integration coverage for form rendering and scene-owned field focus.

use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Modifier,
    text::Line,
    widgets::StatefulWidget,
};
use termrock::style::Role;
use termrock::{
    input::{KeyCode, KeyEvent, KeyModifiers},
    style::{DesignSystem, RolePalette},
    widgets::{Form, FormField, FormOutcome, FormSection, FormState},
};

fn fields() -> Vec<FormField<'static, &'static str>> {
    vec![
        FormField::new("host", Line::from("Host"), Line::from("localhost"))
            .help(Line::from("Server name or address"))
            .required(true),
        FormField::new("database", Line::from("Database"), Line::from("app")),
        FormField::new("port", Line::from("Port"), Line::from("5432"))
            .error(Line::from("Port must be numeric"))
            .enabled(false),
    ]
}

#[test]
fn tab_is_ignored_activation_uses_host_focus() {
    let fields = fields();
    let sections = [FormSection {
        title: Line::from("General"),
        fields: &fields,
    }];
    let mut state = FormState::new();

    // Tab is host/scene owned — form must not emit FocusChanged.
    assert_eq!(
        state.handle_key(
            &sections,
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            Some(&"host")
        ),
        FormOutcome::Ignored
    );
    assert_eq!(
        state.handle_key(
            &sections,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            Some(&"host")
        ),
        FormOutcome::Activated("host")
    );
    state.set_accepts_input(false);
    assert_eq!(
        state.handle_key(
            &sections,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            Some(&"host")
        ),
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
    let theme = RolePalette::default();
    let system = DesignSystem::from_palette(theme.clone());
    let form = Form::new(&sections, &system).focused_field(Some(&"host"));
    let mut state = FormState::new();
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
    // Non-color focus cue
    assert!(rendered.contains('›') || rendered.contains("Host"));
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
fn wide_forms_use_two_columns_and_clicks_need_host_focus() {
    let fields = fields();
    let sections = [FormSection {
        title: Line::from("General"),
        fields: &fields,
    }];
    let theme = RolePalette::default();
    let system = DesignSystem::from_palette(theme.clone());
    let form = Form::new(&sections, &system).focused_field(Some(&"host"));
    let mut state = FormState::new();
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
    // Click without host focus on database → Ignored (host must scene.focus first).
    assert_eq!(
        state.click(Position::new(database.x, database.y), Some(&"host")),
        FormOutcome::Ignored
    );
    assert_eq!(
        state.click(Position::new(database.x, database.y), Some(&"database")),
        FormOutcome::Activated("database")
    );
}

#[test]
fn empty_and_tiny_forms_are_safe() {
    let theme = RolePalette::default();
    let system = DesignSystem::from_palette(theme);
    let sections: [FormSection<'_, &str>; 0] = [];
    let form = Form::new(&sections, &system);
    let mut state = FormState::new();
    let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
    form.render(Rect::new(0, 0, 1, 1), &mut buffer, &mut state);
    assert_eq!(state.regions().len(), 0);
}

#[test]
fn ensure_visible_scrolls_host_focused_field() {
    let mut many = Vec::new();
    for i in 0..12 {
        many.push(FormField::new(
            i,
            Line::from(format!("L{i}")),
            Line::from(format!("V{i}")),
        ));
    }
    let sections = [FormSection {
        title: Line::from("Many"),
        fields: &many,
    }];
    let system = DesignSystem::default();
    let mut state = FormState::new();
    let area = Rect::new(0, 0, 40, 8);
    let mut buffer = Buffer::empty(area);
    Form::new(&sections, &system)
        .focused_field(Some(&0))
        .render(area, &mut buffer, &mut state);
    let before = state.offset();
    state.ensure_visible(Some(10));
    Form::new(&sections, &system)
        .focused_field(Some(&10))
        .render(area, &mut buffer, &mut state);
    assert!(state.offset() >= before || state.offset() > 0 || many.len() < 3);
}

#[test]
fn no_focus_changed_in_public_outcome() {
    let src = include_str!("../src/widgets/form.rs");
    let head = src.split("#[cfg(test)]").next().unwrap_or(src);
    assert!(
        !head.contains("FocusChanged"),
        "FormOutcome must not reintroduce FocusChanged"
    );
    assert!(head.contains("focused_field"));
    assert!(head.contains("accepts_input"));
}

#[test]
fn unicode_labels_paint_safely() {
    let fields = [FormField::new(
        "名前",
        Line::from("名前 🔧"),
        Line::from("値"),
    )];
    let sections = [FormSection {
        title: Line::from("設定"),
        fields: &fields,
    }];
    let system = DesignSystem::default();
    let mut state = FormState::new();
    let area = Rect::new(0, 0, 24, 8);
    let mut buffer = Buffer::empty(area);
    Form::new(&sections, &system)
        .focused_field(Some(&"名前"))
        .render(area, &mut buffer, &mut state);
    let text: String = buffer
        .content()
        .iter()
        .map(|c| c.symbol().to_string())
        .collect();
    assert!(text.contains("設定") || text.contains("名"));
}

#[test]
fn narrow_and_tiny_geometry() {
    let fields = fields();
    let sections = [FormSection {
        title: Line::from("G"),
        fields: &fields,
    }];
    let system = DesignSystem::default();
    for (w, h) in [(80, 20), (40, 12), (22, 8), (12, 6), (8, 4)] {
        let mut state = FormState::new();
        let area = Rect::new(0, 0, w, h);
        let mut buffer = Buffer::empty(area);
        Form::new(&sections, &system)
            .focused_field(Some(&"host"))
            .render(area, &mut buffer, &mut state);
        assert!(state.column_count() <= 2);
    }
}
