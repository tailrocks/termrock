// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/tablepro (MIT),
// https://github.com/donbeave/terminal-components-claude

//! TablePro library: deterministic in-memory workbench. The standalone
//! `tablepro` binary and Applications → TablePro mount the same [`App`].

pub mod app;
pub mod connections;
pub mod db;
pub mod grid;
pub mod model;
pub mod paint;
pub mod sql;
pub mod tabs;
pub mod text;
pub mod workbench;

pub use app::{App, MIN_HEIGHT, MIN_WIDTH, Screen};
pub use db::{Catalog, Connection, SafeMode, connections};

use termrock::style::ColorCapability;

/// CLI options for the standalone binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub level: ColorCapability,
    pub connect: Option<String>,
}

/// CLI parse failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    Help(String),
    UnknownColor(String),
    UnknownConnection(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Help(t) => f.write_str(t),
            Self::UnknownColor(v) => {
                write!(f, "unknown --color value {v:?}; use truecolor|256|16|none")
            }
            Self::UnknownConnection(v) => write!(f, "no connection named {v:?}"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Source help (junie-tui `tablepro --help`).
#[must_use]
pub fn help_text() -> String {
    "tablepro — TablePro's core workflow as a terminal application\n\n\
     USAGE: tablepro [--color truecolor|256|16|none] [--connect NAME]\n\n\
     Keys: Ctrl+O open quickly · Ctrl+T new query · Ctrl+R run · Ctrl+Y history · ? help · q quit"
        .into()
}

/// Parse argv without argv0.
pub fn parse_args<I, S>(args: I) -> Result<Options, ParseError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut level = ColorCapability::detect_from_env();
    let mut connect = None;
    let mut iter = args.into_iter();
    while let Some(a) = iter.next() {
        match a.as_ref() {
            "--color" | "-c" => {
                let value = iter.next().map(|s| s.as_ref().to_owned());
                level = match value.as_deref() {
                    Some("truecolor") | Some("24bit") => ColorCapability::Truecolor,
                    Some("256") => ColorCapability::Indexed256,
                    Some("16") => ColorCapability::Ansi16,
                    Some("none") | Some("mono") => ColorCapability::Monochrome,
                    other => {
                        return Err(ParseError::UnknownColor(
                            other.unwrap_or_default().to_owned(),
                        ));
                    }
                };
            }
            "--connect" => {
                connect = iter.next().map(|s| s.as_ref().to_owned());
            }
            "-h" | "--help" => return Err(ParseError::Help(help_text())),
            _ => {}
        }
    }
    Ok(Options { level, connect })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_includes_connect() {
        let t = help_text();
        assert!(t.contains("--connect NAME"));
        assert!(t.contains("--color truecolor|256|16|none"));
        let err = parse_args(["--help"]).unwrap_err();
        assert!(err.to_string().contains("--connect"));
        let err = parse_args(["-h"]).unwrap_err();
        assert!(err.to_string().contains("--connect"));
    }

    #[test]
    fn connect_flag_parses() {
        let o = parse_args(["--connect", "Production"]).unwrap();
        assert_eq!(o.connect.as_deref(), Some("Production"));
    }
}
