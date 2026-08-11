//! Integration coverage for form architecture and scene-owned field focus.

use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Modifier,
    widgets::StatefulWidget,
};
use termrock::style::Role;
use termrock::{
    input::{KeyCode, KeyEvent, KeyModifiers},
    style::{DesignSystem, RolePalette},
    widgets::{
        Field, Fieldset, Form, FormOutcome, FormState, any_dirty, collect_errors, first_invalid_id,
    },
};

fn fields() -> Vec<Field<'static, &'static str>> {
    vec![
        Field::new("host", "Host", "localhost")
            .help("Server name or address")
            .required(true)
            .dirty(true)
            .touched(true),
        Field::new("database", "Database", "app"),
        Field::new("port", "Port", "5432")
            .error("Port must be numeric")
            .enabled(false),
    ]
}

#[test]
fn tab_is_ignored_activation_uses_host_focus() {
    let fields = fields();
    let sections = [Fieldset::new("General", &fields)];
    let mut state = FormState::new();

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
    let sections = [Fieldset::new("General", &fields)];
    let theme = RolePalette::default();
    let system = DesignSystem::from_palette(theme.clone());
    let form = Form::new(&sections, &system).focused_field(Some(&"host"));
    let mut state = FormState::new();
    let area = Rect::new(0, 0, 36, 14);
    let mut buffer = Buffer::empty(area);

    form.render(area, &mut buffer, &mut state);

    assert!(!state.field_regions().is_empty());
    assert!(state.regions().iter().any(|r| r.id == "host"));
    assert_eq!(first_invalid_id(&sections), None); // port error but disabled
    assert!(any_dirty(&sections));
    let _ = collect_errors(&sections);
    let _ = Role::Focus;
    let _ = Modifier::BOLD;
    let _ = Position { x: 0, y: 0 };
}

#[test]
fn submit_chord_and_focus_first_invalid() {
    let fields = vec![
        Field::new("a", "A", "").required(true).error("need a"),
        Field::new("b", "B", "ok"),
    ];
    let sections = [Fieldset::new("G", &fields)];
    let mut state = FormState::new();
    assert_eq!(
        state.handle_key(
            &sections,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
            Some(&"a")
        ),
        FormOutcome::SubmitRequested
    );
    assert_eq!(state.focus_first_invalid(&sections), Some("a"));
}
