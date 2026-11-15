use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Rect, Size};
use ratatui::style::Color;

use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::inputs::pointer_select;
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use junie_tui::core::event::Outcome;
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::MouseTarget;
use junie_tui::terminal::{TerminalState, TerminalViewport};
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::specimen::{Specimen, SpecimenState, StateGrid};
use junie_tui::widgets::button::{Button, row_layout};
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::props::{self, Prop};
use junie_tui::widgets::viewport_ops::ViewportOps;

const ID: WidgetId = WidgetId::of("terminalviewport");
const VIEW: WidgetId = WidgetId::of("terminalviewport.view");
const ROWS: u16 = 12;
const COLS: u16 = 38;
const CONTROLS_MIN_AREA: u16 = 6;
const SPECIMEN_ROWS: u16 = 6;
const SPECIMEN_COLS: u16 = 38;

const FIXTURES: [&str; 3] = ["history", "fullscreen", "empty"];

fn steps(first: u32, last: u32) -> Vec<u8> {
    (first..=last)
        .flat_map(|n| format!("\x1b[32m{n:>2}\x1b[0m step {n} finished\r\n").into_bytes())
        .collect()
}

fn prompt() -> &'static [u8] {
    b"\x1b[1;32mdev@jackin\x1b[0m:\x1b[1;36m~/src\x1b[0m$ "
}

fn history_bytes() -> Vec<u8> {
    let mut bytes = steps(1, 30);
    bytes.extend_from_slice(
        b"\x1b[2mopen\x1b[0m /home/dev/projects/jackin/crates/jackin-console/src/screens/capsule.rs:2381\r\n",
    );
    bytes.extend_from_slice(prompt());
    bytes
}

fn fullscreen_bytes() -> Vec<u8> {
    let mut bytes = steps(1, 20);
    bytes.extend_from_slice(
        "\x1b[?1049h\x1b[?25l\x1b[2J\x1b[H┌─ editor ────────────────────────┐\r\n│ the alternate screen has no      │\r\n│ history, so the wheel stays put  │\r\n└──────────────────────────────────┘"
            .as_bytes(),
    );
    bytes.extend_from_slice(b"\x1b[12;1H\x1b[7m INSERT \x1b[0m");
    bytes
}

fn fixture_bytes(name: &str) -> Vec<u8> {
    match name {
        "fullscreen" => fullscreen_bytes(),
        "empty" => [
            b"\x1b[2mnothing has run yet\x1b[0m\r\n".as_slice(),
            prompt(),
        ]
        .concat(),
        _ => history_bytes(),
    }
}

fn viewport(id: WidgetId, rows: u16, cols: u16, bytes: &[u8]) -> TerminalViewport {
    let mut view = TerminalViewport::new(id, TerminalState::new(rows, cols));
    view.feed(bytes);
    view
}

struct ViewSpecimen {
    id: WidgetId,
    rest: TerminalViewport,
    empty: TerminalViewport,
    scrolled: TerminalViewport,
}

impl ViewSpecimen {
    fn new(id: WidgetId) -> Self {
        let mut scrolled = viewport(id.child(2), SPECIMEN_ROWS, SPECIMEN_COLS, &history_bytes());
        scrolled.wheel(junie_tui::core::hit::Axis::Vertical, -3);
        Self {
            id,
            rest: viewport(id.child(0), SPECIMEN_ROWS, SPECIMEN_COLS, &history_bytes()),
            empty: viewport(
                id.child(1),
                SPECIMEN_ROWS,
                SPECIMEN_COLS,
                &fixture_bytes("empty"),
            ),
            scrolled,
        }
    }
}

impl Specimen for ViewSpecimen {
    fn specimen_id(&self) -> WidgetId {
        self.id
    }

    fn supports(&self, state: SpecimenState) -> bool {
        use SpecimenState::*;
        matches!(state, Rest | Empty | Overflow)
    }

    fn specimen_height(&self, _state: SpecimenState) -> u16 {
        SPECIMEN_ROWS
    }

