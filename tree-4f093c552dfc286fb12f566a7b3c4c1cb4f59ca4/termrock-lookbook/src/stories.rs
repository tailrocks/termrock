// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Product-neutral stories rendered through TermRock's public widget API.

use ratatui::{
    Frame,
    layout::Rect,
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
        DiffState, DiffView, Form, FormField, FormSection, FormState, Hint, HintBar, List, ListRow,
        ListState, MessageDialog, Panel, PanelEmphasis, RowRole, Severity, SplitDirection,
        SplitPane, SplitPaneState, SplitRatio, StatusBar, StatusBarState, StatusSlot, Tab, Tabs,
        TabsState, TextInput, TextInputState, Toast, Tree, TreeNode, TreeNodeStatus, TreeState,
        Validation, Viewport,
    },
};

use crate::interactors::{
    ChoiceDialogInteractor, FormInteractor, ListInteractor, SplitPaneInteractor, StaticStory,
    StoryInteraction, TextInputInteractor, TreeInteractor,
};

type RenderFn = fn(&mut Frame<'_>, Rect, &Theme);
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
    pub(crate) fn render(self, frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
        (self.render)(frame, area, theme);
    }
    pub(crate) fn make_interactor(&self) -> Box<dyn StoryInteraction> {
        (self.interactor)(self.render)
    }
}

fn static_interactor(render: RenderFn) -> Box<dyn StoryInteraction> {
    Box::new(StaticStory {
        render_fn: render,
        theme: Theme::default(),
    })
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

fn choice_dialog_interactor(_render: RenderFn) -> Box<dyn StoryInteraction> {
    Box::new(ChoiceDialogInteractor::new())
}

fn list_interactor(_render: RenderFn) -> Box<dyn StoryInteraction> {
    Box::new(ListInteractor::new())
}

fn text_input_interactor(_render: RenderFn) -> Box<dyn StoryInteraction> {
    Box::new(TextInputInteractor::new())
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
            "Tabs with styled per-item glyphs and state.",
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
        )
        .with_interactor(list_interactor),
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
        )
        .with_interactor(text_input_interactor),
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
        )
        .with_interactor(choice_dialog_interactor),
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

fn panel(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    frame.render_widget(
        &Panel::new(theme)
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
    frame.render_stateful_widget(
        &ActionBar {
            actions: &actions,
            gap: "  ",
            theme,
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

fn form(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let fields = form_fields();
    let sections = [FormSection {
        title: Line::from("General"),
        fields: &fields,
    }];
    let mut state = FormState::new(Some("name"));
    frame.render_stateful_widget(&Form::new(&sections, theme), area, &mut state);
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

fn split_pane(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let mut state = SplitPaneState::new(SplitRatio::from_percent(38));
    state.set_focused(true);
    render_split_pane(frame, area, &mut state, theme);
}

fn tree(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let nodes = tree_nodes();
    let mut state = TreeState::new(Some("workspace"));
    frame.render_stateful_widget(
        &Tree {
            nodes: &nodes,
            theme,
        },
        area,
        &mut state,
    );
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
    frame.render_stateful_widget(
        &Tabs {
            tabs: &items,
            gap: 1,
            theme,
        },
        area,
        &mut state,
    );
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
    frame.render_widget(
        &HintBar {
            hints: &hints,
            separator: "  ",
            theme: &theme,
        },
        area,
    );
}

fn list(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let rows = list_rows();
    let mut state = ListState::new(Some("beta"));
    frame.render_stateful_widget(&List { rows: &rows, theme }, area, &mut state);
}

pub(crate) fn list_rows() -> [ListRow<'static, &'static str>; 4] {
    [
        ListRow {
            id: "section",
            label: Line::from("Workspace"),
            role: RowRole::Separator,
            enabled: true,
        },
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
    ]
}

fn text_input(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let mut state = TextInputState::new("search");
    frame.render_stateful_widget(
        &TextInput {
            label: "Filter",
            placeholder: "Type to filter",
            validation: Validation::Valid,
            theme,
        },
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
        &DetailTable {
            rows: &rows,
            label_width: 14,
            wrap: true,
            theme,
        },
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
        &StatusBar {
            left: &left,
            right: &right,
            theme,
            alpha: 1.0,
        },
        area,
        &mut state,
    );
}

fn dialog(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    frame.render_widget(
        &Dialog {
            title: "Notice",
            body: Line::from("The operation completed.").into(),
            style: Style::new(),
            theme,
            emphasis: termrock::widgets::PanelEmphasis::Focused,
        },
        area,
    );
}

fn choice_dialog(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let mut state = ChoiceDialogState::new(Some("continue"));
    render_choice_dialog(frame, area, &mut state, theme);
}

pub(crate) fn choice_actions() -> [Action<'static, &'static str>; 2] {
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

pub(crate) fn render_choice_dialog(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &mut ChoiceDialogState<&'static str>,
    theme: &Theme,
) {
    let actions = choice_actions();
    frame.render_stateful_widget(
        &ChoiceDialog {
            dialog: Dialog {
                title: "Choose",
                body: Line::from("Continue with this operation?").into(),
                style: Style::new(),
                theme,
                emphasis: termrock::widgets::PanelEmphasis::Focused,
            },
            actions: &actions,
            gap: " ",
        },
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
        &MessageDialog {
            dialog: Dialog {
                title: "Result",
                body: Line::from("The operation completed.").into(),
                style: Style::new(),
                theme,
                emphasis: termrock::widgets::PanelEmphasis::Focused,
            },
            details: &details,
            label_width: 14,
            wrap: true,
            theme,
        },
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
        &DiffView {
            lines: &lines,
            theme: &theme,
        },
        area,
        &mut DiffState::default(),
    );
}

fn toast(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    frame.render_widget(
        &Toast::new(theme, "Updated", Severity::Success).anchor(Anchor::TopRight),
        area,
    );
}
fn backdrop(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let style = if *theme == Theme::tailrocks_phosphor() {
        Style::new().dim()
    } else {
        theme.style(Role::Backdrop)
    };
    frame.render_widget(
        &Backdrop {
            symbol: '░', style
        },
        area,
    );
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
        &Viewport {
            lines: &lines,
            title: Some("Viewport"),
            theme: &theme,
            content_style: Some(Style::new()),
        },
        area,
        &mut state,
    );
}
