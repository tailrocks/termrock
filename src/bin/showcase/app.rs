use std::time::Duration;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;

use crate::inspector::{self, Inspector};
use crate::options::{OptSet, ShellOptions};
use crate::pages::adapter::Showcase;
use crate::pages::{
    Closed, Cx, DIALOG_TAG, Go, HELP_TAG, LayerRes, Page, PageEvent, Report, Request,
};
use junie_tui::clock::{Clock, WallClock};
use junie_tui::core::event::{Input, Key, Outcome};
use junie_tui::core::focus::FocusChange;
use junie_tui::core::id::WidgetId;
use junie_tui::core::pointer::{
    FocusOn, MouseTarget, PointerConfig, PointerEvent, PointerHost, PointerParts, PointerRouter,
};
use junie_tui::core::response::Response;
use junie_tui::shell::{self, Page as _, Shell};
use junie_tui::theme::{Theme, Tone};
use junie_tui::ui::ctx::{RenderCtx, fill};
use junie_tui::ui::layer::{
    DismissReason, Dismissed, Layer, LayerCx, LayerEntry, LayerPointer, LayerTag, ShellLayers,
    Taken,
};
use junie_tui::ui::layers::LayerResult;
use junie_tui::ui::leave::{LeaveGuard, Resolved, Stakes};
use junie_tui::ui::popup::Dismiss;
use junie_tui::widgets::breadcrumb::Breadcrumb;
use junie_tui::widgets::dialog::{Dialog, DialogResult};
use junie_tui::widgets::form_dialog::FormDialog;
use junie_tui::widgets::help::HelpOverlay;
use junie_tui::widgets::too_small::TooSmall;
use junie_tui::widgets::tree::{TreeAction, TreeNode, TreeView};

const BRAND: &str = "Junie Design system";

const NAV: WidgetId = WidgetId::of("app.nav");
const HEADER_HELP: WidgetId = WidgetId::of("app.header.help");

const HEADER_CRUMB: WidgetId = WidgetId::of("app.header.crumb");
const HEADER_INSPECT: WidgetId = WidgetId::of("app.header.inspect");
const HELP_DIALOG: WidgetId = WidgetId::of("app.help");

pub const HEADER_BRAND: WidgetId = WidgetId::of("app.header.brand");

pub const QUIT_DIALOG: WidgetId = WidgetId::of("app.shell.quit");

pub const INSPECTOR_DOCK_MIN: u16 = 100;

const INSPECTOR_W: u16 = 30;

const INSPECTOR_OVERLAY_W: u16 = 34;

const LEVELS: [junie_tui::theme::ColorLevel; 4] = [
    junie_tui::theme::ColorLevel::TrueColor,
    junie_tui::theme::ColorLevel::Ansi256,
    junie_tui::theme::ColorLevel::Ansi16,
    junie_tui::theme::ColorLevel::Mono,
];

fn level_index(level: junie_tui::theme::ColorLevel) -> usize {
    LEVELS.iter().position(|l| *l == level).unwrap_or(0)
}

const CHARSETS: [junie_tui::theme::Charset; 2] = [
    junie_tui::theme::Charset::Unicode,
    junie_tui::theme::Charset::Ascii,
];

fn charset_index(matches: impl Fn(junie_tui::theme::Charset) -> bool) -> usize {
    CHARSETS.iter().position(|c| matches(*c)).unwrap_or(0)
}

pub use junie_tui::motion::Motion;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageId {
    Overview,
    Buttons,
    Forms,
    Inputs,
    TextAreas,
    Panels,
    Sidebars,
    Dialogs,
    Tables,
    Editable,
    Lists,
    Trees,
    Progress,
    Scrolling,
    Terminal,
    TerminalPane,
    TerminalViewport,
    ListDetail,
    FixedDialogs,
    FixedControls,
    Editor,
    Diff,
    Grid,
    Chips,
    Chrome,
    Pickers,
    Settings,
    TaskRunner,
    Selectable,
    CellEditors,

    StatusHints,
    BreadcrumbsRails,
    HelpKeys,
    QuitLeave,
    FocusNavigation,
    ChecksRadios,
    Toolbars,
    Columns,
    Tabs,
    Menus,
    DialogKinds,
    PopupsTooltips,
    FileBrowser,
    EmptyError,
    Meters,
    Notices,
    SplitsStacks,
    MasterDetail,
    PaneTree,
    TextUtilities,
    QueryWorkbench,
    Connections,
    Finder,
    DiskCleanup,
    Activity,
    Workspaces,
    AccountsUsage,
    LaunchCockpit,
    ReadOnlyReport,
}

pub struct NavEntry {
    pub id: PageId,
    pub label: &'static str,

    pub group: &'static str,
}

pub const NAV_GROUPS: &[&str] = &[
    "App shell",
    "Inputs & forms",
    "Tables",
    "Navigation",
    "Overlays",
    "Feedback",
    "Layout",
    "Text & code",
    "Screens",
];

pub const HIDDEN_ENTRIES: &[NavEntry] = &[
    NavEntry {
        id: PageId::TerminalPane,
        label: "Terminal pane",
        group: "Text & code",
    },
    NavEntry {
        id: PageId::FixedControls,
        label: "Fixed controls",
        group: "Inputs & forms",
    },
    NavEntry {
        id: PageId::TerminalViewport,
        label: "Terminal viewport",
        group: "Text & code",
    },
    NavEntry {
        id: PageId::ListDetail,
        label: "List detail",
        group: "Layout",
    },
    NavEntry {
        id: PageId::FixedDialogs,
        label: "Fixed dialogs",
        group: "Overlays",
    },
];

pub fn entries() -> impl Iterator<Item = &'static NavEntry> {
    NAV_ENTRIES.iter().chain(HIDDEN_ENTRIES)
}

pub const NAV_ENTRIES: &[NavEntry] = &[
    NavEntry {
        id: PageId::Overview,
        label: "Overview",
        group: "",
    },
    NavEntry {
        id: PageId::Chrome,
        label: "Chrome",
        group: "App shell",
    },
    NavEntry {
        id: PageId::StatusHints,
        label: "Status & hints",
        group: "App shell",
    },
    NavEntry {
        id: PageId::BreadcrumbsRails,
        label: "Breadcrumbs & rails",
        group: "App shell",
    },
    NavEntry {
        id: PageId::HelpKeys,
        label: "Help & keys",
        group: "App shell",
    },
    NavEntry {
        id: PageId::QuitLeave,
        label: "Quit & leave",
        group: "App shell",
    },
    NavEntry {
        id: PageId::FocusNavigation,
        label: "Focus & navigation",
        group: "App shell",
    },
    NavEntry {
        id: PageId::Buttons,
        label: "Buttons",
        group: "Inputs & forms",
    },
    NavEntry {
        id: PageId::Inputs,
        label: "Inputs",
        group: "Inputs & forms",
    },
    NavEntry {
        id: PageId::TextAreas,
        label: "Text areas",
        group: "Inputs & forms",
    },
    NavEntry {
        id: PageId::Chips,
        label: "Chips & selects",
        group: "Inputs & forms",
    },
    NavEntry {
        id: PageId::Forms,
        label: "Forms",
        group: "Inputs & forms",
    },
    NavEntry {
        id: PageId::ChecksRadios,
        label: "Checks & radios",
        group: "Inputs & forms",
    },
    NavEntry {
        id: PageId::Toolbars,
        label: "Toolbars",
        group: "Inputs & forms",
    },
    NavEntry {
        id: PageId::Tables,
        label: "Basic table",
        group: "Tables",
    },
    NavEntry {
        id: PageId::Editable,
        label: "Editable",
        group: "Tables",
    },
    NavEntry {
        id: PageId::Grid,
        label: "Data grid",
        group: "Tables",
    },
    NavEntry {
        id: PageId::Selectable,
        label: "Selection",
        group: "Tables",
    },
    NavEntry {
        id: PageId::CellEditors,
        label: "Cell editors",
        group: "Tables",
    },
    NavEntry {
        id: PageId::Columns,
        label: "Columns",
        group: "Tables",
    },
    NavEntry {
        id: PageId::Lists,
        label: "Lists",
        group: "Navigation",
    },
    NavEntry {
        id: PageId::Trees,
        label: "Trees",
        group: "Navigation",
    },
    NavEntry {
        id: PageId::Sidebars,
        label: "Sidebars",
        group: "Navigation",
    },
    NavEntry {
        id: PageId::Tabs,
        label: "Tabs",
        group: "Navigation",
    },
    NavEntry {
        id: PageId::Menus,
        label: "Menus",
        group: "Navigation",
    },
    NavEntry {
        id: PageId::Dialogs,
        label: "Dialogs",
        group: "Overlays",
    },
    NavEntry {
        id: PageId::Pickers,
        label: "Pickers",
        group: "Overlays",
    },
    NavEntry {
        id: PageId::DialogKinds,
        label: "Dialog kinds",
        group: "Overlays",
    },
    NavEntry {
        id: PageId::PopupsTooltips,
        label: "Popups & tooltips",
        group: "Overlays",
    },
    NavEntry {
        id: PageId::FileBrowser,
        label: "File browser",
        group: "Overlays",
    },
    NavEntry {
        id: PageId::Progress,
        label: "Progress",
        group: "Feedback",
    },
    NavEntry {
        id: PageId::EmptyError,
        label: "Empty & error",
        group: "Feedback",
    },
    NavEntry {
        id: PageId::Meters,
        label: "Meters",
        group: "Feedback",
    },
    NavEntry {
        id: PageId::Notices,
        label: "Notices",
        group: "Feedback",
    },
    NavEntry {
        id: PageId::Panels,
        label: "Panels",
        group: "Layout",
    },
    NavEntry {
        id: PageId::Scrolling,
        label: "Scrolling",
        group: "Layout",
    },
    NavEntry {
        id: PageId::SplitsStacks,
        label: "Splits & stacks",
        group: "Layout",
    },
    NavEntry {
        id: PageId::MasterDetail,
        label: "Master-detail",
        group: "Layout",
    },
    NavEntry {
        id: PageId::PaneTree,
        label: "Pane tree",
        group: "Layout",
    },
    NavEntry {
        id: PageId::Editor,
        label: "Code editor",
        group: "Text & code",
    },
    NavEntry {
        id: PageId::Diff,
        label: "Diff",
        group: "Text & code",
    },
    NavEntry {
        id: PageId::Terminal,
        label: "Terminal",
        group: "Text & code",
    },
    NavEntry {
        id: PageId::TextUtilities,
        label: "Text utilities",
        group: "Text & code",
    },
    NavEntry {
        id: PageId::Settings,
        label: "Settings",
        group: "Screens",
    },
    NavEntry {
        id: PageId::TaskRunner,
        label: "Task runner",
        group: "Screens",
    },
    NavEntry {
        id: PageId::QueryWorkbench,
        label: "Query workbench",
        group: "Screens",
    },
    NavEntry {
        id: PageId::Connections,
        label: "Connections",
        group: "Screens",
    },
    NavEntry {
        id: PageId::Finder,
        label: "Finder",
        group: "Screens",
    },
    NavEntry {
        id: PageId::DiskCleanup,
        label: "Disk cleanup",
        group: "Screens",
    },
    NavEntry {
        id: PageId::Activity,
        label: "Activity",
        group: "Screens",
    },
    NavEntry {
        id: PageId::Workspaces,
        label: "Workspaces",
        group: "Screens",
    },
    NavEntry {
        id: PageId::AccountsUsage,
        label: "Accounts & usage",
        group: "Screens",
    },
    NavEntry {
        id: PageId::LaunchCockpit,
        label: "Launch cockpit",
        group: "Screens",
    },
    NavEntry {
        id: PageId::ReadOnlyReport,
        label: "Read-only report",
        group: "Screens",
    },
];

