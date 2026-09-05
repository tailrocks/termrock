// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/tablepro/main.rs (MIT),
// https://github.com/donbeave/terminal-components-claude

//! `tablepro` binary. Same [`termrock_catalog::tablepro::App`] as Applications → TablePro.

use std::io;
use std::ops::ControlFlow;
use std::process;

use termrock::runtime::{RunOptions, run};
use termrock_catalog::tablepro::{App, ParseError, parse_args};

fn main() -> io::Result<()> {
    let opts = match parse_args(std::env::args().skip(1)) {
        Ok(o) => o,
        Err(ParseError::Help(t)) => {
            println!("{t}");
            process::exit(0);
        }
        Err(e) => {
            eprintln!("{e}");
            process::exit(2);
        }
    };
    let mut app = App::new(opts.level);
    if let Some(name) = opts.connect {
        if let Err(e) = app.connect_named(&name) {
            eprintln!("{e}");
            process::exit(2);
        }
    }
    run(
        &mut app,
        RunOptions::default(),
        |app, frame, tick| app.render(frame, tick),
        |app, event, tick| {
            let flow = app.handle_event(event, tick);
            if !app.quit {
                app.on_tick(tick);
            }
            flow
        },
        |app| app.next_deadline(),
    )?;
    let _ = ControlFlow::<()>::Continue(());
    Ok(())
}
