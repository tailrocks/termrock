export interface ComponentDoc {
  readonly description: string
  readonly primaryStory: string
  readonly usage: string
}

export const componentDocs = {
  ActionBar: {
    description: 'A horizontal group of stable, caller-owned actions with painted hit regions.',
    primaryStory: 'action-bar/basic',
    usage: `use termrock::{Theme, widgets::{Action, ActionBar, ActionBarState}};

let theme = Theme::default();
let actions = [Action { id: "save", label: "Save", enabled: true, style: None }];
let bar = ActionBar::new(&actions, &theme);
let mut state = ActionBarState::default();
state.focused = Some("save");`,
  },
  Backdrop: {
    description: 'A configurable themed fill painted behind modal content.',
    primaryStory: 'backdrop/basic',
    usage: `use ratatui_core::{buffer::Buffer, layout::Rect, style::{Color, Style}, widgets::Widget};
use termrock::widgets::Backdrop;

let backdrop = Backdrop::new()
    .symbol('░')
    .style(Style::new().bg(Color::Black));
let area = Rect::new(0, 0, 80, 24);
backdrop.render(area, &mut Buffer::empty(area));`,
  },
  ChoiceDialog: {
    description: 'A modal choice prompt with stable action identities and canonical traversal.',
    primaryStory: 'choice-dialog/basic',
    usage: `use ratatui_core::text::Text;
use termrock::{Theme, input::{KeyCode, KeyEvent, KeyModifiers}, widgets::{Action, ChoiceDialog, ChoiceDialogState, Dialog}};

let theme = Theme::default();
let actions = [Action { id: "accept", label: "Accept", enabled: true, style: None }];
let dialog = ChoiceDialog::new(Dialog::new("Confirm", Text::from("Continue?"), &theme), &actions);
let mut state = ChoiceDialogState::new(Some("accept"));
let outcome = state.handle_key(&actions, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));`,
  },
  DetailTable: {
    description: 'A selectable key/value table with stable rows and typed activation capabilities.',
    primaryStory: 'detail-table/basic',
    usage: `use termrock::{Theme, widgets::{DetailCapability, DetailRow, DetailTable, DetailTableState}};

let theme = Theme::default();
let rows = [DetailRow { id: "url", label: "URL", value: "https://example.com", href: Some("https://example.com"), capability: DetailCapability::Link, emphasis: false, style: None }];
let table = DetailTable::new(&rows, &theme).wrap(true);
let mut state = DetailTableState::<&str>::default();
let outcome = state.select_next(&rows);`,
  },
  Dialog: {
    description: 'A framed modal surface with semantic chrome and caller-owned content.',
    primaryStory: 'dialog/message',
    usage: `use ratatui_core::{buffer::Buffer, layout::Rect, text::Text, widgets::Widget};
use termrock::{Theme, widgets::{Dialog, PanelEmphasis}};

let theme = Theme::default();
let dialog = Dialog::new("Notice", Text::from("Saved"), &theme)
    .emphasis(PanelEmphasis::Focused);
let area = Rect::new(0, 0, 40, 8);
dialog.render(area, &mut Buffer::empty(area));`,
  },
  DiffView: {
    description: 'A vertically scrollable, syntax-neutral presentation of projected diff lines.',
    primaryStory: 'diff/basic',
    usage: `use termrock::{Theme, widgets::{DiffKind, DiffLine, DiffState, DiffView}};

let theme = Theme::default();
let lines = [DiffLine { text: "+added", kind: DiffKind::Added }];
let diff = DiffView::new(&lines, &theme);
let mut state = DiffState::default();
state.offset = 1;
let _visible_offset = state.offset;`,
  },
  Form: {
    description: 'A responsive form layout with stable focus, validation, and hit geometry.',
    primaryStory: 'form/responsive',
    usage: `use ratatui_core::text::Line;
use termrock::{Theme, input::{KeyCode, KeyEvent, KeyModifiers}, widgets::{Form, FormField, FormSection, FormState}};

let theme = Theme::default();
let fields = [FormField::new("name", Line::from("Name"), Line::from("Ada")).required(true)];
let sections = [FormSection { title: Line::from("Profile"), fields: &fields }];
let form = Form::new(&sections, &theme);
let mut state = FormState::new(Some("name"));
let outcome = state.handle_key(&sections, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));`,
  },
  HintBar: {
    description: 'A wrapping row of prioritized keyboard hints with semantic styling.',
    primaryStory: 'hint-bar/wrapped',
    usage: `use ratatui_core::{buffer::Buffer, layout::Rect, widgets::Widget};
use termrock::{Theme, widgets::{Hint, HintBar}};

let theme = Theme::default();
let hints = [Hint { chord: "Enter", label: "open", priority: 0, visible: true }];
let bar = HintBar::new(&hints, &theme).separator(" · ");
let area = Rect::new(0, 0, 40, 2);
bar.render(area, &mut Buffer::empty(area));`,
  },
  List: {
    description: 'A selectable, scrollable list over borrowed rows and stable identities.',
    primaryStory: 'list/selection',
    usage: `use ratatui_core::text::Line;
use termrock::{Theme, widgets::{List, ListRow, ListState, RowRole}};

let theme = Theme::default();
let rows = [ListRow { id: "alpha", label: Line::from("Alpha"), trailing: None, role: RowRole::Item, enabled: true }];
let list = List::new(&rows, &theme);
let mut state = ListState::new(Some("alpha"));
let outcome = state.select_next(&rows);`,
  },
  LogPane: {
    description: 'A bounded, scrollable log buffer with freeze-on-scroll and tail following.',
    primaryStory: 'log-pane/follow',
    usage: `use ratatui_core::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};
use termrock::{
  Theme,
  ansi_text::line_from_ansi,
  style::Role,
  widgets::{LogPane, LogPaneState},
};

let theme = Theme::default();
let pane = LogPane::new(&theme).title("Build");
let mut state = LogPaneState::new().with_max_lines(1_000);
state.append(line_from_ansi("\\u{1b}[32mready\\u{1b}[0m", theme.style(Role::Text)));
let area = Rect::new(0, 0, 80, 24);
let mut buffer = Buffer::empty(area);
(&pane).render(area, &mut buffer, &mut state);

// Wheel navigation uses geometry recorded by render. Oldest navigation can
// also be requested before first render and resolves when geometry is known.
let changed = state.scroll_by(-1);
state.scroll_to_oldest();
state.follow();

// Unbounded retention is an explicit opt-in when the caller owns the policy.
let unbounded = LogPaneState::new().unbounded();`,
  },
  MessageDialog: {
    description: 'A message dialog composed with optional scrollable detail rows.',
    primaryStory: 'message-dialog/details',
    usage: `use ratatui_core::text::Text;
use termrock::{Theme, widgets::{DetailCapability, DetailRow, DetailTableState, Dialog, MessageDialog}};

let theme = Theme::default();
let details = [DetailRow { id: "stage", label: "Stage", value: "Build", href: None, capability: DetailCapability::None, emphasis: false, style: None }];
let dialog = MessageDialog::new(Dialog::new("Failure", Text::from("Build failed"), &theme), &details, &theme).wrap(true);
let mut state = DetailTableState::<&str>::default();
let outcome = state.select_next(&details);`,
  },
  Panel: {
    description: 'A themed bordered container with semantic focus emphasis.',
    primaryStory: 'panel/focused',
    usage: `use ratatui_core::layout::Rect;
use termrock::{Theme, widgets::{Panel, PanelEmphasis}};

let theme = Theme::default();
let panel = Panel::new(&theme).title("Files").emphasis(PanelEmphasis::Focused);
let inner = panel.inner(Rect::new(0, 0, 80, 24));`,
  },
  Progress: {
    description: 'A deterministic determinate bar or caller-ticked indeterminate indicator.',
    primaryStory: 'progress/determinate',
    usage: `use ratatui_core::{buffer::Buffer, layout::Rect, widgets::Widget};
use termrock::{Theme, widgets::{Progress, ProgressKind}};

let theme = Theme::default();
// Below 16 columns, the percentage yields space to the glyph track.
let progress = Progress::new(ProgressKind::Determinate { fraction: 0.72 }, &theme)
    .label("Indexing");
let area = Rect::new(0, 0, 40, 1);
progress.render(area, &mut Buffer::empty(area));

let frames = ["|", "/", "-", "\\\\"];
let spinner = Progress::new(ProgressKind::Indeterminate { tick: 3 }, &theme)
    .frames(&frames)
    .label("Waiting");`,
  },
  SplitPane: {
    description: 'A resizable two-pane layout with bounded ratios and collapse support.',
    primaryStory: 'split-pane/horizontal',
    usage: `use ratatui_core::layout::Rect;
use termrock::{Theme, widgets::{SplitDirection, SplitPane, SplitPaneState, SplitRatio}};

let theme = Theme::default();
let pane = SplitPane::new(SplitDirection::Horizontal, 20, 20, &theme);
let mut state = SplitPaneState::new(SplitRatio::from_percent(40));
let layout = pane.layout(Rect::new(0, 0, 100, 24), &mut state);`,
  },
  StatusBar: {
    description: 'A one-row collection of prioritized, interactive status slots.',
    primaryStory: 'status-bar/basic',
    usage: `use ratatui_core::style::Style;
use termrock::{Theme, widgets::{StatusBar, StatusBarState, StatusSlot}};

let theme = Theme::default();
let left = [StatusSlot { id: "mode", content: "NORMAL", priority: 10, min_width: 0, enabled: true, style: Style::new(), hover_style: None }];
let bar = StatusBar::new(&left, &[], &theme);
let mut state = StatusBarState::<&str>::default();
state.hovered = Some("mode");`,
  },
  Tabs: {
    description: 'A keyboard- and pointer-navigable tab strip with stable identities.',
    primaryStory: 'tabs/status',
    usage: `use termrock::{Theme, widgets::{Tab, Tabs, TabsState}};

let theme = Theme::default();
let tabs = [Tab { id: "logs", label: "Logs", glyph: None, active: true, enabled: true }];
let strip = Tabs::new(&tabs, &theme);
let mut state = TabsState::default();
state.selected = Some("logs");`,
  },
  TextInput: {
    description: 'A single-line, grapheme-safe input with validation and semantic outcomes.',
    primaryStory: 'text-input/filter',
    usage: `use termrock::{Theme, widgets::{EditAction, TextInput, TextInputState, Validation}};

let theme = Theme::default();
let input = TextInput::new("Filter", &theme)
    .placeholder("type to filter")
    .validation(Validation::Valid);
let mut state = TextInputState::new("").with_max_graphemes(80);
let changed = state.apply(EditAction::Insert('a'));`,
  },
  Toast: {
    description: 'A transient severity notification with reusable placement and deterministic lifetime state.',
    primaryStory: 'toast/success',
    usage: `use ratatui_core::layout::Rect;
use std::time::Duration;
use termrock::{Theme, widgets::{Anchor, Severity, Toast, ToastLifetime, ToastState}};

let theme = Theme::default();
let toast = Toast::new(&theme, "Saved", Severity::Success)
    .anchor(Anchor::BottomRight)
    .margins(1, 1);
let state = ToastState::new(ToastLifetime::ExpiresAfter(Duration::from_secs(2)));
let rect = toast.rect(Rect::new(0, 0, 80, 24));`,
  },
  Tree: {
    description: 'A navigable flattened hierarchy with disclosure and multi-select support.',
    primaryStory: 'tree/navigation',
    usage: `use ratatui_core::text::Line;
use termrock::{Theme, input::{KeyCode, KeyEvent, KeyModifiers}, widgets::{Tree, TreeNode, TreeNodeStatus, TreeState}};

let theme = Theme::default();
let nodes = [TreeNode { id: "src", label: Line::from("src"), trailing: None, depth: 0, branch: true, expanded: true, enabled: true, status: TreeNodeStatus::Ready }];
let tree = Tree::new(&nodes, &theme);
let mut state = TreeState::new(Some("src"));
let outcome = state.handle_key(&nodes, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));`,
  },
  Viewport: {
    description: 'A two-axis scrollable view over borrowed terminal lines.',
    primaryStory: 'viewport/both-axes',
    usage: `use ratatui_core::text::Line;
use termrock::{Theme, scroll::DialogScroll, widgets::Viewport};

let theme = Theme::default();
let lines = [Line::from("long output")];
let viewport = Viewport::new(&lines, &theme).title("Output");
let mut state = DialogScroll::default();
state.scroll_y = 1;
let _vertical_offset = state.scroll_y;`,
  },
} as const satisfies Record<string, ComponentDoc>
