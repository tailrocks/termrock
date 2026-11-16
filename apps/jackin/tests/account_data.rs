#![cfg(feature = "demo")]

mod common;

use common::*;
use jackin_console::domain::account::{
    Account, AccountForm, EditBlocked, LaunchBlock, ProviderChoice, SourceChoice, SourceKind,
};
use jackin_console::domain::account::{CredentialSource, DetectedKind};
use jackin_console::domain::agent::{Agent, AgentBlocked, AuthMode, Provider, UsageSurface};
use jackin_console::domain::fixed::Fixed;
use jackin_console::domain::openshell::{self, env_edit_block};
use jackin_console::domain::vocabulary::Vocabulary;
use jackin_console::domain::workspace::{Usability, usability_of};
use jackin_console::fixtures::{Scenario, world_for};
use jackin_console::world::World;

const USAGE_ONLY: &str = "launch needs a local Codex login · this source is usage only";
const OTHER_AGENT: &str = "usage only · launches Codex only";

fn returning() -> World {
    world_for(Scenario::Returning)
}

fn codex_ids(world: &World) -> Vec<String> {
    world
        .accounts
        .by_provider(Provider::OpenAi)
        .map(|a| a.id.clone())
        .collect()
}

fn block_codex(world: &mut World) {
    world.surfaces.fixed = Some(Fixed::openshell());
    for id in codex_ids(world) {
        world.accounts.get_mut(&id).expect("account").launch_blocked =
            Some(LaunchBlock::UsageOnlySource);
    }
}

fn first_account(world: &World) -> Account {
    world.accounts.accounts[0].clone()
}

fn form(sources: &[(SourceKind, &str)], default_source: Option<usize>) -> AccountForm {
    AccountForm {
        providers: vec![
            ProviderChoice {
                provider: Provider::Cursor,
                label: "Cursor · Cursor".into(),
            },
            ProviderChoice {
                provider: Provider::OpenAi,
                label: "Codex · OpenAI".into(),
            },
        ],
        sources: sources
            .iter()
            .map(|(kind, label)| SourceChoice {
                kind: *kind,
                label: (*label).into(),
            })
            .collect(),
        default_source,
        op_note: Some("Usage only · launching Codex needs a local login".into()),
    }
}

#[test]
fn a_launch_block_makes_the_account_unusable() {
    let mut account = first_account(&returning());
    assert_eq!(usability_of(&account), Usability::Ready);
    account.launch_blocked = Some(LaunchBlock::UsageOnlySource);
    assert_eq!(
        usability_of(&account),
        Usability::Blocked(LaunchBlock::UsageOnlySource)
    );
    assert_eq!(
        usability_of(&account).label(Some(&Fixed::openshell()), &Vocabulary::openshell()),
        USAGE_ONLY
    );
    account.enabled = false;
    assert_eq!(
        usability_of(&account),
        Usability::Disabled("disabled globally".into())
    );
}

#[test]
fn a_launch_blocked_account_is_a_blocked_offer() {
    let mut world = returning();
    let ws = world.workspaces[0].clone();
    let before = world.offer_for(Agent::Codex, Some(&ws), Some("the-architect"));
    assert!(before.preselected.is_some() && before.blocked.is_none());
    block_codex(&mut world);
    for ws in [Some(&ws), None] {
        let offer = world.offer_for(Agent::Codex, ws, Some("the-architect"));
        assert!(offer.accounts.is_empty() && offer.preselected.is_none());
        assert_eq!(offer.blocked.as_deref(), Some(USAGE_ONLY));
        assert!(offer.configured);
    }
}

#[test]
fn an_agent_block_disables_every_other_agent() {
    let mut world = returning();
    let ws = world.workspaces[0].clone();
    let open = world.offer_for(Agent::Codex, Some(&ws), Some("the-architect"));
    world.surfaces.agent_blocked = Some(AgentBlocked {
        launchable: Agent::Codex,
        text: OTHER_AGENT.into(),
    });
    assert_eq!(
        world.offer_for(Agent::Codex, Some(&ws), Some("the-architect")),
        open
    );
    let others: Vec<_> = world
        .offered_agents(Some(&ws), Some("the-architect"))
        .into_iter()
        .filter(|(agent, _)| *agent != Agent::Codex)
        .collect();
    assert!(others.len() >= 3);
    for (agent, offer) in others {
        assert!(offer.accounts.is_empty(), "{agent:?}");
        assert!(offer.preselected.is_none(), "{agent:?}");
        assert_eq!(offer.blocked.as_deref(), Some(OTHER_AGENT), "{agent:?}");
    }
}

