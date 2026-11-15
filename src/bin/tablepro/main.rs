mod app;

mod connections;
mod context;
mod db;
mod filter_editor;
#[cfg(test)]
mod history_identity_tests;
mod model;

mod pages;

mod sql;
mod store;
mod tabs;
mod workbench;

use clap::{CommandFactory, Parser, error::ErrorKind};
use junie_tui::cli::CommonArgs;

use crate::app::App;
use crate::connections::{ConnectionsScreen, NameLookup};
use junie_tui::core::id::ItemKey;

const NO_MOTION_ENV: &str = "TABLEPRO_NO_MOTION";

#[derive(Debug, Parser)]
#[command(
    name = "tablepro",
    about = "TablePro terminal database workbench",
    long_about = "TablePro — terminal database workbench on the Junie design system"
)]
struct Cli {
    #[command(flatten)]
    common: CommonArgs,
    /// Connect to the named connection at start
    #[arg(long, value_name = "NAME")]
    connect: Option<String>,
}

fn cli_after_help() -> String {
    format!(
        "{}\n\
         Frame:     with --motion paused, the exact tick to render\n\n\
         Keys: Ctrl+O open quickly · Ctrl+T new query · Ctrl+R run · Ctrl+Y history · ? help · q quit",
        junie_tui::cli::motion_help(NO_MOTION_ENV)
    )
}

fn connection_key(connections: &ConnectionsScreen, name: &str) -> Result<ItemKey, clap::Error> {
    connections.resolve_name(name).map_err(|why| {
        let message = match why {
            NameLookup::NotFound => format!("no connection named {name:?}"),
            NameLookup::Ambiguous(n) => {
                format!("{n} connections are named {name:?}; rename one to pick it")
            }
        };
        Cli::command().error(ErrorKind::ValueValidation, message)
    })
}

fn main() -> std::io::Result<()> {
    let cli: Cli = junie_tui::cli::parse(cli_after_help());
    let mut app = App::new(cli.common.theme());
    if let Some(name) = cli.connect {
        let key = connection_key(&app.connections, &name).unwrap_or_else(|error| error.exit());
        app.connect_key(key);
    }
    app.set_motion(cli.common.motion(NO_MOTION_ENV), cli.common.frame);
    app.shell.set_clipboard(junie_tui::shell::Clipboard::new());
    junie_tui::shell::run(&mut app)
}
