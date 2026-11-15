mod app;

mod domain;
mod scenario;
mod screens;
mod sim;

use clap::Parser;
use junie_tui::cli::CommonArgs;

use crate::app::App;
use crate::scenario::Scenario;

const NO_MOTION_ENV: &str = "HOLLA_NO_MOTION";

#[derive(Debug, Parser)]
#[command(
    name = "holla",
    about = "Context-adaptive action launcher",
    long_about = "Holla — this folder, this host, right now (deterministic preview on the Junie design system)"
)]
struct Cli {
    #[command(flatten)]
    common: CommonArgs,
    /// The fixture scenario to open
    #[arg(short = 's', long, value_parser = parse_scenario, default_value = "first-use")]
    scenario: Scenario,
}

fn parse_scenario(value: &str) -> Result<Scenario, String> {
    Scenario::from_name(value).ok_or_else(|| {
        format!(
            "unknown scenario {value:?}; use one of {}",
            scenario_names(&Scenario::ALL)
        )
    })
}

fn scenario_names(scenarios: &[Scenario]) -> String {
    scenarios
        .iter()
        .map(|s| s.name())
        .collect::<Vec<_>>()
        .join(", ")
}

fn cli_after_help() -> String {
    format!(
        "Scenarios: {concept}\n\
         Parity:    {parity}\n\
         {motion}\n\
         Frame:     with --motion paused, the exact fixture tick to render\n\n\
         Keys: type to search · ↑↓ move · Enter run · Alt+Enter alternatives · Tab preview · Ctrl+↑↓ scope · Ctrl+G activities · F1 key reference · Ctrl+Q quit\n\
         Preview: every scenario is captured under snapshots/holla/; src/bin/holla/README.md lists what each scenario shows.\n\
         Everything is simulated in memory; no stack command is ever executed.",
        concept = scenario_names(&Scenario::CONCEPT),
        parity = scenario_names(&Scenario::PARITY),
        motion = junie_tui::cli::motion_help(NO_MOTION_ENV),
    )
}

fn main() -> std::io::Result<()> {
    let cli: Cli = junie_tui::cli::parse(cli_after_help());
    let motion = cli.common.motion(NO_MOTION_ENV);
    let history = !std::env::var_os("HOLLA_NO_HISTORY").is_some_and(|v| v == "1");
    let mut app = App::launch(
        cli.scenario,
        motion,
        cli.common.frame,
        cli.common.theme(),
        history,
    );
    junie_tui::shell::run(&mut app)
}
