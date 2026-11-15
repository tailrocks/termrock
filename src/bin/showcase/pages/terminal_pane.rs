use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Rect, Size};
use ratatui::style::Color;
use ratatui::widgets::Widget;

use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::{MouseTarget, PointerEvent};
use junie_tui::terminal::{Modes, TerminalState, TerminalView, encode_key};
use junie_tui::theme::Tone;
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::specimen::{Specimen, SpecimenState, StateGrid};
use junie_tui::widgets::button::{Button, row_layout};
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::props::{self, Prop};

const ID: WidgetId = WidgetId::of("terminalpane");
const PANE: WidgetId = WidgetId::of("terminalpane.pane");
const CONTROLS_MIN_AREA: u16 = 6;

struct Fixture {
    name: &'static str,
    chunks: &'static [&'static [u8]],
    leave: Option<&'static [u8]>,
}

const SESSION: &[&[u8]] = &[
    b"\x1b[1;32mdev@jackin\x1b[0m:\x1b[1;34m~/src\x1b[0m$ ls -l\r\n",
    b"\x1b[34mdocs\x1b[0m  \x1b[32mrun.sh\x1b[0m  \x1b[31marchive\x1b[0m\r\n",
    b"\x1b[38;5;208m256-orange\x1b[0m  \x1b[38;2;72;224;84mrgb-green\x1b[0m\r\n",
    b"\x1b[1mbold\x1b[0m \x1b[2mdim\x1b[0m \x1b[3mitalic\x1b[0m \x1b[4munderline\x1b[0m \x1b[7minverse\x1b[0m\r\n",
    "日本語 wide\r\n".as_bytes(),
    b"\x1b[1;32mdev@jackin\x1b[0m:\x1b[1;34m~/src\x1b[0m$ ",
];

const FULLSCREEN: &[&[u8]] = &[
    b"primary screen\r\n",
    "\x1b[?1049h\x1b[?25l\x1b[?1h\x1b[?2004h\x1b[2J\x1b[H┌────────────┐\r\n│ jackin ❯   │\r\n└────────────┘"
        .as_bytes(),
    b"\x1b[999;1H\x1b[7m NORMAL \x1b[0m",
];

const SPLIT: &[&[u8]] = &[
    b"\x1b",
    b"[38;2;7",
    b"2;224;84mgreen\x1b[0m ",
    &[0xE2, 0x94],
    &[0x82, b'x', b'\r', b'\n'],
    b"\x1b]0;split title",
    b"\x07done\r\n",
];

const MIDSTREAM: &[&[u8]] = &[
    b"\x1b[38;2;72;224;84mtail\x1b[0m\r\n",
    b"\x1b]0;title\x07after\r\n",
    &[0xE2, 0x94, 0x82, b'x', b'\r', b'\n'],
    b"\x1b[2J\x1b[Hready $ ",
];

const FIXTURES: [Fixture; 4] = [
    Fixture {
        name: "session",
        chunks: SESSION,
        leave: None,
    },
    Fixture {
        name: "fullscreen",
        chunks: FULLSCREEN,
        leave: Some(b"\x1b[?1049l"),
    },
    Fixture {
        name: "split",
        chunks: SPLIT,
        leave: None,
    },
    Fixture {
        name: "midstream",
        chunks: MIDSTREAM,
        leave: None,
    },
];

const REPLAY_STARTS: [(usize, usize); 4] = [(0, 0), (0, 9), (1, 2), (2, 2)];

fn fixture_index(name: &str) -> usize {
    FIXTURES.iter().position(|f| f.name == name).unwrap_or(0)
}

fn size_of(value: &str) -> (u16, u16) {
    let (cols, rows) = value.split_once('x').unwrap_or(("80", "24"));
    (rows.parse().unwrap_or(24), cols.parse().unwrap_or(80))
}

