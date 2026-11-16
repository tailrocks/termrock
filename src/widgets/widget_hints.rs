use crate::core::hints::{
    Binding, Bindings, HasHints, HintCx, actions_bindings, nav_bindings, nav_hints,
};
use crate::core::keymap::hint_key;
use crate::widgets::button::Button;
use crate::widgets::code::CodeEditor;
use crate::widgets::completion::Completion;
use crate::widgets::grid::{DataGrid, GridCmd};
use crate::widgets::input::TextInput;
use crate::widgets::keyhint::{EDIT_CELL, Hint, hint};
use crate::widgets::list::{ListBox, ListCmd, SelectMode};
use crate::widgets::select::Select;
use crate::widgets::table::{DataTable, TableCmd};
use crate::widgets::tabs::Tabs;
use crate::widgets::textarea::TextArea;
use crate::widgets::tree::TreeView;

pub const EDIT: Hint = hint(EDIT_PAIR.0, EDIT_PAIR.1);

pub const EDIT_PAIR: (&str, &str) = ("Enter", "Edit");

impl HasHints for ListBox {
    fn hints(&self, _: &HintCx) -> Vec<Hint> {
        if self.items.is_empty() {
            return vec![];
        }
        let mut v = nav_hints(&self.nav, self.type_ahead, false);
        if self.mode == SelectMode::Multi {
            v.push(hint("Space", "Toggle"));
            if let Some(k) = hint_key(&self.keymap, ListCmd::SelectAll) {
                v.push(Hint::new(k, "Select all"));
            }
        }
        v
    }
}

impl HasHints for TreeView {
    fn hints(&self, _: &HintCx) -> Vec<Hint> {
        let mut v = nav_hints(&self.nav, self.type_ahead, false);
        v.push(hint("←→", "Fold"));
        v
    }
}

impl HasHints for Select {
    fn hints(&self, _: &HintCx) -> Vec<Hint> {
        if self.disabled {
            return vec![];
        }
        if !self.open {
            return vec![hint("Enter", "Open")];
        }
        let mut v = nav_hints(&self.list.nav, self.list.type_ahead, false);
        v.extend([hint("Enter", "Choose"), hint("Esc", "Close")]);
        v
    }
}

impl HasHints for Completion {
    fn hints(&self, _: &HintCx) -> Vec<Hint> {
        if !self.is_open() {
            return vec![];
        }

        let mut v = nav_hints(&self.list.nav, false, false);
        if self.has_eligible() {
            v.push(hint("Enter", "Accept"));
        }
        v.push(hint("Esc", "Close"));
        v
    }
}

impl HasHints for Tabs {
    fn hints(&self, _: &HintCx) -> Vec<Hint> {
        vec![hint("←→", "Switch tab")]
    }
}

impl HasHints for DataTable {
    fn hints(&self, _: &HintCx) -> Vec<Hint> {
        if self.is_editing() {
            return crate::widgets::keyhint::EDIT_FIELD.to_vec();
        }
        let mut v = nav_hints(&self.nav, self.type_ahead, false);
        if let Some(k) = hint_key(&self.keymap, TableCmd::SortByColumn) {
            v.push(Hint::new(k, "Sort"));
        }
        v
    }
}

impl HasHints for DataGrid {
    fn hints(&self, _: &HintCx) -> Vec<Hint> {
        if self.is_editing() {
            return EDIT_CELL.to_vec();
        }
        let mut v = vec![hint("↑↓←→", "Cell")];
        if self.cell_editable() {
            v.push(EDIT);
        }
        if let Some(k) = hint_key(&self.keymap, GridCmd::Copy) {
            v.push(Hint::new(k, "Copy"));
        }
        v
    }
}

impl HasHints for TextInput {
    fn hints(&self, _: &HintCx) -> Vec<Hint> {
        TextInput::hints(self)
    }
}

impl HasHints for TextArea {
    fn hints(&self, _: &HintCx) -> Vec<Hint> {
        TextArea::hints(self)
    }
}

impl HasHints for CodeEditor {
    fn hints(&self, _: &HintCx) -> Vec<Hint> {
        CodeEditor::hints(self)
    }
}

impl HasHints for Button {
    fn hints(&self, _: &HintCx) -> Vec<Hint> {
        if self.disabled {
            return vec![];
        }
        vec![self.hint()]
    }
}

impl Bindings for ListBox {
    fn bindings(&self) -> Vec<Binding> {
        let mut v = nav_bindings(&self.nav, self.type_ahead);
        v.push(Binding::group("Enter", "Activate"));
        if self.mode == SelectMode::Multi {
            v.push(Binding::group("Space", "Toggle"));
            v.extend(actions_bindings(&self.keymap));
        }
        v
    }
}

impl Bindings for TreeView {
    fn bindings(&self) -> Vec<Binding> {
        let mut v = nav_bindings(&self.nav, self.type_ahead);
        v.push(Binding::group("←→", "Fold / unfold"));
        v.push(Binding::group("Enter", "Activate"));
        v.extend(actions_bindings(&self.keymap));
        v
    }
}

impl Bindings for Tabs {
    fn bindings(&self) -> Vec<Binding> {
        let mut v = vec![Binding::group("←→", "Switch tab")];
        v.extend(actions_bindings(&self.keymap));
        v
    }
}

impl Bindings for DataTable {
    fn bindings(&self) -> Vec<Binding> {
        let mut v = nav_bindings(&self.nav, self.type_ahead);
        v.push(Binding::group("Enter", "Activate"));
        v.extend(actions_bindings(&self.keymap));
        v
    }
}

impl Bindings for DataGrid {
    fn bindings(&self) -> Vec<Binding> {
        let mut v = vec![Binding::group("↑↓←→", "Cell")];
        if self.editable {
            v.push(Binding::group("Enter F2", "Edit"));
        }
        v.extend(actions_bindings(&self.keymap));
        v
    }
}

impl Bindings for Select {
    fn bindings(&self) -> Vec<Binding> {
        let mut v = vec![Binding::group("Enter Space", "Open")];
        v.extend(nav_bindings(&self.list.nav, self.list.type_ahead));
        v.push(Binding::group("Esc", "Close"));
        v
    }
}
