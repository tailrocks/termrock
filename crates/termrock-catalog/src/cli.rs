// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/showcase/main.rs (MIT).

//! Source-compatible catalog CLI.

use std::path::PathBuf;

use termrock::style::ColorCapability;

use crate::catalog::{CatalogProfile, PageId, nav_entries};
use crate::{DEFAULT_FRAME_COLS, DEFAULT_FRAME_ROWS};

/// Parsed catalog launch options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub level: ColorCapability,
    pub page: Option<PageId>,
    pub profile: CatalogProfile,
}

/// Canonical headless frame command options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameOptions {
    pub page: PageId,
    pub cols: u16,
    pub rows: u16,
}

/// Canonical catalog render command options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderOptions {
    pub out: PathBuf,
}

/// A native catalog command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Interactive(Options),
    Frame(FrameOptions),
    Render(RenderOptions),
}

/// CLI parse failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    Help,
    UnknownColor(String),
    UnknownPage(String),
    InvalidCommand(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Help => f.write_str(&help_text(CatalogProfile::TermRock)),
            Self::UnknownColor(v) => {
                write!(f, "unknown --color value {v:?}; use truecolor|256|16|none")
            }
            Self::UnknownPage(v) => write!(f, "unknown page {v:?}"),
            Self::InvalidCommand(v) => f.write_str(v),
        }
    }
}

impl std::error::Error for ParseError {}

/// Exact source help, with TermRock identity on the default profile.
#[must_use]
pub fn help_text(profile: CatalogProfile) -> String {
    let name = profile.identity().name;
    format!(
        "{name} — Junie-inspired Ratatui design system laboratory\n\n\
         USAGE: {name} [--color truecolor|256|16|none] [--page NAME]\n\
         TOOLS: {name} frame --page NAME [--cols N] [--rows N]\n\
                {name} render --out DIR\n\n\
         Keys: Tab/Shift+Tab focus · arrows move · Enter/Space activate · Esc back · [ ] pages · ? help · q quit"
    )
}

/// Parse either the interactive catalog launch or a canonical headless tool.
///
/// The interactive path delegates to [`parse_args`] unchanged. Tool
/// subcommands intentionally use strict parsing so a typo cannot silently
/// launch the interactive application or produce an incomplete artifact set.
pub fn parse_command<I, S>(args: I) -> Result<Command, ParseError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let args: Vec<String> = args
        .into_iter()
        .map(|arg| arg.as_ref().to_owned())
        .collect();
    match args.first().map(String::as_str) {
        Some("frame") => parse_frame_command(&args[1..]),
        Some("render") => parse_render_command(&args[1..]),
        _ => parse_args(args).map(Command::Interactive),
    }
}

fn parse_frame_command(args: &[String]) -> Result<Command, ParseError> {
    let mut page = None;
    let mut cols = DEFAULT_FRAME_COLS;
    let mut rows = DEFAULT_FRAME_ROWS;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--page" | "-p" => {
                let name = iter.next().ok_or_else(|| {
                    ParseError::InvalidCommand("frame requires a page name".to_owned())
                })?;
                page = PageId::from_name(name, nav_entries(CatalogProfile::TermRock));
                if page.is_none() {
                    return Err(ParseError::UnknownPage(name.clone()));
                }
            }
            "--cols" => cols = parse_dimension("--cols", iter.next())?,
            "--rows" => rows = parse_dimension("--rows", iter.next())?,
            "-h" | "--help" => return Err(ParseError::Help),
            other => {
                return Err(ParseError::InvalidCommand(format!(
                    "unknown frame option {other:?}"
                )));
            }
        }
    }
    let page =
        page.ok_or_else(|| ParseError::InvalidCommand("frame requires --page NAME".to_owned()))?;
    Ok(Command::Frame(FrameOptions { page, cols, rows }))
}

