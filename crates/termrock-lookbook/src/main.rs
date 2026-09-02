// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Lookbook binary is the canonical Junie-style catalog.
//! `frame --story` still emits one [`termrock_lookbook::frame::TerminalFrame`] JSON.

fn main() -> std::io::Result<()> {
    let mut args = std::env::args();
    let _argv0 = args.next();
    if args.next().as_deref() == Some("frame") {
        return cmd_frame(args);
    }
    termrock_catalog::run()
}

fn cmd_frame(args: impl Iterator<Item = String>) -> std::io::Result<()> {
    use termrock_lookbook::frame::{paint_frame_args, parse_frame_args};
    let parsed = parse_frame_args(args).map_err(std::io::Error::other)?;
    let frame = paint_frame_args(&parsed).map_err(std::io::Error::other)?;
    println!(
        "{}",
        serde_json::to_string(&frame).map_err(std::io::Error::other)?
    );
    Ok(())
}
