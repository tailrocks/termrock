use std::time::Duration;

use junie_tui::core::event::{Input, Outcome};
use junie_tui::core::id::WidgetId;
use junie_tui::runtime::Application;
use junie_tui::shell::{App, Shell, render};
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::widgets::code::CodeEditor;
use junie_tui::widgets::input::TextInput;
use junie_tui::widgets::textarea::TextArea;
use junie_tui::widgets::too_small::TooSmall;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

const FIELD: WidgetId = WidgetId::of("focus_edit.field");
const NEXT: WidgetId = WidgetId::of("focus_edit.next");

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

/// Reads the field's edit state for a badge drawn before the field renders,
/// the way a page footer is drawn before the layers on top of it.
struct Badge {
    shell: Shell<()>,
    field: Field,
    draws: usize,
    terminal: Terminal<TestBackend>,
}

impl Badge {
    fn new(field: Field) -> Self {
        Self {
            shell: Shell::new(),
            field,
            draws: 0,
            terminal: Terminal::new(TestBackend::new(30, 4)).expect("test terminal"),
        }
    }

    fn frame(&mut self) -> Buffer {
        let mut terminal = std::mem::replace(
            &mut self.terminal,
            Terminal::new(TestBackend::new(1, 1)).expect("scratch terminal"),
        );
        terminal
            .draw(|f| render(&mut *self, f))
            .expect("draw the frame");
        let buf = terminal.backend().buffer().clone();
        self.terminal = terminal;
        buf
    }

    fn badge(buf: &Buffer) -> String {
        (0..4).map(|x| buf[(x, 0)].symbol()).collect()
    }
}

impl App for Badge {
    type Status = ();

    fn shell(&self) -> &Shell<()> {
        &self.shell
    }

    fn shell_mut(&mut self) -> &mut Shell<()> {
        &mut self.shell
    }

    fn theme(&self) -> Theme {
        Theme::for_level(ColorLevel::TrueColor)
    }

    fn too_small(&self) -> TooSmall<'_> {
        TooSmall::new("badge").min(1, 1)
    }

    fn now_ms(&self) -> u64 {
        0
    }

    fn tick(&self) -> u64 {
        0
    }

    fn tick_interval(&self) -> Duration {
        Duration::from_millis(100)
    }

    fn on_input(&mut self, _input: Input) -> Outcome {
        Outcome::Ignored
    }

    fn draw(&mut self, _area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        self.draws += 1;
        let text = if self.field.editing() { "EDIT" } else { "view" };
        buf.set_string(0, 0, text, ctx.theme.base());
        let bg = ctx.theme.canvas;
        let area = Rect::new(0, 1, 30, 3);
        match &mut self.field {
            Field::Line(f) => f.render(area, buf, ctx, bg),
            Field::Area(f) => f.render(area, buf, ctx, bg),
            Field::Code(f) => f.render(area, buf, ctx, bg),
        }
        ctx.focus_stop(NEXT);
    }
}

fn assert_frame_matches_settled_state(mut host: Badge) {
    let first = host.frame();
    assert!(host.field.editing(), "focusing the field starts its edit");
    assert_eq!(
        Badge::badge(&first),
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

    let again = host.frame();
    assert_eq!(first, again, "the next frame paints the same pixels");
    assert_eq!(host.draws, 4, "a stable frame draws once");
    assert!(!Application::take_render_invalidated(&mut host));
}

fn assert_blur_commit_is_visible_to_widgets_drawn_before_it(mut host: Badge) {
    host.frame();
    assert!(host.field.editing(), "focusing the field starts its edit");

    host.shell_mut().focus_next();
    let draws_before = host.draws;
    let blurred = host.frame();
    assert!(
        !host.field.editing(),
        "moving focus away commits the clean field during render"
    );
    assert_eq!(
        Badge::badge(&blurred),
        "view",
        "a widget drawn before the commit shows the committed state in the same frame"
    );
    assert_eq!(
        host.draws - draws_before,
        2,
        "the commit repaints once: pass one reads the edit, pass two the commit"
    );
    assert!(
        !Application::take_render_invalidated(&mut host),
        "the settled frame asks for nothing more"
    );

    let again = host.frame();
    assert_eq!(blurred, again, "the next frame paints the same pixels");
    assert_eq!(host.draws - draws_before, 3, "a stable frame draws once");
}

#[test]
fn text_input_blur_commit_is_visible_to_widgets_drawn_before_it() {
    assert_blur_commit_is_visible_to_widgets_drawn_before_it(Badge::new(Field::Line(Box::new(
        TextInput::new(FIELD, "Name"),
    ))));
}

#[test]
fn text_area_blur_commit_is_visible_to_widgets_drawn_before_it() {
    assert_blur_commit_is_visible_to_widgets_drawn_before_it(Badge::new(Field::Area(Box::new(
        TextArea::new(FIELD, "Notes", 2),
    ))));
}

#[test]
fn code_editor_blur_commit_is_visible_to_widgets_drawn_before_it() {
    assert_blur_commit_is_visible_to_widgets_drawn_before_it(Badge::new(Field::Code(Box::new(
        CodeEditor::new(FIELD, "let x = 1;"),
    ))));
}

#[test]
fn text_input_focus_edit_is_visible_to_widgets_drawn_before_it() {
    assert_frame_matches_settled_state(Badge::new(Field::Line(Box::new(TextInput::new(
        FIELD, "Name",
    )))));
}

#[test]
fn text_area_focus_edit_is_visible_to_widgets_drawn_before_it() {
    assert_frame_matches_settled_state(Badge::new(Field::Area(Box::new(TextArea::new(
        FIELD, "Notes", 2,
    )))));
}

#[test]
fn code_editor_focus_edit_is_visible_to_widgets_drawn_before_it() {
    assert_frame_matches_settled_state(Badge::new(Field::Code(Box::new(CodeEditor::new(
        FIELD,
        "let x = 1;",
    )))));
}
