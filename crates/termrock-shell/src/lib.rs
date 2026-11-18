mod app;
mod chrome;
pub mod clipboard;
mod cx;
mod footer;
mod nav;
mod page;
pub mod reach;
mod router;
pub mod runtime;
mod status;

pub use app::{App, PageKey, Shell, blur_fan_out, handle, render};
pub use chrome::{Chrome, Menus, Regions};
pub use clipboard::{Clipboard, ClipboardHook, Copied, NativeClipboard};
pub use cx::{Cx, Request, background};
pub use footer::{Footer, FooterStatus, deliverable};
pub use nav::{Nav, Router};
pub use page::{
    Page, PageCx, PageTypes, open_context, rebuild_on_tick, replay_key, row_anchor, row_choice,
};
pub use router::{KeyHost, KeyRouter, ShellAction, ShellKeys};
pub use status::{JobKey, StatusLine, StopHandle};
pub use termrock_navigation::breadcrumb::Breadcrumb;
pub use termrock_overlays::layer::LayerTag;

use crate::runtime::Application;

pub fn run(app: &mut impl Application) -> std::io::Result<()> {
    if let Some(delay) = start_delay() {
        std::thread::sleep(delay);
    }
    discard_startup_input(app, crate::runtime::drain_pending_input_bounded)?;
    crate::runtime::run(app)
}

/// Discards input queued before the app started, at most the app's raw-input
/// event limit. A drain failure means the terminal cannot be read or its raw
/// mode cannot be switched, so the session could not run either: the error is
/// returned instead of being dropped and surfacing later as a vaguer failure.
fn discard_startup_input(
    app: &impl Application,
    drain: impl FnOnce(usize) -> std::io::Result<usize>,
) -> std::io::Result<()> {
    drain(app.raw_input_policy().event_limit()).map(drop)
}

fn start_delay() -> Option<std::time::Duration> {
    std::env::var("JUNIE_TUI_START_DELAY_MS")
        .ok()?
        .parse()
        .ok()
        .map(std::time::Duration::from_millis)
}

#[cfg(test)]
mod startup_tests {
    use std::cell::Cell;

    use ratatui::Frame;

    use super::discard_startup_input;
    use crate::runtime::{Application, RawInputPolicy};
    use termrock_core::event::{Input, Outcome};

    struct Limited(usize);

    impl Application for Limited {
        fn handle(&mut self, _input: Input) -> Outcome {
            Outcome::Ignored
        }

        fn render(&mut self, _frame: &mut Frame) {}

        fn should_quit(&self) -> bool {
            false
        }

        fn tick_interval(&self) -> std::time::Duration {
            std::time::Duration::from_millis(100)
        }

        fn raw_input_policy(&self) -> RawInputPolicy {
            RawInputPolicy {
                max_events: self.0,
                ..RawInputPolicy::default()
            }
        }
    }

    #[test]
    fn startup_drain_is_bounded_by_the_apps_raw_input_limit() {
        let asked = Cell::new(None);
        let result = discard_startup_input(&Limited(7), |limit| {
            asked.set(Some(limit));
            Ok(3)
        });
        assert!(result.is_ok());
        assert_eq!(asked.get(), Some(7));
    }

    #[test]
    fn startup_drain_failure_is_returned_not_dropped() {
        let error = discard_startup_input(&Limited(7), |_| {
            Err(std::io::Error::other("terminal unreadable"))
        })
        .expect_err("a failed startup drain is reported");
        assert_eq!(error.to_string(), "terminal unreadable");
    }
}
