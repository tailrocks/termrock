//! Lookbook-local picker state prototype.

use termrock::{
    input::{KeyCode, KeyEvent},
    interaction::Outcome,
    widgets::{ListRow, ListState, RowRole, TextInputOutcome, TextInputState},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PickerOutcome<Id> {
    Ignored,
    QueryChanged,
    SelectionChanged,
    Activated(Id),
    Cancelled,
}

pub(crate) struct PickerState<Id> {
    pub(crate) query: TextInputState,
    pub(crate) list: ListState<Id>,
    previous_visible: Vec<Id>,
}

impl<Id: Clone + PartialEq> PickerState<Id> {
    pub(crate) fn new(selected: Option<Id>) -> Self {
        Self {
            query: TextInputState::new("").with_max_graphemes(64),
            list: ListState::new(selected),
            previous_visible: Vec::new(),
        }
    }

    pub(crate) fn query_text(&self) -> &str {
        self.query.value()
    }

    pub(crate) fn reconcile(&mut self, visible: &[ListRow<'_, Id>]) {
        let selectable = visible
            .iter()
            .filter(|row| row.enabled && row.role == RowRole::Item)
            .map(|row| row.id.clone())
            .collect::<Vec<_>>();
        if selectable.is_empty() {
            self.list.select(None);
            self.previous_visible = selectable;
            return;
        }
        if self
            .list
            .selected
            .as_ref()
            .is_some_and(|selected| selectable.contains(selected))
        {
            self.previous_visible = selectable;
            return;
        }
        let fallback = self
            .list
            .selected
            .as_ref()
            .and_then(|selected| self.previous_visible.iter().position(|id| id == selected))
            .unwrap_or(0)
            .min(selectable.len() - 1);
        self.list.select(Some(selectable[fallback].clone()));
        self.previous_visible = selectable;
    }

    pub(crate) fn handle_key(
        &mut self,
        visible: &[ListRow<'_, Id>],
        key: KeyEvent,
    ) -> PickerOutcome<Id> {
        match key.code {
            KeyCode::Up | KeyCode::Down | KeyCode::PageUp | KeyCode::PageDown => {
                match self.list.handle_key(visible, key) {
                    Outcome::Changed => PickerOutcome::SelectionChanged,
                    Outcome::Activated(id) => PickerOutcome::Activated(id),
                    Outcome::Cancelled => PickerOutcome::Cancelled,
                    Outcome::Ignored => PickerOutcome::Ignored,
                    _ => PickerOutcome::Ignored,
                }
            }
            KeyCode::Enter => match self.list.activate(visible) {
                Outcome::Activated(id) => PickerOutcome::Activated(id),
                _ => PickerOutcome::Ignored,
            },
            KeyCode::Esc if !self.query.value().is_empty() => {
                self.query = TextInputState::new("").with_max_graphemes(64);
                PickerOutcome::QueryChanged
            }
            KeyCode::Esc => PickerOutcome::Cancelled,
            _ => match self.query.handle_key(key) {
                TextInputOutcome::Changed => PickerOutcome::QueryChanged,
                TextInputOutcome::Cancelled => PickerOutcome::Cancelled,
                TextInputOutcome::Submitted(_) | TextInputOutcome::Ignored => {
                    PickerOutcome::Ignored
                }
                _ => PickerOutcome::Ignored,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::text::Line;
    use termrock::{
        input::{KeyEvent, KeyModifiers},
        widgets::ListRow,
    };

    use super::*;

    fn rows(ids: &[&'static str]) -> Vec<ListRow<'static, &'static str>> {
        ids.iter()
            .map(|id| ListRow {
                id: *id,
                label: Line::from(*id),
                trailing: None,
                role: RowRole::Item,
                enabled: true,
            })
            .collect()
    }

    #[test]
    fn reconciliation_is_id_sticky_then_index_fallback() {
        let cases = [
            (Some("beta"), &["alpha", "beta", "gamma"][..], Some("beta")),
            (Some("beta"), &["beta", "gamma"][..], Some("beta")),
            (Some("beta"), &["alpha", "gamma"][..], Some("gamma")),
            (Some("gamma"), &["alpha"][..], Some("alpha")),
            (Some("alpha"), &[][..], None),
        ];
        for (selected, filtered, expected) in cases {
            let mut state = PickerState::new(selected);
            state.reconcile(&rows(&["alpha", "beta", "gamma"]));
            state.reconcile(&rows(filtered));
            assert_eq!(state.list.selected, expected, "visible={filtered:?}");
        }
    }

    #[test]
    fn typing_and_backspace_report_query_changes() {
        let visible = rows(&["alpha"]);
        let mut state = PickerState::new(Some("alpha"));
        assert_eq!(
            state.handle_key(
                &visible,
                KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE)
            ),
            PickerOutcome::QueryChanged
        );
        assert_eq!(state.query_text(), "a");
        assert_eq!(
            state.handle_key(
                &visible,
                KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)
            ),
            PickerOutcome::QueryChanged
        );
    }

    #[test]
    fn escape_clears_query_then_cancels() {
        let visible = rows(&["alpha"]);
        let mut state = PickerState::new(Some("alpha"));
        let _ = state.handle_key(
            &visible,
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
        );
        assert_eq!(
            state.handle_key(&visible, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            PickerOutcome::QueryChanged
        );
        assert_eq!(
            state.handle_key(&visible, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            PickerOutcome::Cancelled
        );
    }

    #[test]
    fn arrows_move_list_while_text_keys_edit_query() {
        let visible = rows(&["alpha", "beta"]);
        let mut state = PickerState::new(Some("alpha"));
        state.reconcile(&visible);
        assert_eq!(
            state.handle_key(&visible, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            PickerOutcome::SelectionChanged
        );
        assert_eq!(state.list.selected, Some("beta"));
        assert!(state.query_text().is_empty());
    }

    #[test]
    fn enter_activates_the_stable_id() {
        let visible = rows(&["alpha", "beta"]);
        let mut state = PickerState::new(Some("beta"));
        assert_eq!(
            state.handle_key(&visible, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            PickerOutcome::Activated("beta")
        );
    }
}
