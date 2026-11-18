//! The backend the runtime draws through, instrumented for work counts (#56).
//!
//! [`MeteredBackend`] owns the writer end of the terminal, so it is the one
//! place where the cells a frame diff hands over and the bytes finally written
//! can be counted. It records two metrics:
//!
//! - `DiffCellsVisited`: cells passed to [`Backend::draw`], which is what the
//!   buffer diff produced. The comparison of the two buffers that produced
//!   them happens inside `ratatui` and is not counted.
//! - `TerminalBytesWritten`: bytes the writer accepted, escape sequences,
//!   cursor moves and synchronized-update markers included.
//!
//! The module exists only with the `work-metrics` feature. Without it the
//! runtime draws through plain `CrosstermBackend` and
//! [`SessionTerminal`](super::SessionTerminal) is `DefaultTerminal`. The types
//! are not re-exported: they are reachable only as the backend of that alias.

use std::io::{self, Write};

use ratatui::backend::{Backend, ClearType, CrosstermBackend, WindowSize};
use ratatui::buffer::Cell;
use ratatui::layout::{Position, Size};

/// A writer that counts the bytes it accepts.
#[derive(Debug)]
pub struct MeteredWriter<W: Write> {
    inner: W,
}

impl<W: Write> Write for MeteredWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(buf)?;
        termrock_metrics::record_work!(TerminalBytesWritten, written);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// A crossterm backend over a [`MeteredWriter`]. It draws and writes exactly
/// as [`CrosstermBackend`] does.
#[derive(Debug)]
pub struct MeteredBackend<W: Write> {
    inner: CrosstermBackend<MeteredWriter<W>>,
}

impl<W: Write> MeteredBackend<W> {
    pub fn new(writer: W) -> Self {
        Self {
            inner: CrosstermBackend::new(MeteredWriter { inner: writer }),
        }
    }
}

impl<W: Write> Write for MeteredBackend<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Write::write(&mut self.inner, buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Write::flush(&mut self.inner)
    }
}

/// Counts the cells it yields, then hands them on unchanged.
struct Counted<I>(I);

impl<'a, I: Iterator<Item = (u16, u16, &'a Cell)>> Iterator for Counted<I> {
    type Item = (u16, u16, &'a Cell);

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.0.next();
        if item.is_some() {
            termrock_metrics::record_work!(DiffCellsVisited, 1usize);
        }
        item
    }
}

impl<W: Write> Backend for MeteredBackend<W> {
    type Error = io::Error;

    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        self.inner.draw(Counted(content))
    }

    fn append_lines(&mut self, n: u16) -> io::Result<()> {
        self.inner.append_lines(n)
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.inner.hide_cursor()
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.inner.show_cursor()
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        self.inner.get_cursor_position()
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        self.inner.set_cursor_position(position)
    }

    fn clear(&mut self) -> io::Result<()> {
        self.inner.clear()
    }

    fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
        self.inner.clear_region(clear_type)
    }

    fn size(&self) -> io::Result<Size> {
        self.inner.size()
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        self.inner.window_size()
    }

    fn flush(&mut self) -> io::Result<()> {
        Backend::flush(&mut self.inner)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use ratatui::backend::CrosstermBackend;
    use ratatui::layout::Rect;
    use ratatui::style::{Color, Style};
    use ratatui::widgets::Paragraph;
    use ratatui::{Terminal, TerminalOptions, Viewport};

    use super::*;

    #[derive(Clone, Default)]
    struct Shared(Arc<Mutex<Vec<u8>>>);

    impl Write for Shared {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().expect("bytes").write(buf)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn options() -> TerminalOptions {
        TerminalOptions {
            viewport: Viewport::Fixed(Rect::new(0, 0, 20, 4)),
        }
    }

    /// The meter must not change what reaches the terminal.
    #[test]
    fn metered_backend_writes_the_bytes_crossterm_writes() {
        let plain = Shared::default();
        let metered = Shared::default();
        let mut a = Terminal::with_options(CrosstermBackend::new(plain.clone()), options())
            .expect("plain terminal");
        let mut b = Terminal::with_options(MeteredBackend::new(metered.clone()), options())
            .expect("metered terminal");
        for text in ["hello wide world", "hello wide there", "hello wide there"] {
            let paint = |f: &mut ratatui::Frame| {
                f.render_widget(
                    Paragraph::new(text).style(Style::new().fg(Color::Red)),
                    f.area(),
                );
            };
            a.draw(paint).expect("plain draw");
            b.draw(paint).expect("metered draw");
        }
        let plain = plain.0.lock().expect("bytes").clone();
        let metered = metered.0.lock().expect("bytes").clone();
        assert!(!plain.is_empty());
        assert_eq!(metered, plain);
    }
}
