// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Showcase shell, page grammar, and catalog order adapted from junie-tui
// (MIT), https://github.com/donbeave/terminal-components-claude

//! Canonical TermRock catalog: one Junie-style application hosting
//! Foundations, Components, Patterns, Screens, and Applications.
//!
//! Native binary, WASM host, headless capture, and tests mount the same
//! [`App`], catalog registry, and page state.

#![allow(missing_docs)]
#![allow(elided_lifetimes_in_paths)]

pub mod ansi_grid;
pub mod capture;
pub mod catalog;
pub mod cli;
pub mod coverage;
pub mod ctx;
pub mod data;
pub mod draw;
pub mod host;
pub mod id;
pub mod layout;
pub mod outcome;
pub mod page;
pub mod pages;
pub mod profile;
pub mod scenarios;
pub mod shell;
pub mod snapshot;
pub mod tablepro;
pub mod text;

pub use catalog::{CatalogProfile, NavEntry, PageId, SOURCE_NAV, nav_entries};
pub use cli::{Options, ParseError, parse_args};
pub use coverage::catalog_page_for;
pub use id::WidgetId;
pub use outcome::Route;
pub use profile::ProductIdentity;
pub use shell::{App, MIN_HEIGHT, MIN_WIDTH};

/// Native catalog entry: parse argv, then run the crossterm event loop.
#[cfg(feature = "native")]
pub fn run() -> std::io::Result<()> {
    use std::ops::ControlFlow;
    use std::process;
    use termrock::runtime::{RunOptions, run as run_app};

    let opts = match cli::parse_args(std::env::args().skip(1)) {
        Ok(o) => o,
        Err(cli::ParseError::Help) => {
            println!("{}", cli::ParseError::Help);
            process::exit(0);
        }
        Err(e) => {
            eprintln!("{e}");
            process::exit(2);
        }
    };
    let mut app = shell::App::new(opts.profile, opts.level);
    if let Some(p) = opts.page {
        app.goto(p);
    }
    run_app(
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
