#![cfg(any(target_os = "macos", target_os = "linux"))]

/// The engine types the shared harness (`tests/functional/support.rs`) names.
/// Each target that includes it supplies them from the crate it can depend on.
mod engine {
    pub use junie_tui::clock::TestRequest;
    pub use junie_tui::core::pointer::PointerConfig;
    pub use junie_tui::shell::StatusLine;
}

mod bins;
mod cases;
mod holla;
mod lifecycle;
mod showcase;
#[allow(dead_code)]
mod support;
mod tablepro;
mod visual;