    fn specimen(
        &self,
        state: SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) {
        let view = match state {
            SpecimenState::Empty => &self.empty,
            SpecimenState::Overflow => &self.scrolled,
            _ => &self.rest,
        };
        view.draw(area, buf, ctx, bg, false);
    }
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    StateGrid::new(id)
        .label_width(10)
        .column_width(40)
        .rows(&[Rest, Empty, Overflow])
        .column("Terminal viewport", ViewSpecimen::new(id.child(0)))
}

pub struct TerminalViewportPage {
    template: Template,
    opts: PageValues,
    view: TerminalViewport,
    fed: u32,
    copied: Option<String>,
    feed: Button,
    reset: Button,
}

impl TerminalViewportPage {
    pub fn new() -> Self {
        let opts = PageValues::new(page_options(PageId::TerminalViewport));
        let mut page = Self {
            template: Template::new(ID, states_grid()),
            view: TerminalViewport::new(VIEW, TerminalState::new(ROWS, COLS)),
            opts,
            fed: 30,
            copied: None,
            feed: Button::secondary(ID.sub("feed"), "Feed line"),
            reset: Button::secondary(ID.sub("reset"), "Reset"),
        };
        page.load();
        page
    }

    fn fixture_bytes(&self) -> Vec<u8> {
        fixture_bytes(self.opts.value("terminalviewport.fixture"))
    }

    fn load(&mut self) {
        let policy = pointer_select(&self.opts, "terminalviewport.pointer_select");
        self.view = viewport(VIEW, ROWS, COLS, &self.fixture_bytes()).pointer_select(policy);
        self.fed = 30;
        self.copied = None;
    }

    fn reset(&mut self) {
        self.view.reset();
        self.view.feed(&self.fixture_bytes());
        self.fed = 30;
        self.copied = None;
    }

    fn feed_line(&mut self) {
        self.fed += 1;
        let line = format!("\r\n\x1b[32m{n:>2}\x1b[0m step {n} finished", n = self.fed);
        self.view.feed(line.as_bytes());
    }

    fn props(&self) -> Vec<Prop> {
        let state = self.view.state();
        let (rows, cols) = state.size();
        let flag = |on: bool| if on { "on" } else { "off" };
        let scroll = match self.view.offset() {
            0 => "live tail".to_owned(),
            n => format!("{n} rows up"),
        };
        let selection = match self.view.selected_text() {
            Some(text) => format!("{} characters", text.chars().count()),
            None => "none".to_owned(),
        };
        let caret = match state.cursor(Rect::new(0, 0, cols, rows)) {
            Some(at) => format!("row {}, col {}", at.y + 1, at.x + 1),
            None => "hidden".to_owned(),
        };
        let copied = match &self.copied {
            Some(text) => {
                let lines = text.lines().count();
                format!(
                    "{lines} {} · {} characters",
                    if lines == 1 { "line" } else { "lines" },
                    text.chars().count()
                )
            }
            None => "nothing yet".to_owned(),
        };
        vec![
            Prop::new("grid", format!("{cols}x{rows} + track")),
            Prop::new("scroll", scroll),
            Prop::new("history", format!("{} rows", state.scrollback_len())),
            Prop::new("alt screen", flag(state.modes().alt_screen)),
            Prop::new("selection", selection),
            Prop::new("caret", caret),
            Prop::new("copied", copied),
        ]
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let meta = match self.view.position_label() {
            label if label.is_empty() => "live".to_owned(),
            label => label,
        };
        let panel = Panel::framed(Some(FIXTURES[self.fixture()]))
            .meta(&meta)
            .focused(ctx.interaction.focused(VIEW));
        let outer = panel.outer(Size::new(COLS + 1, ROWS));
        let controls_h = if area.height > CONTROLS_MIN_AREA {
            2
        } else {
            0
        };
        let frame = Rect::new(
            area.x,
            area.y,
            outer.width.min(area.width),
            outer.height.min(area.height - controls_h),
        );
        let bg = panel.bg(t);
        let inner = panel.render(frame, buf, t);
        let (rows, cols) = TerminalViewport::grid_size(inner);
        self.view.resize(rows, cols);
        self.view.set_area(inner);
        let focused = ctx.interaction.focused(VIEW);
        if let Some(at) = self.view.draw(inner, buf, ctx, bg, focused) {
            ctx.set_cursor(at);
        }

        let side_x = frame.right() + 2;
        if area.right() > side_x + 24 {
            let side = Rect::new(side_x, area.y, area.right() - side_x, frame.height);
            props::render(side, buf, t, &self.props(), t.canvas);
        }

        let controls_y = frame.bottom() + 1;
        if controls_y < area.bottom() {
            let widths = [self.feed.width(), self.reset.width()];
            let rects = row_layout(Rect::new(area.x, controls_y, area.width, 1), &widths, 2);
            self.feed.render(rects[0], buf, ctx, t.canvas);
            self.reset.render(rects[1], buf, ctx, t.canvas);
        }
    }

