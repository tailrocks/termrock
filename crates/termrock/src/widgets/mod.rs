//! Product-neutral terminal widgets with borrowed render data and stable IDs.

pub use crate::interaction::Outcome;

mod action_bar;
mod completion_menu;
mod detail_table;
mod dialog;
mod diff;
mod edit_core;
mod form;
mod hint_bar;
mod list;
mod log_pane;
mod panel;
mod picker;
mod progress;
mod selection;
mod split_pane;
mod status_bar;
mod table;
mod tabs;
mod text_area;
mod text_input;
mod toast;
mod tree;
mod viewport;
mod virtual_grid;

pub use action_bar::{Action, ActionBar, ActionBarState};
pub use completion_menu::{
    CompletionCandidate, CompletionMenu, CompletionMenuOutcome, CompletionMenuSize,
    CompletionMenuState, place_completion_menu,
};
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
pub use log_pane::{LogPane, LogPaneState};
pub use panel::{Panel, PanelEmphasis};
pub use picker::{Picker, PickerOutcome, PickerState};
pub use progress::{Progress, ProgressKind};
pub use selection::Selection;
pub use split_pane::{
    SplitDirection, SplitPane, SplitPaneLayout, SplitPaneOutcome, SplitPaneState, SplitRatio,
    SplitSide,
};
pub use status_bar::{StatusBar, StatusBarState, StatusSlot};
pub use table::{
    CellAlignment, Column, ColumnWidth, SortDirection, Table, TableHeaderRegion, TableOutcome,
    TableRow, TableRowRegion, TableState, resolve_widths,
};
pub use tabs::{TAB_GAP, Tab, TabCell, Tabs, TabsState, lay_out_tabs, tab_at_column};
pub use text_area::{TextArea, TextAreaOutcome, TextAreaState, TextCursor};
pub use text_input::{
    EditAction, TextInput, TextInputOutcome, TextInputState, TextInputValidity, Validation,
};
pub use toast::{Anchor, Severity, Toast, ToastLifetime, ToastState};
pub use tree::{Tree, TreeNode, TreeNodeStatus, TreeOutcome, TreeState};
pub use viewport::Viewport;
pub use virtual_grid::{
    GridCell, GridCellRegion, GridColumn, GridColumnWidth, GridHeaderRegion, GridRow, VirtualGrid,
    VirtualGridOutcome, VirtualGridState,
};

#[cfg(test)]
mod tests;
