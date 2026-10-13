//! Integration coverage for unconstrained generic state defaults.

use termrock::widgets::{
    ActionBarState, ChoiceDialogState, DetailTableState, ListState, StatusBarState, TabsState,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct NoDefault;

#[test]
fn generic_widget_state_defaults_do_not_constrain_consumer_ids() {
    let _: ActionBarState<NoDefault> = ActionBarState::default();
    let _: ChoiceDialogState<NoDefault> = ChoiceDialogState::default();
    let _: DetailTableState<NoDefault> = DetailTableState::default();
    let _: ListState<NoDefault> = ListState::default();
    let _: StatusBarState<NoDefault> = StatusBarState::default();
    let _: TabsState<NoDefault> = TabsState::default();
}
