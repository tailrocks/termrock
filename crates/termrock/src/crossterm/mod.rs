//! Optional Crossterm backend and scoped terminal-session adapters.

mod session;

pub use ratatui_crossterm::CrosstermBackend;
pub use session::{Session, SessionOptions};
