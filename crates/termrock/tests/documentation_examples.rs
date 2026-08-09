// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Executable mirrors of public documentation and handbook examples.
//!
//! Handbook pages under `docs/content/docs/handbook/` must stay aligned with
//! these tests (component documentation standard).

use ratatui_core::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Text},
};
use termrock::{
    Theme,
    input::{KeyCode, KeyEvent, KeyModifiers},
    interaction::Outcome,
    keymap::{KeyBinding, KeyChord, Keymap, Visibility},
    osc::{PointerShape, Request, encode},
    style::{Density, DesignTokens, Role},
    widgets::{
        Action, ActionBar, ActionBarState, Anchor, CellAlignment, ChoiceDialog, ChoiceDialogState,
        Column, ColumnWidth, CommandPalette, CommandPaletteOutcome, CommandPaletteSize,
        CommandPaletteState, Dialog, DialogSize, InitiatorKind, List, ListRow, ListState,
        ModeIndicator, ModelIndicator, PermissionAction, PermissionOutcome, PermissionPromptState,
        PermissionProvenance, PermissionRequest, PermissionRisk, PromptComposer,
        PromptComposerOutcome, PromptComposerState, ProvenanceHop, Severity, Table, TableRow,
        TableState, Toast, VirtualWindow, data_view_bench, place_command_palette, place_dialog,
    },
};

#[test]
fn toast_documentation_example() {
    let theme = Theme::default();
    let toast = Toast::new(&theme, "Saved", Severity::Success)
        .anchor(Anchor::BottomRight)
        .margins(1, 1);
    assert!(toast.rect(Rect::new(0, 0, 40, 8)).is_some());
}

