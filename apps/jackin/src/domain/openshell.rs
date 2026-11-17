//! The one definition of every Fixed value of the OpenShell world.
//!
//! The fixture world of `openshell-fixed` and jackin's projection both call
//! these functions, so the two cannot disagree. The class of each field is in
//! `fixed_fields`.

use super::account::{Account, AccountOrigin, AccountRegistry, CredentialSource, EditBlocked};
use super::agent::{Agent, AuthMode};
use super::fixed_fields::SurfaceFixed;
use super::instance::Instance;
use super::surfaces::Surfaces;
use super::workspace::{DirtyExitPolicy, Isolation, Mount, RoleEntry, Workspace};
use crate::world::GlobalConfig;

pub const PROJECT_DIR: &str = "/sandbox/project";

pub fn account(a: &mut Account) {
    a.enabled = true;
    a.origin = AccountOrigin::Registered;
    if let CredentialSource::HostEnv { var, .. } = &a.source {
        a.edit_blocked = Some(env_edit_block(var));
    }
}

/// Edit… is refused for an account that reads a host variable: the account
/// form has no such source. The variable name is cut to 16 columns plus `…`,
/// so the text stays within `Reason::TEXT_MAX`.
pub fn env_edit_block(var: &str) -> EditBlocked {
    let var = termrock::ui::text::truncate(var, 17);
    EditBlocked::new(
        &format!("reads {var} · remove it, then add an account"),
        "env account",
    )
}

/// The policy switches the workspace editor shows as fixed. A workspace the
/// wizard creates starts with them, so the editor's boxes match its record.
pub fn policies(ws: &mut Workspace) {
    ws.keep_awake = false;
    ws.git_pull = false;
    ws.dirty_policy = DirtyExitPolicy::Ask;
}

/// `host_path` is Data going in. `accounts` is the registry with `enabled` and
/// `default_for_provider` already set. The set `accounts.enabled` is written
/// here: it holds every account that is not a provider default, so the
/// workspace's effective set is the whole registry.
pub fn workspace(ws: &mut Workspace, host_path: &str, accounts: &AccountRegistry) {
    ws.workdir = PROJECT_DIR.into();
    ws.mounts = vec![Mount::host(host_path, PROJECT_DIR).isolation(Isolation::Clone)];
    policies(ws);
    ws.accounts.disabled_defaults.clear();
    ws.accounts.enabled = accounts
        .accounts
        .iter()
        .filter(|a| !a.default_for_provider)
        .map(|a| a.id.clone())
        .collect();
    ws.accounts.role_preferred.clear();
}

pub fn global(g: &mut GlobalConfig) {
    g.coauthor_trailer = false;
    g.dco_signoff = false;
    g.mounts.clear();
    g.role_env.clear();
    g.agent_modes = Agent::ALL
        .into_iter()
        .map(|agent| {
            let mode = g.agent_modes.get(&agent).copied().unwrap_or(AuthMode::Sync);
            (agent, mode)
        })
        .collect();
    for row in &mut g.trust {
        row.trusted = true;
    }
}

pub fn role(r: &mut RoleEntry) {
    r.trusted = true;
}

pub fn instance(i: &mut Instance) {
    i.workdir = Some(PROJECT_DIR.into());
}

pub fn surfaces(s: &mut Surfaces) {
    let SurfaceFixed {
        vocabulary,
        fixed,
        account_form,
        agent_blocked,
        wizard,
        launch_role,
    } = SurfaceFixed::openshell();
    s.vocabulary = vocabulary;
    s.fixed = fixed;
    s.account_form = account_form;
    s.agent_blocked = agent_blocked;
    s.wizard = wizard;
    s.launch_role = launch_role;
}
