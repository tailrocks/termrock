//! Optional Crossterm backend, event, and scoped terminal-session adapters.

mod event;
mod session;

pub use event::{Event, KeyEvent, MouseEvent, key, mouse_kind};
pub use ratatui_crossterm::CrosstermBackend;
pub use session::{Session, SessionOptions};
