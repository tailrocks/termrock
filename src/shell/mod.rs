mod app;
mod chrome;
pub mod clipboard;
mod cx;
mod footer;
mod nav;
mod page;
pub mod reach;
mod router;
mod status;

pub use crate::ui::layer::LayerTag;
pub use crate::widgets::breadcrumb::Breadcrumb;
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

use crate::runtime::Application;

pub fn run(app: &mut impl Application) -> std::io::Result<()> {
    if let Some(delay) = start_delay() {
        std::thread::sleep(delay);
    }
    let _ = crate::runtime::drain_pending_input_bounded(app.raw_input_policy().event_limit());
    crate::runtime::run(app)
}

fn start_delay() -> Option<std::time::Duration> {
    std::env::var("JUNIE_TUI_START_DELAY_MS")
        .ok()?
        .parse()
        .ok()
        .map(std::time::Duration::from_millis)
}
