use termrock::core::refusal::Reason;

use super::agent::Agent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fixed {
    pub keep_awake: Reason,
    pub git_pull: Reason,
    pub dirty_exit: Reason,
    pub mounts: Reason,
    pub workdir: Reason,
    pub enable_workspace: Reason,
    pub enable_global: Reason,
    pub typed_key: Reason,
    pub wizard_ro: Reason,
    pub destination: Reason,
    pub reveal_open: Reason,

    pub last_role: String,
    pub git_url: String,
    pub save_hint: String,

    pub commits: Reason,
    pub trust: Reason,
    pub env_sources: Reason,
    pub role_env: Reason,
    pub launchable: Agent,
    pub launch_usage_only: String,
    pub agent_launchable: Reason,
    pub agent_other: Reason,
    pub agent_notes: [String; 2],
    pub env_secret_note: [String; 2],
    pub env_reference: String,
}

impl Fixed {
    pub fn openshell() -> Self {
        Self {
            keep_awake: Reason::new("Always off · set sleep on the gateway host"),
            git_pull: Reason::new("Always off · never edits your checkout"),
            dirty_exit: Reason::new("Always ask · work is never dropped silently"),
            mounts: Reason::new("jackin uploads one project and mounts nothing"),
            workdir: Reason::new("Always /sandbox/project · jackin uploads the project there"),
            enable_workspace: Reason::new("All accounts are active here · press p to prefer one"),
            enable_global: Reason::new("Every account stays enabled · remove it to stop using it")
                .short("no disabled state"),
            typed_key: Reason::new("jackin stores no typed keys · use a local login"),
            wizard_ro: Reason::new("Always off · the sandbox edits its own copy"),
            destination: Reason::new("Uploaded to /sandbox/project · you cannot change it"),
            reveal_open: Reason::new("jackin prints the exported path · it opens nothing")
                .short("prints path only"),
            last_role: "Keep one role allowed · allow another first".into(),
            git_url: "Clone the repository first, then choose its folder".into(),
            save_hint: "Saved {name} · set a default role in Roles".into(),
            commits: Reason::new("Both stay off · jackin does not edit commit messages"),
            trust: Reason::new("Sources you add are trusted · remove the role to revoke"),
            env_sources: Reason::new("Plain values only · use an account for credentials"),
            role_env: Reason::new("Set role variables in a workspace instead"),
            launchable: Agent::Codex,
            launch_usage_only: "launch needs a local Codex login · this source is usage only"
                .into(),
            agent_launchable: Reason::new("Set by each account · change it in Accounts (c)"),
            agent_other: Reason::new("Set by each account · change it in Accounts (c)"),
            agent_notes: [
                "The mode comes from each account's credential source.".into(),
                "Explicit Ignore blocks launch; other modes stay available.".into(),
            ],
            env_secret_note: [
                "Keep secrets out of env. Values are stored as plain text.".into(),
                "Use an account for credentials.".into(),
            ],
            env_reference: "Plain values only · use an account for credentials".into(),
        }
    }
}
