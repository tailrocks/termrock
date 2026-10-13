// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Product-neutral stories rendered through TermRock's public widget API.

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
        DiffState, DiffView, Form, FormField, FormSection, FormState, Hint, HintBar, List, ListRow,
        ListState, LogPane, LogPaneState, MessageDialog, Panel, PanelEmphasis, Picker, PickerState,
        Progress, ProgressKind, RowRole, Severity, SplitDirection, SplitPane, SplitPaneState,
        SplitRatio, StatusBar, StatusBarState, StatusSlot, Tab, Tabs, TabsState, TextInput,
        TextInputState, Toast, Tree, TreeNode, TreeNodeStatus, TreeState, Validation, Viewport,
    },
};

use crate::interactors::{
    ChoiceDialogInteractor, FormInteractor, ListInteractor, LogPaneInteractor, PickerInteractor,
    SplitPaneInteractor, StaticStory, StoryInteraction, ToastInteractor, TreeInteractor,
};

type RenderFn = fn(&mut Frame<'_>, Rect, &Theme);
type InteractorFactory = fn(RenderFn) -> Box<dyn StoryInteraction>;

pub(crate) const SPLIT_PANE_MIN: u16 = 12;
pub(crate) const SPLIT_PANE_MAX: u16 = 16;

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
    pub(crate) const fn new(
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

fn picker_interactor(_render: RenderFn) -> Box<dyn StoryInteraction> {
    Box::new(PickerInteractor::new())
}

fn log_pane_interactor(_render: RenderFn) -> Box<dyn StoryInteraction> {
    Box::new(LogPaneInteractor::new())
}

fn toast_interactor(_render: RenderFn) -> Box<dyn StoryInteraction> {
    Box::new(ToastInteractor::new())
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
            "Stable-ID rows with checks and aligned metadata.",
            42,
            6,
            list,
        )
        .with_interactor(list_interactor),
        Story::new(
            "tree/navigation",
            "Tree navigation",
            "Tree",
            "Stable-ID hierarchy with checks, metadata, disclosure, and status.",
            42,
            7,
            tree,
        )
        .with_interactor(tree_interactor),
        Story::new(
            "progress/determinate",
            "Progress",
            "Progress",
            "Caller-ticked determinate and indeterminate progress.",
            42,
            2,
            progress,
        ),
        Story::new(
            "progress/narrow",
            "Narrow progress",
            "Progress",
            "Percentage elision and custom ASCII frames in fourteen columns.",
            14,
            2,
            progress_narrow,
        ),
        Story::new(
            "progress/unicode",
            "Unicode progress labels",
            "Progress",
            "Wide CJK and emoji labels clipped on grapheme boundaries.",
            34,
            2,
            progress_unicode,
        ),
        Story::new(
            "log-pane/follow",
            "Following log pane",
            "LogPane",
            "Tail-following output; scroll up to freeze and End to resume.",
            52,
            8,
            log_pane,
        )
        .with_interactor(log_pane_interactor),
        Story::new(
            "log-pane/scrolled",
            "Frozen log scrollback",
            "LogPane",
            "Scrolled-back distance plus wide CJK and emoji output.",
            52,
            8,
            log_pane_scrolled,
        ),
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
            "picker/basic",
            "Filterable picker",
            "Picker",
            "Caller-filtered rows with stable selection and semantic activation.",
            42,
            7,
            picker_basic,
        )
        .with_interactor(picker_interactor),
        Story::new(
            "picker/empty",
            "Empty picker",
            "Picker",
            "Product-neutral empty-result cue.",
            30,
            4,
            picker_empty,
        ),
        Story::new(
            "picker/narrow-unicode",
            "Narrow Unicode picker",
            "Picker",
            "Wide and combining labels clipped in a narrow result list.",
            22,
            5,
            picker_narrow_unicode,
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
        )
        .with_interactor(toast_interactor),
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
        Story::new(
            "list/narrow",
            "Narrow list",
            "List",
            "Narrow-terminal clipping and metadata priority.",
            14,
            6,
            list,
        ),
        Story::new(
            "tabs/narrow",
            "Narrow tabs",
            "Tabs",
            "Narrow-terminal tab clipping and selection cues.",
            16,
            2,
            tabs,
        ),
        Story::new(
            "form/narrow",
            "Narrow form",
            "Form",
            "Responsive single-column form at narrow width.",
            24,
            12,
            form,
        ),
        Story::new(
            "status-bar/narrow",
            "Narrow status bar",
            "StatusBar",
            "Priority-based slot elision at narrow width.",
            20,
            1,
            status_bar,
        ),
        Story::new(
            "dialog/narrow",
            "Narrow dialog",
            "Dialog",
            "Responsive dialog shell at narrow width.",
            20,
            7,
            dialog,
        ),
        Story::new(
            "toast/narrow",
            "Narrow toast",
            "Toast",
            "Bounded transient message at narrow width.",
            16,
            4,
            toast,
        ),
        Story::new(
            "list/unicode",
            "Unicode list",
            "List",
            "CJK, emoji, and combining-mark row geometry.",
            28,
            5,
            list_unicode,
        ),
        Story::new(
            "text-input/unicode",
            "Unicode text input",
            "TextInput",
            "Wide and combining graphemes with a mid-string cursor.",
            28,
            1,
            text_input_unicode,
        ),
        Story::new(
            "detail-table/unicode",
            "Unicode detail table",
            "DetailTable",
            "CJK labels and emoji values under wrapping.",
            30,
            6,
            detail_table_unicode,
        ),
    ]
}

