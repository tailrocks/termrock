#![allow(dead_code)]

use std::time::Duration;

use junie_tui::core::event::{External, Input, Key, Mouse, MouseKind, Outcome};
use junie_tui::core::focus::FocusRing;
use junie_tui::core::hit::HitRegistry;
use junie_tui::core::id::WidgetId;
use junie_tui::core::refusal::{Reason, raise, take_refusal};
use junie_tui::shell::{App, Cx, Request, Shell, handle};
use junie_tui::theme::{ColorLevel, Theme, Tone};
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::widgets::too_small::TooSmall;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Style};

pub const WHY: &str = "Always off · the host decides";

pub fn key(code: KeyCode) -> Key {
    Key {
        code,
        mods: KeyModifiers::NONE,
    }
}

pub fn chord(code: KeyCode, mods: KeyModifiers) -> Key {
    Key { code, mods }
}

pub fn reason() -> Reason {
    Reason::new(WHY).short("managed")
}

pub fn taken() -> Option<String> {
    take_refusal().map(|r| r.text)
}

pub fn draw(
    focus: Option<WidgetId>,
    paint: impl FnOnce(&mut Buffer, &mut RenderCtx),
) -> (Buffer, FocusRing) {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(Rect::new(0, 0, 60, 8));
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let interaction = Interaction {
        focus,
        ..Default::default()
    };
    {
        let mut ctx = RenderCtx::new(&theme, interaction, &mut hits, &mut ring);
        paint(&mut buf, &mut ctx);
    }
    (buf, ring)
}

pub fn row(buf: &Buffer, y: u16) -> String {
    (0..buf.area.width)
        .map(|x| buf[(x, y)].symbol().to_owned())
        .collect::<String>()
}

pub fn fg_at(buf: &Buffer, x: u16, y: u16) -> ratatui::style::Color {
    buf[(x, y)].fg
}

pub enum Act {
    Nothing,
    Raise(Reason),
    Status(&'static str, Tone),
    Sticky(&'static str),
    Refuse(Reason),
}

pub type Paint = Box<dyn FnMut(&mut Buffer, &mut RenderCtx)>;

pub struct Host {
    shell: Shell<()>,
    pub act: Act,
    pub seen_at_entry: Option<Option<Reason>>,
    pub inputs: usize,
    pub stepped: Vec<u64>,
    pub draws: usize,
    paint: Option<Paint>,
}

impl Host {
    pub fn new(act: Act) -> Self {
        Self {
            shell: Shell::new(),
            act,
            seen_at_entry: None,
            inputs: 0,
            stepped: Vec::new(),
            draws: 0,
            paint: None,
        }
    }

    pub fn painting(paint: impl FnMut(&mut Buffer, &mut RenderCtx) + 'static) -> Self {
        Self {
            paint: Some(Box::new(paint)),
            ..Self::new(Act::Nothing)
        }
    }

    pub fn status(&self) -> Option<(String, Tone)> {
        self.shell
            .status
            .get()
            .map(|(text, tone)| (text.to_owned(), tone))
    }

    pub fn stop_handle(&self) -> junie_tui::shell::StopHandle {
        self.shell.stop_handle()
    }

    pub fn test_requests(&mut self, on: bool) {
        self.shell.test_requests = on;
    }

    pub fn set_status(&mut self, text: &str, tone: Tone) {
        self.shell.status.set(text, tone);
    }

    pub fn sticky(&mut self, text: &str) {
        self.shell.status.sticky(text);
    }

    pub fn input(&mut self, input: Input) -> Outcome {
        handle(self, input)
    }

    pub fn press(&mut self, code: KeyCode) -> Outcome {
        self.input(Input::Key(key(code)))
    }
}

pub fn every_input() -> Vec<(&'static str, Input)> {
    vec![
        ("key", Input::Key(key(KeyCode::Char('x')))),
        (
            "mouse press",
            Input::Mouse(Mouse::new(MouseKind::Down, Position::new(1, 1))),
        ),
        ("paste", Input::Paste("text".into())),
        ("tick", Input::Tick),
        ("resize", Input::Resize(100, 30)),
        ("external", Input::External(External::new(7_u8))),
    ]
}

impl App for Host {
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
        TooSmall::new("host")
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

    fn draw(&mut self, _area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        self.draws += 1;
        if let Some(paint) = self.paint.as_mut() {
            paint(buf, ctx);
        }
    }

    fn advance_clock(&mut self, ms: u64) -> Option<Outcome> {
        self.stepped.push(ms);
        Some(Outcome::Changed)
    }

    fn on_input(&mut self, _input: Input) -> Outcome {
        self.inputs += 1;
        self.seen_at_entry = Some(take_refusal());
        match &self.act {
            Act::Nothing => Outcome::Ignored,
            Act::Raise(reason) => {
                raise(reason);
                Outcome::Consumed
            }
            Act::Status(text, tone) => {
                self.shell.status.set(*text, *tone);
                Outcome::Consumed
            }
            Act::Sticky(text) => {
                self.shell.status.sticky(*text);
                Outcome::Consumed
            }
            Act::Refuse(reason) => {
                let mut cx: Cx<'_, (), (), ()> = Cx::new(&mut self.shell.focus, &self.shell.ring);
                cx.refused(reason.clone());
                for request in cx.take() {
                    if let Request::Status(text, tone) = request {
                        self.shell.status.set(text, tone);
                    }
                }
                Outcome::Consumed
            }
        }
    }
}

pub fn sentinel(area: Rect) -> Buffer {
    let mut buf = Buffer::empty(area);
    for cell in &mut buf.content {
        cell.set_symbol("#");
        cell.set_style(Style::new().fg(Color::Magenta).bg(Color::Blue));
    }
    buf
}

pub fn assert_outside_untouched(buf: &Buffer, owner: Rect, what: &str) {
    let area = buf.area;
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            if owner.contains((x, y).into()) {
                continue;
            }
            let cell = &buf[(x, y)];
            assert!(
                cell.symbol() == "#"
                    && cell.fg == Color::Magenta
                    && cell.bg == Color::Blue
                    && cell.modifier.is_empty(),
                "{what}: cell ({x},{y}) outside {owner:?} was written: {:?} {:?}",
                cell.symbol(),
                cell.style()
            );
        }
    }
}

pub fn contains(outer: Rect, inner: Rect) -> bool {
    inner.is_empty() || outer.intersection(inner) == inner
}

pub struct Painted {
    pub buf: Buffer,
    pub hits: HitRegistry,
    pub ring: FocusRing,
}

pub fn paint_in(
    screen: Rect,
    focus: Option<WidgetId>,
    paint: impl FnOnce(&mut Buffer, &mut RenderCtx),
) -> Painted {
    paint_with(
        &Theme::for_level(ColorLevel::TrueColor),
        screen,
        focus,
        paint,
    )
}

pub fn paint_with(
    theme: &Theme,
    screen: Rect,
    focus: Option<WidgetId>,
    paint: impl FnOnce(&mut Buffer, &mut RenderCtx),
) -> Painted {
    let mut buf = sentinel(screen);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let interaction = Interaction {
        focus,
        ..Default::default()
    };
    let mut ctx = RenderCtx::new(theme, interaction, &mut hits, &mut ring);
    paint(&mut buf, &mut ctx);
    ctx.flush_overlays(&mut buf);
    Painted { buf, hits, ring }
}

pub fn assert_hits_inside(p: &Painted, owner: Rect, what: &str) {
    for r in p.hits.regions() {
        assert!(
            contains(owner, r.area),
            "{what}: hit {:?} escapes {owner:?}",
            r.area
        );
    }
}
