#[path = "../../../../crates/termrock-functional/tests/functional/support.rs"]
mod shared;

pub use shared::*;
pub const JACKIN: &str = env!("CARGO_BIN_EXE_jackin-preview");