    fn fixture(&self) -> usize {
        self.opts.get("terminalviewport.fixture").unwrap_or(0)
    }

    fn pointer(&mut self, ev: &junie_tui::core::pointer::PointerEvent, cx: &mut Cx) -> Outcome {
        if let Some(response) = self.view.route(ev) {
            let outcome = response.outcome();
            let released = ev.is(Phase::DragEnd) || (ev.is(Phase::Click) && ev.count() >= 2);
            if released && let Some(text) = self.view.selected_text() {
                self.copied = Some(text.clone());
                cx.copy(text);
                return Outcome::Changed;
            }
            return outcome;
        }
        if self.feed.route(ev).is_some_and(|r| r.has_actions()) {
            self.feed_line();
            return Outcome::Changed;
        }
        if self.reset.route(ev).is_some_and(|r| r.has_actions()) {
            self.load();
            return Outcome::Changed;
        }
        Outcome::Ignored
    }

    fn view_key(&mut self, code: KeyCode) -> Outcome {
        match code {
            KeyCode::PageUp => self.view.page_up(),
            KeyCode::PageDown => self.view.page_down(),
            KeyCode::Home => self.view.jump_start(),
            KeyCode::End => self.view.set_follow(true),
            KeyCode::Esc if self.view.has_selection() => {
                return self.view.clear_selection();
            }
            _ => return Outcome::Ignored,
        }
        Outcome::Changed
    }
}

impl Page for TerminalViewportPage {
    fn title(&self) -> &'static str {
        "Terminal viewport"
    }

    fn blurb(&self) -> &'static str {
        "A terminal grid with history and a scroll track · wheel, drag and double-click · copy joins wrapped lines"
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let body = self.template.render(area, buf, ctx);
        match self.template.section() {
            Section::Playground => self.render_playground(body, buf, ctx),
            Section::Options => self.template.render_options(body, buf, ctx),
            Section::States => self.template.render_states(body, buf, ctx),
        }
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut Cx) -> Outcome {
        if let Some(o) = self.template.handle(ev, cx) {
            return o;
        }
        if self.template.section() != Section::Playground {
            return Outcome::Ignored;
        }
        match ev {
            PageEvent::Key(key) => {
                if cx.focus.is(VIEW) {
                    return self.view_key(key.code);
                }
                if cx.focus.is(self.feed.id) {
                    let (o, fired) = self.feed.on_key(key);
                    if fired {
                        self.feed_line();
                    }
                    return o;
                }
                if cx.focus.is(self.reset.id) {
                    let (o, fired) = self.reset.on_key(key);
                    if fired {
                        self.reset();
                    }
                    return o;
                }
                Outcome::Ignored
            }
            PageEvent::Pointer(ev) => self.pointer(ev, cx),
            _ => Outcome::Ignored,
        }
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        if focus == Some(VIEW) {
            vec![
                ("PgUp PgDn", "Scroll"),
                ("Home", "Oldest"),
                ("End", "Live tail"),
                ("Esc", "Clear selection"),
            ]
        } else {
            vec![("Enter", "Activate")]
        }
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        self.opts.get(key)
    }

    fn set_option(&mut self, key: &str, value: usize) {
        if self.opts.set(key, value) {
            self.load();
        }
    }

    fn template(&self) -> Option<&Template> {
        Some(&self.template)
    }

    fn template_mut(&mut self) -> Option<&mut Template> {
        Some(&mut self.template)
    }
}
