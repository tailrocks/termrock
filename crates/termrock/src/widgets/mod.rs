//! Product-neutral terminal widgets with borrowed render data and stable IDs.

mod action_bar;
mod detail_table;
mod dialog;
mod diff;
mod hint_bar;
mod list;
mod panel;
mod status_bar;
mod tabs;
mod text_input;
mod toast;

pub use action_bar::{Action, ActionBar, ActionBarState};
pub use detail_table::{
    DetailCapability, DetailRow, DetailTable, DetailTableOutcome, DetailTableState,
};
pub use dialog::{Backdrop, ChoiceDialog, Dialog, DialogAction, MessageDialog};
pub use diff::{DiffKind, DiffLine, DiffState, DiffView};
pub use hint_bar::{Hint, HintBar};
pub use list::{List, ListOutcome, ListRow, ListState, RowRole};
pub use panel::{Panel, PanelEmphasis};
pub use status_bar::{StatusBar, StatusSlot};
pub use tabs::{Tab, Tabs, TabsState};
pub use text_input::{EditAction, TextInput, TextInputState, Validation};
pub use toast::{Anchor, Severity, Toast};

#[cfg(test)]
mod tests;
