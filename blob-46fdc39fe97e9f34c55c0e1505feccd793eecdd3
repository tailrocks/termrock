// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Product-neutral stories rendered through TermRock's public widget API.

use ratatui::{Frame, layout::Rect, style::Style, text::Line, widgets::Paragraph};
use termrock::{
    Theme,
    scroll::DialogScroll,
    style::Role,
    widgets::{
        Action, ActionBar, ActionBarState, Anchor, Backdrop, ChoiceDialog, DetailCapability,
        DetailRow, DetailTable, DetailTableState, Dialog, DialogAction, DiffKind, DiffLine,
        DiffState, DiffView, Form, FormField, FormSection, FormState, Hint, HintBar, List, ListRow,
        ListState, MessageDialog, Panel, PanelEmphasis, RowRole, Severity, SplitDirection,
        SplitPane, SplitPaneState, SplitRatio, StatusBar, StatusSlot, Tab, Tabs, TabsState,
        TextInput, TextInputState, Toast, Tree, TreeNode, TreeNodeStatus, TreeState, Validation,
        Viewport,
    },
};

use crate::interactors::{
    FormInteractor, SplitPaneInteractor, StaticStory, StoryInteraction, TreeInteractor,
};

type RenderFn = fn(&mut Frame<'_>, Rect);
type InteractorFactory = fn(RenderFn) -> Box<dyn StoryInteraction>;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Story {
    pub id: &'static str,
    pub title: &'static str,
    pub component: &'static str,
    pub description: &'static str,
    pub width: u16,
    pub height: u16,
    render: RenderFn,
    interactor: InteractorFactory,
}

impl Story {
    const fn new(
        id: &'static str,
        title: &'static str,
        component: &'static str,
        description: &'static str,
        width: u16,
        height: u16,
        render: RenderFn,
    ) -> Self {
        Self {
            id,
            title,
            component,
            description,
            width,
            height,
            render,
            interactor: static_interactor,
        }
    }
    const fn with_interactor(mut self, interactor: InteractorFactory) -> Self {
        self.interactor = interactor;
        self
    }
    pub(crate) fn render(self, frame: &mut Frame<'_>, area: Rect) {
        (self.render)(frame, area);
    }
    pub(crate) fn make_interactor(&self) -> Box<dyn StoryInteraction> {
        (self.interactor)(self.render)
    }
}

fn static_interactor(render: RenderFn) -> Box<dyn StoryInteraction> {
    Box::new(StaticStory { render_fn: render })
}

fn tree_interactor(_render: RenderFn) -> Box<dyn StoryInteraction> {
    Box::new(TreeInteractor::new())
}

fn form_interactor(_render: RenderFn) -> Box<dyn StoryInteraction> {
    Box::new(FormInteractor::new())
}

fn split_pane_interactor(_render: RenderFn) -> Box<dyn StoryInteraction> {
    Box::new(SplitPaneInteractor::new())
}

pub(crate) fn stories() -> Vec<Story> {
    vec![
        Story::new(
            "panel/focused",
            "Focused panel",
            "Panel",
            "A semantically focused bordered panel.",
            48,
            7,
            panel,
        ),
        Story::new(
            "action-bar/basic",
            "Action bar",
            "ActionBar",
            "Stable-ID caller-defined actions.",
            48,
            2,
            action_bar,
        ),
        Story::new(
            "tabs/status",
            "Tabs",
            "Tabs",
            "Tabs with per-item glyph and state.",
            52,
            2,
            tabs,
        ),
        Story::new(
            "hint-bar/wrapped",
            "Hint bar",
            "HintBar",
            "Prioritized caller-defined hints.",
            42,
            2,
            hint_bar,
        ),
        Story::new(
            "list/selection",
            "List",
            "List",
            "Borrowed rows selected by stable ID.",
            42,
            6,
            list,
        ),
        Story::new(
            "tree/navigation",
            "Tree navigation",
            "Tree",
            "Stable-ID hierarchy with disclosure and status states.",
            42,
            7,
            tree,
        )
        .with_interactor(tree_interactor),
        Story::new(
            "form/responsive",
            "Responsive form",
            "Form",
            "Sections, validation, disabled state, and stable-ID focus.",
            68,
            12,
            form,
        )
        .with_interactor(form_interactor),
        Story::new(
            "split-pane/horizontal",
            "Horizontal split pane",
            "SplitPane",
            "Bounded resizable panes with focus, drag, and collapse.",
            68,
            10,
            split_pane,
        )
        .with_interactor(split_pane_interactor),
        Story::new(
            "text-input/filter",
            "Filter composition",
            "TextInput",
            "Text input composed as a caller-owned filter.",
            42,
            1,
            text_input,
        ),
        Story::new(
            "detail-table/basic",
            "Detail table",
            "DetailTable",
            "Neutral label/value rows with capabilities.",
            54,
            5,
            detail_table,
        ),
        Story::new(
            "status-bar/basic",
            "Status bar",
            "StatusBar",
            "Caller-owned left and right status slots.",
            60,
            1,
            status_bar,
        ),
        Story::new(
            "dialog/message",
            "Message dialog",
            "Dialog",
            "Responsive neutral dialog shell.",
            48,
            7,
            dialog,
        ),
        Story::new(
            "choice-dialog/basic",
            "Choice dialog",
            "ChoiceDialog",
            "Caller-owned stable actions in a neutral dialog shell.",
            48,
            7,
            choice_dialog,
        ),
        Story::new(
            "message-dialog/details",
            "Detailed message dialog",
            "MessageDialog",
            "Caller-owned detail rows composed into a neutral message shell.",
            52,
            8,
            message_dialog,
        ),
        Story::new(
            "diff/basic",
            "Diff view",
            "DiffView",
            "Borrowed projected diff lines.",
            54,
            6,
            diff,
        ),
        Story::new(
            "toast/success",
            "Toast",
            "Toast",
            "Caller-owned transient message.",
            34,
            4,
            toast,
        ),
        Story::new(
            "backdrop/basic",
            "Backdrop",
            "Backdrop",
            "Neutral modal backdrop policy.",
            34,
            4,
            backdrop,
        ),
        Story::new(
            "viewport/both-axes",
            "Scrollable viewport",
            "Viewport",
            "Borrowed lines with bounded horizontal and vertical scroll state.",
            44,
            7,
            viewport,
        ),
    ]
}

fn panel(frame: &mut Frame<'_>, area: Rect) {
    let theme = Theme::default();
    frame.render_widget(
        &Panel {
            title: Some("Summary"),
            emphasis: PanelEmphasis::Focused,
            style: None,
            theme: &theme,
        },
        area,
    );
    if area.width > 2 && area.height > 2 {
        frame.render_widget(
            Paragraph::new("State   Ready\nMode    Interactive"),
            Rect::new(area.x + 1, area.y + 1, area.width - 2, area.height - 2),
        );
    }
}

fn action_bar(frame: &mut Frame<'_>, area: Rect) {
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
    frame.render_stateful_widget(
        &ActionBar {
            actions: &actions,
            gap: "  ",
        },
        area,
        &mut state,
    );
}

pub(crate) fn tree_nodes() -> Vec<TreeNode<'static, &'static str>> {
    vec![
        TreeNode {
            id: "workspace",
            label: Line::from("Workspace"),
            depth: 0,
            branch: true,
            expanded: true,
            enabled: true,
            status: TreeNodeStatus::Ready,
        },
        TreeNode {
            id: "documents",
            label: Line::from("Documents"),
            depth: 1,
            branch: true,
            expanded: false,
            enabled: true,
            status: TreeNodeStatus::Ready,
        },
        TreeNode {
            id: "loading",
            label: Line::from("Remote items"),
            depth: 1,
            branch: true,
            expanded: false,
            enabled: false,
            status: TreeNodeStatus::Loading,
        },
        TreeNode {
            id: "notes",
            label: Line::from("Wide 🧪 notes"),
            depth: 1,
            branch: false,
            expanded: false,
            enabled: true,
            status: TreeNodeStatus::Ready,
        },
    ]
}

pub(crate) fn form_fields() -> Vec<FormField<'static, &'static str>> {
    vec![
        FormField {
            id: "name",
            label: Line::from("Name"),
            value: Line::from("Example profile"),
            help: Some(Line::from("A recognizable display name")),
            error: None,
            required: true,
            enabled: true,
        },
        FormField {
            id: "endpoint",
            label: Line::from("Endpoint"),
            value: Line::from("localhost"),
            help: None,
            error: Some(Line::from("Enter a reachable address")),
            required: true,
            enabled: true,
        },
        FormField {
            id: "mode",
            label: Line::from("Managed mode"),
            value: Line::from("Unavailable"),
            help: None,
            error: None,
            required: false,
            enabled: false,
        },
    ]
}

fn form(frame: &mut Frame<'_>, area: Rect) {
    let fields = form_fields();
    let sections = [FormSection {
        title: Line::from("General"),
        fields: &fields,
    }];
    let theme = Theme::default();
    let mut state = FormState::new(Some("name"));
    frame.render_stateful_widget(&Form::new(&sections, &theme), area, &mut state);
}

pub(crate) fn render_split_pane(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &mut SplitPaneState,
    theme: &Theme,
) {
    let split = SplitPane::new(SplitDirection::Horizontal, 12, 16, theme);
    let layout = split.layout(area, state);
    if !layout.first.is_empty() {
        frame.render_widget(
            Paragraph::new("First pane\nCaller-owned content"),
            layout.first,
        );
    }
    if !layout.second.is_empty() {
        frame.render_widget(
            Paragraph::new("Second pane\nDrag the divider"),
            layout.second,
        );
    }
    frame.render_stateful_widget(&split, area, state);
}

fn split_pane(frame: &mut Frame<'_>, area: Rect) {
    let theme = Theme::default();
    let mut state = SplitPaneState::new(SplitRatio::from_percent(38));
    state.set_focused(true);
    render_split_pane(frame, area, &mut state, &theme);
}

fn tree(frame: &mut Frame<'_>, area: Rect) {
    let nodes = tree_nodes();
    let mut state = TreeState::new(Some("workspace"));
    let theme = Theme::default();
    frame.render_stateful_widget(
        &Tree {
            nodes: &nodes,
            theme: &theme,
        },
        area,
        &mut state,
    );
}

fn tabs(frame: &mut Frame<'_>, area: Rect) {
    let items = [
        Tab {
            id: "overview",
            label: "Overview",
            glyph: Some("●"),
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
    frame.render_stateful_widget(
        &Tabs {
            tabs: &items,
            gap: 1,
        },
        area,
        &mut state,
    );
}

fn hint_bar(frame: &mut Frame<'_>, area: Rect) {
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
    frame.render_widget(
        &HintBar {
            hints: &hints,
            separator: "  ",
        },
        area,
    );
}

fn list(frame: &mut Frame<'_>, area: Rect) {
    let rows = [
        ListRow {
            id: "alpha",
            label: Line::from("Alpha"),
            role: RowRole::Item,
            enabled: true,
        },
        ListRow {
            id: "beta",
            label: Line::from("Beta"),
            role: RowRole::Item,
            enabled: true,
        },
        ListRow {
            id: "gamma",
            label: Line::from("Gamma"),
            role: RowRole::Item,
            enabled: false,
        },
    ];
    let mut state = ListState {
        selected: Some("beta"),
        ..ListState::default()
    };
    frame.render_stateful_widget(&List { rows: &rows }, area, &mut state);
}

fn text_input(frame: &mut Frame<'_>, area: Rect) {
    let mut state = TextInputState::new("search");
    frame.render_stateful_widget(
        &TextInput {
            label: "Filter",
            placeholder: "Type to filter",
            validation: Validation::Valid,
            style: Style::new(),
        },
        area,
        &mut state,
    );
}

fn detail_table(frame: &mut Frame<'_>, area: Rect) {
    let rows = [
        DetailRow {
            id: "state",
            label: "State",
            value: "Ready",
            capability: DetailCapability::None,
        },
        DetailRow {
            id: "link",
            label: "Reference",
            value: "https://example.invalid",
            capability: DetailCapability::Link,
        },
    ];
    let mut state = DetailTableState::default();
    frame.render_stateful_widget(
        &DetailTable {
            rows: &rows,
            label_width: 14,
        },
        area,
        &mut state,
    );
}

fn status_bar(frame: &mut Frame<'_>, area: Rect) {
    let left = [StatusSlot {
        id: "state",
        content: " Ready ",
        priority: 1,
        min_width: 0,
        enabled: true,
        style: Style::new().reversed(),
    }];
    let right = [StatusSlot {
        id: "position",
        content: " 3/12 ",
        priority: 1,
        min_width: 0,
        enabled: true,
        style: Style::new().dim(),
    }];
    frame.render_widget(
        &StatusBar {
            left: &left,
            right: &right,
        },
        area,
    );
}

fn dialog(frame: &mut Frame<'_>, area: Rect) {
    frame.render_widget(
        &Dialog {
            title: "Notice",
            body: Line::from("The operation completed."),
            style: Style::new(),
        },
        area,
    );
}

fn choice_dialog(frame: &mut Frame<'_>, area: Rect) {
    let actions = [
        DialogAction {
            action: Action {
                id: "continue",
                label: "Continue",
                enabled: true,
                style: None,
            },
            destructive: false,
        },
        DialogAction {
            action: Action {
                id: "cancel",
                label: "Cancel",
                enabled: true,
                style: None,
            },
            destructive: true,
        },
    ];
    frame.render_widget(
        &ChoiceDialog {
            dialog: Dialog {
                title: "Choose",
                body: Line::from("Continue with this operation?"),
                style: Style::new(),
            },
            actions: &actions,
        },
        area,
    );
}

fn message_dialog(frame: &mut Frame<'_>, area: Rect) {
    let details = [
        DetailRow {
            id: "state",
            label: "State",
            value: "Ready",
            capability: DetailCapability::None,
        },
        DetailRow {
            id: "reference",
            label: "Reference",
            value: "example-42",
            capability: DetailCapability::Copy,
        },
    ];
    frame.render_widget(
        &MessageDialog {
            dialog: Dialog {
                title: "Result",
                body: Line::from("The operation completed."),
                style: Style::new(),
            },
            details: &details,
        },
        area,
    );
}

fn diff(frame: &mut Frame<'_>, area: Rect) {
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
        &DiffView {
            lines: &lines,
            added_style: Style::new().bold(),
            removed_style: Style::new().dim(),
        },
        area,
        &mut DiffState::default(),
    );
}

fn toast(frame: &mut Frame<'_>, area: Rect) {
    frame.render_widget(
        &Toast {
            message: "Updated",
            severity: Severity::Success,
            anchor: Anchor::TopRight,
            style: Style::new(),
        },
        area,
    );
}
fn backdrop(frame: &mut Frame<'_>, area: Rect) {
    frame.render_widget(
        &Backdrop {
            symbol: '░',
            style: Style::new().dim(),
        },
        area,
    );
}

fn viewport(frame: &mut Frame<'_>, area: Rect) {
    let lines = [
        Line::from("alpha: short"),
        Line::from("beta: a deliberately wide borrowed row for horizontal scrolling"),
        Line::from("gamma: 🧪 Unicode"),
        Line::from("delta: fourth row"),
        Line::from("epsilon: fifth row"),
        Line::from("zeta: sixth row"),
    ];
    let theme = Theme::default();
    let mut state = DialogScroll::default();
    frame.render_stateful_widget(
        &Viewport {
            lines: &lines,
            title: Some("Viewport"),
            content_style: Style::new(),
            border_style: theme.style(Role::BorderFocused),
            title_style: theme.style(Role::Text),
            scroll_track_style: theme.style(Role::ScrollTrack),
            scroll_thumb_style: theme.style(Role::ScrollThumb),
        },
        area,
        &mut state,
    );
}
