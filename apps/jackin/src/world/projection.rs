//! Derived views of the world and the counters that prove how often they are
//! rebuilt.
//!
//! Every screen projection (Manager rows, Accounts rows, the daemon liveness
//! scan in [`World::tick`](super::World::tick)) is a pure function of world
//! data. The world stamps that data with [`ProjectionEpochs`], bumps an epoch on
//! every real mutation, and keeps the grouped indices here. A consumer rebuilds
//! only when the epoch it built from is not the current one.
//!
//! These epochs are not the authoritative refresh fence of the account refresh
//! tickets. They count presentation-relevant mutations and nothing else.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use super::{InstanceEntry, InstanceId, InstanceStatus, WorkspaceId, entry_is_running};
use crate::domain::account::AccountRegistry;
use crate::domain::agent::UsageSurface;

/// Which derived views a mutation invalidates.
///
/// `membership` covers which instance belongs to which workspace and in what
/// status. `roster` covers everything a Manager row or card shows, so a
/// membership change always bumps it too. `accounts` covers the account
/// registry and the surface vocabulary the Accounts page reads.
///
/// Mutating `World` public fields after a page has projected them, without
/// calling the matching `mark_*` method or an owning method, is unsupported:
/// the page keeps showing the earlier projection.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProjectionEpochs {
    pub membership: u64,
    pub roster: u64,
    pub accounts: u64,
}

fn bump(epoch: &mut u64) {
    *epoch = epoch.checked_add(1).expect("projection epoch overflow");
}

impl ProjectionEpochs {
    pub(super) fn bump_membership(&mut self) {
        bump(&mut self.membership);
        bump(&mut self.roster);
    }

    pub(super) fn bump_roster(&mut self) {
        bump(&mut self.roster);
    }

    pub(super) fn bump_accounts(&mut self) {
        bump(&mut self.accounts);
    }

    /// A complete snapshot replaced everything since `before`: every epoch is
    /// one past where it stood, however many partial edits happened on the way.
    pub(super) fn bump_all_from(&mut self, before: ProjectionEpochs) {
        let mut next = before;
        bump(&mut next.membership);
        bump(&mut next.roster);
        bump(&mut next.accounts);
        *self = next;
    }
}

/// Everything the world derives from its own data: the epochs, the indices
/// built at an epoch and the counters. One field of `World`, so a pattern that
/// destructures the world names it once.
#[derive(Debug, Default)]
pub struct Projection {
    pub work: WorkCounters,
    pub(super) epochs: ProjectionEpochs,
    pub(super) roster: Mutex<Arc<RosterIndex>>,
    pub(super) accounts: Mutex<Arc<AccountIndex>>,
}

/// Actual work done by derived views, counted where the work happens. A test
/// reads [`World::work`](super::World::work) before and after an interaction and
/// compares the difference; draw calls and elapsed time are never the budget.
#[derive(Debug, Default)]
pub struct WorkCounters {
    source_builds: AtomicU64,
    membership_visits: AtomicU64,
    running_lookups: AtomicU64,
    link_polls: AtomicU64,
    account_sorts: AtomicU64,
    manager_builds: AtomicU64,
    accounts_builds: AtomicU64,
    changed_ticks: AtomicU64,
}

/// A copy of [`WorkCounters`] at one moment.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WorkSnapshot {
    /// Roster index builds (one enumeration of the instance list each).
    pub source_builds: u64,
    /// Instances visited by those builds.
    pub membership_visits: u64,
    /// Running-instance membership lookups made by the daemon scan.
    pub running_lookups: u64,
    /// Daemons whose links were polled by a world tick.
    pub link_polls: u64,
    /// Full account sorts.
    pub account_sorts: u64,
    /// Manager row projections.
    pub manager_builds: u64,
    /// Accounts row projections.
    pub accounts_builds: u64,
    /// App ticks that asked for a frame.
    pub changed_ticks: u64,
}

impl WorkSnapshot {
    /// Work done since `earlier`.
    pub fn since(self, earlier: WorkSnapshot) -> WorkSnapshot {
        WorkSnapshot {
            source_builds: self.source_builds - earlier.source_builds,
            membership_visits: self.membership_visits - earlier.membership_visits,
            running_lookups: self.running_lookups - earlier.running_lookups,
            link_polls: self.link_polls - earlier.link_polls,
            account_sorts: self.account_sorts - earlier.account_sorts,
            manager_builds: self.manager_builds - earlier.manager_builds,
            accounts_builds: self.accounts_builds - earlier.accounts_builds,
            changed_ticks: self.changed_ticks - earlier.changed_ticks,
        }
    }

    /// Builds and sorts that a settled page must not repeat.
    pub fn projection_work(self) -> u64 {
        self.source_builds + self.account_sorts + self.manager_builds + self.accounts_builds
    }
}

fn add(counter: &AtomicU64, n: u64) {
    counter.fetch_add(n, Ordering::Relaxed);
}

impl WorkCounters {
    pub fn snapshot(&self) -> WorkSnapshot {
        let get = |c: &AtomicU64| c.load(Ordering::Relaxed);
        WorkSnapshot {
            source_builds: get(&self.source_builds),
            membership_visits: get(&self.membership_visits),
            running_lookups: get(&self.running_lookups),
            link_polls: get(&self.link_polls),
            account_sorts: get(&self.account_sorts),
            manager_builds: get(&self.manager_builds),
            accounts_builds: get(&self.accounts_builds),
            changed_ticks: get(&self.changed_ticks),
        }
    }

    pub(crate) fn count_manager_build(&self) {
        termrock::record_work!(ProjectionRebuilds, 1usize);
        add(&self.manager_builds, 1);
    }

