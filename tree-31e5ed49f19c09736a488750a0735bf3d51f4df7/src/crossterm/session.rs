use std::io::{self, Write};

use crossterm::{
    event::{DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionOptions {
    pub alternate_screen: bool,
    pub mouse_capture: bool,
    pub bracketed_paste: bool,
    pub raw_mode: bool,
}

impl Default for SessionOptions {
    fn default() -> Self {
        Self {
            alternate_screen: true,
            mouse_capture: true,
            bracketed_paste: true,
            raw_mode: true,
        }
    }
}

pub struct Session<W: Write> {
    writer: W,
    alternate_screen: bool,
    mouse_capture: bool,
    bracketed_paste: bool,
    raw_mode: bool,
}

impl<W: Write> Session<W> {
    pub fn enter(writer: W, options: SessionOptions) -> io::Result<Self> {
        let mut session = Self {
            writer,
            alternate_screen: false,
            mouse_capture: false,
            bracketed_paste: false,
            raw_mode: false,
        };
        let result = (|| {
            if options.raw_mode {
                enable_raw_mode()?;
                session.raw_mode = true;
            }
            if options.alternate_screen {
                execute!(&mut session.writer, EnterAlternateScreen)?;
                session.alternate_screen = true;
            }
            if options.mouse_capture {
                execute!(&mut session.writer, EnableMouseCapture)?;
                session.mouse_capture = true;
            }
            if options.bracketed_paste {
                execute!(&mut session.writer, EnableBracketedPaste)?;
                session.bracketed_paste = true;
            }
            Ok(())
        })();
        if let Err(error) = result {
            let _ = session.restore();
            return Err(error);
        }
        Ok(session)
    }

    pub fn restore(&mut self) -> io::Result<()> {
        let mut first = None;
        if self.bracketed_paste {
            if let Err(error) = execute!(&mut self.writer, DisableBracketedPaste) {
                first = Some(error);
            } else {
                self.bracketed_paste = false;
            }
        }
        if self.mouse_capture {
            if let Err(error) = execute!(&mut self.writer, DisableMouseCapture) {
                if first.is_none() {
                    first = Some(error);
                }
            } else {
                self.mouse_capture = false;
            }
        }
        if self.alternate_screen {
            if let Err(error) = execute!(&mut self.writer, LeaveAlternateScreen) {
                if first.is_none() {
                    first = Some(error);
                }
            } else {
                self.alternate_screen = false;
            }
        }
        if self.raw_mode {
            if let Err(error) = disable_raw_mode() {
                if first.is_none() {
                    first = Some(error);
                }
            } else {
                self.raw_mode = false;
            }
        }
        self.writer.flush()?;
        first.map_or(Ok(()), Err)
    }

    #[must_use]
    pub fn writer_mut(&mut self) -> &mut W {
        &mut self.writer
    }
}

impl<W: Write> Drop for Session<W> {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}
