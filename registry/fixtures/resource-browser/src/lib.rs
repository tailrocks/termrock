//! Source-owned ResourceBrowser block (Plan 053).

use termrock::input::KeyEvent;
use termrock::patterns::{ResourceBrowserOutcome, ResourceBrowserState};
use termrock::widgets::SidebarItem;

/// Route keys through sidebar; selection becomes LoadRequested.
pub fn handle_key<Id: Clone + PartialEq>(
    state: &mut ResourceBrowserState<Id>,
    key: KeyEvent,
    items: &[SidebarItem<Id>],
) -> ResourceBrowserOutcome<Id> {
    state.handle_key(key, items)
}

/// Stale-preview guard: compare against selection_generation.
pub fn is_stale(state: &ResourceBrowserState<impl Clone + PartialEq>, seen_gen: u64) -> bool {
    state.selection_generation != seen_gen
}
