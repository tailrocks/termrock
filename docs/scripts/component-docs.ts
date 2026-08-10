export interface ComponentDoc {
  readonly description: string
  readonly primaryStory: string
  readonly usage: string
}

export const componentDocs = {
  AlertDialog: {
    description:
      'High-risk confirmation distinct from Dialog: exact scope, consequences, reversibility, target, safer alternatives; typed confirmation, justified countdown, safe initial focus, non-dismissable critical state.',
    primaryStory: 'alert-dialog/delete',
    usage: `use termrock::style::DesignSystem;
use termrock::widgets::{
    AlertDialog, AlertDialogState, AlertKind, AlertScope, AlertConfirmGates,
    open_alert_dialog_widget_overlay,
};

let system = DesignSystem::default();
let mut state = AlertDialogState::new(
    AlertKind::Delete,
    AlertScope::example_delete(),
    "delete",
    "keep",
);
state.set_gates(AlertConfirmGates::typed("prod-db.customers"));
// open: state.open_on_stack(&mut stack, bounds, Some("trigger"));
// Enter on safe focus cancels; move to confirm only after gates pass.
AlertDialog::new(&system).paint(area, buf, &mut state);`,
  },
  ActionBar: {
    description: 'A horizontal group of stable, caller-owned actions with painted hit regions.',
    primaryStory: 'action-bar/basic',
    usage: `use termrock::{Theme, widgets::{Action, ActionBar, ActionBarState}};

let theme = Theme::default();
let actions = [Action { id: "save", label: "Save", enabled: true, style: None }];
let bar = ActionBar::new(&actions, &theme);
let state = ActionBarState { focused: Some("save"), ..ActionBarState::default() };`,
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
    description:
      'Modal choice prompt on the canonical Dialog engine — stable actions, loading gate, Esc dismiss / confirm-only policy via dialog_mut().',
    primaryStory: 'choice-dialog/basic',
    usage: `use ratatui_core::text::Text;
use termrock::input::{KeyCode, KeyEvent, KeyModifiers};
use termrock::style::DesignSystem;
use termrock::widgets::{Action, ChoiceDialog, ChoiceDialogState, Dialog};

let system = DesignSystem::default();
let actions = [Action { id: "accept", label: "Accept", enabled: true, style: None }];
let dialog = ChoiceDialog::new(Dialog::new("Confirm", Text::from("Continue?"), &system), &actions);
let mut state = ChoiceDialogState::new(Some("accept"));
let outcome = state.handle_key(&actions, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));`,
  },
  CompletionMenu: {
    description:
      'Anchored suggestion surface for editors: groups, fuzzy ranges, kind glyphs, details, docs preview, async generation gates, loading/empty/stale, commit characters, active-descendant navigation (editor keeps focus), Tab/Enter/Esc intents, clamp/flip/fullscreen promotion.',
    primaryStory: 'completion-menu/basic',
    usage: `use ratatui_core::layout::Rect;
use termrock::style::DesignSystem;
use termrock::widgets::{
    CompletionCandidate, CompletionMenu, CompletionMenuSize, CompletionMenuState,
    open_completion_overlay,
};

let system = DesignSystem::default();
let candidates = [
    CompletionCandidate::new("select", "SELECT")
        .kind("keyword")
        .group("Keywords")
        .documentation("Select rows."),
];
let mut state = CompletionMenuState::new(Some("select"));
state.set_commit_characters("().");
// async: let gen = state.begin_async(); … state.apply_results(gen, &candidates);
let bounds = Rect::new(0, 0, 80, 24);
let anchor = Rect::new(12, 6, 1, 1);
let _ = open_completion_overlay(&mut stack, bounds, anchor, CompletionMenuSize::default(), Some("editor"));
CompletionMenu::new(&candidates, &system, bounds, anchor).paint(area, buf, &mut state);`,
  },
  DesignInspector: {
    description: 'Studio debug strip for focus, layer, density, and color capability.',
    primaryStory: 'design-inspector/basic',
    usage: `use termrock::{Theme, style::ColorCapability, widgets::{DesignInspector, DesignInspectorFrame}};

let theme = Theme::default();
let frame = DesignInspectorFrame {
    focused: Some("list"),
    layer: Some("root"),
    capability: ColorCapability::Truecolor,
    density: "comfortable",
    layers: &[],
    recipes: &[],
    selection_chrome: "gutter",
};
let _inspector = DesignInspector::new(frame, &theme);`,
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
  Table: {
    description: 'A stable-ID columnar data view with deterministic widths, sorting requests, and visible-window rendering.',
    primaryStory: 'table/basic',
    usage: `use std::num::NonZeroU16;
use ratatui_core::text::Line;
use termrock::{style::DesignTokens, Theme, input::{KeyCode, KeyEvent, KeyModifiers}, widgets::{CellAlignment, Column, ColumnWidth, Table, TableRow, TableState}};

let theme = Theme::default();
let columns = [Column { id: "name", title: Line::from("Name"), width: ColumnWidth::Fill(NonZeroU16::new(1).unwrap()), alignment: CellAlignment::Left, sortable: true, sort: None }];
let cells = [Line::from("termrock")];
let rows = [TableRow { id: "termrock", cells: &cells, leading: None, badge: None, enabled: true, emphasis: false, style: None }];
let tokens = DesignTokens::default();
let table = Table::new(&columns, &rows, &tokens);
let mut state = TableState::<&str, &str>::new(Some("termrock"));
state.set_focused(true);
let outcome = state.handle_key(&rows, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));`,
  },
  Dialog: {
    description:
      'Canonical modal surface: title, description, body, actions, close policy, focus trap via OverlayStack, initial focus, opener restoration, scrolling, loading, validation; recipes normal/compact/wide/fullscreen/destructive; safe Enter (no accidental submit from body).',
    primaryStory: 'dialog/message',
    usage: `use ratatui_core::text::Text;
use termrock::interaction::OverlayStack;
use termrock::style::DesignSystem;
use termrock::widgets::{
    Dialog, DialogRecipe, DialogSize, DialogState, open_dialog_overlay,
};

let system = DesignSystem::default();
let mut state = DialogState::<&str>::new();
state.set_recipe(DialogRecipe::Normal);
let mut stack = OverlayStack::<&str>::new();
let _ = state.open_on_stack(&mut stack, bounds, DialogSize::default(), Some("trigger"));
Dialog::new("Notice", Text::from("Saved"), &system)
    .description("Write completed.")
    .recipe(DialogRecipe::Normal)
    .footer_hint("esc dismiss")
    .paint(entry.rect, buf, &mut state, 0);`,
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
  FormWizard: {
    description:
      'Multi-step form wizard chrome with stepper, validation gates, review, failure/retry, and resume. Domain fields stay consumer-owned.',
    primaryStory: 'blocks/form-wizard',
    usage: `use termrock::style::DesignSystem;
use termrock::widgets::{FormWizard, FormWizardState, WizardStep};

let system = DesignSystem::default();
let mut state = FormWizardState::with_steps([
    WizardStep::new("account", "Account"),
    WizardStep::new("confirm", "Confirm"),
]);
state.set_focused(true);
let _ = FormWizard::new(&system).title("Setup");`,
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
use termrock::{style::DesignTokens, Theme, widgets::{List, ListRow, ListState, RowRole}};

let theme = Theme::default();
let rows = [ListRow { id: "alpha", label: Line::from("Alpha"), leading: None, secondary: None, badge: None, shortcut: None, trailing: None, role: RowRole::Item, enabled: true, loading: false }];
let tokens = DesignTokens::default();
let list = List::new(&rows, &tokens);
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
    description:
      'Composable container with variants, body modes, collapsible/interactive state; focus ≠ selection.',
    primaryStory: 'panel/variants',
    usage: `use termrock::{style::{DesignSystem, PanelChrome}, widgets::{Panel, PanelBody, PanelVariant}};

let system = DesignSystem::default();
let body = Panel::new(&system)
    .title("Inbox")
    .variant(PanelVariant::Bordered)
    .emphasis(PanelChrome::Focused)
    .body(PanelBody::Empty)
    .body_title("No messages")
    .paint(area, buf, None);`,
  },
  Card: {
    description:
      'Raised Panel composition with description band for metrics and tool cards.',
    primaryStory: 'card/basic',
    usage: `use termrock::{style::DesignSystem, widgets::Card};

let system = DesignSystem::default();
let body = Card::new(&system)
    .title("Latency")
    .description("p99 last hour")
    .footer("dashboard")
    .paint(area, buf, None);`,
  },
  Picker: {
    description: 'A filterable stable-ID list composition with caller-owned matching and ordering.',
    primaryStory: 'picker/basic',
    usage: `use ratatui_core::text::Line;
use termrock::{style::DesignTokens, Theme, input::{KeyCode, KeyEvent, KeyModifiers}, widgets::{ListRow, Picker, PickerOutcome, PickerState, RowRole}};

let theme = Theme::default();
let candidates = [("open", "Open file"), ("logs", "Show logs")];
let project = |query: &str| candidates.iter()
    .filter(|(_, label)| label.to_lowercase().contains(&query.to_lowercase()))
    .map(|(id, label)| ListRow { id: *id, label: Line::from(*label), leading: None, secondary: None, badge: None, shortcut: None, trailing: None, role: RowRole::Item, enabled: true, loading: false })
    .collect::<Vec<_>>();
let mut state = PickerState::new(Some("open"));
let mut rows = project(state.query_text());
if matches!(state.handle_key(&rows, KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE)), PickerOutcome::QueryChanged) {
    rows = project(state.query_text());
    state.reconcile(&rows);
}
let tokens = DesignTokens::default();
let picker = Picker::new(&rows, &tokens);`,
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
    description:
      'A composable tab strip with roving focus, manual/automatic activation, badges/status, close/reorder hooks, and narrow overflow/Select contraction. Panel state stays host-owned by id.',
    primaryStory: 'tabs/status',
    usage: `use termrock::style::DesignSystem;
use termrock::widgets::{Tab, TabStatus, Tabs, TabsState};

let system = DesignSystem::default();
let tabs = [
    Tab::new("logs", "Logs").status(TabStatus::Running).closable(true),
];
let mut state = TabsState::new().with_selected("logs");
state.set_focused(true);
let _ = Tabs::new(&tabs, &system).gap(1);`,
  },
  Sidebar: {
    description:
      'Primary application navigation with sections, hierarchy, badges, status, and rail/drawer collapse. Route state is distinct from keyboard focus.',
    primaryStory: 'sidebar/settings',
    usage: `use termrock::style::DesignSystem;
use termrock::widgets::{Sidebar, SidebarState, example_settings_nav};

let system = DesignSystem::default();
let items = example_settings_nav();
let mut state = SidebarState::new(Some("profile"));
state.set_focused(true);
let _ = Sidebar::new(&items, &system).title("Settings").show_panel(true);`,
  },
  NavigationList: {
    description:
      'Route-oriented navigation list with hierarchy, filter, and focus distinct from active route.',
    primaryStory: 'navigation-list/basic',
    usage: `use termrock::widgets::{NavItem, NavigationList, NavigationListState};

let items = [NavItem::new("a", "Inbox"), NavItem::new("b", "Starred")];
let mut state = NavigationListState::new(Some("a"));
state.set_focused(true);`,
  },
  TreeNavigation: {
    description:
      'Hierarchical route navigation with expansion, lazy children, typeahead, active ancestors, and route distinct from focus. Distinct from data Tree.',
    primaryStory: 'tree-navigation/project',
    usage: `use termrock::style::DesignSystem;
use termrock::widgets::{TreeNavigation, TreeNavigationState, example_project_tree};

let system = DesignSystem::default();
let nodes = example_project_tree();
let mut state = TreeNavigationState::new(Some("main"));
state.set_focused(true);
state.reconcile_route(&nodes);
let _ = TreeNavigation::new(&nodes, &system);`,
  },
  Breadcrumbs: {
    description:
      'Location context and ancestor navigation with collapse, overflow menu, editable path mode, and a single Tab stop.',
    primaryStory: 'breadcrumbs/path',
    usage: `use termrock::style::DesignSystem;
use termrock::widgets::{BreadcrumbItem, Breadcrumbs, BreadcrumbsState};

let system = DesignSystem::default();
let items = [
    BreadcrumbItem::new("h", "home"),
    BreadcrumbItem::new("s", "src").current(true),
];
let mut state = BreadcrumbsState::new();
state.set_focused(true);
let _ = Breadcrumbs::new(&items, &system).ascii(true);`,
  },
  Pagination: {
    description:
      'Page navigation for remote datasets with prev/next, page numbers, unknown totals, page size, loading, jump entry, and narrow contraction. Not scroll virtualization.',
    primaryStory: 'pagination/full',
    usage: `use termrock::style::DesignSystem;
use termrock::widgets::{PageTotal, Pagination, PaginationState};

let system = DesignSystem::default();
let mut state = PaginationState::new(1, 25, PageTotal::Known(1000));
state.set_focused(true);
// PageRequested { request } → host.fetch(request.offset(), request.limit())
let _ = Pagination::new(&system);`,
  },
  TextArea: {
    description:
      'A multi-line grapheme-safe editor with selection, undo/redo, soft wrap, line numbers, and host clipboard/external-editor hooks.',
    primaryStory: 'text-area/basic',
    usage: `use termrock::style::DesignSystem;
use termrock::widgets::{TextArea, TextAreaState, TextCursor, TextWrap};

let system = DesignSystem::default();
let area = TextArea::new(&system)
    .title("Notes")
    .placeholder("Write…")
    .line_numbers(true)
    .soft_wrap();
let mut state = TextAreaState::new("first line\nsecond line");
state.set_accepts_input(true);
state.set_wrap(TextWrap::Soft);
let valid = state.set_cursor(TextCursor { line: 1, byte: 0 });`,
  },
  PasswordInput: {
    description:
      'A secure secret-entry field that masks paint, redacts Debug/semantic output, and never embeds secrets in outcomes.',
    primaryStory: 'password-input/basic',
    usage: `use termrock::style::DesignSystem;
use termrock::widgets::{
    ClipboardPolicy, PasswordInput, PasswordInputState, PasswordStrengthHint, RevealPolicy,
};

let system = DesignSystem::default();
let mut state = PasswordInputState::new()
    .with_reveal_policy(RevealPolicy::Explicit)
    .with_clipboard_policy(ClipboardPolicy::PasteOnly);
state.set_focused(true);
let _ = PasswordInput::new("Password", &system)
    .placeholder("••••")
    .strength(PasswordStrengthHint::Weak);`,
  },
  NumberInput: {
    description:
      'A numeric field with draft text separate from committed value, min/max/step, units, and steppers.',
    primaryStory: 'number-input/basic',
    usage: `use termrock::style::DesignSystem;
use termrock::widgets::{NumberConstraints, NumberInput, NumberInputState, NumberKind};

let system = DesignSystem::default();
let mut state = NumberInputState::new()
    .with_kind(NumberKind::decimal2())
    .with_constraints(NumberConstraints::bounded(0.0, 100.0, 0.5))
    .with_value(12.5);
state.set_focused(true);
let _ = NumberInput::new("Opacity", &system).unit("%");`,
  },
  SearchInput: {
    description:
      'A search field with query, status, clear, history, filter chips, and host-polled debounce signals.',
    primaryStory: 'search-input/basic',
    usage: `use termrock::style::DesignSystem;
use termrock::widgets::{SearchInput, SearchInputState, SearchStatus};

let system = DesignSystem::default();
let mut state = SearchInputState::new().with_query("table");
state.set_focused(true);
// each frame: state.poll(tick) → DebouncedQuery
let _ = SearchInput::new(&system)
    .status(SearchStatus::Results { count: 12 })
    .placeholder("Search…");`,
  },
  PathInput: {
    description:
      'A filesystem-aware path field with host-projected status, completion/browse hooks, and no FS coupling.',
    primaryStory: 'path-input/basic',
    usage: `use termrock::style::DesignSystem;
use termrock::widgets::{PathFsStatus, PathInput, PathInputState, PathStyle};

let system = DesignSystem::default();
let mut state = PathInputState::new()
    .with_style(PathStyle::Unix)
    .with_path("/usr/local/bin");
state.set_focused(true);
state.set_fs_status(PathFsStatus::Directory);
let _ = PathInput::new(&system).label("Install dir").show_browse(true);`,
  },
  TokenField: {
    description:
      'An editable token/chip collection with free-text draft, completion hooks, and single-surface focus.',
    primaryStory: 'token-field/basic',
    usage: `use termrock::style::DesignSystem;
use termrock::widgets::{FieldToken, TokenField, TokenFieldState};

let system = DesignSystem::default();
let mut state = TokenFieldState::new();
state.set_focused(true);
let _ = state.push_token(FieldToken::new("1".into(), "alice@ex.com"));
let _ = TokenField::new(&system).label("To").placeholder("Add…");`,
  },
  Select: {
    description:
      'A single-choice select with CollectionState navigation, value≠highlight, recipes, and popover/fullscreen.',
    primaryStory: 'select/basic',
    usage: `use termrock::style::DesignSystem;
use termrock::widgets::{Select, SelectOption, SelectState};

let system = DesignSystem::default();
let options = [
  SelectOption::option("apple", "Apple"),
  SelectOption::option("banana", "Banana"),
];
let mut state = SelectState::new().with_value("apple");
state.set_focused(true);
let _ = Select::new(&options, &system).label("Fruit");`,
  },
  MultiSelect: {
    description:
      'A searchable multi-choice selector with chip summary, select-all, max selection, and highlight≠checked.',
    primaryStory: 'multi-select/basic',
    usage: `use termrock::style::DesignSystem;
use termrock::widgets::{MultiSelect, MultiSelectState, SelectOption};

let system = DesignSystem::default();
let options = [
  SelectOption::option("rs", "Rust"),
  SelectOption::option("go", "Go"),
];
let mut state = MultiSelectState::new().with_selected(["rs"]);
state.set_focused(true);
let _ = MultiSelect::new(&options, &system).label("Filters");`,
  },
  Combobox: {
    description:
      'An editable input with CompletionMenu suggestions, async generation gating, and separate draft/active/value.',
    primaryStory: 'combobox/basic',
    usage: `use termrock::style::DesignSystem;
use termrock::widgets::{Combobox, ComboboxState, CompletionCandidate};

let system = DesignSystem::default();
let mut state: ComboboxState<&str> = ComboboxState::new();
state.set_focused(true);
// DraftChanged { generation } → host fetch → apply_suggestions(generation, …)
let _ = Combobox::new(&system).label("Language");`,
  },
  Autocomplete: {
    description:
      'Creatable free-text field with optional suggestions (Combobox autocomplete mode).',
    primaryStory: 'autocomplete/basic',
    usage: `use termrock::widgets::ComboboxState;
let mut state: ComboboxState<&str> = ComboboxState::autocomplete();
state.set_focused(true);`,
  },
  FilePicker: {
    description:
      'A host-driven file/directory browser with breadcrumbs, multi-select, path entry, and optional preview — no filesystem I/O in TermRock.',
    primaryStory: 'file-picker/unix',
    usage: `use termrock::style::DesignSystem;
use termrock::widgets::{
    FileEntry, FilePicker, FilePickerMode, FilePickerState, FilePreview,
};

let system = DesignSystem::default();
let mut state = FilePickerState::new("/home/u")
    .with_mode(FilePickerMode::OpenFile)
    .with_preview(true);
state.set_focused(true);
// ListRequested { path, generation } → host list → apply_listing(generation, …)
let _ = FilePicker::new(&system).title("Open file");`,
  },
  DateTimePicker: {
    description:
      'Civil date, time, and range picker with text entry, calendar/time-list browse, min/max, and non-color day states. Prefer TextInput for rare ISO paste.',
    primaryStory: 'date-time-picker/date',
    usage: `use termrock::style::DesignSystem;
use termrock::widgets::{
    CivilDate, DateTimePicker, DateTimePickerKind, DateTimePickerState,
};

let system = DesignSystem::default();
let mut state = DateTimePickerState::new(DateTimePickerKind::Date)
    .with_timezone_label("UTC");
state.set_today(CivilDate::new(2026, 8, 10).unwrap());
state.set_focused(true);
let _ = DateTimePicker::new(&system).label("Due");`,
  },
  KeybindingRecorder: {
    description:
      'A settings control that captures and validates keybindings with escape law, conflicts, reserved chords, and protocol limits.',
    primaryStory: 'keybinding-recorder/idle',
    usage: `use termrock::input::KeyCode;
use termrock::keymap::KeyChord;
use termrock::style::DesignSystem;
use termrock::widgets::{KeybindingRecorder, KeybindingRecorderState};

let system = DesignSystem::default();
let mut state = KeybindingRecorderState::new("app.save", "Save")
    .with_chords([KeyChord::ctrl(KeyCode::Char('s'))]);
state.set_focused(true);
let _ = KeybindingRecorder::new(&system);`,
  },
  TextInput: {
    description: 'A single-line, grapheme-safe input with validation and semantic outcomes.',
    primaryStory: 'text-input/unicode',
    usage: `use termrock::{Theme, widgets::{EditAction, TextInput, TextInputState, Validation}};

let theme = Theme::default();
let input = TextInput::new("Filter", &theme)
    .placeholder("type to filter")
    .validation(Validation::Valid);
let mut state = TextInputState::new("").with_max_graphemes(80);
let changed = state.apply(EditAction::Insert('a'));`,
  },
  Toast: {
    description:
      'Transient notifications with priority, queue/dedup/replace, timeout pause, progress/undo kinds, and missed-item archive for NotificationCenter — never steals keyboard focus.',
    primaryStory: 'toast/success',
    usage: `use termrock::style::DesignSystem;
use termrock::runtime::FrameTick;
use termrock::widgets::{
    Anchor, Severity, Toast, ToastLifetime, ToastQueue, ToastSpec, ToastStack, TOAST_DEFAULT_TTL,
};
use std::time::{Duration, Instant};

let system = DesignSystem::default();
let toast = Toast::new(&system, "Saved", Severity::Success).anchor(Anchor::TopRight);
let tick = FrameTick::manual(Instant::now(), Duration::ZERO, Duration::ZERO);
let mut queue = ToastQueue::new();
let _ = queue.push(tick, ToastSpec::message("s", "Saved").severity(Severity::Success));
ToastStack::new(&system).paint(area, buf, &mut queue);
// missed: queue.drain_missed() → NotificationCenter
let _ = (toast, TOAST_DEFAULT_TTL, ToastLifetime::default_ttl());`,
  },
  NotificationCenter: {
    description:
      'Persistent notification history: unread, filter/group, timestamps, actions, progress, source, dismiss/clear-all; drawer or full-page; host-owned persistence; ingests ToastArchive without duplicating models.',
    primaryStory: 'notification-center/drawer',
    usage: `use termrock::style::DesignSystem;
use termrock::widgets::{
    NotificationCenter, NotificationCenterState, NotificationRecipe, example_notifications,
};

let system = DesignSystem::default();
let mut state = NotificationCenterState::new();
state.replace_items(example_notifications(now_secs)); // or ingest_from_toast_queue
state.set_recipe(NotificationRecipe::Drawer);
let _ = state.open();
NotificationCenter::new(&system).paint(area, buf, &mut state);
// Host persists: state.items().to_vec()`,
  },
  Tree: {
    description: 'A navigable flattened hierarchy with disclosure and multi-select support.',
    primaryStory: 'tree/navigation',
    usage: `use ratatui_core::text::Line;
use termrock::{style::DesignTokens, Theme, input::{KeyCode, KeyEvent, KeyModifiers}, widgets::{Tree, TreeNode, TreeNodeStatus, TreeState}};

let theme = Theme::default();
let nodes = [TreeNode { id: "src", label: Line::from("src"), leading: None, secondary: None, badge: None, shortcut: None, trailing: None, depth: 0, branch: true, expanded: true, enabled: true, status: TreeNodeStatus::Ready }];
let tokens = DesignTokens::default();
let tree = Tree::new(&nodes, &tokens);
let mut state = TreeState::new(Some("src"));
let outcome = state.handle_key(&nodes, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));`,
  },
  VirtualGrid: {
    description: 'A two-axis virtualized grid whose paint cost is bounded by caller-projected resident rows.',
    primaryStory: 'virtual-grid/basic',
    usage: `use termrock::{Theme, widgets::{GridCell, GridColumn, GridRow, VirtualGrid, VirtualGridState}};

let theme = Theme::default();
let columns = [
  GridColumn::fixed("id", "ID", 8),
  GridColumn::min("name", "Name", 16),
];
let cells = [GridCell::text("42"), GridCell::text("termrock")];
let rows = [GridRow::new("row-42", 42, &cells)];
let grid = VirtualGrid::new(&columns, &rows, &theme).total_rows(1_000_000);
let state = VirtualGridState::<&str, &str>::new();`,
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

  ApprovalCard: {
    description: 'Fail-safe permission card with default Deny and typed outcomes (no side effects).',
    primaryStory: 'approval-card/basic',
    usage: `use termrock::{Theme, input::{KeyCode, KeyEvent, KeyModifiers}, widgets::{ApprovalCard, ApprovalCardOutcome, ApprovalCardState, ApprovalRisk}};

let theme = Theme::default();
let card = ApprovalCard::new("Permission", "Run cargo publish?", ApprovalRisk::High, &theme);
let mut state = ApprovalCardState::new(); // selected = Deny
match state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)) {
    ApprovalCardOutcome::Confirmed(decision) => { let _ = decision; }
    ApprovalCardOutcome::Cancelled => {}
    _ => {}
}`,
  },
  Banner: {
    description: 'A single-line severity banner with non-color glyphs.',
    primaryStory: 'banner/basic',
    usage: `use termrock::{Theme, widgets::{Banner, Severity}};

let theme = Theme::default();
let banner = Banner::new("Deployed", Severity::Success, &theme);`,
  },
  BarSeries: {
    description: 'Labeled horizontal bars for density dashboards.',
    primaryStory: 'bar-series/basic',
    usage: `use termrock::{Theme, widgets::{BarDatum, BarSeries}};

let theme = Theme::default();
let bars = [BarDatum { label: "cpu", fraction: 0.72 }];
let series = BarSeries::new(&bars, &theme);`,
  },
  CodeBlock: {
    description: 'Source listing with optional line numbers and pluggable syntax.',
    primaryStory: 'code-block/basic',
    usage: `use termrock::{Theme, widgets::CodeBlock};

let theme = Theme::default();
let lines = ["fn main() {}"];
let block = CodeBlock::new(&lines, &theme).language("rust").line_numbers(true);`,
  },
  CommandPalette: {
    description:
      'Flagship universal command surface: fuzzy search, groups, recent/contextual actions, nested pages, arguments, async generation gates, history, and fullscreen promotion.',
    primaryStory: 'command-palette/basic',
    usage: `use termrock::style::DesignSystem;
use termrock::widgets::{CommandPalette, CommandPaletteState, example_command_catalog};

let system = DesignSystem::default();
let catalog = example_command_catalog();
let mut state = CommandPaletteState::new(None);
state.set_focused(true);
let visible = state.refilter(&catalog);
CommandPalette::new("Commands", &visible, &system).paint(area, buf, &mut state);`,
  },
  JumpMode: {
    description:
      'Easymotion-style jump navigation over SemanticScene regions with prefix-free multi-key labels, role/action filters, and OverlayStack Esc restore.',
    primaryStory: 'jump-mode/multi',
    usage: `use termrock::style::DesignSystem;
use termrock::widgets::{JumpOverlay, JumpOverlayState, JumpTarget, assign_jump_badges_from_semantics};

let system = DesignSystem::default();
let targets = assign_jump_badges_from_semantics(&scene);
let mut state = JumpOverlayState::new();
state.open();
JumpOverlay::from_state(&targets, &system, &state).render(area, buf);`,
  },
  FocusLens: {
    description:
      'Studio focus inspection lens: tab-order indices and focused markers over a FocusGraph without mutating widgets.',
    primaryStory: 'focus-lens/combined',
    usage: `use termrock::interaction::{FocusGraph, FocusLens, FocusLensMode};
use termrock::style::DesignSystem;

let system = DesignSystem::default();
FocusLens::new(&graph, &system).mode(FocusLensMode::Combined).render(area, buf);`,
  },
  Tooltip: {
    description:
      'Delayed non-focus-stealing contextual help with pointer/focus triggers, plain/shortcut/rich variants, reduced-motion, and essential-elsewhere policy.',
    primaryStory: 'tooltip/plain',
    usage: `use termrock::style::DesignSystem;
use termrock::widgets::{Tooltip, TooltipContent, TooltipState};

let system = DesignSystem::default();
let mut state = TooltipState::new();
// host: state.set_pointer_over / set_focus_within + advance(tick, motion)
Tooltip::content(
    TooltipContent::plain("Help").essential_elsewhere(true),
    &system,
).paint(area, buf, &state);`,
  },
  KeyboardHelp: {
    description:
      'Contextual generated keyboard help from live keymaps, zones, overlays, and semantic actions — footer or searchable modal, never stale hardcoded shortcuts.',
    primaryStory: 'keyboard-help/footer',
    usage: `use termrock::style::DesignSystem;
use termrock::widgets::{KeyboardHelp, KeyboardHelpState, example_help_entries};

let system = DesignSystem::default();
let entries = example_help_entries(&system);
let mut state = KeyboardHelpState::new();
KeyboardHelp::new(&entries, &system).paint(area, buf, &mut state);`,
  },
  HistoryPicker: {
    description:
      'Reusable recent-history selector with pin/delete, search, groups, preview, redaction hooks, draft preservation, and popover/fullscreen placement.',
    primaryStory: 'history-picker/basic',
    usage: `use termrock::style::DesignSystem;
use termrock::widgets::{HistoryPicker, HistoryPickerState, example_history_entries, filter_history_entries};

let system = DesignSystem::default();
let entries = example_history_entries();
let visible = filter_history_entries(&entries, "");
let mut state = HistoryPickerState::new();
let _ = state.open(None);
HistoryPicker::new(&visible, &system).paint(area, buf, &mut state);`,
  },
  Stepper: {
    description:
      'Progress/navigation chrome for multi-step flows with status marks, horizontal/vertical layout, policy-gated jumps, and narrow numeric/menu contraction.',
    primaryStory: 'stepper/horizontal',
    usage: `use termrock::style::DesignSystem;
use termrock::widgets::{Stepper, StepperState, StepperNavPolicy, example_onboarding_steps};

let system = DesignSystem::default();
let items = example_onboarding_steps();
let mut state = StepperState::with_len(items.len()).policy(StepperNavPolicy::Linear);
state.set_focused(true);
Stepper::new(&items, &system).paint(area, buf, &mut state);`,
  },
  QuickOpen: {
    description:
      'High-performance multi-provider fuzzy resource opener with async streaming, previews, query syntax, JumpMode and fullscreen integration.',
    primaryStory: 'quick-open/basic',
    usage: `use termrock::style::DesignSystem;
use termrock::widgets::{QuickOpen, QuickOpenState, example_quick_open_files, example_quick_open_providers};

let system = DesignSystem::default();
let providers = example_quick_open_providers();
let items = example_quick_open_files();
let mut state = QuickOpenState::new();
state.set_focused(true);
let _ = state.apply_results(0, &items, true, None);
QuickOpen::new(&providers, &items, &system).paint(area, buf, &mut state);`,
  },
  EmptyState: {
    description: 'Centered empty surface with a non-color glyph.',
    primaryStory: 'empty-state/basic',
    usage: `use termrock::{Theme, widgets::EmptyState};

let theme = Theme::default();
let empty = EmptyState::new("No results", &theme).detail("Try another query");`,
  },
  ErrorView: {
    description: 'Centered failure surface with danger marker.',
    primaryStory: 'error-view/basic',
    usage: `use termrock::{Theme, widgets::ErrorView};

let theme = Theme::default();
let error = ErrorView::new("Failed", &theme).detail("Timed out");`,
  },
  JumpOverlay: {
    description: 'Letter-badge jump navigation over registered rectangles.',
    primaryStory: 'jump-overlay/basic',
    usage: `use ratatui_core::layout::Rect;
use termrock::{Theme, widgets::{JumpOverlay, JumpOverlayState, JumpTarget}};

let theme = Theme::default();
let targets = [JumpTarget { id: "files", area: Rect::new(0, 0, 10, 1), badge: 'f' }];
let overlay = JumpOverlay::new(&targets, &theme);
let mut state = JumpOverlayState::new();
state.open();`,
  },
  LoadingView: {
    description: 'Centered loading label with a caller-ticked spinner frame.',
    primaryStory: 'loading-view/basic',
    usage: `use termrock::{Theme, widgets::LoadingView};

let theme = Theme::default();
let loading = LoadingView::new("Loading…", "⠋", &theme);`,
  },
  Spinner: {
    description:
      'Semantic activity spinner with verb label, phases (indeterminate/waiting/queued/reconnecting), capability glyphs, reduced-motion static frames, and idle redraw when inactive/hidden.',
    primaryStory: 'spinner/labeled',
    usage: `use termrock::style::{DesignSystem, Motion};
use termrock::runtime::FrameTick;
use termrock::widgets::{Spinner, SpinnerState, ActivityPhase};

let system = DesignSystem::default();
let mut state = SpinnerState::new();
state.set_phase(ActivityPhase::Indeterminate);
// host: only tick while state.should_tick()
Spinner::labeled("Fetching packages", &system)
    .paint(area, buf, &state, tick, Motion::Full);`,
  },
  ActivityIndicator: {
    description:
      'Activity line with phase spinner, required verb, and optional detail (reconnect attempts, targets).',
    primaryStory: 'activity-indicator/basic',
    usage: `use termrock::widgets::{ActivityIndicator, SpinnerState, ActivityPhase};
use termrock::style::Motion;

let mut state = SpinnerState::new();
state.set_phase(ActivityPhase::Reconnecting);
ActivityIndicator::new("Reconnecting to agent", &system)
    .detail("attempt 2/5")
    .paint(area, buf, &state, tick, Motion::Full);`,
  },
  MarkdownView: {
    description: 'Projected markdown-like blocks with semantic roles.',
    primaryStory: 'markdown-view/basic',
    usage: `use termrock::{Theme, widgets::{MarkdownBlock, MarkdownBlockKind, MarkdownView}};

let theme = Theme::default();
let blocks = [MarkdownBlock { kind: MarkdownBlockKind::Heading, text: "Plan" }];
let view = MarkdownView::new(&blocks, &theme);`,
  },
  PromptBox: {
    description: 'Agent prompt chrome over the multi-line editor.',
    primaryStory: 'prompt-box/basic',
    usage: `use termrock::{Theme, widgets::{PromptBox, PromptBoxState}};

let theme = Theme::default();
let prompt = PromptBox::new(&theme).placeholder("Message…");
let mut state = PromptBoxState::new();`,
  },
  SegmentedMeter: {
    description: 'Single-row proportional meter for stacked shares.',
    primaryStory: 'segmented-meter/basic',
    usage: `use termrock::{Theme, style::Role, widgets::{MeterSegment, SegmentedMeter}};

let theme = Theme::default();
let segments = [MeterSegment { label: "used", weight: 1.0, role: Role::Success }];
let meter = SegmentedMeter::new(&segments, &theme);`,
  },
  Skeleton: {
    description: 'Placeholder loading lines for list surfaces.',
    primaryStory: 'skeleton/basic',
    usage: `use termrock::{Theme, widgets::Skeleton};

let theme = Theme::default();
let skeleton = Skeleton::new(4, &theme);`,
  },
  Sparkline: {
    description: 'One-row sparkline over normalized samples.',
    primaryStory: 'sparkline/basic',
    usage: `use termrock::{Theme, widgets::Sparkline};

let theme = Theme::default();
let samples = [0.1, 0.5, 0.9];
let spark = Sparkline::new(&samples, &theme);`,
  },
  StreamView: {
    description: 'Stable-ID conversation stream with fold markers.',
    primaryStory: 'stream-view/basic',
    usage: `use termrock::{Theme, widgets::{StreamItem, StreamItemKind, StreamView}};

let theme = Theme::default();
let items = [StreamItem { id: "u1", kind: StreamItemKind::User, text: "Hello", folded: false }];
let stream = StreamView::new(&items, &theme);`,
  },
  ThinkingBlock: {
    description: 'Collapsible thinking/reasoning chrome.',
    primaryStory: 'thinking-block/basic',
    usage: `use termrock::{Theme, widgets::ThinkingBlock};

let theme = Theme::default();
let block = ThinkingBlock::new("Planning", &theme).expanded(true).body("Details");`,
  },
  Timeline: {
    description: 'Vertical activity timeline with active markers.',
    primaryStory: 'timeline/basic',
    usage: `use termrock::{Theme, widgets::{Timeline, TimelineEvent}};

let theme = Theme::default();
let events = [TimelineEvent { when: "12:01", text: "Started", active: true }];
let timeline = Timeline::new(&events, &theme);`,
  },
  TokenMeter: {
    description: 'Token or cost usage meter with threshold roles.',
    primaryStory: 'token-meter/basic',
    usage: `use termrock::{Theme, widgets::TokenMeter};

let theme = Theme::default();
let meter = TokenMeter::new(128_000, 200_000, &theme);`,
  },
  Transcript: {
    description: 'Variable-height streaming transcript with stable visual anchors.',
    primaryStory: 'transcript/basic',
    usage: `use termrock::{Theme, widgets::{Transcript, TranscriptBlock, TranscriptKind, TranscriptState}};

let theme = Theme::default();
let lines = ["hello", "world"];
let blocks = [TranscriptBlock::new("b1", TranscriptKind::User, &lines)];
let mut state: TranscriptState<&str> = TranscriptState::new();
let view = Transcript::new(&blocks, &theme);
let _ = (&mut state, view);`,
  },
  ToolCard: {
    description: 'Mutable tool invocation card with non-color status glyphs.',
    primaryStory: 'tool-card/basic',
    usage: `use termrock::{Theme, widgets::{ToolCard, ToolStatus}};

let theme = Theme::default();
let card = ToolCard::new("shell", "cargo test", ToolStatus::Running, &theme);`,
  },

  ThemePicker: {
    description: 'A live theme preset list; selection changes drive caller re-render.',
    primaryStory: 'theme-picker/basic',
    usage: `use termrock::{Theme, widgets::{BUILTIN_THEME_PRESETS, ThemePicker, ThemePickerState, theme_from_preset_id}};

let theme = Theme::default();
let picker = ThemePicker::new(BUILTIN_THEME_PRESETS, &theme);
let mut state = ThemePickerState::new(0);
let preview = theme_from_preset_id("slate");`,
  },
  ImageSurface: {
    description: 'A framed image slot with protocol labels; pixels stay caller-owned.',
    primaryStory: 'image-surface/basic',
    usage: `use termrock::{Theme, widgets::{ImageMeta, ImageProtocol, ImageSurface}};

let theme = Theme::default();
let meta = ImageMeta { label: "shot.png", pixel_width: Some(64), pixel_height: Some(64), protocol: ImageProtocol::Placeholder, pending: false, stale: false, generation: 0 };
let surface = ImageSurface::new(meta, &theme);`,
  },


  Button: {
    description: 'Activation primitive. Enter/Space or pointer activate once; disabled/loading never activate.',
    primaryStory: 'button/activation',
    usage: `use termrock::{style::DesignTokens, widgets::{Button, ButtonState}};

let tokens = DesignTokens::default();
let mut state = ButtonState::new();
let button = Button::new("Save", &tokens);`,
  },
  Checkbox: {
    description: 'Controlled checkbox with typed value-change outcomes.',
    primaryStory: 'checkbox/switch',
    usage: `use termrock::{style::DesignTokens, widgets::{Checkbox, CheckboxState}};

let tokens = DesignTokens::default();
let mut state = CheckboxState::new(false);
let box_ = Checkbox::new("enable", "Enable", &tokens);`,
  },
  DataTable: {
    description: 'Scalable table chrome; SelectAll is request-only over projected rows.',
    primaryStory: 'data-table/toolbar',
    usage: `use termrock::{style::DesignTokens, widgets::{ColumnModel, DataColumn, DataColumnWidth, DataTable, DataTableState}};

let tokens = DesignTokens::default();
let columns = ColumnModel::new(vec![DataColumn::new("c", "C", DataColumnWidth::Min(4))]);
let rows: [(u64, &[&str]); 0] = [];
let mut state = DataTableState::<u64, &str>::new();
let table = DataTable::new(&tokens, &columns, &rows);`,
  },
  Menu: {
    description:
      'Flat single-panel menu adapter over MenuItem (prefer DropdownMenu/MenuNode for nested command menus).',
    primaryStory: 'menu/roving',
    usage: `use termrock::style::DesignSystem;
use termrock::widgets::{Menu, MenuItem, MenuState};

let system = DesignSystem::default();
let items = [MenuItem::new("a", "Open")];
let mut state = MenuState::new();
Menu::new(&items, &system).render(area, buf, &mut state);`,
  },
  DropdownMenu: {
    description:
      'Anchored command menu with nested items, checkbox/radio, shortcuts, typeahead, OverlayStack cascade, and CommandPalette promotion when deep or oversized.',
    primaryStory: 'dropdown-menu/basic',
    usage: `use ratatui_core::layout::Rect;
use termrock::style::DesignSystem;
use termrock::widgets::{
    DropdownMenu, DropdownMenuState, MenuNode, open_dropdown_menu_overlay, measure_menu_panel,
};

let system = DesignSystem::default();
let nodes = vec![
    MenuNode::command("open", "Open").shortcut("C-o"),
    MenuNode::submenu("more", "More", vec![MenuNode::command("a", "A")]),
];
let mut state = DropdownMenuState::new();
let bounds = Rect::new(0, 0, 80, 24);
let _ = state.open_from_keyboard(&nodes, bounds);
// stack: open_dropdown_menu_overlay(...); paint: DropdownMenu::new(&nodes, &system).paint(...)`,
  },
  ContextMenu: {
    description:
      'Pointer/context-key command menu at origin with nested cascade, same item model as DropdownMenu, OverlayKind::ContextMenu placement.',
    primaryStory: 'context-menu/basic',
    usage: `use termrock::widgets::{
    ContextMenu, DropdownMenuState, MenuNode, open_context_menu_overlay,
};

let nodes = vec![
    MenuNode::command("copy", "Copy").shortcut("C-c"),
    MenuNode::command("delete", "Delete").destructive(true),
];
let mut state = DropdownMenuState::context();
let _ = state.open_from_context_pointer(&nodes, bounds);
// open_context_menu_overlay(&mut stack, bounds, origin, size, opener);
// ContextMenu::new(&nodes, &system).paint(rect, buf, &mut state);`,
  },
  MenuBar: {
    description:
      'Desktop-style top-level menus with nested cascade, mnemonics, checked/radio rows, OverlayStack helpers, and narrow CommandPalette replacement.',
    primaryStory: 'menu-bar/basic',
    usage: `use termrock::style::DesignSystem;
use termrock::widgets::{MenuBar, MenuBarState, example_app_menus};

let system = DesignSystem::default();
let menus = example_app_menus();
let mut state = MenuBarState::new();
state.set_focused(true);
let _ = state.open_menu_at(&menus, 0);
MenuBar::new(&menus, &system).paint_all(bar_area, bounds, buf, &mut state);`,
  },

  Badge: {
    description: 'Non-interactive status badge with semantic role paint.',
    primaryStory: 'badge/basic',
    usage: `use termrock::{style::DesignTokens, widgets::Badge};

let tokens = DesignTokens::default();
let badge = Badge::new("NEW", &tokens);`,
  },
  Callout: {
    description:
      'Inline feedback with tone gutter rail, title/description/details/source, compact or section recipes; readable in ASCII/no-color.',
    primaryStory: 'callout/basic',
    usage: `use termrock::style::DesignSystem;
use termrock::widgets::{Callout, CalloutTone};

let system = DesignSystem::default();
let callout = Callout::new("Heads up", &system)
    .body("Non-color risk glyph present.")
    .tone(CalloutTone::Warning)
    .source("validator");
// Section: .section() · ASCII: .ascii(true)`,
  },
  Alert: {
    description:
      'Stronger dismissible/acknowledgeable inline alert with actions, details, source, compact or banner recipes — not AlertDialog.',
    primaryStory: 'alert/danger',
    usage: `use termrock::style::DesignSystem;
use termrock::widgets::{Alert, AlertState, AlertTone, Action};

let system = DesignSystem::default();
let mut state = AlertState::new();
state.set_focused(true);
let actions = [Action { id: "retry", label: "Retry", enabled: true, style: None }];
Alert::new("Deploy failed", &system)
    .tone(AlertTone::Danger)
    .body("Rollout aborted")
    .source("pipeline #42")
    .actions(&actions)
    .banner()
    .paint(area, buf, &mut state);`,
  },
  Drawer: {
    description:
      'Edge-mounted secondary surface for inspectors, task rails, filters, details — left/right/top/bottom, modal/non-modal, resizable depth, focus trap, opener restore, nested overlays, compact handle, fullscreen promotion; preserves host selection/scroll.',
    primaryStory: 'drawer/basic',
    usage: `use termrock::style::DesignSystem;
use termrock::widgets::{
    Drawer, DrawerState, DrawerEdge, DrawerModality, open_drawer_overlay,
};

let system = DesignSystem::default();
let mut state = DrawerState::new(); // or ::sheet() / ::non_modal()
state.set_edge(DrawerEdge::Right);
let _ = state.open_on_stack(&mut stack, bounds, Some("main"));
// Host keeps list selection/scroll of underlying view.
Drawer::new("Inspector", &system)
    .footer(Some("esc · [ ] resize"))
    .paint(entry.rect, buf, &mut state);
// Host paints domain content into state.body_area().`,
  },
  Sheet: {
    description:
      'Bottom-edge drawer (shadcn Sheet metaphor) — same engine as Drawer with DrawerEdge::Bottom default.',
    primaryStory: 'drawer/sheet',
    usage: `use termrock::widgets::{Sheet, DrawerState};

let mut state = DrawerState::sheet();
state.open();
Sheet::new("Actions", &system).paint(area, buf, &mut state);`,
  },
  FullscreenViewer: {
    description:
      'Promotion chrome compact→detail→fullscreen with frozen SourceContext (selection, scroll anchor, focus, breadcrumbs). Host paints CodeBlock/Diff/logs/objects/tasks/media into body slot; nested Esc peels stack first.',
    primaryStory: 'fullscreen-viewer/basic',
    usage: `use termrock::style::DesignSystem;
use termrock::widgets::{
    Action, FullscreenViewer, FullscreenViewerState, SourceContext, ScrollAnchor,
    ViewerContentKind, open_fullscreen_viewer_overlay,
};

let system = DesignSystem::default();
let actions = [Action { id: "copy", label: "Copy", enabled: true, style: None }];
let mut state = FullscreenViewerState::new();
state.zoom_mut().set_content_kind(ViewerContentKind::Code);
let ctx = SourceContext::new("main.rs")
    .scroll(ScrollAnchor::at(42, 0))
    .path(["repo", "src", "main.rs"]);
let _ = state.enter_fullscreen(ctx, "main.rs");
let _ = state.open_on_stack(&mut stack, bounds, Some("list"));
FullscreenViewer::new(&system, &actions).paint(entry.rect, buf, &mut state);
// Host: CodeBlock::…paint(state.body_area(), buf, &mut code_state);`,
  },
  PreviewCard: {
    description:
      'Non-essential delayed resource preview (file/command/symbol/session) with metadata, loading/error, pin-to-open, and generation-gated async — never focus-stealing when unpinned; required facts must exist outside the card.',
    primaryStory: 'preview-card/file',
    usage: `use termrock::style::DesignSystem;
use termrock::widgets::{
    PreviewCard, PreviewCardOutcome, PreviewCardState, example_file_preview,
};

let system = DesignSystem::default();
let mut state = PreviewCardState::new();
let _ = state.set_selection("main.rs");
if let PreviewCardOutcome::Loading { generation } = state.begin_fetch() {
    let _ = state.apply_ready(generation); // host async first
}
let (content, _, _) = example_file_preview();
let _ = state.tick_hover(300, true);
PreviewCard::new(content, &system).paint(area, buf, &mut state);`,
  },
  Heading: {
    description: 'Semantic heading line with terminal typography levels.',
    primaryStory: 'heading/basic',
    usage: `use termrock::{style::DesignTokens, widgets::{Heading, HeadingLevel}};

let tokens = DesignTokens::default();
let h = Heading::new("Title", &tokens).level(HeadingLevel::H1);`,
  },
  Kbd: {
    description: 'Key chord chrome for keymap hint projection.',
    primaryStory: 'kbd/basic',
    usage: `use termrock::{style::DesignTokens, widgets::Kbd};

let tokens = DesignTokens::default();
let kbd = Kbd::new("C-k", &tokens);`,
  },
  ModeRibbon: {
    description: 'Product-neutral agent mode strip with selection outcomes.',
    primaryStory: 'mode-ribbon/basic',
    usage: `use termrock::{style::DesignTokens, widgets::{ModeRibbon, WorkbenchMode}};

let tokens = DesignTokens::default();
let modes = [WorkbenchMode { id: "plan", label: "Plan", active: true, enabled: true }];
let ribbon = ModeRibbon::new(&modes, &tokens);`,
  },
  Paragraph: {
    description: 'Body paragraph with grapheme-safe display-column wrap.',
    primaryStory: 'paragraph/basic',
    usage: `use termrock::{style::DesignTokens, widgets::Paragraph};

let tokens = DesignTokens::default();
let p = Paragraph::new("Body text", &tokens);`,
  },
  PermissionPrompt: {
    description: 'Fail-safe permission/trust surface with default-deny focus.',
    primaryStory: 'permission-prompt/basic',
    usage: `use termrock::{Theme, widgets::{PermissionPrompt, PermissionPromptState, PermissionRequest}};

let theme = Theme::default();
let prompt = PermissionPrompt::new(&theme);
let mut state = PermissionPromptState::new();
state.enqueue(PermissionRequest::new("r1", "bash", "workspace"));`,
  },
  PlanReview: {
    description: 'Plan step review list with accept markers.',
    primaryStory: 'plan-review/basic',
    usage: `use termrock::{style::DesignTokens, widgets::{PlanReview, PlanStep}};

let tokens = DesignTokens::default();
let steps = [PlanStep { id: "s1", title: "Inspect", detail: None, accepted: false }];
let review = PlanReview::new(&steps, &tokens);`,
  },
  Popover: {
    description:
      'Anchored interactive surface for settings, filters, pickers, and details — OverlayStack placement, non-modal default with explicit modal policy, presentation contraction, and header/body/footer slots without forcing Panel.',
    primaryStory: 'popover/basic',
    usage: `use ratatui_core::layout::Rect;
use termrock::interaction::{OverlaySize, OverlayStack};
use termrock::style::DesignSystem;
use termrock::widgets::{
    Popover, PopoverState, open_popover_overlay, POPOVER_OVERLAY_ID,
};

let system = DesignSystem::default();
let mut stack = OverlayStack::<&str>::new();
let bounds = Rect::new(0, 0, 80, 24);
let anchor = Rect::new(10, 5, 8, 1);
let size = OverlaySize::menu(28, 10);
let mut state = PopoverState::new();
let _ = state.open_on_stack(&mut stack, bounds, anchor, size, Some("trigger"));
// Paint: Popover::new("Settings", &system).paint(entry.rect, buf, &mut state);
// Host fills state.slots().body. Dismiss via stack → opener focus restored.`,
  },
  PromptComposer: {
    description: 'Flagship agent prompt composer with chips, policy, and completion overlays.',
    primaryStory: 'prompt-composer/basic',
    usage: `use termrock::{style::DesignTokens, Theme, widgets::{PromptComposer, PromptComposerState}};

let tokens = DesignTokens::default();
let theme = Theme::default();
let mut state = PromptComposerState::new();
let composer = PromptComposer::new(&tokens, &theme);`,
  },
  QuestionFlow: {
    description: 'Multi-step interview flow with option selection outcomes.',
    primaryStory: 'question-flow/basic',
    usage: `use termrock::{style::DesignTokens, widgets::{QuestionFlow, QuestionOption, QuestionStep}};

let tokens = DesignTokens::default();
let opts = [QuestionOption { id: "y", label: "Yes" }];
let steps = [QuestionStep { id: "q1", prompt: "Continue?", options: &opts, required: true }];
let flow = QuestionFlow::new(&steps, &tokens);`,
  },
  SeparatorLine: {
    description: 'Horizontal or vertical separator rule using border role paint.',
    primaryStory: 'separator/basic',
    usage: `use termrock::{style::DesignTokens, widgets::SeparatorLine};

let tokens = DesignTokens::default();
let sep = SeparatorLine::horizontal(&tokens);`,
  },
  SessionPicker: {
    description: 'Session list picker over borrowed session projections.',
    primaryStory: 'session-picker/basic',
    usage: `use termrock::{style::DesignTokens, widgets::{SessionItem, SessionPicker}};

let tokens = DesignTokens::default();
let sessions = [SessionItem { id: "s1", title: "Main", meta: None }];
let picker = SessionPicker::new(&sessions, &tokens);`,
  },
  Surface: {
    description:
      'Lowest-level visual ownership: fill, padding, border, clip, and hit geometry with canvas→destructive recipes.',
    primaryStory: 'surface/ladder',
    usage: `use termrock::{style::DesignSystem, widgets::{Surface, SurfaceRecipe, SurfaceFill}};

let system = DesignSystem::default();
let content = Surface::new(&system)
    .recipe(SurfaceRecipe::Focused)
    .paint(area, buf);
// Children paint only inside content (clip contract).
// Canvas / terminal-default:
Surface::new(&system).recipe(SurfaceRecipe::Canvas).fill(SurfaceFill::TerminalDefault);`,
  },
  TaskRail: {
    description: 'Titled task list rail composing Panel + List.',
    primaryStory: 'task-rail/basic',
    usage: `use ratatui_core::text::Line;
use termrock::{style::DesignTokens, widgets::{ListRow, RowRole, TaskRail}};

let tokens = DesignTokens::default();
let rows = [ListRow { id: "t1", label: Line::from("Task"), leading: None, secondary: None, badge: None, shortcut: None, trailing: None, role: RowRole::Item, enabled: true, loading: false }];
let rail = TaskRail::new(&rows, &tokens, "Tasks");`,
  },
} as const satisfies Record<string, ComponentDoc>
