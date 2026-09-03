use std::io::{self, Write};

use crossterm::{
    cursor::{Hide, Show},
    event::{DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture},
    execute,
    terminal::{
        DisableLineWrap, EnableLineWrap, EnterAlternateScreen, LeaveAlternateScreen,
        disable_raw_mode, enable_raw_mode,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Terminal modes acquired and restored by a [`Session`].
pub struct SessionOptions {
    /// Enter the terminal's alternate screen buffer.
    pub alternate_screen: bool,
    /// Enable mouse event capture.
    pub mouse_capture: bool,
    /// Enable bracketed paste reporting.
    pub bracketed_paste: bool,
    /// Enable terminal raw mode.
    pub raw_mode: bool,
    /// Hide the terminal cursor for the session.
    pub hide_cursor: bool,
    /// Disable terminal line wrapping for the session.
    pub disable_line_wrap: bool,
}

impl Default for SessionOptions {
    fn default() -> Self {
        Self {
            alternate_screen: true,
            mouse_capture: true,
            bracketed_paste: true,
            raw_mode: true,
            hide_cursor: true,
            disable_line_wrap: true,
        }
    }
}

/// An owned Crossterm terminal session with deterministic cleanup.
pub struct Session<W: Write> {
    writer: W,
    alternate_screen: bool,
    mouse_capture: bool,
    bracketed_paste: bool,
    line_wrap_disabled: bool,
    cursor_hidden: bool,
    raw_mode: bool,
}

impl<W: Write> Session<W> {
    /// Acquires the requested terminal modes and records their cleanup obligations.
    pub fn enter(writer: W, options: SessionOptions) -> io::Result<Self> {
        let mut session = Self {
            writer,
            alternate_screen: false,
            mouse_capture: false,
            bracketed_paste: false,
            line_wrap_disabled: false,
            cursor_hidden: false,
            raw_mode: false,
        };
        let result = (|| {
            if options.raw_mode {
                session.raw_mode = true;
                enable_raw_mode()?;
            }
            if options.alternate_screen {
                session.alternate_screen = true;
                execute!(&mut session.writer, EnterAlternateScreen)?;
            }
            if options.mouse_capture {
                session.mouse_capture = true;
                execute!(&mut session.writer, EnableMouseCapture)?;
            }
            if options.bracketed_paste {
                session.bracketed_paste = true;
                execute!(&mut session.writer, EnableBracketedPaste)?;
            }
            if options.disable_line_wrap {
                session.line_wrap_disabled = true;
                execute!(&mut session.writer, DisableLineWrap)?;
            }
            if options.hide_cursor {
                session.cursor_hidden = true;
                execute!(&mut session.writer, Hide)?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            let _ = session.restore();
            return Err(error);
        }
        Ok(session)
    }

    /// Restores every acquired terminal mode in reverse acquisition order.
    pub fn restore(&mut self) -> io::Result<()> {
        let mut first = None;
        if self.cursor_hidden && record_first(&mut first, execute!(&mut self.writer, Show)) {
            self.cursor_hidden = false;
        }
        if self.line_wrap_disabled
            && record_first(&mut first, execute!(&mut self.writer, EnableLineWrap))
        {
            self.line_wrap_disabled = false;
        }
        if self.bracketed_paste
            && record_first(
                &mut first,
                execute!(&mut self.writer, DisableBracketedPaste),
            )
        {
            self.bracketed_paste = false;
        }
        if self.mouse_capture
            && record_first(&mut first, execute!(&mut self.writer, DisableMouseCapture))
        {
            self.mouse_capture = false;
        }
        if self.alternate_screen
            && record_first(&mut first, execute!(&mut self.writer, LeaveAlternateScreen))
        {
            self.alternate_screen = false;
        }
        if self.raw_mode && record_first(&mut first, disable_raw_mode()) {
            self.raw_mode = false;
        }
        record_first(&mut first, self.writer.flush());
        first.map_or(Ok(()), Err)
    }

    #[must_use]
    /// Returns mutable access to the session writer.
    pub fn writer_mut(&mut self) -> &mut W {
        &mut self.writer
    }
}

fn record_first(first: &mut Option<io::Error>, result: io::Result<()>) -> bool {
    match result {
        Ok(()) => true,
        Err(error) => {
            if first.is_none() {
                *first = Some(error);
            }
            false
        }
    }
}

impl<W: Write> Drop for Session<W> {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::*;

    #[test]
    fn owns_and_restores_every_writer_backed_mode_in_reverse_order() {
        let options = SessionOptions {
            raw_mode: false,
            ..SessionOptions::default()
        };
        let mut session = Session::enter(Vec::new(), options).expect("in-memory session");

        session.restore().expect("restore session");
        let bytes = session.writer_mut();
        let text = String::from_utf8_lossy(bytes);
        let sequences = [
            "\u{1b}[?1049h",
            "\u{1b}[?1000h",
            "\u{1b}[?2004h",
            "\u{1b}[?7l",
            "\u{1b}[?25l",
            "\u{1b}[?25h",
            "\u{1b}[?7h",
            "\u{1b}[?2004l",
            "\u{1b}[?1000l",
            "\u{1b}[?1049l",
        ];
        let positions = sequences.map(|sequence| {
            text.find(sequence)
                .unwrap_or_else(|| panic!("missing terminal sequence {sequence:?}"))
        });
        assert!(
            positions.windows(2).all(|pair| pair[0] < pair[1]),
            "terminal modes must acquire forward and restore in exact reverse order"
        );
    }

    #[test]
    fn inline_session_can_hide_cursor() {
        let options = SessionOptions {
            alternate_screen: false,
            mouse_capture: false,
            bracketed_paste: false,
            raw_mode: false,
            hide_cursor: true,
            disable_line_wrap: false,
        };
        let mut session = Session::enter(Vec::new(), options).expect("in-memory session");
        assert_eq!(session.writer_mut().as_slice(), b"\x1b[?25l");

        session.restore().expect("restore session");
        assert_eq!(session.writer_mut().as_slice(), b"\x1b[?25l\x1b[?25h");
    }

    #[test]
    fn alternate_screen_can_keep_cursor_visible() {
        let options = SessionOptions {
            alternate_screen: true,
            mouse_capture: false,
            bracketed_paste: false,
            raw_mode: false,
            hide_cursor: false,
            disable_line_wrap: false,
        };
        let mut session = Session::enter(Vec::new(), options).expect("in-memory session");
        assert_eq!(session.writer_mut().as_slice(), b"\x1b[?1049h");

        session.restore().expect("restore session");
        assert_eq!(session.writer_mut().as_slice(), b"\x1b[?1049h\x1b[?1049l");
    }

    #[test]
    fn default_writer_backed_modes_are_unchanged() {
        let options = SessionOptions {
            raw_mode: false,
            ..SessionOptions::default()
        };
        let mut session = Session::enter(Vec::new(), options).expect("in-memory session");
        assert_eq!(
            session.writer_mut().as_slice(),
            b"\x1b[?1049h\x1b[?1000h\x1b[?1002h\x1b[?1003h\x1b[?1015h\x1b[?1006h\x1b[?2004h\x1b[?7l\x1b[?25l"
        );
    }

    #[test]
    fn restore_is_idempotent_for_all_writer_backed_modes() {
        let options = SessionOptions {
            raw_mode: false,
            ..SessionOptions::default()
        };
        let mut session = Session::enter(Vec::new(), options).expect("in-memory session");
        session.restore().expect("first restore");
        let first_length = session.writer_mut().len();
        session.restore().expect("second restore");
        assert_eq!(session.writer_mut().len(), first_length);
    }

    #[test]
    fn partial_write_at_each_acquisition_restores_every_armed_mode() {
        let inverses = [
            "\u{1b}[?1049l",
            "\u{1b}[?1000l",
            "\u{1b}[?2004l",
            "\u{1b}[?7h",
            "\u{1b}[?25h",
        ];
        for target_acquisition in 1..=inverses.len() {
            let state = Rc::new(RefCell::new(PartialWriterState {
                target_acquisition,
                ..PartialWriterState::default()
            }));
            let writer = PartialThenFailWriter {
                state: Rc::clone(&state),
            };
            let options = SessionOptions {
                raw_mode: false,
                ..SessionOptions::default()
            };

            assert!(Session::enter(writer, options).is_err());
            let bytes = state.borrow();
            assert!(
                bytes.partial_written,
                "target {target_acquisition} was not partially written"
            );
            let text = String::from_utf8_lossy(&bytes.bytes);
            for inverse in &inverses[..target_acquisition] {
                assert!(
                    text.contains(inverse),
                    "partial acquisition {target_acquisition} did not restore {inverse:?}"
                );
            }
        }
    }

    #[test]
    fn failure_at_each_acquisition_flush_restores_every_armed_mode() {
        let inverses = [
            "\u{1b}[?1049l",
            "\u{1b}[?1000l",
            "\u{1b}[?2004l",
            "\u{1b}[?7h",
            "\u{1b}[?25h",
        ];
        for fail_flush_at in 1..=inverses.len() {
            let state = Rc::new(RefCell::new(WriterState {
                fail_flush_at,
                ..WriterState::default()
            }));
            let writer = FailOnceWriter {
                state: Rc::clone(&state),
            };
            let options = SessionOptions {
                raw_mode: false,
                ..SessionOptions::default()
            };

            assert!(Session::enter(writer, options).is_err());
            let bytes = state.borrow();
            let text = String::from_utf8_lossy(&bytes.bytes);
            for inverse in &inverses[..fail_flush_at] {
                assert!(
                    text.contains(inverse),
                    "flush {fail_flush_at} did not restore {inverse:?}"
                );
            }
        }
    }

    #[test]
    fn flush_failure_after_acquisition_still_runs_safe_inverse() {
        let state = Rc::new(RefCell::new(WriterState {
            fail_flush_at: 1,
            ..WriterState::default()
        }));
        let writer = FailOnceWriter {
            state: Rc::clone(&state),
        };
        let options = SessionOptions {
            alternate_screen: true,
            mouse_capture: false,
            bracketed_paste: false,
            raw_mode: false,
            hide_cursor: false,
            disable_line_wrap: false,
        };

        assert!(Session::enter(writer, options).is_err());
        let bytes = state.borrow();
        let text = String::from_utf8_lossy(&bytes.bytes);
        assert!(text.contains("\u{1b}[?1049h"));
        assert!(text.contains("\u{1b}[?1049l"));
    }

    #[test]
    fn failed_restore_keeps_cleanup_armed_for_retry() {
        let state = Rc::new(RefCell::new(WriterState::default()));
        let writer = FailOnceWriter {
            state: Rc::clone(&state),
        };
        let options = SessionOptions {
            raw_mode: false,
            ..SessionOptions::default()
        };
        let mut session = Session::enter(writer, options).expect("enter session");
        {
            let mut state = state.borrow_mut();
            state.fail_write_at = state.writes + 1;
            state.failed_write = false;
        }

        assert!(session.restore().is_err());
        session.restore().expect("retry failed cleanup");
        assert!(
            String::from_utf8_lossy(&state.borrow().bytes).contains("\u{1b}[?25h"),
            "cursor restore must be retried"
        );
    }

    #[test]
    fn multiple_restore_failures_return_the_earliest_error() {
        let state = Rc::new(RefCell::new(MultiFailState::default()));
        let writer = MultiFailWriter {
            state: Rc::clone(&state),
        };
        let options = SessionOptions {
            raw_mode: false,
            ..SessionOptions::default()
        };
        let mut session = Session::enter(writer, options).expect("enter session");
        {
            let mut state = state.borrow_mut();
            let next = state.writes + 1;
            state.failures = vec![
                (next, "first cleanup failure"),
                (next + 1, "second cleanup failure"),
            ];
        }

        let error = session.restore().expect_err("cleanup must fail");
        assert_eq!(error.to_string(), "first cleanup failure");
        assert!(
            state.borrow().failures.is_empty(),
            "all cleanup paths must run"
        );
    }

    #[derive(Debug)]
    struct WriterState {
        writes: usize,
        flushes: usize,
        fail_write_at: usize,
        fail_flush_at: usize,
        failed_write: bool,
        failed_flush: bool,
        bytes: Vec<u8>,
    }

    impl Default for WriterState {
        fn default() -> Self {
            Self {
                writes: 0,
                flushes: 0,
                fail_write_at: usize::MAX,
                fail_flush_at: usize::MAX,
                failed_write: false,
                failed_flush: false,
                bytes: Vec::new(),
            }
        }
    }

    #[derive(Debug)]
    struct FailOnceWriter {
        state: Rc<RefCell<WriterState>>,
    }

    impl Write for FailOnceWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            let mut state = self.state.borrow_mut();
            state.writes += 1;
            if !state.failed_write && state.writes == state.fail_write_at {
                state.failed_write = true;
                return Err(io::Error::other("injected writer failure"));
            }
            state.bytes.extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            let mut state = self.state.borrow_mut();
            state.flushes += 1;
            if !state.failed_flush && state.flushes == state.fail_flush_at {
                state.failed_flush = true;
                return Err(io::Error::other("injected flush failure"));
            }
            Ok(())
        }
    }

    #[derive(Debug)]
    struct PartialThenFailWriter {
        state: Rc<RefCell<PartialWriterState>>,
    }

    #[derive(Debug)]
    struct PartialWriterState {
        target_acquisition: usize,
        completed_acquisitions: usize,
        partial_written: bool,
        failed: bool,
        bytes: Vec<u8>,
    }

    impl Default for PartialWriterState {
        fn default() -> Self {
            Self {
                target_acquisition: usize::MAX,
                completed_acquisitions: 0,
                partial_written: false,
                failed: false,
                bytes: Vec::new(),
            }
        }
    }

    impl Write for PartialThenFailWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            let mut state = self.state.borrow_mut();
            if state.partial_written && !state.failed {
                state.failed = true;
                return Err(io::Error::other("injected failure after partial write"));
            }
            if !state.failed
                && state.completed_acquisitions + 1 == state.target_acquisition
                && buffer.len() > 1
            {
                let accepted = buffer.len() - 1;
                state.bytes.extend_from_slice(&buffer[..accepted]);
                state.partial_written = true;
                return Ok(accepted);
            }
            state.bytes.extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            let mut state = self.state.borrow_mut();
            if !state.failed {
                state.completed_acquisitions += 1;
            }
            Ok(())
        }
    }

    #[derive(Debug, Default)]
    struct MultiFailState {
        writes: usize,
        failures: Vec<(usize, &'static str)>,
        bytes: Vec<u8>,
    }

    #[derive(Debug)]
    struct MultiFailWriter {
        state: Rc<RefCell<MultiFailState>>,
    }

    impl Write for MultiFailWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            let mut state = self.state.borrow_mut();
            state.writes += 1;
            if state
                .failures
                .first()
                .is_some_and(|(write, _)| *write == state.writes)
            {
                let (_, message) = state.failures.remove(0);
                return Err(io::Error::other(message));
            }
            state.bytes.extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
}