    pub(crate) fn count_accounts_build(&self) {
        termrock::record_work!(ProjectionRebuilds, 1usize);
        add(&self.accounts_builds, 1);
    }

    pub(crate) fn count_changed_tick(&self) {
        add(&self.changed_ticks, 1);
    }

    pub(super) fn count_running_lookup(&self) {
        add(&self.running_lookups, 1);
    }

    pub(super) fn count_link_poll(&self) {
        add(&self.link_polls, 1);
    }
}

/// The instances of one workspace group, summarised in the same pass that
/// grouped them.
#[derive(Debug, Default)]
pub struct WorkspaceMembers {
    /// Positions in `World::instances`, valid at the index's `built_at` epoch.
    pub indices: Vec<usize>,
    pub running: usize,
    pub failed: bool,
    pub restore: bool,
}

/// Instances grouped by workspace, and the ids of the running ones, built
/// together from one enumeration.
#[derive(Debug, Default)]
pub struct RosterIndex {
    built_at: Option<u64>,
    groups: BTreeMap<Option<WorkspaceId>, WorkspaceMembers>,
    running_ids: BTreeSet<InstanceId>,
}

impl RosterIndex {
    /// The membership epoch this index was built at.
    pub fn built_at(&self) -> Option<u64> {
        self.built_at
    }

    pub fn group(&self, workspace: Option<WorkspaceId>) -> Option<&WorkspaceMembers> {
        self.groups.get(&workspace)
    }

    pub fn groups(&self) -> impl Iterator<Item = (Option<WorkspaceId>, &WorkspaceMembers)> {
        self.groups.iter().map(|(key, members)| (*key, members))
    }

    pub fn is_running(&self, id: &InstanceId) -> bool {
        self.running_ids.contains(id)
    }

    pub fn running_len(&self) -> usize {
        self.running_ids.len()
    }

    pub(super) fn build(entries: &[InstanceEntry], epoch: u64, work: &WorkCounters) -> Self {
        termrock::record_work!(ProjectionRebuilds, 1usize);
        add(&work.source_builds, 1);
        add(&work.membership_visits, entries.len() as u64);
        let (groups, running_ids) = group_instances(entries);
        Self {
            built_at: Some(epoch),
            groups,
            running_ids,
        }
    }
}

/// One enumeration of `entries`: insertion-order indices per workspace with
/// the same hidden-instance rule as `World::entries_of`, plus every running
/// id (hidden instances included, as `World::running_count` counts them).
pub(super) fn group_instances(
    entries: &[InstanceEntry],
) -> (
    BTreeMap<Option<WorkspaceId>, WorkspaceMembers>,
    BTreeSet<InstanceId>,
) {
    let mut groups: BTreeMap<Option<WorkspaceId>, WorkspaceMembers> = BTreeMap::new();
    let mut running_ids = BTreeSet::new();
    for (index, entry) in entries.iter().enumerate() {
        let running = entry_is_running(entry);
        if running {
            running_ids.insert(entry.id().clone());
        }
        if entry.as_available().is_some_and(|i| i.status.hidden()) {
            continue;
        }
        let group = groups.entry(entry.workspace_id()).or_default();
        group.indices.push(index);
        group.running += usize::from(running);
        if let Some(instance) = entry.as_available() {
            group.failed |= matches!(
                instance.status,
                InstanceStatus::FailedSetup | InstanceStatus::Crashed
            );
            group.restore |= instance.status == InstanceStatus::RestoreAvailable;
        }
    }
    (groups, running_ids)
}

/// The account registry sorted once, grouped by surface. Positions refer to
/// `World::accounts.accounts` at the `built_at` accounts epoch.
#[derive(Debug, Default)]
pub struct AccountIndex {
    built_at: Option<u64>,
    sorted: Vec<usize>,
    by_surface: BTreeMap<UsageSurface, Vec<usize>>,
}

impl AccountIndex {
    pub fn built_at(&self) -> Option<u64> {
        self.built_at
    }

    /// Positions of every account in display order.
    pub fn sorted(&self) -> &[usize] {
        &self.sorted
    }

    /// Positions of the accounts of one surface in display order.
    pub fn surface(&self, surface: UsageSurface) -> &[usize] {
        self.by_surface.get(&surface).map_or(&[], Vec::as_slice)
    }

    pub(super) fn build(registry: &AccountRegistry, epoch: u64, work: &WorkCounters) -> Self {
        termrock::record_work!(ProjectionRebuilds, 1usize);
        termrock::record_work!(RankRowsVisited, registry.accounts.len());
        add(&work.account_sorts, 1);
        let mut sorted: Vec<usize> = (0..registry.accounts.len()).collect();
        sorted.sort_by(|a, b| {
            let (a, b) = (&registry.accounts[*a], &registry.accounts[*b]);
            a.surface
                .cmp(&b.surface)
                .then(b.default_for_provider.cmp(&a.default_for_provider))
                .then(a.display_name.cmp(&b.display_name))
        });
        let mut by_surface: BTreeMap<UsageSurface, Vec<usize>> = BTreeMap::new();
        for index in &sorted {
            by_surface
                .entry(registry.accounts[*index].surface)
                .or_default()
                .push(*index);
        }
        Self {
            built_at: Some(epoch),
            sorted,
            by_surface,
        }
    }
}

/// What one world tick did besides delivering messages.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorldTick {
    pub messages: Vec<super::Msg>,
    /// Instances with a link that drained something new this tick, in id
    /// order. Grid-only output appears here and nowhere else.
    pub panes_changed: Vec<InstanceId>,
    /// The subset whose published daemon summary changed.
    pub summaries_changed: Vec<InstanceId>,
}
