//! Integration coverage for unconstrained generic state defaults.

use termrock::widgets::{
    ActionBarState, ChoiceDialogState, DetailTableState, ListState, StatusBarState, TabsState,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct NoDefault;

struct UnconstrainedId;

#[test]
fn generic_widget_state_defaults_do_not_constrain_consumer_ids() {
    let _: ActionBarState<NoDefault> = ActionBarState::default();
    let _: ChoiceDialogState<NoDefault> = ChoiceDialogState::default();
    let _: DetailTableState<NoDefault> = DetailTableState::default();
    let _: ListState<NoDefault> = ListState::default();
    let _: StatusBarState<NoDefault> = StatusBarState::default();
    let _: TabsState<NoDefault> = TabsState::default();
}

#[test]
fn list_state_ownership_accessors_do_not_constrain_consumer_ids() {
    let mut state = ListState::new(Some(UnconstrainedId));

    assert!(state.selected().is_some());
    assert!(state.is_focused());
    state.set_focused(false);
    state.enable_multi_select();

    assert!(!state.is_focused());
    assert!(state.selection().is_some());
    assert_eq!(state.offset(), 0);
    assert!(state.regions().is_empty());
}