#[test]
fn an_agent_block_replaces_the_settings_suffix() {
    let mut world = returning();
    let ws = world.workspaces[0].clone();
    world
        .global
        .agent_modes
        .insert(Agent::ClaudeCode, AuthMode::Ignore);
    let demo = world.offer_for(Agent::ClaudeCode, Some(&ws), None);
    assert!(
        demo.blocked
            .expect("ignored")
            .ends_with("set in Settings › Agents")
    );
    world.surfaces.agent_blocked = Some(AgentBlocked {
        launchable: Agent::Codex,
        text: OTHER_AGENT.into(),
    });
    let live = world.offer_for(Agent::ClaudeCode, Some(&ws), None);
    assert_eq!(live.blocked.as_deref(), Some(OTHER_AGENT));
}

#[test]
fn a_blocked_edit_leaves_the_other_actions_open() {
    let reason = "reads $OPENAI_API_KEY · remove it, then add an account";
    let mut blocked = world_for(Scenario::AccountsMixed);
    blocked.accounts.accounts[0].edit_blocked = Some(EditBlocked::new(reason, "env account"));
    let id = blocked.accounts.accounts[0].id.clone();
    assert!(!blocked.accounts.accounts[0].edit_allowed());
    assert!(blocked.accounts.accounts[0].mutations_allowed());

    let mut gated = app_with(Scenario::AccountsMixed, blocked);
    press(&mut gated, "down down shift+f10");
    hover_text(&mut gated, "Edit…");
    let menu = screen(&mut gated);
    assert!(menu.contains(" env account "), "{menu}");
    press(&mut gated, "esc e");
    let shown = screen(&mut gated);
    assert!(shown.contains(reason), "{shown}");
    assert!(!shown.contains("Edit account"));
    assert!(!shown.contains("e Edit…"));
    let enabled = gated.world.accounts.get(&id).expect("account").enabled;
    press(&mut gated, "t");
    assert_ne!(
        gated.world.accounts.get(&id).expect("account").enabled,
        enabled
    );

    let mut open = app(Scenario::AccountsMixed);
    press(&mut open, "down down e");
    assert!(screen(&mut open).contains("Edit account"));
}

#[test]
fn the_form_lists_come_from_the_data() {
    let mut world = world_for(Scenario::AccountsMixed);
    world.surfaces.account_form = form(
        &[
            (SourceKind::LocalFolder, "Local agent folder  (recommended)"),
            (
                SourceKind::OnePassword,
                "1Password item / field  (usage only)",
            ),
            (SourceKind::PlainApiKey, "Plain-text API key"),
        ],
        Some(0),
    );
    let mut app = app_with(Scenario::AccountsMixed, world);
    press(&mut app, "a");
    let screen = screen(&mut app);
    assert!(screen.contains("Cursor · Cursor"), "{screen}");
    assert!(screen.contains("(●) Local agent folder  (recommended)"));
    assert!(screen.contains("( ) 1Password item / field  (usage only)"));
    assert!(!screen.contains("1Password item / field  (recommended)"));
    assert!(!screen.contains("Usage only · launching Codex"));
}

#[test]
fn the_default_source_decides_which_fields_show() {
    let sources = [
        (SourceKind::PlainApiKey, "Plain-text API key"),
        (SourceKind::LocalFolder, "Local agent folder"),
        (SourceKind::OnePassword, "1Password item / field"),
    ];
    let mut world = world_for(Scenario::AccountsMixed);
    world.surfaces.account_form = form(&sources, None);
    let mut app = app_with(Scenario::AccountsMixed, world);
    press(&mut app, "a");
    let plain = screen(&mut app);
    assert!(
        plain.contains("API key") && plain.contains("paste the key"),
        "{plain}"
    );
    assert!(!plain.contains("1Password reference") && !plain.contains("Local folder"));

    let mut world = world_for(Scenario::AccountsMixed);
    world.surfaces.account_form = form(&sources, Some(2));
    let mut app = app_with(Scenario::AccountsMixed, world);
    press(&mut app, "a");
    let one_password = screen(&mut app);
    assert!(
        one_password.contains("1Password reference"),
        "{one_password}"
    );
    assert!(one_password.contains("Usage only · launching Codex needs a local login"));
    assert!(!one_password.contains("paste the key"));
}

#[test]
fn the_default_form_is_the_demo_form() {
    let mut app = app(Scenario::AccountsMixed);
    press(&mut app, "a");
    let screen = screen(&mut app);
    assert!(screen.contains("Claude Code · Anthropic / Claude"));
    assert!(screen.contains("(●) 1Password item / field  (recommended)"));
    assert!(screen.contains("( ) Local agent folder"));
    assert!(screen.contains("( ) Plain-text API key"));
}

#[test]
fn the_new_providers_have_no_usage_surface() {
    for provider in [Provider::Cursor, Provider::Gemini, Provider::Openrouter] {
        assert_eq!(provider.usage_surface(), UsageSurface::Unsupported);
        assert_eq!(provider.agent(), None);
        assert!(!provider.label().is_empty() && !provider.short().is_empty());
        assert!(provider.plain_key_label().ends_with("API key"));
        assert!(provider.folder_label().ends_with("profile folder"));
    }
}

