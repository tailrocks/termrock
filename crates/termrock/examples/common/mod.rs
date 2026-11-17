//! What every example shares: one headless frame and one key constructor.
//! A real application replaces `frame` with its `Terminal::draw` call.

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use termrock::core::event::Key;
use termrock::core::focus::FocusRing;
use termrock::core::hit::HitRegistry;
use termrock::theme::{ColorLevel, Theme};
use termrock::ui::ctx::{Interaction, RenderCtx};

pub fn key(code: KeyCode) -> Key {
    Key {
        code,
        mods: KeyModifiers::NONE,
    }
}

/// Paints one `width` x `height` frame into a `TestBackend`. Returns the screen
/// as text, one line per row, and whatever `paint` returns.
pub fn frame<R>(
    width: u16,
    height: u16,
    paint: impl FnOnce(&mut Buffer, &mut RenderCtx) -> R,
) -> (String, R) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test backend");
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut out = None;
    terminal
        .draw(|f| {
            let mut hits = HitRegistry::default();
            let mut ring = FocusRing::default();
            let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
            out = Some(paint(f.buffer_mut(), &mut ctx));
            ctx.flush_overlays(f.buffer_mut());
        })
        .expect("draw");
    let buf = terminal.backend().buffer();
    let screen = (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n");
    (screen, out.expect("the closure ran"))
}
