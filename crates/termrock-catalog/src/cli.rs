// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/showcase/main.rs (MIT).

//! Source-compatible catalog CLI.

use termrock::style::ColorCapability;

use crate::catalog::{CatalogProfile, PageId, nav_entries};

/// Parsed catalog launch options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub level: ColorCapability,
    pub page: Option<PageId>,
    pub profile: CatalogProfile,
}

/// CLI parse failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    Help,
    UnknownColor(String),
    UnknownPage(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Help => f.write_str(&help_text(CatalogProfile::TermRock)),
            Self::UnknownColor(v) => {
                write!(f, "unknown --color value {v:?}; use truecolor|256|16|none")
            }
            Self::UnknownPage(v) => write!(f, "unknown page {v:?}"),
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
         USAGE: {name} [--color truecolor|256|16|none] [--page NAME]\n\n\
         Keys: Tab/Shift+Tab focus · arrows move · Enter/Space activate · Esc back · [ ] pages · ? help · q quit"
    )
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