#[test]
fn registered_mimo_accounts_are_associated_with_mimo_code() {
    let mimo = Account::registered(
        "mimo-account",
        "Mimo",
        Provider::Mimo,
        CredentialSource::HostEnv {
            var: "MIMO_API_KEY".into(),
            detected: DetectedKind::Unknown,
        },
    );
    let unrelated = Account::registered(
        "cursor-account",
        "Cursor",
        Provider::Cursor,
        CredentialSource::HostEnv {
            var: "CURSOR_API_KEY".into(),
            detected: DetectedKind::Unknown,
        },
    );
    assert_eq!(mimo.agent, Some(Agent::MimoCode));
    assert_eq!(unrelated.agent, None);
}

#[test]
fn the_picker_lists_blocked_agents_as_unavailable_rows() {
    let mut world = returning();
    world.surfaces.tuples.clear();
    block_codex(&mut world);
    world.surfaces.agent_blocked = Some(AgentBlocked {
        launchable: Agent::Codex,
        text: OTHER_AGENT.into(),
    });
    let mut app = app_with(Scenario::Returning, world);
    press(&mut app, "down enter");
    let screen = screen(&mut app);
    let row = |agent: &str| {
        screen
            .lines()
            .find(|l| l.contains(&format!("▪ {agent}")))
            .unwrap_or_else(|| panic!("no {agent} row\n{screen}"))
    };
    assert!(
        row("Codex").contains("launch needs a local Codex login"),
        "{screen}"
    );
    assert!(row("Codex").contains("unavailable"));
    assert!(row("Claude Code").contains("usage only · launches Codex only"));
    assert!(row("Claude Code").contains("unavailable"));
    assert!(!screen.contains("ready"));
}

#[test]
fn only_a_host_variable_account_of_the_fixed_world_cannot_be_edited() {
    let world = world_for(Scenario::OpenshellFixed);
    let mut blocked = 0;
    for account in &world.accounts.accounts {
        match &account.source {
            CredentialSource::HostEnv { var, .. } => {
                blocked += 1;
                assert_eq!(account.edit_blocked, Some(env_edit_block(var)), "{var}");
                assert!(!account.edit_allowed());
                assert!(account.mutations_allowed());
            }
            _ => {
                assert_eq!(account.edit_blocked, None, "{}", account.display_name);
                assert!(account.edit_allowed());
            }
        }
    }
    assert_eq!(blocked, 2);
}

#[test]
fn the_edit_block_names_the_variable_and_cuts_a_long_name() {
    let short = env_edit_block("ZAI_API_KEY");
    assert_eq!(
        short.text,
        "reads ZAI_API_KEY · remove it, then add an account"
    );
    assert_eq!(short.short, "env account");
    let long = env_edit_block("A_VERY_LONG_VARIABLE_NAME_INDEED");
    assert_eq!(
        long.text,
        "reads A_VERY_LONG_VARI… · remove it, then add an account"
    );
    assert!(long.reason().within_limits(), "{}", long.text);
    assert!(env_edit_block("MINIMAX_API_TOKEN").reason().within_limits());
}

#[test]
fn the_openshell_projection_of_an_account_blocks_the_edit_of_a_host_variable() {
    let world = world_for(Scenario::Returning);
    let mut account = world.accounts.accounts[0].clone();
    account.edit_blocked = None;
    account.source = CredentialSource::HostEnv {
        var: "ZAI_API_KEY".into(),
        detected: DetectedKind::ZaiApiKeyEnv,
    };

    openshell::account(&mut account);

    assert_eq!(account.edit_blocked, Some(env_edit_block("ZAI_API_KEY")));
    assert!(!account.edit_allowed());
}

#[test]
fn the_enable_toggle_stays_fixed_when_its_reason_has_no_short_text() {
    use ratatui::layout::Position;
    use termrock::core::event::{Input, Mouse, MouseKind};
    use termrock::runtime::Application;

    let mut world = world_for(Scenario::AccountsMixed);
    let mut fixed = Fixed::openshell();
    fixed.enable_global.short = None;
    world.surfaces.fixed = Some(fixed);
    let mut app = app_with(Scenario::AccountsMixed, world);
    let shown = screen(&mut app);
    let (y, line) = shown
        .lines()
        .enumerate()
        .find(|(_, l)| l.contains("Personal"))
        .expect("the account row");
    let x = line.find("Personal").expect("its column") as u16;
    Application::handle(
        &mut app,
        Input::Mouse(Mouse::new(MouseKind::Secondary, Position::new(x, y as u16))),
    );
    screen(&mut app);

    press(&mut app, "down down down down down");

    let menu = screen(&mut app);
    assert!(menu.contains("Every account stay"), "{menu}");
}
