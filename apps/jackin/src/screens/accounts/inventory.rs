use crate::backend::Ticket;
use crate::usage_connections::{UsageAction, UsageEntry, UsageUpdate};
use crate::world::World;

#[derive(Default)]
pub(super) struct Inventory {
    ticket: Option<Ticket>,
    revision: u64,
    pub entries: Vec<UsageEntry>,
    loaded: bool,
    error: Option<String>,
}

impl Inventory {
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn load(&mut self, world: &mut World) {
        let ticket = world.next_ticket();
        if let Some(service) = world.backend.usage_connections() {
            self.revision = self.revision.wrapping_add(1);
            self.ticket = Some(ticket);
            self.error = None;
            service.request(ticket, UsageAction::List);
        }
    }

    pub fn update(&mut self, ticket: Ticket, update: &UsageUpdate) -> bool {
        if self.ticket != Some(ticket) {
            return false;
        }
        match update {
            UsageUpdate::Accounts(entries) => self.replace(entries),
            UsageUpdate::Failed(error) => {
                self.ticket = None;
                self.error = Some(error.clone());
                self.revision = self.revision.wrapping_add(1);
            }
            _ => return false,
        }
        true
    }

    pub fn replace(&mut self, entries: &[UsageEntry]) {
        self.revision = self.revision.wrapping_add(1);
        self.entries = entries.to_vec();
        self.loaded = true;
        self.ticket = None;
        self.error = None;
    }

    pub fn count(&self) -> Option<usize> {
        self.loaded.then_some(self.entries.len())
    }

    pub fn count_caption(&self) -> String {
        match self.count() {
            Some(count) => format!(
                "{count} usage{}",
                if self.error.is_some() {
                    " (cached)"
                } else {
                    ""
                }
            ),
            None if self.error.is_some() => "Usage unavailable".into(),
            None => "Usage loading".into(),
        }
    }

    pub fn row_status(&self) -> String {
        if self.error.is_some() {
            "Unavailable".into()
        } else if !self.loaded {
            "Loading…".into()
        } else {
            self.status()
        }
    }

    pub fn status(&self) -> String {
        if let Some(error) = &self.error {
            return format!("Unavailable · {error}");
        }
        if !self.loaded {
            return "Loading usage accounts…".into();
        }
        let count = self.entries.len();
        let noun = if count == 1 { "account" } else { "accounts" };
        format!(
            "{count} {noun}{}",
            if self.ticket.is_some() {
                " · refreshing…"
            } else {
                ""
            }
        )
    }
}
