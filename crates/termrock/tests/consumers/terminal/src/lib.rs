//! An independent consumer of `termrock` with only the `terminal` feature.
//!
//! The package has its own `[workspace]` and lockfile. `cli` is not enabled,
//! so `termrock::cli` and `clap` do not exist here; the terminal viewport
//! resolves from the declared feature alone.

use termrock::core::id::WidgetId;
use termrock::terminal::{TerminalState, TerminalViewport};

/// A viewport over an empty `rows` by `cols` grid.
pub fn viewport(rows: u16, cols: u16) -> TerminalViewport {
    TerminalViewport::new(
        WidgetId::of("terminal.consumer"),
        TerminalState::new(rows, cols),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consumer_terminal_constructs_public_viewport() {
        let view = viewport(5, 20);
        assert_eq!(view.id, WidgetId::of("terminal.consumer"));
        assert_eq!(view.state().size(), (5, 20));
        assert_eq!(view.offset(), 0);
    }

    #[test]
    fn consumer_terminal_feeds_output_without_moving_the_live_tail() {
        let mut view = viewport(3, 10);
        view.feed(b"one\r\ntwo\r\nthree\r\nfour\r\n");
        assert_eq!(view.offset(), 0);
        assert_eq!(view.state().size(), (3, 10));
    }
}
