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

pub use catalog::{
    CatalogProfile, NavEntry, PageId, SOURCE_NAV, catalog_authority, nav_entries,
    scenario_descriptors,
};
pub use cli::{Options, ParseError, parse_args};
pub use coverage::catalog_page_for;
pub use id::WidgetId;
pub use outcome::Route;
pub use profile::ProductIdentity;
pub use shell::{App, MIN_HEIGHT, MIN_WIDTH};

/// Default dimensions used by the deterministic catalog render command.
pub const DEFAULT_FRAME_COLS: u16 = 120;
/// Default dimensions used by the deterministic catalog render command.
pub const DEFAULT_FRAME_ROWS: u16 = 40;

fn capture_profile() -> CatalogProfile {
    match std::env::var("TERMROCK_CATALOG_PROFILE").as_deref() {
        Ok("junie-reference" | "junie") => CatalogProfile::JunieReference,
        _ => CatalogProfile::TermRock,
    }
}

/// Serialize one canonical catalog page through [`host::CatalogSession`].
pub fn canonical_frame_json(page: PageId, cols: u16, rows: u16) -> Result<String, String> {
    canonical_frame_json_for_profile(page, cols, rows, &[], capture_profile())
}

/// Serialize one canonical page under an explicit catalog profile.
pub fn canonical_frame_json_for_profile(
    page: PageId,
    cols: u16,
    rows: u16,
    keys: &[String],
    profile: CatalogProfile,
) -> Result<String, String> {
    let entry = nav_entries(profile)
        .iter()
        .find(|entry| entry.id == page)
        .ok_or_else(|| format!("unknown catalog page id {}", page.0))?;
    let page_name = catalog::normalize(entry.label);
    let mut session = host::CatalogSession::mount_profile(&page_name, cols, rows, profile)?;
    for key in keys {
        session.dispatch(host::DemoEvent::Key {
            key: key.clone(),
            kind: "press".to_owned(),
            shift: false,
            ctrl: false,
            alt: false,
            meta: false,
        })?;
    }
    serde_json::to_string_pretty(&session.frame()).map_err(|error| error.to_string())
}

/// Serialize one canonical representative scenario through the shared host.
pub fn canonical_scenario_frame_json(
    scenario: &str,
    cols: u16,
    rows: u16,
    keys: &[String],
) -> Result<String, String> {
    canonical_scenario_frame_json_for_profile(scenario, cols, rows, keys, capture_profile())
}

/// Serialize one canonical scenario under an explicit catalog profile.
pub fn canonical_scenario_frame_json_for_profile(
    scenario: &str,
    cols: u16,
    rows: u16,
    keys: &[String],
    profile: CatalogProfile,
) -> Result<String, String> {
    let mut session = host::CatalogSession::mount_profile(scenario, cols, rows, profile)?;
    for key in keys {
        session.dispatch(host::DemoEvent::Key {
            key: key.clone(),
            kind: "press".to_owned(),
            shift: false,
            ctrl: false,
            alt: false,
            meta: false,
        })?;
    }
    serde_json::to_string_pretty(&session.frame()).map_err(|error| error.to_string())
}

/// Serialize one real TablePro application frame through the headless host.
pub fn canonical_tablepro_frame_json(
    connect: Option<&str>,
    cols: u16,
    rows: u16,
    keys: &[String],
) -> Result<String, String> {
    let frame = host::tablepro_frame(connect, cols, rows, keys)?;
    serde_json::to_string_pretty(&frame).map_err(|error| error.to_string())
}

/// Native catalog entry: parse argv, then run the crossterm event loop.
#[cfg(feature = "native")]
pub fn run() -> std::io::Result<()> {
    use std::ops::ControlFlow;
    use std::process;
    use termrock::runtime::{RunOptions, run as run_app};

    let command = match cli::parse_command(std::env::args().skip(1)) {
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
    match command {
        cli::Command::Authority => {
            println!(
                "{}",
                serde_json::to_string_pretty(&catalog::catalog_authority())
                    .map_err(std::io::Error::other)?
            );
            return Ok(());
        }
        cli::Command::Scenarios => {
            println!(
                "{}",
                serde_json::to_string_pretty(&catalog::scenario_descriptors())
                    .map_err(std::io::Error::other)?
            );
            return Ok(());
        }
        cli::Command::Frame(opts) => {
            let json = match (opts.page, opts.scenario.as_deref()) {
                (Some(page), None) => canonical_frame_json_for_profile(
                    page,
                    opts.cols,
                    opts.rows,
                    &opts.keys,
                    capture_profile(),
                ),
                (None, Some(scenario)) => {
                    canonical_scenario_frame_json(scenario, opts.cols, opts.rows, &opts.keys)
                }
                _ => Err("frame requires exactly one target".to_owned()),
            }
            .map_err(std::io::Error::other)?;
            println!("{json}");
            return Ok(());
        }
        cli::Command::TableProFrame(opts) => {
            let json = canonical_tablepro_frame_json(
                opts.connect.as_deref(),
                opts.cols,
                opts.rows,
                &opts.keys,
            )
            .map_err(std::io::Error::other)?;
            println!("{json}");
            return Ok(());
        }
        cli::Command::Capture(opts) => {
            std::fs::create_dir_all(&opts.out)?;
            let selected: Vec<_> = match opts.scenario.as_deref() {
                Some(id) => vec![
                    scenarios::capture_scenarios()
                        .find(|scenario| scenario.id == id)
                        .ok_or_else(|| std::io::Error::other(format!("unknown scenario {id:?}")))?,
                ],
                None => scenarios::capture_scenarios().collect(),
            };
            for scenario in selected {
                let stem = opts.out.join(scenario.id);
                capture::replay(scenario)
                    .write_five(&stem)
                    .map_err(std::io::Error::other)?;
            }
            return Ok(());
        }
        cli::Command::Render(opts) => {
            std::fs::create_dir_all(&opts.out)?;
            let profile = capture_profile();
            if opts.scenarios {
                for scenario in catalog::scenario_descriptors() {
                    let json = canonical_scenario_frame_json_for_profile(
                        scenario.id,
                        scenario.cols,
                        scenario.rows,
                        &[],
                        capture_profile(),
                    )
                    .map_err(std::io::Error::other)?;
                    let path = opts
                        .out
                        .join(format!("{}.json", scenario.id.replace('/', "-")));
                    std::fs::write(path, format!("{json}\n"))?;
                }
            } else {
                for entry in nav_entries(profile) {
                    let json = canonical_frame_json_for_profile(
                        entry.id,
                        DEFAULT_FRAME_COLS,
                        DEFAULT_FRAME_ROWS,
                        &[],
                        profile,
                    )
                    .map_err(std::io::Error::other)?;
                    let path = opts
                        .out
                        .join(format!("{}.json", catalog::normalize(entry.label)));
                    std::fs::write(path, format!("{json}\n"))?;
                }
            }
            return Ok(());
        }
        cli::Command::Interactive(opts) => {
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
        }
    }
    let _ = ControlFlow::<()>::Continue(());
    Ok(())
}
