//! The history list selects a query by its entry id, not its position.

use junie_tui::core::id::WidgetId;

use crate::model::{History, HistoryEntry};
use crate::tabs::HistoryTab;

fn entry(sql: &str) -> HistoryEntry {
    let mut e = History::seeded().entries[0].clone();
    e.sql = sql.into();
    e.connection = "Production".into();
    e
}

fn tab_over(history: &History) -> HistoryTab {
    let mut tab = HistoryTab::new(WidgetId::of("history.keyed"));
    tab.scope_all = true;
    tab.refresh(history, "Production");
    tab
}

#[test]
fn history_selection_follows_the_entry_when_newer_queries_arrive() {
    let mut history = History::seeded();
    let mut tab = tab_over(&history);
    tab.list.cursor = 2;
    let selected = tab.current_entry(&history).expect("an entry").id;

    history.push(entry("SELECT 1 /* newer */"));
    tab.refresh(&history, "Production");
    assert_eq!(
        tab.current_entry(&history).map(|e| e.id),
        Some(selected),
        "a new first row must not move the selection onto another query"
    );
    assert_eq!(tab.list.cursor, 3, "the view position follows the key");

    history.push(entry("SELECT 2 /* newer still */"));
    history.entries.retain(|e| e.id != selected);
    tab.refresh(&history, "Production");
    assert!(
        tab.current_entry(&history).is_some(),
        "a pruned selection falls back to a neighbour deterministically"
    );
}
