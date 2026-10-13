//! Product-neutral terminal widgets with borrowed render data and stable IDs.

pub use crate::interaction::Outcome;

mod action_bar;
mod detail_table;
mod dialog;
mod diff;
mod form;
mod hint_bar;
mod list;
mod panel;
mod selection;
mod split_pane;
mod status_bar;
mod tabs;
mod text_input;
mod toast;
mod tree;
mod viewport;

pub use action_bar::{Action, ActionBar, ActionBarState};
pub use detail_table::{
    DetailCapability, DetailRow, DetailTable, DetailTableOutcome, DetailTableState,
};
pub use dialog::{Backdrop, ChoiceDialog, ChoiceDialogState, Dialog, MessageDialog};
pub use diff::{DiffKind, DiffLine, DiffState, DiffView};
pub use form::{Form, FormField, FormFieldRegion, FormOutcome, FormSection, FormState};
pub use hint_bar::{
    Hint, HintBar, HintSpan, hint_row_cols, render_hint_bar, styled_hint_spans, wrapped_hint_lines,
};
pub use list::{List, ListRow, ListState, RowRole};
pub use panel::{Panel, PanelEmphasis};
pub use selection::Selection;
pub use split_pane::{
    SplitDirection, SplitPane, SplitPaneLayout, SplitPaneOutcome, SplitPaneState, SplitRatio,
    SplitSide,
};
pub use status_bar::{StatusBar, StatusBarState, StatusSlot};
pub use tabs::{TAB_GAP, Tab, TabCell, Tabs, TabsState, lay_out_tabs, tab_at_column};
pub use text_input::{
    EditAction, TextInput, TextInputOutcome, TextInputState, TextInputValidity, Validation,
};
pub use toast::{Anchor, Severity, Toast};
pub use tree::{Tree, TreeNode, TreeNodeStatus, TreeOutcome, TreeState};
pub use viewport::Viewport;

#[cfg(test)]
mod tests;