/// Interactive-gallery entries, including compile-proven design prototypes.
/// Catalog generation deliberately uses [`stories`] instead.
pub(crate) fn gallery_stories() -> Vec<Story> {
    let mut entries = stories();
    entries.push(crate::table::story());
    entries
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

fn log_pane(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let mut state = LogPaneState::new().with_max_lines(200);
    for line in [
        "[12:04:01] resolving workspace",
        "[12:04:02] compiling termrock",
        "[12:04:03] running 205 tests",
        "[12:04:04] result: ok ✓",
    ] {
        state.append(line);
    }
    frame.render_stateful_widget(&LogPane::new(theme).title("Build log"), area, &mut state);
}

fn log_pane_scrolled(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let mut state = LogPaneState::new();
    for line in [
        "[12:04:01] resolving workspace",
        "[12:04:02] 東京 worker ready 🪨",
        "[12:04:03] compiling termrock",
        "[12:04:04] running tests",
        "[12:04:05] rendering previews",
        "[12:04:06] result: ok ✓",
        "[12:04:07] waiting for changes",
    ] {
        state.append(line);
    }
    let pane = LogPane::new(theme).title("Frozen build log");
    state.scroll_to_oldest();
    frame.render_stateful_widget(&pane, area, &mut state);
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

pub(crate) fn tree_nodes() -> Vec<TreeNode<'static, &'static str>> {
    vec![
        TreeNode {
            id: "workspace",
            label: Line::from("Workspace"),
            trailing: Some(Line::from("4 items")),
            depth: 0,
            branch: true,
            expanded: true,
            enabled: true,
            status: TreeNodeStatus::Ready,
        },
        TreeNode {
            id: "documents",
            label: Line::from("Documents"),
            trailing: Some(Line::from("2 items")),
            depth: 1,
            branch: true,
            expanded: false,
            enabled: true,
            status: TreeNodeStatus::Ready,
        },
        TreeNode {
            id: "loading",
            label: Line::from("Remote items"),
            trailing: None,
            depth: 1,
            branch: true,
            expanded: false,
            enabled: false,
            status: TreeNodeStatus::Loading,
        },
        TreeNode {
            id: "notes",
            label: Line::from("Wide 🧪 notes"),
            trailing: Some(Line::from("12 KiB")),
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
        FormField::new("name", Line::from("Name"), Line::from("Example profile"))
            .help(Line::from("A recognizable display name"))
            .required(true),
        FormField::new("endpoint", Line::from("Endpoint"), Line::from("localhost"))
            .error(Line::from("Enter a reachable address"))
            .required(true),
        FormField::new(
            "mode",
            Line::from("Managed mode"),
            Line::from("Unavailable"),
        )
        .enabled(false),
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
    let split = SplitPane::new(
        SplitDirection::Horizontal,
        SPLIT_PANE_MIN,
        SPLIT_PANE_MAX,
        theme,
    );
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
    state.enable_multi_select();
    state.selection_mut().unwrap().toggle(&"notes");
    frame.render_stateful_widget(&Tree::new(&nodes, theme), area, &mut state);
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

pub(crate) fn list_rows() -> [ListRow<'static, &'static str>; 4] {
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

fn picker_basic(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let rows = picker_rows("");
    let mut state = PickerState::new(Some("alpha"));
    frame.render_stateful_widget(&Picker::new(&rows, theme), area, &mut state);
}

fn picker_empty(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let mut state = PickerState::<&str>::new(None);
    frame.render_stateful_widget(&Picker::new(&[], theme), area, &mut state);
}

fn picker_narrow_unicode(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let rows = [
        ListRow {
            id: "tokyo",
            label: Line::from("東京デプロイ 🧪"),
            trailing: Some(Line::from("操作")),
            role: RowRole::Item,
            enabled: true,
        },
        ListRow {
            id: "cafe",
            label: Line::from("Cafe\u{301} logs"),
            trailing: Some(Line::from("表示")),
            role: RowRole::Item,
            enabled: true,
        },
    ];
    let mut state = PickerState::new(Some("tokyo"));
    let _ = state.query_mut().insert_str("東");
    frame.render_stateful_widget(&Picker::new(&rows, theme), area, &mut state);
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

pub(crate) fn picker_rows(query: &str) -> Vec<ListRow<'static, &'static str>> {
    let query = query.to_ascii_lowercase();
    [
        ("alpha", "Alpha project", "workspace"),
        ("beta", "Beta release", "command"),
        ("gamma", "Gamma logs", "view"),
        ("delta", "Delta settings", "command"),
    ]
    .into_iter()
    .filter(|(_, label, _)| label.to_ascii_lowercase().contains(&query))
    .map(|(id, label, kind)| ListRow {
        id,
        label: Line::from(label),
        trailing: Some(Line::from(kind)),
        role: RowRole::Item,
        enabled: true,
    })
    .collect()
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
