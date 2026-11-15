use std::cell::RefCell;
use std::rc::Rc;

use junie_tui::core::id::WidgetId;
use junie_tui::runtime::Application;
use junie_tui::shell::render;
use junie_tui::widgets::code::CodeEditor;
use junie_tui::widgets::input::TextInput;
use junie_tui::widgets::textarea::TextArea;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::common::Host;

const FIELD: WidgetId = WidgetId::of("focus_edit.field");

enum Field {
    Line(Box<TextInput>),
    Area(Box<TextArea>),
    Code(Box<CodeEditor>),
}

impl Field {
    fn editing(&self) -> bool {
        match self {
            Self::Line(f) => f.editing,
            Self::Area(f) => f.editing,
            Self::Code(f) => f.editing,
        }
    }
}

/// A host whose page reads the field's edit state for a badge drawn before
/// the field renders, the way a page footer is drawn before the layers on top
/// of it.
fn badge_host(field: &Rc<RefCell<Field>>) -> Host {
    let field = Rc::clone(field);
    Host::painting(move |buf, ctx| {
        let mut field = field.borrow_mut();
        let text = if field.editing() { "EDIT" } else { "view" };
        buf.set_string(0, 0, text, ctx.theme.base());
        let bg = ctx.theme.canvas;
        let area = Rect::new(0, 1, 30, 3);
        match &mut *field {
            Field::Line(f) => f.render(area, buf, ctx, bg),
            Field::Area(f) => f.render(area, buf, ctx, bg),
            Field::Code(f) => f.render(area, buf, ctx, bg),
        }
    })
}

fn frame(host: &mut Host, terminal: &mut Terminal<TestBackend>) -> Buffer {
    terminal
        .draw(|f| render(&mut *host, f))
        .expect("draw the frame");
    terminal.backend().buffer().clone()
}

fn badge(buf: &Buffer) -> String {
    (0..4).map(|x| buf[(x, 0)].symbol()).collect()
}

fn assert_frame_matches_settled_state(field: Field) {
    let field = Rc::new(RefCell::new(field));
    let mut host = badge_host(&field);
    let mut terminal = Terminal::new(TestBackend::new(72, 20)).expect("test terminal");

    let first = frame(&mut host, &mut terminal);
    assert!(
        field.borrow().editing(),
        "focusing the field starts its edit"
    );
    assert_eq!(
        badge(&first),
        "EDIT",
        "a widget drawn before the field shows the edit state the frame settles on"
    );
    assert_eq!(
        host.draws, 3,
        "focus arrives after pass one and the edit starts in pass two"
    );
    assert!(
        !Application::take_render_invalidated(&mut host),
        "the settled frame asks for nothing more"
    );

    let again = frame(&mut host, &mut terminal);
    assert_eq!(first, again, "the next frame paints the same pixels");
    assert_eq!(host.draws, 4, "a stable frame draws once");
    assert!(!Application::take_render_invalidated(&mut host));
}

#[test]
fn text_input_focus_edit_is_visible_to_widgets_drawn_before_it() {
    assert_frame_matches_settled_state(Field::Line(Box::new(TextInput::new(FIELD, "Name"))));
}

#[test]
fn text_area_focus_edit_is_visible_to_widgets_drawn_before_it() {
    assert_frame_matches_settled_state(Field::Area(Box::new(TextArea::new(FIELD, "Notes", 2))));
}

#[test]
fn code_editor_focus_edit_is_visible_to_widgets_drawn_before_it() {
    assert_frame_matches_settled_state(Field::Code(Box::new(CodeEditor::new(FIELD, "let x = 1;"))));
}
