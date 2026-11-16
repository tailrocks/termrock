mod app;
mod coverage;

mod data;
mod feed;
mod inspector;
mod options;
mod pages;

#[cfg(all(test, feature = "work-metrics"))]
mod work_metrics_tests;

use crate::app::{App, NavExpand, PageId};
use clap::{Parser, ValueEnum};
use junie_tui::cli::CommonArgs;

const NO_MOTION_ENV: &str = "SHOWCASE_NO_MOTION";

#[derive(Clone, Copy, Debug, ValueEnum)]
enum NavArg {
    /// Only the current page's group (default).
    Current,
    /// Only the current page's group, after every navigation too.
    Accordion,
    /// Every group.
    All,
}

impl From<NavArg> for NavExpand {
    fn from(value: NavArg) -> Self {
        match value {
            NavArg::Current => NavExpand::CurrentGroup,
            NavArg::Accordion => NavExpand::Accordion,
            NavArg::All => NavExpand::All,
        }
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "junie-tui",
    about = "Junie-inspired Ratatui design system laboratory",
    long_about = "Junie-tui — Junie-inspired Ratatui design system laboratory"
)]
struct Cli {
    #[command(flatten)]
    common: CommonArgs,
    /// The page to open
    #[arg(short = 'p', long, value_parser = parse_page, value_name = "NAME")]
    page: Option<PageId>,
    /// Which sidebar groups are open.
    #[arg(long, value_enum, value_name = "POLICY")]
    nav: Option<NavArg>,
    /// Set a live option at launch, as the Inspector's Options tab would
    /// (repeatable): shell.KEY=VALUE or PAGE.KEY=VALUE. An
    /// unknown key or value lists the valid ones.
    #[arg(long = "opt", value_parser = options::parse_opt, value_name = "KEY=VALUE")]
    opt: Vec<options::OptSet>,
}

fn parse_page(value: &str) -> Result<PageId, String> {
    PageId::from_name(value).ok_or_else(|| format!("unknown page {value:?}"))
}

fn cli_after_help() -> String {
    format!(
        "{}\n\
         Frame:     with --motion paused, the exact tick to render\n\n\
         Keys: Tab/Shift+Tab focus · arrows move · Enter/Space activate · Esc back · [ ] pages · ? help · q quit",
        junie_tui::cli::motion_help(NO_MOTION_ENV)
    )
}

fn main() -> std::io::Result<()> {
    let _ = (
        coverage::PICKER_COVERAGE,
        coverage::TERMINAL_SESSION_COVERAGE,
        coverage::TEXT_INPUT_COVERAGE,
        coverage::GRID_EDIT_COVERAGE,
        coverage::DOCUMENT_WORKSPACE_COVERAGE,
        coverage::DOCUMENT_WORKSPACE_ALLOWLIST,
        coverage::GRID_CHANGE_ALLOWLIST,
        coverage::GRID_PARSER_ALLOWLIST,
        coverage::SCROLL_WINDOW_COVERAGE,
        coverage::SCROLL_WINDOW_ALLOWLIST,
        coverage::EXTERNAL_COVERAGE,
        coverage::EXTERNAL_ALLOWLIST,
        coverage::RUNTIME_ALLOWLIST,
        coverage::KEYED_TABLE_ALLOWLIST,
        coverage::INSPECT_CHANGES_COVERAGE,
        coverage::INSPECT_CHANGES_ALLOWLIST,
        coverage::SEARCH_SESSION_ALLOWLIST,
        coverage::SEARCH_DETAIL_PAGES,
        coverage::SEARCH_DETAIL_COVERAGE,
        coverage::PROOFS,
        coverage::EXCEPTIONS,
    );
    let cli: Cli = junie_tui::cli::parse(cli_after_help());
    let motion = cli.common.motion(NO_MOTION_ENV);
    let mut app = App::with_motion(cli.common.theme(), motion, cli.common.frame);
    if let Some(p) = cli.page {
        app.open_page(p);
    }
    app.set_nav_expand(cli.nav.map(NavExpand::from).unwrap_or_default());
    for set in cli.opt {
        app.apply_opt(set);
    }
    junie_tui::shell::run(&mut app)
}
