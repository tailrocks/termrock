#![cfg(any(target_os = "macos", target_os = "linux"))]

#[allow(dead_code)]
#[path = "../../../../tests/functional/bins.rs"]
mod bins;
mod cases;
#[allow(dead_code)]
mod support;
