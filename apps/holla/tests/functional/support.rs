#[path = "../../../../tests/functional/support.rs"]
mod shared;

pub use shared::*;
pub const HOLLA: &str = env!("CARGO_BIN_EXE_holla");
