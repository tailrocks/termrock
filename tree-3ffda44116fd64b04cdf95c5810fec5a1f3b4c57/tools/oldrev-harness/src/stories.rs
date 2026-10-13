// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};
use termrock::{
    Theme,
    scroll::DialogScroll,
    style::Role,
    widgets::{
        Action, ActionBar, ActionBarState, Anchor, Backdrop, ChoiceDialog, ChoiceDialogState,
        DetailCapability, DetailRow, DetailTable, DetailTableState, Dialog, DiffKind, DiffLine,
        DiffState, DiffView, Hint, HintBar, List, ListRow, ListState, MessageDialog, Panel,
        PanelEmphasis, Progress, ProgressKind, RowRole, Severity, StatusBar, StatusBarState,
        StatusSlot, Tab, Tabs, TabsState, TextInput, TextInputState, Toast, Validation, Viewport,
    },
};

type RenderFn = fn(&mut Frame<'_>, Rect, &Theme);

#[derive(Debug, Clone, Copy)]
pub(crate) struct Story {
    pub(crate) id: &'static str,
    pub(crate) width: u16,
    pub(crate) height: u16,
    render: RenderFn,
}

impl Story {
    const fn new(id: &'static str, width: u16, height: u16, render: RenderFn) -> Self {
        Self {
            id,
            width,
            height,
            render,
        }
    }

    pub(crate) fn render(self, frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
        (self.render)(frame, area, theme);
    }
}

pub(crate) fn stories() -> Vec<Story> {
    vec![
        Story::new("panel/focused", 48, 7, panel),
        Story::new("action-bar/basic", 48, 2, action_bar),
        Story::new("tabs/status", 52, 2, tabs),
        Story::new("tabs/narrow", 16, 2, tabs),
        Story::new("hint-bar/wrapped", 42, 2, hint_bar),
        Story::new("list/selection", 42, 6, list),
        Story::new("list/narrow", 14, 6, list),
        Story::new("list/unicode", 28, 5, list_unicode),
        Story::new("progress/determinate", 42, 2, progress),
        Story::new("progress/narrow", 14, 2, progress_narrow),
        Story::new("progress/unicode", 34, 2, progress_unicode),
        Story::new("detail-table/basic", 54, 5, detail_table),
        Story::new("detail-table/unicode", 30, 6, detail_table_unicode),
        Story::new("status-bar/basic", 60, 1, status_bar),
        Story::new("status-bar/narrow", 20, 1, status_bar),
        Story::new("dialog/message", 48, 7, dialog),
        Story::new("dialog/narrow", 20, 7, dialog),
        Story::new("choice-dialog/basic", 48, 7, choice_dialog),
        Story::new("message-dialog/details", 52, 8, message_dialog),
        Story::new("diff/basic", 54, 6, diff),
        Story::new("toast/success", 34, 4, toast),
        Story::new("toast/narrow", 16, 4, toast),
        Story::new("backdrop/basic", 34, 4, backdrop),
        Story::new("viewport/both-axes", 44, 7, viewport),
        Story::new("text-input/unicode", 28, 1, text_input_unicode),
    ]
}

fn panel(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    frame.render_widget(
        Panel::new(theme)
            .title("Summary")
            .emphasis(PanelEmphasis::Focused),
        area,
    );
    if area.width > 2 && area.height > 2 {
        frame.render_widget(
            Paragraph::new("State   Ready\nMode    Interactive"),
            Rect::new(area.x + 1, area.y + 1, area.width - 2, area.height - 2),
        );
    }
}

fn progress(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let determinate = Rect::new(area.x, area.y, area.width, area.height.min(1));
    frame.render_widget(
        Progress::new(ProgressKind::Determinate { fraction: 0.62 }, theme).label("Processing"),
        determinate,
    );
    if area.height > 1 {
        frame.render_widget(
            Progress::new(ProgressKind::Indeterminate { tick: 3 }, theme).label("Waiting"),
            Rect::new(area.x, area.y.saturating_add(1), area.width, 1),
        );
    }
}

fn progress_narrow(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    const ASCII_FRAMES: [&str; 4] = ["|", "/", "-", "\\"];
    let [bar, spinner] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(area);
    frame.render_widget(
        Progress::new(ProgressKind::Determinate { fraction: 0.62 }, theme).label("Build"),
        bar,
    );
    frame.render_widget(
        Progress::new(ProgressKind::Indeterminate { tick: 3 }, theme)
            .frames(&ASCII_FRAMES)
            .label("Waiting"),
        spinner,
    );
}

fn progress_unicode(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let [bar, spinner] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(area);
    frame.render_widget(
        Progress::new(ProgressKind::Determinate { fraction: 0.5 }, theme).label("東京を処理中 🪨"),
        bar,
    );
    frame.render_widget(
        Progress::new(ProgressKind::Indeterminate { tick: 6 }, theme).label("検証中 ✓"),
        spinner,
    );
}

fn action_bar(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let actions = [
        Action {
            id: "accept",
            label: "Accept",
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
    let mut state = ActionBarState {
        focused: Some("accept"),
        ..ActionBarState::default()
    };
    frame.render_stateful_widget(&ActionBar::new(&actions, theme).gap("  "), area, &mut state);
}

fn tabs(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let items = [
        Tab {
            id: "overview",
            label: "Overview",
            glyph: Some(Span::styled("●", theme.style(Role::Success))),
            active: true,
            enabled: true,
        },
        Tab {
            id: "details",
            label: "Details",
            glyph: None,
            active: false,
            enabled: true,
        },
    ];
    let mut state = TabsState {
        selected: Some("overview"),
        focused: true,
        ..TabsState::default()
    };
    frame.render_stateful_widget(&Tabs::new(&items, theme).gap(1), area, &mut state);
}

fn hint_bar(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let theme = if *theme == Theme::tailrocks_phosphor() {
        theme
            .clone()
            .with_role(Role::HintKey, Style::new().bold())
            .with_role(Role::HintText, Style::new())
            .with_role(Role::HintDim, Style::new())
            .with_role(Role::HintSeparator, Style::new())
    } else {
        theme.clone()
    };
    let hints = [
        Hint {
            chord: "↑↓",
            label: "navigate",
            priority: 1,
            visible: true,
        },
        Hint {
            chord: "Enter",
            label: "choose",
            priority: 1,
            visible: true,
        },
        Hint {
            chord: "Esc",
            label: "close",
            priority: 2,
            visible: true,
        },
    ];
    frame.render_widget(HintBar::new(&hints, &theme).separator("  "), area);
}

fn list(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let rows = list_rows();
    let mut state = ListState::new(Some("beta"));
    state.enable_multi_select();
    state.selection_mut().unwrap().toggle(&"alpha");
    frame.render_stateful_widget(&List::new(&rows, theme), area, &mut state);
}

fn list_unicode(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let rows = [
        ListRow {
            id: "cjk",
            label: Line::from("東京 設定"),
            trailing: Some(Line::from("日本語")),
            role: RowRole::Item,
            enabled: true,
        },
        ListRow {
            id: "emoji",
            label: Line::from("🧪 Laboratory"),
            trailing: Some(Line::from("✅")),
            role: RowRole::Item,
            enabled: true,
        },
        ListRow {
            id: "combining",
            label: Line::from("Cafe\u{301} profile"),
            trailing: Some(Line::from("e\u{301}")),
            role: RowRole::Item,
            enabled: true,
        },
    ];
    let mut state = ListState::new(Some("cjk"));
    frame.render_stateful_widget(&List::new(&rows, theme), area, &mut state);
}

fn list_rows() -> [ListRow<'static, &'static str>; 4] {
    [
        ListRow {
            id: "section",
            label: Line::from("Workspace"),
            trailing: Some(Line::from("3 entries")),
            role: RowRole::Separator,
            enabled: true,
        },
        ListRow {
            id: "alpha",
            label: Line::from("Alpha"),
            trailing: Some(Line::from("12 ms")),
            role: RowRole::Item,
            enabled: true,
        },
        ListRow {
            id: "beta",
            label: Line::from("Beta"),
            trailing: Some(Line::from("28 ms")),
            role: RowRole::Item,
            enabled: true,
        },
        ListRow {
            id: "gamma",
            label: Line::from("Gamma"),
            trailing: None,
            role: RowRole::Item,
            enabled: false,
        },
    ]
}

fn text_input_unicode(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let mut state = TextInputState::new("東京🧪 Cafe\u{301}");
    assert!(state.set_cursor_byte("東京".len()));
    frame.render_stateful_widget(
        &TextInput::new("Query", theme).validation(Validation::Valid),
        area,
        &mut state,
    );
}

fn detail_table(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let rows = [
        DetailRow {
            id: "state",
            label: "State",
            value: "Ready",
            href: None,
            capability: DetailCapability::Copy,
            emphasis: true,
            style: None,
        },
        DetailRow {
            id: "link",
            label: "Reference",
            value: "https://example.invalid",
            href: Some("https://example.invalid"),
            capability: DetailCapability::CopyAndLink,
            emphasis: false,
            style: None,
        },
    ];
    let mut state = DetailTableState::default();
    frame.render_stateful_widget(
        &DetailTable::new(&rows, theme).label_width(14).wrap(true),
        area,
        &mut state,
    );
}

fn detail_table_unicode(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let rows = [
        DetailRow {
            id: "region",
            label: "地域",
            value: "東京 🇯🇵",
            href: None,
            capability: DetailCapability::None,
            emphasis: true,
            style: None,
        },
        DetailRow {
            id: "status",
            label: "状態",
            value: "準備完了 ✅ Cafe\u{301}",
            href: None,
            capability: DetailCapability::Copy,
            emphasis: false,
            style: None,
        },
    ];
    let mut state = DetailTableState::default();
    frame.render_stateful_widget(
        &DetailTable::new(&rows, theme).label_width(8).wrap(true),
        area,
        &mut state,
    );
}

fn status_bar(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let left = [StatusSlot {
        id: "state",
        content: " Ready ",
        priority: 1,
        min_width: 0,
        enabled: true,
        style: Style::new().reversed(),
        hover_style: Some(Style::new().bold().reversed()),
    }];
    let right = [StatusSlot {
        id: "position",
        content: " 3/12 ",
        priority: 1,
        min_width: 0,
        enabled: true,
        style: Style::new().dim(),
        hover_style: Some(Style::new().bold()),
    }];
    let mut state = StatusBarState::default();
    frame.render_stateful_widget(
        &StatusBar::new(&left, &right, theme).alpha(1.0),
        area,
        &mut state,
    );
}

fn dialog(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    frame.render_widget(
        Dialog::new(
            "Notice",
            Line::from("The operation completed.").into(),
            theme,
        )
        .style(Style::new())
        .emphasis(termrock::widgets::PanelEmphasis::Focused),
        area,
    );
}

fn choice_dialog(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let mut state = ChoiceDialogState::new(Some("continue"));
    render_choice_dialog(frame, area, &mut state, theme);
}

fn choice_actions() -> [Action<'static, &'static str>; 2] {
    [
        Action {
            id: "continue",
            label: "Continue",
            enabled: true,
            style: None,
        },
        Action {
            id: "cancel",
            label: "Cancel",
            enabled: true,
            style: Some(Style::new().bold()),
        },
    ]
}

fn render_choice_dialog(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &mut ChoiceDialogState<&'static str>,
    theme: &Theme,
) {
    let actions = choice_actions();
    frame.render_stateful_widget(
        &ChoiceDialog::new(
            Dialog::new(
                "Choose",
                Line::from("Continue with this operation?").into(),
                theme,
            )
            .style(Style::new())
            .emphasis(termrock::widgets::PanelEmphasis::Focused),
            &actions,
        )
        .gap(" "),
        area,
        state,
    );
}

fn message_dialog(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let details = [
        DetailRow {
            id: "state",
            label: "State",
            value: "Ready",
            href: None,
            capability: DetailCapability::None,
            emphasis: false,
            style: None,
        },
        DetailRow {
            id: "reference",
            label: "Reference",
            value: "example-42",
            href: None,
            capability: DetailCapability::Copy,
            emphasis: false,
            style: None,
        },
    ];
    let mut state = DetailTableState::default();
    frame.render_stateful_widget(
        &MessageDialog::new(
            Dialog::new(
                "Result",
                Line::from("The operation completed.").into(),
                theme,
            )
            .style(Style::new())
            .emphasis(termrock::widgets::PanelEmphasis::Focused),
            &details,
            theme,
        )
        .label_width(14)
        .wrap(true),
        area,
        &mut state,
    );
}

fn diff(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let theme = if *theme == Theme::tailrocks_phosphor() {
        theme
            .clone()
            .with_role(Role::DiffAdded, Style::new().bold())
            .with_role(Role::DiffRemoved, Style::new().dim())
    } else {
        theme.clone()
    };
    let lines = [
        DiffLine {
            text: " context",
            kind: DiffKind::Context,
        },
        DiffLine {
            text: "-before",
            kind: DiffKind::Removed,
        },
        DiffLine {
            text: "+after",
            kind: DiffKind::Added,
        },
    ];
    frame.render_stateful_widget(
        &DiffView::new(&lines, &theme),
        area,
        &mut DiffState::default(),
    );
}

fn toast(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    frame.render_widget(
        Toast::new(theme, "Updated", Severity::Success).anchor(Anchor::TopRight),
        area,
    );
}

fn backdrop(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let style = if *theme == Theme::tailrocks_phosphor() {
        Style::new().dim()
    } else {
        theme.style(Role::Backdrop)
    };
    frame.render_widget(Backdrop::new().symbol('░').style(style), area);
}

fn viewport(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let lines = [
        Line::from("alpha: short"),
        Line::from("beta: a deliberately wide borrowed row for horizontal scrolling"),
        Line::from("gamma: 🧪 Unicode"),
        Line::from("delta: fourth row"),
        Line::from("epsilon: fifth row"),
        Line::from("zeta: sixth row"),
    ];
    let border_style = theme.style(Role::BorderFocused);
    let theme = theme.clone().with_role(Role::Border, border_style);
    let mut state = DialogScroll::default();
    frame.render_stateful_widget(
        &Viewport::new(&lines, &theme)
            .title("Viewport")
            .content_style(Style::new()),
        area,
        &mut state,
    );
}
