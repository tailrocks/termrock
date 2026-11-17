use termrock::clock::SimClock;
use termrock::motion::Motion;
use termrock::theme::Theme;

use super::world_for;
use crate::app::{App, AppOptions, ClipboardMode, Entry, EntryPage, assemble};
use crate::domain::agent::Agent;
use crate::launch::LaunchTarget;
use crate::panes::{ClipboardWrite, PanePolicy};
use crate::world::World;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scenario {
    FirstUse,

    Returning,

    AccountsMixed,

    LaunchRunning,

    LaunchFailure,

    CapsuleMulti,

    OutroLast,

    HardCases,

    RoleEditor,

    RoleBinding,

    DockerSetting,

    DockerNoEngine,

    Doctor,

    LaunchOpenshell,

    OpenshellFixed,

    LaunchChecks,

    CapsuleTerminal,
}

impl Scenario {
    pub const ALL: [Scenario; 17] = [
        Scenario::FirstUse,
        Scenario::Returning,
        Scenario::AccountsMixed,
        Scenario::LaunchRunning,
        Scenario::LaunchFailure,
        Scenario::CapsuleMulti,
        Scenario::OutroLast,
        Scenario::HardCases,
        Scenario::RoleEditor,
        Scenario::RoleBinding,
        Scenario::DockerSetting,
        Scenario::DockerNoEngine,
        Scenario::Doctor,
        Scenario::LaunchOpenshell,
        Scenario::OpenshellFixed,
        Scenario::LaunchChecks,
        Scenario::CapsuleTerminal,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Scenario::FirstUse => "first-use",
            Scenario::Returning => "returning",
            Scenario::AccountsMixed => "accounts-mixed",
            Scenario::LaunchRunning => "launch-running",
            Scenario::LaunchFailure => "launch-failure",
            Scenario::CapsuleMulti => "capsule-multi",
            Scenario::OutroLast => "outro-last",
            Scenario::HardCases => "hard-cases",
            Scenario::RoleEditor => "role-editor",
            Scenario::RoleBinding => "role-binding",
            Scenario::DockerSetting => "docker-setting",
            Scenario::DockerNoEngine => "docker-no-engine",
            Scenario::Doctor => "doctor",
            Scenario::LaunchOpenshell => "launch-openshell",
            Scenario::OpenshellFixed => "openshell-fixed",
            Scenario::LaunchChecks => "launch-checks",
            Scenario::CapsuleTerminal => "capsule-terminal",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|sc| sc.name() == s)
    }
}

pub const EPOCH_SECS: i64 = 1_788_401_640;

pub const UTC_OFFSET_SECS: i64 = 7 * 3600;

pub const CLOCK: SimClock = SimClock::new(EPOCH_SECS, UTC_OFFSET_SECS);

pub fn entry_for(scenario: Scenario) -> Entry {
    let page = match scenario {
        Scenario::AccountsMixed => EntryPage::Accounts,
        Scenario::RoleEditor => EntryPage::RoleEditor {
            role: "sentinel".into(),
        },
        Scenario::RoleBinding => EntryPage::Roles {
            select: Some("sentinel".into()),
        },
        Scenario::LaunchRunning | Scenario::LaunchFailure => EntryPage::Launch(LaunchTarget {
            workspace: Some(1),
            role: "the-architect".into(),
            instances: vec![crate::domain::tuple::InstanceChoice {
                agent: Agent::ClaudeCode,
                provider: crate::domain::agent::Provider::Anthropic,
                account: Some("acct-claude-work".into()),
                model: "claude-sonnet".into(),
            }],
            task: None,
        }),
        Scenario::LaunchOpenshell => EntryPage::Launch(LaunchTarget {
            workspace: Some(1),
            role: "sentinel".into(),
            instances: vec![crate::domain::tuple::InstanceChoice {
                agent: Agent::Codex,
                provider: crate::domain::agent::Provider::OpenAi,
                account: Some("acct-codex-primary".into()),
                model: "gpt-5-codex".into(),
            }],
            task: None,
        }),
        Scenario::CapsuleMulti | Scenario::OutroLast | Scenario::CapsuleTerminal => {
            EntryPage::Attach {
                instance: crate::domain::instance::InstanceId::simulated("jk-7f3a"),
                pane: None,
            }
        }
        Scenario::FirstUse
        | Scenario::Returning
        | Scenario::HardCases
        | Scenario::DockerSetting
        | Scenario::DockerNoEngine
        | Scenario::Doctor
        | Scenario::OpenshellFixed
        | Scenario::LaunchChecks => EntryPage::Manager,
    };
    Entry {
        page,
        outro_when_framed: scenario == Scenario::OutroLast,
    }
}

pub fn app_for(scenario: Scenario, motion: Motion, frame: u64, theme: Theme) -> App {
    app_with_world(scenario, world_for(scenario), motion, frame, theme)
}

pub fn app_with_world(
    scenario: Scenario,
    world: World,
    motion: Motion,
    frame: u64,
    theme: Theme,
) -> App {
    assemble(
        world,
        entry_for(scenario),
        AppOptions {
            motion,
            frame,
            theme,
            clipboard: ClipboardMode::Preview,
            panes: PanePolicy {
                clipboard_write: ClipboardWrite::Deny,
                ..PanePolicy::default()
            },
        },
    )
}