pub const PAGE_ALIASES: &[(&str, PageId)] = &[
    ("tables", PageId::Tables),
    ("editabletables", PageId::Editable),
    ("selectabletable", PageId::Selectable),
    ("optioneditors", PageId::CellEditors),
    ("options", PageId::CellEditors),
    ("enums", PageId::CellEditors),
    ("choices", PageId::CellEditors),
    ("selection", PageId::Selectable),
    ("bulkedit", PageId::Selectable),
    ("selectable", PageId::Selectable),
    ("hints", PageId::StatusHints),
    ("breadcrumbs", PageId::BreadcrumbsRails),
    ("rails", PageId::BreadcrumbsRails),
    ("help", PageId::HelpKeys),
    ("quit", PageId::QuitLeave),
    ("leave", PageId::QuitLeave),
    ("focus", PageId::FocusNavigation),
    ("checks", PageId::ChecksRadios),
    ("radios", PageId::ChecksRadios),
    ("popups", PageId::PopupsTooltips),
    ("tooltips", PageId::PopupsTooltips),
    ("empty", PageId::EmptyError),
    ("splits", PageId::SplitsStacks),
    ("text", PageId::TextUtilities),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NavExpand {
    #[default]
    CurrentGroup,

    Accordion,

    All,
}

impl PageId {
    pub fn index(self) -> usize {
        entries().position(|e| e.id == self).unwrap_or(0)
    }

    pub fn entry(self) -> &'static NavEntry {
        entries().nth(self.index()).unwrap_or(&NAV_ENTRIES[0])
    }

    pub fn nav_path(self) -> Vec<usize> {
        let e = self.entry();
        let roots = nav_roots();
        if HIDDEN_ENTRIES.iter().any(|h| h.id == self) {
            return vec![
                roots
                    .iter()
                    .position(|r| *r == NavRoot::Group(e.group))
                    .unwrap_or(0),
            ];
        }
        if e.group.is_empty() {
            return vec![
                roots
                    .iter()
                    .position(|r| *r == NavRoot::Page(self))
                    .unwrap_or(0),
            ];
        }
        let g = roots
            .iter()
            .position(|r| *r == NavRoot::Group(e.group))
            .unwrap_or(0);
        let k = NAV_ENTRIES
            .iter()
            .filter(|x| x.group == e.group)
            .position(|x| x.id == self)
            .unwrap_or(0);
        vec![g, k]
    }

    pub fn from_nav_path(path: &[usize]) -> Option<Self> {
        let roots = nav_roots();
        match (path, path.first().and_then(|&r| roots.get(r))) {
            ([_], Some(NavRoot::Page(id))) => Some(*id),
            ([_, k], Some(NavRoot::Group(g))) => NAV_ENTRIES
                .iter()
                .filter(|e| e.group == *g)
                .nth(*k)
                .map(|e| e.id),
            _ => None,
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        let norm = |s: &str| -> String {
            s.chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .map(|c| c.to_ascii_lowercase())
                .collect()
        };
        let n = norm(name);
        entries()
            .find(|e| norm(e.label) == n)
            .map(|e| e.id)
            .or_else(|| {
                PAGE_ALIASES
                    .iter()
                    .find(|(alias, _)| *alias == n)
                    .map(|&(_, id)| id)
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavRoot {
    Page(PageId),

    Group(&'static str),
}

pub fn nav_roots() -> Vec<NavRoot> {
    let mut out = Vec::new();
    for e in NAV_ENTRIES {
        let root = if e.group.is_empty() {
            NavRoot::Page(e.id)
        } else {
            NavRoot::Group(e.group)
        };
        if !out.contains(&root) {
            out.push(root);
        }
    }
    out
}

fn nav_tree() -> TreeView {
    let nodes = nav_roots()
        .into_iter()
        .map(|root| match root {
            NavRoot::Page(id) => TreeNode::leaf(NAV_ENTRIES[id.index()].label),
            NavRoot::Group(g) => TreeNode::dir(
                g,
                NAV_ENTRIES
                    .iter()
                    .filter(|e| e.group == g)
                    .map(|e| TreeNode::leaf(e.label))
                    .collect(),
            ),
        })
        .collect::<Vec<_>>();
    debug_assert_eq!(
        nodes.iter().filter(|n| !n.children.is_empty()).count(),
        NAV_GROUPS.len()
    );

    TreeView::new(NAV, nodes)
        .click(junie_tui::core::selection::ClickPolicy::NAVIGATION)
        .type_ahead(true)
        .selected_marker(Some("›"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shown {
    pub page: PageId,
    focus: Option<WidgetId>,
}

impl Shown {
    fn new(page: PageId) -> Self {
        Self { page, focus: None }
    }
}

pub struct App {
    pub theme: Theme,
    pub pages: Vec<Box<dyn Page>>,

    pub router: shell::Router<Shown>,

    crumb: Breadcrumb,

    pub nav: TreeView,

    pub nav_expand: NavExpand,

    nav_auto: Option<usize>,

    pub shell: Shell,

    pub(crate) clock: WallClock,

    pub layers: Layers,
    pub inspector: bool,

    pub insp: Inspector,

    pub options: ShellOptions,

    pub track_click: junie_tui::core::scroll::TrackClick,

    pub tooltips: junie_tui::ui::tooltip::Tooltips,

    pub reveal: junie_tui::ui::reveal::Reveal,

    pub input: junie_tui::core::event::InputConfig,

    pub keyboard_protocol: junie_tui::core::keyboard::KeyboardProtocol,

    alt_keys: usize,

    inspector_focus: Option<WidgetId>,
    pub tick: u64,
    pub last_key: Option<String>,

    pub keys: shell::ShellKeys<ShowCmd>,

    leave: LeaveGuard<()>,
    nav_reveal: bool,

    pub nav_filter: Option<String>,
    layout: ShellLayout,
}

pub type Layers = ShellLayers<LayerRes, (), Option<usize>>;
type LayerSlot = LayerEntry<LayerRes, (), Option<usize>>;

fn is_window_layer(e: &LayerSlot) -> bool {
    e.owner.is_none() || matches!(e.tag.kind, DIALOG_TAG | HELP_TAG)
}

fn closed_report(entry: LayerSlot, result: Option<LayerResult>) -> Report {
    Report {
        result,
        values: entry.get::<FormDialog>().map(FormDialog::values),
        closed: Some(Closed(std::rc::Rc::from(entry.layer))),
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct ShellLayout {
    header: Rect,
    sidebar: Rect,
    main: Rect,
    inspector: Rect,
    footer: Rect,
}

impl App {
    #[allow(dead_code)]
    pub fn sidebar_area(&self) -> Rect {
        self.layout.sidebar
    }

    #[allow(dead_code)]
    pub fn new(theme: Theme) -> Self {
        Self::with_motion(theme, Motion::Full, 0)
    }

    pub fn with_motion(theme: Theme, motion: Motion, frame: u64) -> Self {
        use crate::pages::*;
        let pages: Vec<Box<dyn Page>> = entries()
            .map(|e| -> Box<dyn Page> {
                match e.id {
                    PageId::Overview => Box::new(overview::OverviewPage::new()),
                    PageId::Buttons => Box::new(buttons::ButtonsPage::new()),
                    PageId::Forms => Box::new(forms::FormsPage::new()),
                    PageId::Inputs => Box::new(inputs::InputsPage::new()),
                    PageId::TextAreas => Box::new(textareas::TextAreasPage::new()),
                    PageId::Panels => Box::new(panels::PanelsPage::new()),
                    PageId::Sidebars => Box::new(sidebars::SidebarsPage::new()),
                    PageId::Dialogs => Box::new(dialogs::DialogsPage::new()),
                    PageId::Tables => Box::new(tables::TablesPage::new()),
                    PageId::Editable => Box::new(editable::EditablePage::new()),
                    PageId::Lists => Box::new(lists::ListsPage::new()),
                    PageId::Trees => Box::new(trees::TreesPage::new()),
                    PageId::Progress => Box::new(progress::ProgressPage::new()),
                    PageId::Scrolling => Box::new(scrolling::ScrollingPage::new()),
                    PageId::Terminal => Box::new(terminal::TerminalPage::new()),
                    PageId::TerminalPane => Box::new(terminal_pane::TerminalPanePage::new()),
                    PageId::TerminalViewport => {
                        Box::new(terminal_viewport::TerminalViewportPage::new())
                    }
                    PageId::FixedControls => fixed_controls::page(),
                    PageId::ListDetail => list_detail::page(),
                    PageId::FixedDialogs => fixed_dialogs::page(),
                    PageId::Editor => Box::new(editor::EditorPage::new()),
                    PageId::Diff => Box::new(diff::DiffPage::new()),
                    PageId::Grid => Box::new(grid::GridPage::new()),
                    PageId::Chips => Box::new(chips::ChipsPage::new()),
                    PageId::Pickers => Box::new(pickers::PickersPage::new()),
                    PageId::Chrome => Box::new(chrome::ChromePage::new()),
                    PageId::Settings => Box::new(settings::SettingsPage::new()),
                    PageId::TaskRunner => Box::new(taskrunner::TaskRunnerPage::new()),
                    PageId::Selectable => Box::new(selectable::SelectablePage::new()),
                    PageId::CellEditors => Box::new(celleditors::CellEditorsPage::new()),
                    PageId::StatusHints => status_hints::page(),
                    PageId::BreadcrumbsRails => breadcrumbs_rails::page(),
                    PageId::HelpKeys => help_keys::page(),
                    PageId::QuitLeave => quit_leave::page(),
                    PageId::FocusNavigation => focus_navigation::page(),
                    PageId::ChecksRadios => checks_radios::page(),
                    PageId::Toolbars => toolbars::page(),
                    PageId::Columns => columns::page(),
                    PageId::Tabs => tabs::page(),
                    PageId::Menus => menus::page(),
                    PageId::DialogKinds => dialog_kinds::page(),
                    PageId::PopupsTooltips => popups_tooltips::page(),
                    PageId::FileBrowser => file_browser::page(),
                    PageId::EmptyError => empty_error::page(),
                    PageId::Meters => meters::page(),
                    PageId::Notices => notices::page(),
                    PageId::SplitsStacks => splits_stacks::page(),
                    PageId::MasterDetail => master_detail::page(),
                    PageId::PaneTree => pane_tree::page(),
                    PageId::TextUtilities => text_utilities::page(),
                    PageId::QueryWorkbench => query_workbench::page(),
                    PageId::Connections => connections::page(),
                    PageId::Finder => finder::page(),
                    PageId::DiskCleanup => disk_cleanup::page(),
                    PageId::Activity => activity::page(),
                    PageId::Workspaces => workspaces::page(),
                    PageId::AccountsUsage => accounts_usage::page(),
                    PageId::LaunchCockpit => launch_cockpit::page(),
                    PageId::ReadOnlyReport => readonly_report::page(),
                }
            })
            .collect();
        let mut shell = Shell::new();
        shell.focus.focus(NAV);
        shell.pointer = PointerRouter::new(PointerConfig::DEFAULT);
        shell.motion = motion;
        let mut options = ShellOptions::default();
        options.set("shell.color", level_index(theme.level));

        options.set(
            "shell.glyphs",
            charset_index(|c| c.glyphs() == theme.glyphs),
        );
        options.set(
            "shell.strings",
            charset_index(|c| c.strings() == theme.strings),
        );
        options.set(
            "shell.motion",
            match motion {
                Motion::Full => 0,
                Motion::Reduced => 1,
                Motion::Paused => 2,
            },
        );
        let mut app = Self {
            theme,
            pages,
            router: shell::Router::new(Shown::new(PageId::Overview)),
            crumb: Breadcrumb::new(HEADER_CRUMB, vec![]),
            nav: nav_tree(),
            nav_expand: NavExpand::default(),
            nav_auto: None,
            shell,
            clock: WallClock::for_run(),
            layers: Layers::new(),
            inspector: false,
            insp: Inspector::new(PageId::Overview),
            options,
            track_click: junie_tui::core::scroll::TrackClick::Page,
            tooltips: junie_tui::ui::tooltip::Tooltips::default(),
            reveal: junie_tui::ui::reveal::Reveal::default(),
            input: junie_tui::core::event::InputConfig::default(),
            keyboard_protocol: junie_tui::core::keyboard::KeyboardProtocol::current(),
            alt_keys: 0,
            inspector_focus: None,
            tick: frame,
            last_key: None,
            keys: shell_keys(),
            leave: LeaveGuard::new(WidgetId::of("showcase.leave")).save(false),
            nav_reveal: true,
            nav_filter: None,
            layout: ShellLayout::default(),
        };
        app.sync_nav(None);

        if motion == Motion::Paused {
            for _ in 0..frame {
                for i in 0..app.pages.len() {
                    let mut cx = Cx::new(&mut app.shell.focus, &app.shell.ring);
                    app.pages[i].on_tick(&mut (), &mut cx);
                }
            }
        }
        app
    }

    fn sync_nav(&mut self, from: Option<PageId>) {
        let path = self.page().nav_path();

        let group = (path.len() > 1).then_some(path[0]);
        let prev_auto = self.nav_auto.take();
        match (self.nav_expand, from) {
            (NavExpand::All, None) => self.nav.expand_all(),
            (NavExpand::All, Some(_)) => {}
            (NavExpand::CurrentGroup, None) | (NavExpand::Accordion, _) => {
                self.nav.expanded.clear();
            }
            (NavExpand::CurrentGroup, Some(_)) => {
                if let Some(g) = prev_auto
                    && Some(g) != group
                {
                    self.nav.expanded.remove(&vec![g]);
                }
            }
        }

        if let Some(group) = group {
            let opened_now = !self.nav.expanded.contains(&vec![group]);
            if self.nav_expand == NavExpand::CurrentGroup
                && (opened_now || prev_auto == Some(group))
            {
                self.nav_auto = Some(group);
            }
            self.nav.expanded.insert(vec![group]);
        }

        self.nav.flatten();
        self.nav.selected = Some(path.clone());
        self.nav.reveal(&path);
        self.nav_reveal = true;
    }

    pub fn set_nav_expand(&mut self, policy: NavExpand) {
        self.nav_expand = policy;
        self.nav_auto = None;
        self.sync_nav(None);
    }

    pub fn page(&self) -> PageId {
        self.router.current().page
    }

    pub fn open_page(&mut self, page: PageId) {
        if self.page() != page || self.router.depth() > 1 {
            self.navigate(shell::Nav::Reset(page));
        }
    }

    pub fn navigate(&mut self, nav: shell::Nav<PageId>) -> Outcome {
        let from = self.page();
        let focus = self.shell.focus.current();
        let pushing = matches!(nav, shell::Nav::Push(_));
        if pushing {
            self.router.current_mut().focus = focus;
        }
        if self.router.apply(nav, Shown::new).is_none() {
            return Outcome::Ignored;
        }
        self.sync_nav(Some(from));
        if !self.shell.focus.is(NAV) {
            let back = (!pushing).then(|| self.router.current_mut().focus.take());
            self.shell.focus.set(back.flatten());
        }
        Outcome::Changed
    }

    pub fn animating(&self) -> bool {
        if self.shell.motion == Motion::Paused {
            return false;
        }
        let page = &self.pages[self.page().index()];
        shell::Page::<Showcase>::animating(page.as_ref(), &())
            || self.shell.pointer.has_flash()
            || self
                .dialog()
                .is_some_and(|d| d.actions.iter().any(|b| b.busy))
    }

    pub fn tick_interval(&self) -> Duration {
        if self.shell.pointer.autoscrolling() {
            return Duration::from_millis(self.shell.pointer.config.autoscroll_ms);
        }

        if self.shell.pointer.track_repeating() {
            return Duration::from_millis(self.shell.pointer.config.track_repeat_ms);
        }
        if self.animating() {
            Duration::from_millis(80)
        } else {
            Duration::from_millis(400)
        }
    }

    fn set_status(&mut self, s: String) {
        self.shell
            .status
            .set_at(s, Tone::Secondary, self.clock.now());
    }

    fn blur_all(&mut self, ch: FocusChange) -> Outcome {
        let mut out = Outcome::Ignored;
        out = out.or(self.layers.on_blur(ch));
        out = out.or(self.insp.on_blur(Some(ch.from)));
        for i in 0..self.pages.len() {
            out = out.or(self.with_page(i, |p, cx| p.on_blur(ch, &mut (), cx)));
        }
        out
    }

    fn handle_input(&mut self, input: Input) -> Outcome {
        match input {
            Input::Resize(..) => Outcome::Changed,
            Input::Tick => self.on_tick(),
            Input::Key(key) => {
                self.last_key =
                    Some(junie_tui::core::keymap::KeyBinding::new(key.code, key.mods).label());
                shell::KeyRouter::route(self, &key)
            }
            Input::Mouse(m) => PointerRouter::route(self, m),
            Input::External(message) => self.on_external(&message),
            other => shell::KeyRouter::input(self, other),
        }
    }

    fn external_page(&self) -> Option<usize> {
        self.pages.iter().position(|p| p.external().is_some())
    }

    fn on_external(&mut self, message: &junie_tui::core::event::External) -> Outcome {
        match self.external_page() {
            Some(i) => self.with_page(i, |page, cx| page.on_external(message, cx)),
            None => Outcome::Ignored,
        }
    }

    fn on_tick(&mut self) -> Outcome {
        let auto = PointerRouter::tick(self);
        if self.shell.motion == Motion::Paused {
            return auto;
        }
        let mut out = auto;
        if self.animating() {
            if self.shell.motion.animates() {
                self.tick = self.tick.wrapping_add(1);
            }
            out = Outcome::Changed;
        }
        if self.shell.status.expire(self.clock.now()) {
            out = Outcome::Changed;
        }

        let shown = self.page().index();
        if self.layers.top().is_none_or(|e| e.owner == Some(shown)) {
            let page_out = self.with_shown(|p, cx| p.on_tick(&mut (), cx));
            out = out.or(page_out);
        }
        if let Some(top) = self.layers.top_mut() {
            let tick = top.layer.on_tick(&());
            out = out.or(self.settle_layer(tick));
        }
        out
    }

    fn with_shown(
        &mut self,
        f: impl FnOnce(&mut (dyn Page + 'static), &mut Cx) -> Outcome,
    ) -> Outcome {
        self.with_page(self.page().index(), f)
    }

    fn with_page(
        &mut self,
        i: usize,
        f: impl FnOnce(&mut (dyn Page + 'static), &mut Cx) -> Outcome,
    ) -> Outcome {
        let mut cx = Cx::new(&mut self.shell.focus, &self.shell.ring);
        let out = f(self.pages[i].as_mut(), &mut cx);
        let mut requests = cx.take();
        if i != self.page().index() {
            requests = shell::background(requests).collect();
        }
        self.apply_requests(requests, i);
        out
    }

    fn apply_requests(&mut self, requests: Vec<Request>, owner: usize) {
        for r in requests {
            match r {
                Request::Status(s, tone) => self.shell.status.set_at(s, tone, self.clock.now()),
                Request::StatusAs(s, kind) => {
                    self.shell.status.set_at(s, kind.tone(), self.clock.now());
                }
                Request::Notice(s, tone) => self.shell.status.notice_at(s, tone, self.clock.now()),
                Request::Sticky(s) => self.shell.status.sticky_at(s, self.clock.now()),
                Request::Progress(s, job) => {
                    self.shell.status.progress_at(job, s, self.clock.now());
                }
                Request::Open(layer, tag) => self.open_layer(layer, tag, Some(owner)),
                Request::Go(Go::SetOption(set)) => self.apply_opt(set),
                Request::Go(Go::Nav(nav)) => {
                    self.navigate(nav);
                }
                Request::Back => {
                    self.navigate(shell::Nav::Back);
                }
                Request::Copy(text, what) => {
                    let now = self.clock.now();
                    self.shell.copy_at(&text, what.as_deref(), now);
                }
                Request::Help => self.open_help(),
                Request::RefusedOutsideClick(dismiss) => {
                    let now = self.clock.now_ms();
                    junie_tui::ui::modal::refuse_outside_click(
                        &mut self.shell.pointer,
                        dismiss,
                        now,
                    );
                }

                Request::Close => {
                    if self.layers.top().is_some_and(|e| e.owner == Some(owner)) {
                        self.layers.pop(&mut self.shell.focus);
                    }
                }
                Request::Top(f) => {
                    if let Some(top) = self.layers.top_mut()
                        && top.owner == Some(owner)
                    {
                        f(&mut *top.layer);
                    }
                }
            }
        }
    }

    pub fn quit_stakes(&mut self) -> Stakes {
        self.pages
            .iter_mut()
            .fold(Stakes::Clean, |acc, p| acc.max(p.stakes(&())))
    }

    fn request_quit(&mut self) -> Outcome {
        let stakes = self.quit_stakes();
        match self.leave.request_quit(&stakes, &self.theme.strings) {
            Ok(_) => {
                self.shell.quit = true;
                Outcome::Consumed
            }
            Err(d) => {
                self.open_dialog(*d);
                Outcome::Changed
            }
        }
    }

    pub(crate) fn open_dialog(&mut self, d: Dialog) {
        self.open_layer(Box::new(d), LayerTag::new(DIALOG_TAG), None);
    }

    pub fn dialog(&self) -> Option<&Dialog> {
        self.layers.iter().rev().find_map(|e| e.get::<Dialog>())
    }

    #[allow(dead_code)]
    pub fn help(&self) -> Option<&HelpOverlay> {
        self.layers
            .iter()
            .rev()
            .find_map(|e| e.get::<HelpOverlay>())
    }

    fn open_layer(&mut self, layer: Box<dyn Layer<LayerRes>>, tag: LayerTag, owner: Option<usize>) {
        if tag.kind == HELP_TAG && !self.layers.is_empty() {
            self.set_status("Close the open dialog first".into());
            return;
        }
        self.layers.open(layer, tag, owner, &mut self.shell.focus);
        if let Some(d) = self.layers.top_as_mut::<Dialog>() {
            d.chrome = (d.chrome.0.max(1), d.chrome.1.max(1));
        }
        self.shell.pointer.hover = None;
        self.shell.pointer.pressed = None;
    }

    fn layer_key(&mut self, key: &Key) -> Option<Outcome> {
        if let Some(owner) = self.layers.top()?.owner {
            let out = self.with_page(owner, |p, cx| p.on_layer_key(key, cx));
            if out != Outcome::Ignored {
                return Some(self.settle_layer(out));
            }
        }
        let top = self.layers.top_mut()?;
        let mut cx = LayerCx {
            focus: &mut self.shell.focus,
            ring: &self.shell.ring,
            app: &(),
        };
        let out = top.layer.on_key(key, &mut cx);
        if out == Outcome::Ignored && key.code == KeyCode::Esc && key.mods.is_empty() {
            let dismiss = top.layer.spec().dismiss;
            return Some(
                match self
                    .layers
                    .dismiss(DismissReason::Esc, &mut self.shell.focus)
                {
                    Dismissed::Refused => Outcome::Consumed,
                    d => self.layer_dismissed(d, dismiss),
                },
            );
        }
        Some(self.settle_layer(out))
    }

    fn settle_layer(&mut self, out: Outcome) -> Outcome {
        Layers::settle(
            self,
            |app| (&mut app.layers, &mut app.shell.focus),
            out,
            |app, taken| match taken {
                Taken::Event(r) => app.layer_event(r),
                Taken::Closed(entry, r) => app.layer_closed(entry, r),
            },
        )
    }

    fn layer_event(&mut self, r: LayerRes) -> Outcome {
        let Some(top) = self.layers.top() else {
            return Outcome::Ignored;
        };
        let (Some(owner), LayerRes::Lib(result)) = (top.owner, r) else {
            return Outcome::Ignored;
        };
        let tag = top.tag.clone();
        let report = Report {
            result: Some(result),
            values: top.get::<FormDialog>().map(FormDialog::values),
            closed: None,
        };
        self.page_layer_result(owner, &tag, LayerRes::Page(report))
            .or(Outcome::Changed)
    }

    fn layer_dismissed(
        &mut self,
        d: Dismissed<LayerRes, (), Option<usize>>,
        dismiss: Dismiss,
    ) -> Outcome {
        match d {
            Dismissed::None => Outcome::Changed,
            Dismissed::Closed(entry, None) => match entry.owner {
                Some(owner) if !is_window_layer(&entry) => {
                    let tag = entry.tag.clone();
                    let report = closed_report(entry, None);
                    self.page_layer_result(owner, &tag, LayerRes::Page(report))
                        .or(Outcome::Changed)
                }
                _ => Outcome::Changed,
            },
            Dismissed::Refused => {
                let now = self.clock.now_ms();
                junie_tui::ui::modal::refuse_outside_click(&mut self.shell.pointer, dismiss, now);
                Outcome::Changed
            }
            Dismissed::Closed(entry, Some(r)) => self.layer_closed(entry, r).or(Outcome::Changed),
        }
    }

    fn layer_closed(&mut self, entry: LayerSlot, r: LayerRes) -> Outcome {
        let dialog = entry.get::<Dialog>().map(|d| d.id);
        let result = match &r {
            LayerRes::Lib(LayerResult::Dialog { action, .. }) => Some(match action {
                Some(i) => DialogResult::Action(*i),
                None => DialogResult::Cancelled,
            }),
            _ => None,
        };
        let Some(owner) = entry.owner else {
            if dialog == Some(self.leave.id) {
                if let Some(result) = result
                    && let Resolved::Discard(_) = self.leave.resolve(result)
                {
                    self.shell.quit = true;
                }
                return Outcome::Changed;
            }

            if dialog == Some(QUIT_DIALOG) && result == Some(DialogResult::Action(1)) {
                return self.request_quit();
            }
            return Outcome::Changed;
        };
        let tag = entry.tag.clone();
        let r = match (dialog, result, r) {
            (Some(id), Some(result), LayerRes::Lib(LayerResult::Dialog { text, .. })) => {
                LayerRes::Dialog {
                    id,
                    result,
                    value: text,
                }
            }
            (_, _, LayerRes::Lib(r)) if !is_window_layer(&entry) => {
                LayerRes::Page(closed_report(entry, Some(r)))
            }
            (_, _, r) => r,
        };
        self.page_layer_result(owner, &tag, r).or(Outcome::Changed)
    }

    fn page_layer_result(&mut self, i: usize, tag: &LayerTag, r: LayerRes) -> Outcome {
        self.with_page(i, |p, cx| p.on_layer_result(tag, r, &mut (), cx))
    }

    fn nav_key(&mut self, key: &Key) -> Outcome {
        if !self.shell.focus.is(NAV) {
            return Outcome::Ignored;
        }
        if self.nav_filter.is_some() {
            return self.on_filter_key(key);
        }
        if key.is_char('/') {
            self.nav_filter = Some(String::new());
            return Outcome::Changed;
        }
        self.nav_reveal = true;
        let leaf = self.nav.cursor_path().and_then(PageId::from_nav_path);
        if let Some(target) = leaf
            && matches!(key.code, KeyCode::Right | KeyCode::Char('l'))
            && key.plain()
        {
            self.open_page(target);
            self.shell.focus.set(None);
            self.shell.focus.next(&self.shell.ring);
            if self.shell.focus.is(NAV) {
                self.shell.focus.next(&self.shell.ring);
            }
            return Outcome::Changed;
        }
        self.nav_event(|nav| nav.on_key(key))
    }

    fn step_page(&mut self, step: isize) -> Outcome {
        let n = NAV_ENTRIES.len() as isize;
        let i = (self.page().index() as isize + step).rem_euclid(n) as usize;
        self.open_page(NAV_ENTRIES[i].id);
        Outcome::Changed
    }

    pub fn inspector_overlay(&self) -> bool {
        self.inspector && self.shell.size.0 < INSPECTOR_DOCK_MIN
    }

    pub fn toggle_inspector(&mut self) {
        if self.inspector {
            self.close_inspector();
        } else {
            self.inspector = true;
            self.insp.for_page(self.page());
            if self.inspector_overlay() {
                self.inspector_focus = self.shell.focus.current();
                self.shell.focus.set(Some(self.insp.tabs.id));
            }
        }
    }

    fn close_inspector(&mut self) {
        let overlay = self.inspector_overlay();
        self.inspector = false;
        let focus = self.inspector_focus.take();
        if overlay || self.insp.has_focus(self.shell.focus.current()) {
            self.shell.focus.set(focus.or(Some(NAV)));
        }
    }

    fn inspector_key(&mut self, key: &Key) -> Option<Outcome> {
        if !self.inspector {
            return None;
        }
        if self.insp.has_focus(self.shell.focus.current())
            && let Some(out) = self.dispatch_inspector(PageEvent::Key(*key))
            && out.consumed()
        {
            return Some(out);
        }
        if !self.inspector_overlay() {
            return None;
        }
        Some(match key.code {
            KeyCode::Esc => {
                self.close_inspector();
                Outcome::Changed
            }
            KeyCode::Char('i') if key.plain() => {
                self.close_inspector();
                Outcome::Changed
            }
            KeyCode::Tab => {
                self.shell.focus.next(&self.shell.ring);
                Outcome::Changed
            }
            KeyCode::BackTab => {
                self.shell.focus.prev(&self.shell.ring);
                Outcome::Changed
            }
            _ => Outcome::Consumed,
        })
    }

    fn dispatch_inspector(&mut self, ev: PageEvent) -> Option<Outcome> {
        let mut cx = Cx::new(&mut self.shell.focus, &self.shell.ring);
        let (out, set) = self.insp.handle(&ev, &mut cx)?;
        if let Some(set) = set {
            self.apply_opt(set);
            return Some(Outcome::Changed);
        }
        Some(out)
    }

    pub fn apply_opt(&mut self, set: OptSet) {
        match crate::options::page_of(set.key) {
            None => {
                self.options.set(set.key, set.value);
                self.apply_shell();
            }
            Some(page) => self.pages[page.index()].set_option(set.key, set.value),
        }
    }

    pub fn option_value(&self, opt: &crate::options::Opt) -> usize {
        match crate::options::page_of(opt.key) {
            None => self.options.get(opt.key),
            Some(page) => self.pages[page.index()]
                .option_value(opt.key)
                .unwrap_or(opt.default),
        }
    }

    fn apply_shell(&mut self) {
        use junie_tui::core::edit::{EditEntry, FieldKeys};
        use junie_tui::core::event::ClientPlatform;
        use junie_tui::core::pointer::SecondaryOutside;
        use junie_tui::core::scroll::TrackClick;
        use junie_tui::core::shape::CaretShape;
        use junie_tui::theme::{Charset, LevelOverrides};
        use junie_tui::ui::reveal::Reveal;
        use junie_tui::ui::tooltip::Tooltips;
        let o = &self.options;
        let overrides = if o.get("shell.overrides") == 1 {
            LevelOverrides::ASCII_MONO
        } else {
            LevelOverrides::default()
        };
        let mut theme = Theme::for_level_with(LEVELS[o.get("shell.color")], &overrides);

        if o.get("shell.glyphs") == 1 {
            theme = theme.with_glyphs(Charset::Ascii.glyphs());
        }
        if o.get("shell.strings") == 1 {
            theme = theme.with_strings(Charset::Ascii.strings());
        }
        theme = theme.with_card_heading(
            junie_tui::widgets::panel::CardHeading::ALL[o.get("shell.card_heading")],
        );
        theme = theme.with_inactive_selection(
            junie_tui::theme::InactiveSelection::ALL[o.get("shell.inactive_selection")],
        );
        theme = theme.with_edit_entry(
            [
                EditEntry::Immediate,
                EditEntry::TypeToEdit,
                EditEntry::Explicit,
            ][o.get("shell.edit_entry")],
        );
        let platform = [
            None,
            Some(ClientPlatform::MacOs),
            Some(ClientPlatform::Other),
        ][o.get("shell.platform")];
        let keys = match o.get("shell.field_keys") {
            1 => Some(FieldKeys::Emacs),
            2 => Some(FieldKeys::Desktop),
            _ => platform.map(FieldKeys::for_platform),
        };
        if let Some(keys) = keys {
            theme = theme.with_field_keys(keys);
        }
        self.theme = theme;
        let num = |key: &str| o.value(key).parse::<u64>().unwrap_or(0);
        let (delay, every) = [(150, 25), (300, 50), (500, 100)][o.get("shell.track_repeat_ms")];
        self.shell.pointer.config = PointerConfig::DEFAULT
            .focus_on([FocusOn::Press, FocusOn::Release][o.get("shell.focus_on")])
            .double_click_ms(num("shell.double_click_ms"))
            .double_click_slop(num("shell.double_click_slop") as u16)
            .wheel_lines(num("shell.wheel_lines") as i32)
            .hover_refresh_after_scroll(o.on("shell.hover_refresh_after_scroll"))
            .flash_ms(num("shell.flash_ms"))
            .secondary_outside(
                [
                    SecondaryOutside::Reopen,
                    SecondaryOutside::Close,
                    SecondaryOutside::Ignore,
                ][o.get("shell.secondary_outside")],
            )
            .end_edit_on_dead_space(o.on("shell.end_edit_on_dead_space"))
            .autoscroll(o.on("shell.autoscroll"))
            .autoscroll_ms(num("shell.autoscroll_ms"))
            .track_repeat(o.on("shell.track_repeat"))
            .track_repeat_ms(delay, every)
            .pointer_shapes(o.on("shell.pointer_shapes"))
            .caret_shape(
                [
                    CaretShape::Bar,
                    CaretShape::Block,
                    CaretShape::Underline,
                    CaretShape::Terminal,
                ][o.get("shell.caret_shape")],
            )
            .click_skips_animation(o.on("shell.click_skips_animation"));
        self.shell.motion = [Motion::Full, Motion::Reduced, Motion::Paused][o.get("shell.motion")];
        self.track_click = [TrackClick::Page, TrackClick::Jump][o.get("shell.track_click")];
        self.tooltips = if o.on("shell.tooltips") {
            Tooltips::default()
        } else {
            Tooltips::OFF
        };
        self.reveal = if o.on("shell.reveal") {
            Reveal::default()
        } else {
            Reveal::OFF
        };

        let mut input = junie_tui::core::event::InputConfig::for_platform(
            platform.unwrap_or_else(ClientPlatform::current),
        );
        input.shift_wheel_horizontal = o.on("shell.shift_wheel_horizontal");
        input.legacy_ctrl_punctuation = o.on("shell.legacy_ctrl_punctuation");
        self.input = input;
        {
            use junie_tui::core::keyboard::KeyboardProtocol as P;
            self.keyboard_protocol =
                [P::current(), P::On, P::Off][o.get("shell.keyboard_protocol")];
        }

        let alt = o.get("shell.alt_keys");
        if alt != self.alt_keys {
            use junie_tui::core::keyboard::{AltKeys as A, set_alt_keys};
            self.alt_keys = alt;
            set_alt_keys(match alt {
                1 => A::Delivered,
                2 => A::Refused,
                3 => A::UntilSeen,
                4 => A::WithProtocol,
                _ => A::from_env(platform.unwrap_or_else(ClientPlatform::current), |k| {
                    std::env::var(k).ok()
                }),
            });
        }
        self.shell.focus_fallback = [
            junie_tui::core::focus::FocusFallback::First,
            junie_tui::core::focus::FocusFallback::Nearest,
        ][o.get("shell.focus_fallback")];
    }

    pub fn input_config(&self) -> junie_tui::core::event::InputConfig {
        self.input
    }

    pub fn keyboard_protocol(&self) -> junie_tui::core::keyboard::KeyboardProtocol {
        self.keyboard_protocol
    }

    #[allow(dead_code)]
    pub fn live_config(&self) -> String {
        format!(
            "{:?} | {:?} | {:?} | {:?} | {:?} | {:?} | {:?} | {:?} | {:?} | {:?} | {:?} | alt keys {} {:?}",
            self.shell.pointer.config,
            self.theme.edit_entry,
            self.theme.field_keys,
            self.theme.strings,
            self.shell.motion,
            self.track_click,
            self.tooltips,
            self.reveal,
            self.input,
            self.keyboard_protocol,
            self.shell.focus_fallback,
            self.alt_keys,
            junie_tui::core::keyboard::AltKeys::current()
        )
    }

    fn open_quit(&mut self) {
        let d = Dialog::confirm(
            QUIT_DIALOG,
            "Quit the showcase?",
            "Pages keep no state between runs.",
            "Quit",
        );
        self.open_dialog(d);
    }

    fn open_help(&mut self) {
        use junie_tui::core::hints::Bindings;
        use junie_tui::widgets::help::HelpSection;
        let shell = self.keys.help_section("Shell", |c| {
            Some(match c {
                ShowCmd::Inspector => "The Inspector",
                ShowCmd::Sidebar => "Jump to navigation",
            })
        });
        let sidebar = HelpSection::new("Sidebar")
            .bindings(&self.nav.bindings())
            .row("/", "Filter (Esc clears)");
        let mouse = HelpSection::new("Mouse")
            .row("Hover", "Preview")
            .row("Click", "Focus and activate")
            .row("Wheel", "Scroll")
            .row("Drag", "A scrollbar thumb");
        let h = HelpOverlay::from_keymaps(HELP_DIALOG, "Showcase", vec![shell, sidebar, mouse])
            .title("Keyboard & mouse");
        self.show_help(h);
    }

    pub(crate) fn show_help(&mut self, h: HelpOverlay) {
        self.open_layer(Box::new(h), LayerTag::new(HELP_TAG), None);
    }

    fn modal_open(&self) -> bool {
        !self.layers.is_empty()
    }

    fn page_editing(&self) -> bool {
        match self.layers.top() {
            Some(e) if !is_window_layer(e) => e.layer.is_editing(),
            _ => self.pages[self.page().index()].editing(),
        }
    }

    fn layer_pointer(&mut self, ev: &PointerEvent) -> Option<Outcome> {
        let dismiss = self.layers.top()?.layer.spec().dismiss;
        Some(
            match self
                .layers
                .pointer(ev, &mut self.shell.focus, &self.shell.ring, &())
            {
                LayerPointer::Handled(out) => self.settle_layer(out),
                LayerPointer::Dismissed(d) => self.layer_dismissed(d, dismiss),
            },
        )
    }

    fn chrome_pointer(&mut self, ev: &PointerEvent) -> Option<Outcome> {
        use junie_tui::core::id::Part;
        use junie_tui::core::intent::Phase;
        if let Some(key) = self.shell.footer_keys.clicked(ev) {
            return Some(self.handle_input(Input::Key(key)).or(Outcome::Changed));
        }
        if ev.is(Phase::Click) {
            let done = match ev.owner() {
                HEADER_HELP => {
                    self.open_help();
                    true
                }
                HEADER_BRAND => {
                    self.open_quit();
                    true
                }
                HEADER_INSPECT => {
                    self.toggle_inspector();
                    true
                }
                _ => match self.router.crumb_click(&self.crumb, 0, ev.id) {
                    Some(nav) => self.navigate(nav).consumed(),
                    None => false,
                },
            };
            if done {
                return Some(Outcome::Changed);
            }
        }
        if !ev.on(NAV) {
            return None;
        }

        let row = self.nav.row_at(ev);
        if ev.is(Phase::Press)
            && let Some(row) = ev.index_of(Part::ROW)
        {
            self.shell.focus.focus(NAV);
            self.nav.cursor = row;
            return Some(Outcome::Changed);
        }
        if ev.is(Phase::Click) && (row.is_some() || ev.id == NAV) {
            self.shell.focus.focus(NAV);
        }
        let out = self.nav_event(|nav| nav.pointer(ev));
        Some(out)
    }

    fn inspector_pointer(&mut self, ev: &PointerEvent) -> Option<Outcome> {
        use junie_tui::core::id::Part;
        use junie_tui::core::intent::Phase;
        if !self.inspector {
            return None;
        }
        let overlay = self.inspector_overlay();
        if overlay
            && ev.is(Phase::Click)
            && ((ev.on(inspector::ID) && ev.part().part == Part::BACKDROP)
                || ev.on(junie_tui::ui::modal::CLOSE))
        {
            self.close_inspector();
            return Some(Outcome::Changed);
        }
        if let Some(out) = self.dispatch_inspector(PageEvent::Pointer(*ev)) {
            return Some(out);
        }

        overlay.then_some(Outcome::Ignored)
    }

    fn on_filter_key(&mut self, key: &Key) -> Outcome {
        let query = self.nav_filter.get_or_insert_with(String::new);
        match key.code {
            KeyCode::Esc => {
                self.end_filter();
                return Outcome::Changed;
            }
            KeyCode::Backspace => {
                query.pop();
            }
            KeyCode::Char(c) if !key.chord() => query.push(c),
            KeyCode::Enter => {
                let rows = self.nav.rows();
                let leaf = |i: usize| rows.get(i).and_then(|r| PageId::from_nav_path(&r.path));
                let target = leaf(self.nav.cursor).or_else(|| (0..rows.len()).find_map(leaf));
                if let Some(page) = target {
                    self.end_filter();
                    self.open_page(page);
                }
                return Outcome::Changed;
            }
            KeyCode::Up | KeyCode::Down | KeyCode::Home | KeyCode::End => {
                return self.nav_event(|nav| nav.on_key(key));
            }
            KeyCode::Tab | KeyCode::BackTab => {
                self.end_filter();
                return Outcome::Ignored;
            }
            _ => return Outcome::Consumed,
        }
        let q = query.clone();
        self.nav.set_filter(Some(&q));

        if let Some(i) = (0..self.nav.rows().len())
            .find(|&i| PageId::from_nav_path(&self.nav.rows()[i].path).is_some())
        {
            self.nav.cursor = i;
        }
        Outcome::Changed
    }

    fn end_filter(&mut self) {
        self.nav_filter = None;
        self.nav.set_filter(None);
        let path = self.page().nav_path();
        self.nav.reveal(&path);
        self.nav_reveal = true;
    }

    fn nav_event(&mut self, f: impl FnOnce(&mut TreeView) -> Response<TreeAction>) -> Outcome {
        let auto_open =
            |nav: &TreeView, g: Option<usize>| g.map(|g| nav.expanded.contains(&vec![g]));
        let before = auto_open(&self.nav, self.nav_auto);
        let (out, actions) = f(&mut self.nav).into_parts();
        if auto_open(&self.nav, self.nav_auto) != before {
            self.nav_auto = None;
        }
        for action in actions {
            if let TreeAction::Activate(path) = action
                && let Some(page) = PageId::from_nav_path(&path)
            {
                self.open_page(page);
                return Outcome::Changed;
            }
        }
        out
    }

    fn compute_layout(&self, area: Rect) -> ShellLayout {
        let header = Rect::new(area.x, area.y, area.width, 1);
        let footer = Rect::new(area.x, area.bottom() - 1, area.width, 1);
        let body = Rect::new(
            area.x,
            area.y + 2,
            area.width,
            area.height.saturating_sub(4),
        );
        let wide = area.width >= 110;
        let sidebar_w = if wide { 24 } else { 19 };
        let inspector_w = if self.inspector && area.width >= INSPECTOR_DOCK_MIN {
            INSPECTOR_W
        } else {
            0
        };

        let sidebar = Rect::new(body.x, body.y, sidebar_w + 1, body.height);
        let main_x = body.x + sidebar_w + 2;
        let main_w = body
            .width
            .saturating_sub(sidebar_w + 2 + inspector_w + if inspector_w > 0 { 2 } else { 0 });
        let main = Rect::new(main_x, body.y, main_w, body.height);
        let inspector = if inspector_w > 0 {
            Rect::new(main.right() + 2, body.y, inspector_w, body.height)
        } else {
            Rect::ZERO
        };
        ShellLayout {
            header,
            sidebar,
            main,
            inspector,
            footer,
        }
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let layout = self.compute_layout(area);
        self.layout = layout;
        self.draw_header(layout.header, buf, ctx);
        self.draw_sidebar(layout.sidebar, buf, ctx);
        self.draw_main(layout.main, buf, ctx);
        if !layout.inspector.is_empty() {
            self.draw_inspector(layout.inspector, buf, ctx);
        }
        self.footer()
            .render(layout.footer, buf, ctx, &mut self.shell);
        if self.inspector_overlay() {
            self.draw_inspector_overlay(area, buf, ctx);
        }

        for e in self.layers.iter_mut().filter(|e| is_window_layer(e)) {
            e.layer.render(area, buf, ctx, &());
        }
    }

    fn draw_header(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = self.theme;
        let mut x = area.x + 1;

        let brand = Rect::new(x, area.y, 7, 1);
        let lifted = ctx.interaction.hovered(HEADER_BRAND) || ctx.interaction.pressed(HEADER_BRAND);
        let ladder = ctx.title_style(HEADER_BRAND, t.canvas);
        let (mark, name) = if lifted {
            (ladder.fg(t.accent), ladder)
        } else {
            (t.accent_fg(), t.title())
        };
        buf.set_string(x, area.y, "▪", mark);
        if lifted {
            buf.set_string(x + 1, area.y, " ", ladder);
        }
        x += 2;
        buf.set_string(x, area.y, "Junie", name);
        ctx.clickable(HEADER_BRAND, brand);
        x += 6;
        buf.set_string(x, area.y, "Design system", t.secondary());
        x += 14;

        let cap = format!(
            "{} · {}×{}",
            t.level.label(),
            self.shell.size.0,
            self.shell.size.1
        );
        let mut rx = area.right().saturating_sub(1);
        let help = " ? Help ";
        let insp = if self.inspector {
            " i Inspector · on "
        } else {
            " i Inspector "
        };
        for (label, id) in [(help, HEADER_HELP), (insp, HEADER_INSPECT)] {
            let w = junie_tui::ui::text::width(label) as u16;
            rx = rx.saturating_sub(w);

            let style = ctx.title_style(id, t.canvas);
            buf.set_string(rx, area.y, label, style);
            ctx.clickable(id, Rect::new(rx, area.y, w, 1));
            rx = rx.saturating_sub(1);
        }

        let mut crumbs = self.header_crumb();
        let x = x + 1;
        crumbs.render(
            Rect::new(x, area.y, rx.saturating_sub(x + 1), 1),
            buf,
            ctx,
            t.canvas,
        );
        self.crumb = crumbs.clone();
        let crumb_w = crumbs
            .areas
            .last()
            .map_or(0, |r| r.right().saturating_sub(x));
        let cw = junie_tui::ui::text::width(&cap) as u16;
        if rx > x + crumb_w + cw + 2 {
            buf.set_string(rx.saturating_sub(cw + 1), area.y, &cap, t.faint());
        }
    }

    fn header_crumb(&self) -> Breadcrumb {
        if self.router.depth() > 1 {
            return self
                .router
                .breadcrumb(HEADER_CRUMB, |s| s.page.entry().label.to_owned());
        }
        let entry = self.page().entry();
        let segments = [entry.group, entry.label]
            .iter()
            .filter(|s| !s.is_empty())
            .map(|s| (*s).to_owned())
            .collect();
        Breadcrumb::new(HEADER_CRUMB, segments).clickable(false)
    }

    fn draw_sidebar(&mut self, card: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = self.theme;

        let count = NAV_ENTRIES.len().to_string();
        let panel = junie_tui::widgets::panel::Panel::card(Some("Components"))
            .pad_x(1)
            .focused(ctx.interaction.focused(NAV))
            .meta(&count);
        let bg = panel.bg(&t);

        ctx.clickable_under(NAV, card);
        ctx.scrollable(NAV, card);
        let Some(area) = panel.try_render(card, buf, &t) else {
            return;
        };

        let area = match &self.nav_filter {
            Some(q) if area.height > 2 => {
                let row = Rect::new(area.x, area.y, area.width, 1);
                let fs = t.field_style(junie_tui::ui::ctx::VisualState {
                    editing: true,
                    ..Default::default()
                });
                fill(buf, row, fs);
                buf.set_string(row.x + 1, row.y, "/", fs.fg(t.text_muted));
                let shown = junie_tui::ui::text::truncate(q, row.width.saturating_sub(4) as usize);
                buf.set_string(row.x + 3, row.y, &shown, fs);
                if ctx.interaction.focused(NAV) {
                    ctx.set_cursor(ratatui::layout::Position::new(
                        row.x + 3 + junie_tui::ui::text::width(&shown) as u16,
                        row.y,
                    ));
                }
                Rect::new(area.x, area.y + 1, area.width, area.height - 1)
            }
            _ => area,
        };

        let resized = self.nav.scroll.viewport_len != area.height as usize;
        self.nav.scroll.set_viewport(area.height as usize);
        if self.nav_reveal || resized {
            let cursor = self.nav.cursor;
            self.nav.scroll.ensure_visible(cursor);
            self.nav_reveal = false;
        }

        self.nav.render(area, buf, ctx, bg);
    }

    fn draw_main(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = self.theme;
        let shown = self.page();
        let i = shown.index();
        let page = self.pages[i].as_mut();

        let opts = crate::options::page_options(shown);
        if !opts.is_empty() {
            let values: Vec<usize> = opts
                .iter()
                .map(|o| page.option_value(o.key).unwrap_or(o.default))
                .collect();
            if let Some(tpl) = page.template_mut() {
                tpl.sync_options(opts, |o| {
                    opts.iter()
                        .position(|p| p.key == o.key)
                        .map_or(o.default, |i| values[i])
                });
            }
        }

        buf.set_string(area.x, area.y, page.title(), t.title());
        let tw = junie_tui::ui::text::width(page.title()) as u16;
        if area.width > tw + 4 {
            let blurb = junie_tui::ui::text::truncate(page.blurb(), (area.width - tw - 3) as usize);
            buf.set_string(area.x + tw + 2, area.y, &blurb, t.muted());
        }
        let body = Rect::new(
            area.x,
            area.y + 2,
            area.width,
            area.height.saturating_sub(2),
        );
        shell::Page::<Showcase>::render(page, body, buf, ctx, &());

        let screen = *buf.area();
        for e in self.layers.iter_mut().filter(|e| !is_window_layer(e)) {
            e.layer.render(screen, buf, ctx, &());
        }
    }

    fn state_props(&self) -> Vec<junie_tui::widgets::props::Prop> {
        use junie_tui::widgets::props::Prop;
        let mode = if self.dialog().is_some() {
            "MODAL"
        } else if self.page_editing() {
            "EDIT"
        } else {
            "NAV"
        };
        let fmt_rect = |r: Option<Rect>| match r {
            Some(r) => format!("{}·{} {}×{}", r.x, r.y, r.width, r.height),
            None => "—".to_owned(),
        };
        let focus_area = self
            .shell
            .focus
            .current()
            .and_then(|f| self.shell.hits.area_of(f));
        let hover_area = self
            .shell
            .pointer
            .hover
            .and_then(|h| self.shell.hits.area_of(h));
        vec![
            Prop::new("mode", mode),
            Prop::new("focus", fmt_rect(focus_area)),
            Prop::new(
                "hover",
                if self.shell.pointer.hover_suppressed {
                    "suppressed".into()
                } else {
                    fmt_rect(hover_area)
                },
            ),
            Prop::new(
                "pressed",
                fmt_rect(
                    self.shell
                        .pointer
                        .pressed
                        .and_then(|p| self.shell.hits.area_of(p)),
                ),
            ),
            Prop::new(
                "mouse",
                self.shell
                    .pointer
                    .pos
                    .map(|p| format!("{}·{}", p.x, p.y))
                    .unwrap_or("—".into()),
            ),
            Prop::new("last key", self.last_key.clone().unwrap_or("—".into())),
            Prop::new(
                "focus ring",
                format!("{} stops", self.shell.ring.reachable().len()),
            ),
            Prop::new("hit regions", format!("{}", self.hits_len())),
            Prop::new("tick", format!("{}", self.tick)),
            Prop::new("colors", self.theme.level.label()),
            Prop::new(
                "size",
                format!("{}×{}", self.shell.size.0, self.shell.size.1),
            ),
            Prop::new(
                "inputs",
                format!("{} · {} ticks", self.insp.inputs, self.insp.ticks),
            ),
        ]
    }

    fn render_inspector(
        &mut self,
        inner: Rect,
        bg: ratatui::style::Color,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
    ) {
        self.options
            .set("shell.color", level_index(self.theme.level));
        self.insp.for_page(self.page());
        let values: Vec<(&str, usize)> = crate::options::all()
            .map(|o| (o.key, self.option_value(o)))
            .collect();
        self.insp.options.sync(|o| {
            values
                .iter()
                .find(|(k, _)| *k == o.key)
                .map_or(o.default, |(_, v)| *v)
        });
        let state = self.state_props();
        self.insp.render(inner, buf, ctx, bg, &state);
    }

    fn draw_inspector(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = self.theme;
        let within = self.insp.has_focus(self.shell.focus.current());
        let panel = junie_tui::widgets::panel::Panel::card(Some("Inspector")).focused(within);
        let bg = panel.bg(&t);
        let inner = panel.render(area, buf, &t);
        self.render_inspector(inner, bg, buf, ctx);
    }

    fn draw_inspector_overlay(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        use junie_tui::ui::popup::{self, Dismiss, Placement};
        let body_h = area.height.saturating_sub(2);
        let w = INSPECTOR_OVERLAY_W.min(area.width);

        let anchor = Rect::new(area.right().saturating_sub(1), area.y + 1, 1, 1);
        let frame = junie_tui::ui::modal::Frame::new(w, body_h)
            .title("Inspector")
            .dismiss(Dismiss::LIGHT)
            .anchor(anchor, Placement::Left)
            .margin(ratatui::layout::Margin::new(2, 1));
        let (_, inner) = frame.render(area, buf, ctx);
        popup::backdrop(ctx, inspector::ID, Dismiss::LIGHT, area);
        let bg = self.theme.surface_elevated;
        self.render_inspector(inner, bg, buf, ctx);
    }

    fn hits_len(&self) -> usize {
        self.shell.hits.len()
    }

    pub(crate) fn footer(&self) -> shell::Footer {
        let focus = self.shell.focus.current();
        let pairs =
            |v: Vec<(&'static str, &'static str)>| -> Vec<junie_tui::widgets::keyhint::Hint> {
                v.into_iter()
                    .map(|(key, action)| junie_tui::widgets::keyhint::hint(key, action))
                    .collect()
            };
        let top = self.layers.top();
        let window_modal = self.layers.iter().any(is_window_layer);
        let footer = shell::Footer::new()
            .editing(self.page_editing() && !window_modal)
            .status(self.shell.status.get());
        if let Some(top) = top.filter(|e| is_window_layer(e)) {
            return footer.layer(Some(top.layer.hints(focus)));
        }
        if let Some(own) = top.map(|e| e.layer.hints(focus)).filter(|h| !h.is_empty()) {
            return footer.layer(Some(own));
        }
        let page = &self.pages[self.page().index()];
        let mut hints: Vec<(&'static str, &'static str)> = Vec::new();
        if let Some(own) = self.inspector.then(|| self.insp.hints(focus)).flatten() {
            hints.extend(own);
            if !self.insp.options.popup_open() {
                hints.push(("Tab", "Next"));
                if self.inspector_overlay() {
                    hints.push(("Esc", "Close"));
                } else {
                    hints.push(("i", "Close"));
                }
            }
            return footer.layer(Some(pairs(hints)));
        }
        if self.shell.focus.is(NAV) {
            if self.nav_filter.is_some() {
                hints.push(("type", "Filter"));
                hints.push(("Enter", "Open"));
                hints.push(("Esc", "Clear"));
            }
            hints.push(("↑↓", "Move"));
            let on_group = self
                .nav
                .cursor_path()
                .is_some_and(|p| PageId::from_nav_path(p).is_none());
            if on_group {
                hints.push(("←→", "Fold"));
                hints.push(("Enter", "Toggle"));
            } else if self.nav_filter.is_none() {
                hints.push(("Enter", "Open"));
            }
            if self.nav_filter.is_none() {
                hints.push(("/", "Filter"));
            }
            hints.push(("Tab", "Into page"));
            hints.push(("q", "Quit"));
            return footer.layer(Some(pairs(hints)));
        }
        footer
            .layer(Some(pairs(Page::hints(page.as_ref(), focus))))
            .tab_next(!self.page_editing())
    }
}

impl shell::App for App {
    type Status = ();
    fn shell(&self) -> &Shell {
        &self.shell
    }
    fn shell_mut(&mut self) -> &mut Shell {
        &mut self.shell
    }
    fn theme(&self) -> Theme {
        self.theme
    }
    fn too_small(&self) -> TooSmall<'_> {
        TooSmall::new(BRAND)
    }
    fn now_ms(&self) -> u64 {
        self.clock.now_ms()
    }
    fn tick(&self) -> u64 {
        self.tick
    }
    fn tick_interval(&self) -> Duration {
        App::tick_interval(self)
    }
    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        App::draw(self, area, buf, ctx)
    }

    fn configure(&self, ctx: &mut RenderCtx) {
        ctx.track_click = self.track_click;
        ctx.tooltips = self.tooltips;
        ctx.reveal = self.reveal;
    }
    fn on_input(&mut self, input: Input) -> Outcome {
        self.handle_input(input)
    }

    fn on_blur(&mut self, ch: FocusChange) -> Outcome {
        self.blur_all(ch)
    }
    fn after_input(&mut self, input: &Input, out: Outcome) -> Outcome {
        self.insp.record(input, out);

        if self.inspector && !matches!(input, Input::Tick) {
            out.or(Outcome::Changed)
        } else {
            out
        }
    }
    fn layer_open(&self) -> bool {
        self.modal_open()
    }
    fn page_under_layer(&self) -> bool {
        self.layers.hides_page_focus()
            || self.inspector_overlay()
            || self.pages[self.page().index()].layer_open()
    }
    fn input_config(&self) -> junie_tui::core::event::InputConfig {
        App::input_config(self)
    }
    fn keyboard_protocol(&self) -> junie_tui::core::keyboard::KeyboardProtocol {
        App::keyboard_protocol(self)
    }
    fn external(&self) -> Option<&junie_tui::runtime::ExternalReceiver> {
        self.pages.iter().find_map(|p| p.external())
    }
}

impl PointerHost for App {
    fn pointer_parts(&mut self) -> PointerParts<'_> {
        PointerParts {
            router: &mut self.shell.pointer,
            hits: &self.shell.hits,
            focus: &mut self.shell.focus,
            ring: &self.shell.ring,
        }
    }

    fn layer_open(&self) -> bool {
        self.modal_open() || self.inspector_overlay()
    }

    fn now_ms(&self) -> u64 {
        self.clock.now_ms()
    }

    fn deliver(&mut self, ev: &PointerEvent) -> Response<()> {
        let out = match self.layer_pointer(ev) {
            Some(out) => out,
            None => match self.chrome_pointer(ev) {
                Some(out) => out,
                None => match self.inspector_pointer(ev) {
                    Some(out) => out,
                    None => self.with_shown(|p, cx| p.pointer(ev, &mut (), cx)),
                },
            },
        };
        out.into()
    }

    fn outside(&mut self, _pos: ratatui::layout::Position) -> Response<()> {
        let Some(dismiss) = self.layers.top().map(|e| e.layer.spec().dismiss) else {
            return Response::changed();
        };
        let d = self
            .layers
            .dismiss(DismissReason::OutsideClick, &mut self.shell.focus);
        self.layer_dismissed(d, dismiss);
        Response::changed()
    }
}

pub(crate) fn describe_mods(mods: KeyModifiers) -> String {
    let mut s = String::new();
    for (m, name) in [
        (KeyModifiers::CONTROL, "Ctrl+"),
        (KeyModifiers::ALT, "Alt+"),
        (KeyModifiers::SHIFT, "Shift+"),
    ] {
        if mods.contains(m) {
            s.push_str(name);
        }
    }
    s
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShowCmd {
    Inspector,

    Sidebar,
}

pub fn shell_keys() -> shell::ShellKeys<ShowCmd> {
    use junie_tui::core::keymap::KeyBinding;
    use shell::ShellAction;
    shell::ShellKeys::new()
        .bind_fallback(KeyBinding::char(']'), ShellAction::NextPage)
        .bind_fallback(KeyBinding::char('['), ShellAction::PrevPage)
        .bind_fallback(KeyBinding::char('i'), ShellAction::App(ShowCmd::Inspector))
        .bind_fallback(KeyBinding::char('0'), ShellAction::App(ShowCmd::Sidebar))
}

impl shell::KeyHost for App {
    type Cmd = ShowCmd;

    fn shell_keys(&self) -> &shell::ShellKeys<ShowCmd> {
        &self.keys
    }

    fn layer_key(&mut self, key: &Key) -> Option<Outcome> {
        if self.layers.top().is_some() {
            return App::layer_key(self, key);
        }
        self.inspector_key(key)
    }

    fn capturing(&self) -> bool {
        self.pages[self.page().index()].is_editing()
    }

    fn dirty(&mut self) -> bool {
        !self.quit_stakes().is_clean()
    }

    fn shell_action(&mut self, action: shell::ShellAction<ShowCmd>, _key: &Key) -> Outcome {
        use shell::ShellAction;
        match action {
            ShellAction::Help => {
                self.open_help();
                Outcome::Changed
            }
            ShellAction::NextPage => self.step_page(1),
            ShellAction::PrevPage => self.step_page(-1),
            ShellAction::App(ShowCmd::Inspector) => {
                self.toggle_inspector();
                Outcome::Changed
            }
            ShellAction::App(ShowCmd::Sidebar) => {
                self.shell.focus.focus(NAV);
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    fn chrome_key(&mut self, key: &Key) -> Outcome {
        self.nav_key(key)
    }

    fn layer_paste(&mut self, text: &str) -> Option<Outcome> {
        let top = self.layers.top_mut()?;
        if top.get::<HelpOverlay>().is_some() {
            return Some(Outcome::Consumed);
        }
        Some(top.layer.on_paste_focused(text, &self.shell.focus))
    }

    fn page_paste(&mut self, text: &str) -> Outcome {
        self.with_shown(|p, cx| p.on_paste(text, &mut (), cx))
    }

    fn page_key(&mut self, key: &Key) -> Outcome {
        self.with_shown(|p, cx| p.on_key(key, &mut (), cx))
    }

    fn esc_top(&mut self) -> Outcome {
        if !self.shell.focus.is(NAV) && self.navigate(shell::Nav::Back).consumed() {
            return Outcome::Changed;
        }
        if self.shell.focus.is(NAV) {
            return Outcome::Consumed;
        }
        self.shell.focus.focus(NAV);
        Outcome::Changed
    }

    fn request_quit(&mut self) -> Outcome {
        App::request_quit(self)
    }

    fn note(&mut self, text: &str) {
        self.set_status(text.into());
    }
}