#[test]
fn list_documentation_example() {
    let tokens = DesignTokens::default();
    let rows = [
        ListRow::item("a", Line::from("Alpha")),
        ListRow::item("b", Line::from("Beta")),
    ];
    let _widget = List::new(&rows, &tokens);
    let mut state = ListState::new(Some("a"));
    let outcome = state.handle_key(&rows, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert!(matches!(outcome, Outcome::Changed));
    assert_eq!(state.selected(), Some(&"b"));
}

#[test]
fn handbook_button_action_bar_example() {
    let theme = Theme::default();
    let actions = [
        Action {
            id: "save",
            label: "Save",
            enabled: true,
            style: None,
        },
        Action {
            id: "cancel",
            label: "Cancel",
            enabled: true,
            style: None,
        },
    ];
    let bar = ActionBar::new(&actions, &theme);
    let mut state = ActionBarState::default();
    state.focused = Some("save");
    let _ = (bar, state);
}

#[test]
fn handbook_datatable_virtual_window_example() {
    let mut rows = VirtualWindow::new(data_view_bench::ROWS_1M, 40);
    let (start, end) = rows.visible_range();
    assert!(end - start <= 40);
    assert_eq!(start, 0);
    let _ = rows.scroll_by(100);
}

#[test]
fn handbook_table_selection_example() {
    let tokens = DesignTokens::default();
    let columns = [Column::new("name", Line::from("Name"), ColumnWidth::Min(12))];
    let cells = [Line::from("alpha")];
    let rows = [TableRow::new("r1", &cells)];
    let table = Table::new(&columns, &rows, &tokens);
    let mut state: TableState<&str, &str> = TableState::new(Some("r1"));
    let _ = (
        table,
        state.handle_key(&rows, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
        CellAlignment::Left,
    );
}

#[test]
fn handbook_dialog_examples() {
    let theme = Theme::default();
    let dialog = Dialog::new("Notice", Text::from("Done."), &theme);
    let area = place_dialog(Rect::new(0, 0, 80, 24), DialogSize::default());
    assert!(area.width > 0);

    let actions = [
        Action {
            id: "ok",
            label: "OK",
            enabled: true,
            style: None,
        },
        Action {
            id: "cancel",
            label: "Cancel",
            enabled: true,
            style: None,
        },
    ];
    let choice = ChoiceDialog::new(
        Dialog::new("Confirm", Text::from("Continue?"), &theme),
        &actions,
    );
    let mut state = ChoiceDialogState::new(Some("cancel"));
    let outcome = state.handle_key(
        &actions,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
    );
    assert!(matches!(outcome, Outcome::Activated("cancel")));
    let _ = (dialog, choice);
}

#[test]
fn handbook_command_palette_example() {
    let tokens = DesignTokens::default();
    let rows = [ListRow::item("quit", Line::from("Quit"))];
    let palette = CommandPalette::new("Commands", &rows, &tokens);
    let rect = place_command_palette(Rect::new(0, 0, 80, 24), CommandPaletteSize::default());
    assert!(rect.width > 0);
    let mut state = CommandPaletteState::new(Some("quit"));
    let outcome = CommandPalette::handle_key(
        &mut state,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        &rows,
    );
    assert!(matches!(outcome, CommandPaletteOutcome::Activated("quit")));
    let _ = palette;
}

#[test]
fn handbook_prompt_composer_example() {
    let theme = Theme::default();
    let tokens = DesignTokens::new(theme.clone(), Density::Comfortable);
    let mut state = PromptComposerState::new();
    state.set_placeholder("Ask anything…");
    state.set_mode(Some(ModeIndicator {
        label: "EDIT".into(),
        warning: false,
    }));
    state.set_model(Some(ModelIndicator {
        label: "model".into(),
    }));
    let _composer = PromptComposer::new(&tokens, &theme);
    state.set_text("ship it");
    let out = state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(
        out,
        PromptComposerOutcome::Submit { ref text, .. } if text == "ship it"
    ));
    state.set_text("keep");
    state.set_focused(false);
    assert_eq!(state.text(), "keep");
}

#[test]
fn handbook_permission_prompt_example() {
    let mut ui = PermissionPromptState::new();
    let req = PermissionRequest::new("r1", "bash", "workspace")
        .risk(PermissionRisk::High)
        .command("rm -rf build/")
        .irreversible()
        .provenance(
            PermissionProvenance::main_agent("run", "main").push(ProvenanceHop::new(
                InitiatorKind::Subagent,
                "s1",
                "reviewer",
            )),
        );
    let generation = ui.enqueue(req);
    assert_eq!(ui.head_generation(), Some(generation));
    assert!(!ui.selected().grants());

    let mut ui = PermissionPromptState::new();
    ui.enqueue(PermissionRequest::new("r1", "read", "src/lib.rs").risk(PermissionRisk::Low));
    let out = ui.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(
        out,
        PermissionOutcome::Decided {
            action: PermissionAction::Deny,
            ..
        }
    ));
}

#[test]
fn theme_documentation_example() {
    let theme = Theme::default().with_role(Role::Accent, Style::new().fg(Color::Cyan));
    assert_eq!(theme.style(Role::Accent).fg, Some(Color::Cyan));
}

#[test]
fn osc_documentation_example() {
    assert_eq!(
        encode(Request::Pointer(PointerShape::Pointer)),
        b"\x1b]22;pointer\x1b\\",
    );
}

#[test]
fn keymap_documentation_example() {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Action {
        Quit,
    }
    static BINDINGS: &[KeyBinding<Action>] = &[KeyBinding::borrowed(
        &[KeyChord::plain(KeyCode::Char('q'))],
        Action::Quit,
        Some("quit"),
        Visibility::Shown,
        None,
    )];

    let keymap = Keymap::from_static(BINDINGS);
    assert_eq!(
        keymap.dispatch(KeyChord::plain(KeyCode::Char('q'))),
        Some(Action::Quit)
    );

    let mut runtime_keymap = keymap.clone();
    runtime_keymap.remap(Action::Quit, vec![KeyChord::ctrl(KeyCode::Char('c'))]);
    assert_eq!(runtime_keymap.glyph_for(Action::Quit), "Ctrl-C");
}
