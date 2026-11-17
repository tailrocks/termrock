#![cfg(any(target_os = "macos", target_os = "linux"))]

#[allow(dead_code)]
#[path = "../../../../tests/functional/bins.rs"]
mod bins;
mod jackin;
mod legacy;
mod legacy_cross_app;
#[allow(dead_code)]
mod support;
mod terminal_panes;
