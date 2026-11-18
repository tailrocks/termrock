#![cfg(any(target_os = "macos", target_os = "linux"))]

/// The engine types the shared harness (`crates/termrock-functional/tests/functional/support.rs`) names.
/// Each target that includes it supplies them from the crate it can depend on.
mod engine {
    pub use termrock::clock::TestRequest;
    pub use termrock::core::pointer::PointerConfig;
    pub use termrock::shell::StatusLine;
}

#[allow(dead_code)]
#[path = "../../../../crates/termrock-functional/tests/functional/bins.rs"]
mod bins;
mod cases;
#[allow(dead_code)]
mod support;