fn caret(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| match b {
            0x1b => "ESC".to_owned(),
            0x7f => "DEL".to_owned(),
            0..=0x1f => format!("^{}", (b + 0x40) as char),
            _ => (*b as char).to_string(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn key_name(key: &Key) -> String {
    let mut name = String::new();
    if key.mods.contains(KeyModifiers::CONTROL) {
        name.push_str("Ctrl+");
    }
    if key.mods.contains(KeyModifiers::ALT) {
        name.push_str("Alt+");
    }
    match key.code {
        KeyCode::Char(c) => name.push(c),
        KeyCode::F(n) => name.push_str(&format!("F{n}")),
        other => name.push_str(&format!("{other:?}")),
    }
    name
}

struct PaneSpecimen {
    id: WidgetId,
    rest: TerminalState,
    empty: TerminalState,
    overflow: TerminalState,
    narrow: TerminalState,
}

impl PaneSpecimen {
    fn new(id: WidgetId) -> Self {
        let fed = |rows: u16, cols: u16, bytes: &[u8]| {
            let mut state = TerminalState::new(rows, cols);
            state.feed(bytes);
            state
        };
        let session: Vec<u8> = SESSION.concat();
        let long = "x".repeat(120);
        Self {
            id,
            rest: fed(6, 36, &session),
            empty: TerminalState::new(6, 36),
            overflow: fed(6, 36, format!("{long}\r\n$ ").as_bytes()),
            narrow: fed(6, 20, &session),
        }
    }
}

impl Specimen for PaneSpecimen {
    fn specimen_id(&self) -> WidgetId {
        self.id
    }

    fn supports(&self, state: SpecimenState) -> bool {
        use SpecimenState::*;
        matches!(state, Rest | Focused | Empty | Overflow | Narrow)
    }

    fn specimen_height(&self, _state: SpecimenState) -> u16 {
        6
    }

    fn specimen(
        &self,
        state: SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        _ctx: &mut RenderCtx,
        _bg: Color,
    ) {
        let source = match state {
            SpecimenState::Empty => &self.empty,
            SpecimenState::Overflow => &self.overflow,
            SpecimenState::Narrow => &self.narrow,
            _ => &self.rest,
        };
        TerminalView::new(source)
            .focused(state == SpecimenState::Focused)
            .render(area, buf);
    }
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    StateGrid::new(id)
        .label_width(10)
        .column_width(40)
        .rows(&[Rest, Focused, Empty, Overflow, Narrow])
        .column("Terminal pane", PaneSpecimen::new(id.child(0)))
}

pub struct TerminalPanePage {
    template: Template,
    opts: PageValues,
    state: TerminalState,
    fixture: usize,
    next: usize,
    leave_fed: bool,
    capturing: bool,
    echo: Option<String>,
    step: Button,
    play: Button,
    replay: Button,
    reset: Button,
}

impl TerminalPanePage {
    pub fn new() -> Self {
        let mut page = Self {
            template: Template::new(ID, states_grid()),
            opts: PageValues::new(page_options(PageId::TerminalPane)),
            state: TerminalState::new(12, 40),
            fixture: 0,
            next: 0,
            leave_fed: false,
            capturing: false,
            echo: None,
            step: Button::secondary(ID.sub("step"), "Step"),
            play: Button::secondary(ID.sub("play"), "Play all"),
            replay: Button::secondary(ID.sub("replay"), "Replay"),
            reset: Button::secondary(ID.sub("reset"), "Reset"),
        };
        page.load();
        page
    }

    fn fixture(&self) -> &'static Fixture {
        &FIXTURES[self.fixture]
    }

    fn fresh(&mut self) {
        let (rows, cols) = size_of(self.opts.value("terminalpane.size"));
        self.state = TerminalState::new(rows, cols);
        self.next = 0;
        self.leave_fed = false;
    }

    fn load(&mut self) {
        self.fixture = fixture_index(self.opts.value("terminalpane.fixture"));
        self.fresh();
        self.echo = None;
        if self.fixture().name != "midstream" {
            self.play_all();
        }
    }

    fn step(&mut self) {
        let fixture = self.fixture();
        if let Some(chunk) = fixture.chunks.get(self.next) {
            self.state.feed(chunk);
            self.next += 1;
        } else if let Some(leave) = fixture.leave
            && !self.leave_fed
        {
            self.state.feed(leave);
            self.leave_fed = true;
        }
    }

    fn play_all(&mut self) {
        while self.next < self.fixture().chunks.len() {
            self.step();
        }
    }

    fn replay(&mut self) {
        let (chunk, offset) = REPLAY_STARTS[self
            .opts
            .get("terminalpane.replay")
            .unwrap_or(0)
            .min(REPLAY_STARTS.len() - 1)];
        self.fresh();
        let chunks = self.fixture().chunks;
        if let Some(bytes) = chunks.get(chunk) {
            self.state.feed(&bytes[offset.min(bytes.len())..]);
            self.next = chunk + 1;
        }
    }

    fn reset(&mut self) {
        self.fresh();
        self.echo = None;
    }

    fn press(&mut self, key: &Key) {
        let modes = self.state.modes();
        self.echo = Some(match encode_key(key, &modes) {
            Some(bytes) => {
                let hex: Vec<String> = bytes.iter().map(|b| format!("{b:02x}")).collect();
                format!("{} → {}  ({})", key_name(key), caret(&bytes), hex.join(" "))
            }
            None => format!("{} → not encoded", key_name(key)),
        });
    }

    fn scroll(&mut self, rows: isize) {
        let at = self.state.scrollback() as isize;
        let max = self.state.scrollback_len() as isize;
        self.state
            .set_scrollback((at + rows).clamp(0, max) as usize);
    }

    fn props(&self) -> Vec<Prop> {
        let Modes {
            app_cursor,
            bracketed_paste,
            alt_screen,
            hide_cursor,
        } = self.state.modes();
        let (rows, cols) = self.state.size();
        let flag = |on: bool| if on { "on" } else { "off" };
        let cursor = match self.state.cursor(Rect::new(0, 0, cols, rows)) {
            Some(p) => format!("row {}, col {}", p.y + 1, p.x + 1),
            None => "hidden".to_owned(),
        };
        vec![
            Prop::new("app cursor", flag(app_cursor)),
            Prop::new("bracketed paste", flag(bracketed_paste)),
            Prop::new("alt screen", flag(alt_screen)),
            Prop::new("hide cursor", flag(hide_cursor)),
            Prop::new("size", format!("{cols}x{rows}")),
            Prop::new("cursor", cursor),
            Prop::new(
                "scrollback",
                format!(
                    "{} rows · viewing {} up",
                    self.state.scrollback_len(),
                    self.state.scrollback()
                ),
            ),
        ]
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let (rows, cols) = self.state.size();
        let fixture = self.fixture();
        let title = fixture.name;
        let progress = format!(
            "{} of {} chunks",
            self.next + usize::from(self.leave_fed),
            fixture.chunks.len() + usize::from(fixture.leave.is_some()),
        );
        let panel = Panel::framed(Some(title))
            .meta(&progress)
            .focused(ctx.interaction.focused(PANE));
        let outer = panel.outer(Size::new(cols, rows));
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
        ctx.control(PANE, inner, false);
        ctx.clickable(PANE, frame);
        TerminalView::new(&self.state)
            .focused(
                self.opts.on("terminalpane.focused") && ctx.interaction.focused(PANE)
                    || self.opts.on("terminalpane.focused") && self.capturing,
            )
            .color(self.opts.on("terminalpane.colour"))
            .render(inner, buf);

        let side_x = frame.right() + 2;
        if area.right() > side_x + 24 {
            let side = Rect::new(side_x, area.y, area.right() - side_x, frame.height);
            props::render(side, buf, t, &self.props(), t.canvas);
        }

        let controls_y = frame.bottom() + 1;
        if controls_y < area.bottom() {
            let widths = [
                self.step.width(),
                self.play.width(),
                self.replay.width(),
                self.reset.width(),
            ];
            let rects = row_layout(Rect::new(area.x, controls_y, area.width, 1), &widths, 2);
            self.step.render(rects[0], buf, ctx, t.canvas);
            self.play.render(rects[1], buf, ctx, t.canvas);
            self.replay.render(rects[2], buf, ctx, t.canvas);
            self.reset.render(rects[3], buf, ctx, t.canvas);
        }

        let echo_y = controls_y + 2;
        let (label, tone) = match (&self.echo, self.capturing) {
            (Some(e), _) => (format!("key  {e}"), Tone::Normal),
            (None, true) => (
                "key  press a key · nothing is sent anywhere".to_owned(),
                Tone::Muted,
            ),
            (None, false) => (
                "key  Enter on the pane captures keys".to_owned(),
                Tone::Muted,
            ),
        };
        if echo_y < area.bottom() {
            buf.set_string(
                area.x,
                echo_y,
                junie_tui::ui::text::truncate(&label, area.width as usize),
                ratatui::style::Style::new().fg(t.tone(tone)).bg(t.canvas),
            );
        }
        if echo_y + 1 < area.bottom() {
            buf.set_string(
                area.x,
                echo_y + 1,
                junie_tui::ui::text::truncate(
                    "A stream that starts inside a sequence prints its tail as text until the next full redraw.",
                    area.width as usize,
                ),
                t.faint().bg(bg),
            );
        }
        let _ = bg;
    }

    fn pointer(&mut self, ev: &PointerEvent, cx: &mut Cx) -> Outcome {
        if ev.id == PANE && ev.is(Phase::Click) {
            cx.focus.focus(PANE);
            return Outcome::Changed;
        }
        let fired = (0..4).find(|&i| self.button(i).route(ev).is_some_and(|r| r.has_actions()));
        match fired {
            Some(i) => {
                self.run(i);
                Outcome::Changed
            }
            None => Outcome::Ignored,
        }
    }

    fn button(&mut self, i: usize) -> &mut Button {
        match i {
            0 => &mut self.step,
            1 => &mut self.play,
            2 => &mut self.replay,
            _ => &mut self.reset,
        }
    }

    fn run(&mut self, i: usize) {
        match i {
            0 => self.step(),
            1 => self.play_all(),
            2 => self.replay(),
            _ => self.reset(),
        }
    }
}

impl Page for TerminalPanePage {
    fn title(&self) -> &'static str {
        "Terminal pane"
    }

    fn blurb(&self) -> &'static str {
        "A terminal emulator state and view · feed bytes in chunks · see the modes and the encoded keys"
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
        if self.capturing
            && let PageEvent::Key(key) = ev
        {
            if key.ctrl() && key.code == KeyCode::Char('g') {
                self.capturing = false;
                cx.status("Keys released");
            } else {
                self.press(key);
            }
            return Outcome::Changed;
        }
        if let Some(o) = self.template.handle(ev, cx) {
            return o;
        }
        if self.template.section() != Section::Playground {
            return Outcome::Ignored;
        }
        match ev {
            PageEvent::Key(key) => {
                if cx.focus.is(PANE) {
                    match key.code {
                        KeyCode::Enter => {
                            self.capturing = true;
                            return Outcome::Changed;
                        }
                        KeyCode::PageUp => {
                            self.scroll(self.state.size().0 as isize);
                            return Outcome::Changed;
                        }
                        KeyCode::PageDown => {
                            self.scroll(-(self.state.size().0 as isize));
                            return Outcome::Changed;
                        }
                        _ => return Outcome::Ignored,
                    }
                }
                let focused = (0..4).find(|&i| cx.focus.is(self.button(i).id));
                match focused {
                    Some(i) => {
                        let (o, fired) = self.button(i).on_key(key);
                        if fired {
                            self.run(i);
                        }
                        o
                    }
                    None => Outcome::Ignored,
                }
            }
            PageEvent::Pointer(ev) => self.pointer(ev, cx),
            _ => Outcome::Ignored,
        }
    }

    fn editing(&self) -> bool {
        self.capturing
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if self.capturing {
            return vec![("Ctrl+G", "Release")];
        }
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        if focus == Some(PANE) {
            vec![("Enter", "Capture keys"), ("PgUp PgDn", "Scrollback")]
        } else {
            vec![("Enter", "Activate")]
        }
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        self.opts.get(key)
    }

    fn set_option(&mut self, key: &str, value: usize) {
        if self.opts.set(key, value) && matches!(key, "terminalpane.fixture" | "terminalpane.size")
        {
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
