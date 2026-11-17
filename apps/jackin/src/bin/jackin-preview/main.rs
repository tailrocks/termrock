use clap::Parser;
use termrock::cli::CommonArgs;

use jackin_console::fixtures::{Scenario, app_for};

const NO_MOTION_ENV: &str = "JACKIN_NO_MOTION";

#[derive(Debug, Parser)]
#[command(
    name = "jackin-preview",
    about = "Jackin terminal preview",
    long_about = "Jackin-preview — Jackin redesigned on the Junie design system (deterministic preview)"
)]
struct Cli {
    #[command(flatten)]
    common: CommonArgs,
    /// The fixture scenario to open
    #[arg(short = 's', long, value_parser = parse_scenario, default_value = "first-use")]
    scenario: Scenario,
}

fn scenario_names() -> String {
    Scenario::ALL
        .iter()
        .map(|s| s.name())
        .collect::<Vec<_>>()
        .join(", ")
}

fn parse_scenario(value: &str) -> Result<Scenario, String> {
    Scenario::from_name(value).ok_or_else(|| {
        format!(
            "unknown scenario {value:?}; use one of {}",
            scenario_names()
        )
    })
}

fn cli_after_help() -> String {
    let scenarios = scenario_names();
    let motion = termrock::cli::motion_help(NO_MOTION_ENV);
    format!(
        "Scenarios: {scenarios}\n\
         {motion}\n\
         Frame:     with --motion paused, the exact fixture tick to render (intro, cockpit, outro phases)\n\n\
         Keys: Tab/Shift+Tab focus · ↑↓ move · Enter launch/activate · Esc back · u Accounts & Usage · s Settings · R Roles · D Doctor · ? help · q quit\n\
         Everything is simulated in memory; the real Jackin CLI is never touched."
    )
}

fn main() -> std::io::Result<()> {
    let cli: Cli = termrock::cli::parse(cli_after_help());
    let motion = cli.common.motion(NO_MOTION_ENV);
    let mut app = app_for(cli.scenario, motion, cli.common.frame, cli.common.theme());
    termrock::shell::run(&mut app)
}