fn parse_render_command(args: &[String]) -> Result<Command, ParseError> {
    let mut out = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--out" => {
                let path = iter.next().ok_or_else(|| {
                    ParseError::InvalidCommand("render requires an output directory".to_owned())
                })?;
                if path.is_empty() {
                    return Err(ParseError::InvalidCommand(
                        "render requires a non-empty output directory".to_owned(),
                    ));
                }
                out = Some(PathBuf::from(path));
            }
            "-h" | "--help" => return Err(ParseError::Help),
            other => {
                return Err(ParseError::InvalidCommand(format!(
                    "unknown render option {other:?}"
                )));
            }
        }
    }
    let out =
        out.ok_or_else(|| ParseError::InvalidCommand("render requires --out DIR".to_owned()))?;
    Ok(Command::Render(RenderOptions { out }))
}

fn parse_dimension(flag: &str, value: Option<&String>) -> Result<u16, ParseError> {
    let value = value
        .ok_or_else(|| ParseError::InvalidCommand(format!("{flag} requires a positive integer")))?;
    let parsed = value.parse::<u16>().map_err(|_| {
        ParseError::InvalidCommand(format!("{flag} requires a positive integer, got {value:?}"))
    })?;
    if parsed == 0 {
        return Err(ParseError::InvalidCommand(format!(
            "{flag} requires a positive integer"
        )));
    }
    Ok(parsed)
}

/// Parse argv (without argv0). `TERMROCK_CATALOG_PROFILE=junie-reference`
/// selects the capture profile; not a user-facing flag.
pub fn parse_args<I, S>(args: I) -> Result<Options, ParseError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let profile = match std::env::var("TERMROCK_CATALOG_PROFILE") {
        Ok(v) if v == "junie-reference" || v == "junie" => CatalogProfile::JunieReference,
        _ => CatalogProfile::TermRock,
    };
    let nav = nav_entries(profile);
    let mut level = ColorCapability::detect_from_env();
    let mut page = None;
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
            "--page" | "-p" => {
                let name = iter
                    .next()
                    .map(|s| s.as_ref().to_owned())
                    .unwrap_or_default();
                page = PageId::from_name(&name, nav);
                if page.is_none() {
                    return Err(ParseError::UnknownPage(name));
                }
            }
            "-h" | "--help" => return Err(ParseError::Help),
            _ => {}
        }
    }
    Ok(Options {
        level,
        page,
        profile,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_aliases() {
        let o = parse_args(["--color", "24bit"]).unwrap();
        assert_eq!(o.level, ColorCapability::Truecolor);
        let o = parse_args(["-c", "mono"]).unwrap();
        assert_eq!(o.level, ColorCapability::Monochrome);
        let o = parse_args(["--color", "256"]).unwrap();
        assert_eq!(o.level, ColorCapability::Indexed256);
        let o = parse_args(["--color", "16"]).unwrap();
        assert_eq!(o.level, ColorCapability::Ansi16);
        let o = parse_args(["--color", "none"]).unwrap();
        assert_eq!(o.level, ColorCapability::Monochrome);
        assert!(matches!(
            parse_args(["--color", "octarine"]),
            Err(ParseError::UnknownColor(_))
        ));
    }

    #[test]
    fn page_aliases() {
        let o = parse_args(["--page", "datagrid"]).unwrap();
        assert_eq!(o.page, Some(PageId::GRID));
        let o = parse_args(["-p", "chips-selects"]).unwrap();
        assert_eq!(o.page, Some(PageId::CHIPS));
        assert!(matches!(
            parse_args(["--page", "nope"]),
            Err(ParseError::UnknownPage(_))
        ));
    }

    #[test]
    fn help_is_showcase_usage() {
        let err = parse_args(["--help"]).unwrap_err();
        let t = err.to_string();
        assert!(t.contains("Keys: Tab/Shift+Tab"));
        assert!(t.contains("--page NAME"));
        assert!(t.contains("--color truecolor|256|16|none"));
        assert!(!t.contains("terminal|list"));
        assert!(!t.contains("termrock-lookbook"));
    }
}
