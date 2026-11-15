use crate::domain::instance::InstanceId;
use crate::screens::Jackin;
use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::hit::{Axis, Bar};
use junie_tui::core::id::{Part, WidgetId};
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::{MouseTarget, PointerEvent};
use junie_tui::core::response::Response;
use junie_tui::core::selection::ClickPolicy;
use junie_tui::shell::Page;
use junie_tui::theme::{Theme, Tone};
use junie_tui::ui::context::ContextConfig;
use junie_tui::ui::ctx::{RenderCtx, fill};
use junie_tui::ui::layer;
use junie_tui::ui::layout::SplitDir;
use junie_tui::ui::text::{clip, truncate, truncate_middle, width};
use junie_tui::widgets::brand::Lockup;
use junie_tui::widgets::button::Button;
use junie_tui::widgets::dialog::Dialog;
use junie_tui::widgets::empty::{self, EmptyState};
use junie_tui::widgets::input::TextInput;
use junie_tui::widgets::keyhint::{Hint, hint};
use junie_tui::widgets::menu::{
    ContextMenu, MenuAction, MenuBar, MenuBarAction, MenuItem, Placement,
};
use junie_tui::widgets::meter_list::{MeterList, MeterRow};
use junie_tui::widgets::option_list::Item;
use junie_tui::widgets::picker::Picker;
use junie_tui::widgets::progress::{Meter, MeterVisual, spinner_frame};
use junie_tui::widgets::props::{self, Prop};
use junie_tui::widgets::scrollbar;
use junie_tui::widgets::segments::{self, Segment};
use junie_tui::widgets::splitter::Splitter;
use junie_tui::widgets::statusbar::{StatusBar, StatusItem};
use junie_tui::widgets::tabs::{TabAction, TabItem, Tabs};
use junie_tui::widgets::viewport_ops::ViewportOps;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};

use super::modals::{ChoiceDialog, InfoDialog, InfoResult};
use super::{Cx, Go, LayerTag, ModalResult, Screen};
use crate::backend::{ChangeSet, Effect, Ticket};
use crate::domain::account::AccountId;
use crate::domain::agent::Agent;
use crate::domain::fixed::Fixed;
use crate::domain::instance::AgentState;
use crate::domain::usage::Freshness;
use crate::domain::vocabulary::{Vocabulary, fill_in};
use crate::panes::{
    Daemon, Direction, MAX_LABEL, MIN_PANE_COLS, MIN_PANE_ROWS, PaneEffect, PaneFace, PaneId,
    PaneKey, PaneKind, PaneKindTag, PaneNode, PaneNote, PaneRequest, PasteOutcome, Seam, nearest,
};
use crate::world::{Msg, World, job};
use junie_tui::ui::layer::{DismissReason, Layer, LayerCx, LayerOut, LayerSpec};
use junie_tui::ui::modal::modal_frame;
use junie_tui::ui::text::plural;

pub const STRIP: WidgetId = WidgetId::of("capsule.strip");
pub const MENUBAR: WidgetId = WidgetId::of("capsule.menubar");
pub const TAB_MENU: WidgetId = WidgetId::of("capsule.tabmenu");
pub const STATUS: WidgetId = WidgetId::of("capsule.status");
pub const CONTEXT: WidgetId = WidgetId::of("capsule.context");
pub const USAGE_CHIP: WidgetId = WidgetId::of("capsule.chip.usage");
pub const CONTAINER_CHIP: WidgetId = WidgetId::of("capsule.chip.container");
pub const PANES: WidgetId = WidgetId::of("capsule.panes");

pub const TAKEOVER_RECONNECT: WidgetId = WidgetId::of("capsule.takeover.reconnect");

pub const TAKEOVER_LEAVE: WidgetId = WidgetId::of("capsule.takeover.leave");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    PrefixAwait,
    Select,
    Drag,
    Dialog,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Intent {
    NewTab,
    Split(SplitDir, bool),
}

pub struct CapsuleScreen {
    pub instance: InstanceId,

    pub prefix_until: Option<i64>,

    pub prefix_timeout_ms: Option<i64>,
    drag: Option<(Vec<u8>, Rect)>,
    selecting: Option<PaneId>,

    hbar_held: Option<PaneId>,
    vbar_held: Option<(PaneId, Bar)>,
    pub pane_rects: Vec<(PaneId, Rect)>,
    seams: Vec<Seam>,

    tabs: Tabs,

    menubar: MenuBar,

    tab_menu: Option<(usize, ContextMenu)>,

    pub context: ContextConfig,
    pub body: Rect,
    pending_intent: Option<Intent>,
    pending_agent: Option<Agent>,
    picker_accounts: Vec<AccountId>,

    picker_agents: Vec<Option<Agent>>,
    palette_cmds: Vec<Command>,
    pub dialog_open: bool,
    pub takeover: Option<String>,
    export_kind: Option<(bool, bool)>,
    pub redraw_flash: u8,
    pending_spawn: Option<PendingSpawn>,
    reading: Option<Reading>,
    pending_paste: Option<(PaneId, String, bool)>,
}

/// The changes the screen has asked the backend for, and where the dialog
/// that shows them returns to.
struct Reading {
    ticket: Ticket,
    from_exit: bool,
}

type PendingSpawn = (Intent, Option<Agent>, Option<AccountId>, String, i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    NewTab,
    SplitPane,
    ZoomPane,
    ExportFile,
    ExportFileReveal,
    ExportFileOpen,
    ExportUnderCursor,
    ExportUnderCursorReveal,
    ExportUnderCursorOpen,
    ExportSelected,
    ExportSelectedReveal,
    ExportSelectedOpen,
    StageClipboardPath,
    PasteHostImage,
    StageWithoutPasting,
    OpenLink,
    ClearPane,
    Usage,
    Close,
    Quit,
}

const PALETTE: [Command; 20] = [
    Command::NewTab,
    Command::SplitPane,
    Command::ZoomPane,
    Command::ExportFile,
    Command::ExportFileReveal,
    Command::ExportFileOpen,
    Command::ExportUnderCursor,
    Command::ExportUnderCursorReveal,
    Command::ExportUnderCursorOpen,
    Command::ExportSelected,
    Command::ExportSelectedReveal,
    Command::ExportSelectedOpen,
    Command::StageClipboardPath,
    Command::PasteHostImage,
    Command::StageWithoutPasting,
    Command::OpenLink,
    Command::ClearPane,
    Command::Usage,
    Command::Close,
    Command::Quit,
];

impl Command {
    fn label(self) -> &'static str {
        match self {
            Command::NewTab => "New tab",
            Command::SplitPane => "Split pane",
            Command::ZoomPane => "Zoom / unzoom pane",
            Command::ExportFile => "Export file",
            Command::ExportFileReveal => "Export file and reveal",
            Command::ExportFileOpen => "Export file and open",
            Command::ExportUnderCursor => "Export file under cursor",
            Command::ExportUnderCursorReveal => "Export file under cursor and reveal",
            Command::ExportUnderCursorOpen => "Export file under cursor and open",
            Command::ExportSelected => "Export selected file",
            Command::ExportSelectedReveal => "Export selected file and reveal",
            Command::ExportSelectedOpen => "Export selected file and open",
            Command::StageClipboardPath => "Stage image from clipboard path",
            Command::PasteHostImage => "Paste image from host clipboard",
            Command::StageWithoutPasting => "Stage image without pasting",
            Command::OpenLink => "Open link under cursor",
            Command::ClearPane => "Clear pane",
            Command::Usage => "Usage",
            Command::Close => "Close",
            Command::Quit => "Quit",
        }
    }

    fn reveals(self) -> bool {
        matches!(
            self,
            Command::ExportFileReveal
                | Command::ExportUnderCursorReveal
                | Command::ExportSelectedReveal
        )
    }

    fn opens(self) -> bool {
        matches!(
            self,
            Command::ExportFileOpen | Command::ExportUnderCursorOpen | Command::ExportSelectedOpen
        )
    }
}

impl CapsuleScreen {
    pub fn new(instance: &InstanceId, w: &mut World, pane: Option<u64>) -> Self {
        if let Some(p) = pane
            && let Some(d) = w.daemons.get_mut(instance)
            && let Some(tab) = d.active_tab_mut()
            && tab.leaves().contains(&p)
        {
            tab.focused = p;
        }
        Self {
            instance: instance.clone(),
            prefix_until: None,
            prefix_timeout_ms: None,
            drag: None,
            selecting: None,
            hbar_held: None,
            vbar_held: None,
            pane_rects: vec![],
            seams: vec![],
            tabs: Tabs::with_items(STRIP, vec![]),
            menubar: Self::build_menubar(&w.surfaces.vocabulary, w.surfaces.fixed.as_ref()),
            tab_menu: None,
            context: ContextConfig::DEFAULT,
            body: Rect::ZERO,
            pending_intent: None,
            pending_agent: None,
            picker_accounts: vec![],
            picker_agents: vec![],
            palette_cmds: vec![],
            dialog_open: false,
            takeover: None,
            export_kind: None,
            redraw_flash: 0,
            pending_spawn: None,
            reading: None,
            pending_paste: None,
        }
    }

    pub fn mode(&self) -> Mode {
        if self.dialog_open {
            Mode::Dialog
        } else if self.drag.is_some() {
            Mode::Drag
        } else if self.selecting.is_some() {
            Mode::Select
        } else if self.prefix_until.is_some() {
            Mode::PrefixAwait
        } else {
            Mode::Normal
        }
    }

    fn daemon<'a>(&self, w: &'a World) -> Option<&'a Daemon> {
        w.daemons.get(&self.instance)
    }

    fn daemon_mut<'a>(&self, w: &'a mut World) -> Option<&'a mut Daemon> {
        w.daemons.get_mut(&self.instance)
    }

    fn focused_pane(&self, w: &World) -> Option<PaneId> {
        self.daemon(w).and_then(|d| d.focused_pane())
    }

    fn account_suffix(w: &World, p: &crate::panes::Pane) -> Option<String> {
        let id = p.account.as_ref()?;
        let a = w.accounts.get(id)?;
        if w.accounts.by_provider(a.provider).count() >= 2 {
            Some(a.display_name.clone())
        } else {
            None
        }
    }

    fn layout(&mut self, body: Rect, w: &World) {
        self.pane_rects.clear();
        self.seams.clear();
        let Some(d) = self.daemon(w) else { return };
        let Some(tab) = d.active_tab() else { return };
        if let Some(z) = tab.zoomed {
            self.pane_rects.push((z, body));
            return;
        }
        let mut leaves = vec![];
        let mut seams = vec![];
        tab.root
            .layout(body, 1, &mut leaves, &mut seams, &mut vec![]);
        self.pane_rects = leaves;
        self.seams = seams;
    }

    fn framed(&self, w: &World) -> bool {
        self.daemon(w)
            .and_then(|d| d.active_tab())
            .is_some_and(|t| t.leaves().len() > 1 || t.zoomed.is_some())
    }

    fn inner_of(&self, w: &World, r: Rect) -> Rect {
        if self.framed(w) {
            r.inner(ratatui::layout::Margin::new(1, 1))
        } else {
            r
        }
    }

    fn build_menubar(words: &Vocabulary, fixed: Option<&Fixed>) -> MenuBar {
        let info_label = words.info_label();
        let reveal = MenuItem::new("Export selected file and reveal…").separator();
        let reveal = match fixed {
            Some(f) => reveal.reason(f.reveal_open.short_or_text()),
            None => reveal,
        };
        let entries: Vec<(&str, Vec<MenuItem>)> = vec![
            (
                "File",
                vec![
                    MenuItem::new("New tab").shortcut("Ctrl+B c"),
                    MenuItem::new("Split right").shortcut("Ctrl+B %"),
                    MenuItem::new("Split below")
                        .shortcut("Ctrl+B \"")
                        .separator(),
                    MenuItem::new("Export selected file…"),
                    reveal,
                    MenuItem::new("Close pane").shortcut("Ctrl+B x"),
                ],
            ),
            (
                "Edit",
                vec![
                    MenuItem::new("Copy selection").shortcut("y"),
                    MenuItem::new(&words.paste_label),
                    MenuItem::new("Clear selection").shortcut("Esc").separator(),
                    MenuItem::new("Clear pane").shortcut("Ctrl+B Ctrl+L"),
                ],
            ),
            (
                "View",
                vec![
                    MenuItem::new("Zoom pane").shortcut("Ctrl+B z"),
                    MenuItem::new("Redraw").shortcut("Ctrl+B r").separator(),
                    MenuItem::new("Usage").shortcut("Ctrl+B u"),
                    MenuItem::new(&info_label).shortcut("Ctrl+B i"),
                    MenuItem::new("GitHub context"),
                    MenuItem::new("Inspect changes"),
                ],
            ),
            (
                "Session",
                vec![
                    MenuItem::new("New tab with account…"),
                    MenuItem::new("Change tab title…")
                        .shortcut("Ctrl+B ,")
                        .separator(),
                    MenuItem::new("Detach").shortcut("Ctrl+B d"),
                    MenuItem::new("Close tab")
                        .shortcut("Ctrl+B &")
                        .danger()
                        .separator(),
                    MenuItem::new("Quit").shortcut("Ctrl+Q").danger(),
                ],
            ),
            (
                "Help",
                vec![
                    MenuItem::new("Key reference").shortcut("?"),
                    MenuItem::new("Command palette").shortcut("Ctrl+\\"),
                ],
            ),
        ];
        MenuBar::new(MENUBAR, entries).brand(Lockup::new(crate::app::BRAND_MARK))
    }

    fn open_brand_menu(&mut self, w: &World) {
        let anchor = self.menubar.brand_area;
        let items = vec![
            MenuItem::new(&w.surfaces.vocabulary.about_label),
            MenuItem::new("Usage").shortcut("Ctrl+B u").separator(),
            MenuItem::new("Workspace manager").shortcut("Ctrl+B d"),
        ];
        self.tab_menu = Some((
            usize::MAX,
            ContextMenu::new(TAB_MENU, items).anchor(anchor, Placement::Below),
        ));
    }

    fn open_pane_menu(&mut self, at: Position, w: &World) {
        let pane = self
            .focused_pane(w)
            .and_then(|p| self.daemon(w).and_then(|d| d.pane(p)));
        let selected = pane.is_some_and(|p| p.body.viewport().has_selection());
        let panes = self
            .daemon(w)
            .and_then(|d| d.active_tab())
            .map_or(1, |t| t.leaves().len());
        let zoomed = self
            .daemon(w)
            .and_then(|d| d.active_tab())
            .is_some_and(|t| t.zoomed.is_some());
        let mut copy = MenuItem::new("Copy selection");
        if pane.is_none_or(|p| p.body.viewport().keyboard_copy()) {
            copy = copy.shortcut("y");
        }
        let mut clear = MenuItem::new("Clear selection").separator();
        if !selected {
            copy = copy.reason("Select text first");
            clear = clear.reason("Nothing is selected");
        }
        let mut paste = MenuItem::new(&w.surfaces.vocabulary.paste_label);
        if w.clipboard.is_none() {
            paste = paste.reason(w.surfaces.vocabulary.paste_empty_reason.as_str());
        }
        let mut zoom = MenuItem::new("Zoom pane").shortcut("Ctrl+B z").separator();
        if zoomed {
            zoom.item.label = "Unzoom pane".into();
        } else if panes < 2 {
            zoom = zoom.reason("One pane");
        }
        let items = vec![
            copy,
            paste,
            clear,
            MenuItem::new("Split right").shortcut("Ctrl+B %"),
            MenuItem::new("Split below").shortcut("Ctrl+B \""),
            zoom,
            MenuItem::new("Close pane").shortcut("Ctrl+B x").danger(),
        ];
        self.tab_menu = Some((usize::MAX, ContextMenu::new(TAB_MENU, items).at(at)));
    }

    fn open_tab_menu(&mut self, tab: usize, w: &World) {
        let Some(area) = self.tabs.areas.get(tab).copied().filter(|r| !r.is_empty()) else {
            return;
        };
        let title = self
            .daemon(w)
            .and_then(|d| d.tabs.get(tab).map(|t| d.tab_label(t, &|_| None)))
            .unwrap_or_default();
        let items = vec![
            MenuItem::new("Change title…").shortcut("2×click"),
            MenuItem::new("Split right")
                .shortcut("Ctrl+B %")
                .separator(),
            MenuItem::new("Close tab").shortcut("Ctrl+B &").danger(),
        ];
        self.tab_menu = Some((
            tab,
            ContextMenu::new(TAB_MENU, items)
                .anchor(area, Placement::Below)
                .title(title),
        ));
    }

    fn open_rename(&mut self, tab: usize, w: &World, cx: &mut Cx) {
        let d = self.daemon(w);
        let current = d
            .and_then(|d| d.tabs.get(tab))
            .and_then(|t| t.custom_label.clone())
            .unwrap_or_default();
        let auto = d
            .and_then(|d| d.tabs.get(tab).map(|t| d.tab_label(t, &|_| None)))
            .unwrap_or_default();
        let input = TextInput::new(WidgetId::of("capsule.rename.input"), "Label")
            .value(&current)
            .placeholder(&auto)
            .help("Empty restores the automatic name · 16 max")
            .plain_label();
        let dlg = Dialog::prompt(
            WidgetId::of("capsule.rename"),
            "Change tab title",
            input,
            "Rename",
        );
        cx.open(dlg, LayerTag::new("rename").n(tab));
    }

    fn open_about(&mut self, w: &World, cx: &mut Cx) {
        let d =
            super::modals::InfoDialog::new(WidgetId::of("capsule.about"), "About", w.about_props())
                .width(64);
        cx.open(d, LayerTag::new("about"));
    }

    fn run_menu(&mut self, label: &str, tab: Option<usize>, w: &mut World, cx: &mut Cx) {
        match label {
            "New tab" => self.spawn_flow(Intent::NewTab, w, cx),
            "New tab with account…" => self.spawn_flow(Intent::NewTab, w, cx),
            "Split right" => self.spawn_flow(Intent::Split(SplitDir::Horizontal, false), w, cx),
            "Split below" => self.spawn_flow(Intent::Split(SplitDir::Vertical, false), w, cx),
            "Export selected file…" | "Export selected file and reveal…" => {
                let sel = self
                    .focused_pane(w)
                    .and_then(|p| self.daemon(w).and_then(|d| d.pane(p)))
                    .and_then(|p| p.body.viewport().selected_text());
                match sel {
                    Some(text) => {
                        let path = text.lines().next().unwrap_or("").trim().to_owned();
                        self.export_kind = Some((label.contains("reveal"), false));
                        self.finish_export(&path, w, cx);
                    }
                    None => cx.status("Select a path in a pane first"),
                }
            }
            "Close pane" => {
                let single = self
                    .daemon(w)
                    .and_then(|d| d.active_tab())
                    .is_some_and(|t| t.leaves().len() == 1);
                self.close_pane_flow(single, w, cx);
            }
            "Copy selection" => {
                let sel = self
                    .focused_pane(w)
                    .and_then(|p| self.daemon(w).and_then(|d| d.pane(p)))
                    .and_then(|p| p.body.viewport().selected_text());
                match sel {
                    Some(t) => {
                        cx.copy_as(t, "selection");
                    }
                    None => cx.status("Nothing selected"),
                }
            }
            l if l == w.surfaces.vocabulary.paste_label => {
                let text = w.clipboard.clone();
                match text {
                    Some(t) => self.paste_last_copy(&t, w, cx),
                    None => cx.status(w.surfaces.vocabulary.paste_empty.as_str()),
                }
            }
            "Clear selection" => {
                if let Some(p) = self.focused_pane(w)
                    && let Some(pane) = self.daemon_mut(w).and_then(|d| d.pane_mut(p))
                {
                    pane.body.viewport_mut().clear_selection();
                }
            }
            "Clear pane" => self.clear_pane(w, cx),
            "Zoom pane" | "Unzoom pane" => self.toggle_zoom(w, cx),
            "Redraw" => self.redraw(w, cx),
            "Usage" => self.open_usage(w, cx),
            l if l == w.surfaces.vocabulary.info_label() => self.open_container_info(w, cx),
            "GitHub context" => self.open_github(w, cx),
            "Inspect changes" => self.open_inspect(w, cx, false),
            "Change title…" | "Change tab title…" => {
                let t = tab.unwrap_or(self.daemon(w).map(|d| d.active).unwrap_or(0));
                self.open_rename(t, w, cx);
            }
            "Detach" | "Workspace manager" => self.detach(cx),
            "Close tab" => {
                if let Some(t) = tab
                    && let Some(d) = self.daemon_mut(w)
                {
                    d.active = t.min(d.tabs.len().saturating_sub(1));
                }
                self.confirm_close(true, w, cx);
            }
            "Quit" => self.request_exit(w, cx),
            "Key reference" => cx.help(),
            "Command palette" => self.open_palette(w, cx),
            l if l == w.surfaces.vocabulary.about_label => self.open_about(w, cx),
            unknown => cx.error(format!("No command is bound to {unknown}")),
        }
    }

    fn clear_pane(&mut self, w: &mut World, cx: &mut Cx) {
        if let Some(p) = self.focused_pane(w)
            && let Some(pane) = self.daemon_mut(w).and_then(|d| d.pane_mut(p))
        {
            pane.body.clear();
            cx.status(w.surfaces.vocabulary.pane_cleared.as_str());
        }
    }

    fn open_inspect(&mut self, w: &mut World, cx: &mut Cx, from_exit: bool) {
        if self.reading.is_some() {
            return;
        }
        let touched = self
            .daemon(w)
            .map(|d| d.touched_files())
            .unwrap_or_default();
        let inst = w.instance(&self.instance);
        let (uncommitted, unpushed) = (
            inst.map(|i| i.uncommitted).unwrap_or(0),
            inst.map(|i| i.unpushed).unwrap_or(0),
        );
        let ticket = w.next_ticket();
        self.reading = Some(Reading { ticket, from_exit });
        w.perform(Effect::ReadChanges {
            ticket,
            instance: self.instance.clone(),
            touched,
            uncommitted,
            unpushed,
        });
        cx.progress(job::CHANGES_READ, "Reading changes…");
    }

    fn changes_read(
        &mut self,
        ticket: Ticket,
        result: &Result<ChangeSet, String>,
        w: &World,
        cx: &mut Cx,
    ) -> Outcome {
        let Some(Reading { from_exit, .. }) = self.reading.take_if(|r| r.ticket == ticket) else {
            return Outcome::Ignored;
        };
        let changes = match result {
            Ok(changes) => changes.clone(),
            Err(error) => {
                cx.error(error.clone());
                return Outcome::Changed;
            }
        };
        if changes.is_empty() {
            cx.status("Nothing changed in this instance");
            return Outcome::Changed;
        }
        let ws = self
            .daemon(w)
            .map(|d| d.workspace.clone())
            .unwrap_or_default();
        let d = junie_tui::widgets::inspect_changes::InspectChanges::new(
            WidgetId::of("capsule.inspect"),
            &format!("Inspect changes · {ws}"),
            changes,
            junie_tui::widgets::inspect_changes::InspectMode::Compact,
        )
        .words(junie_tui::widgets::inspect_changes::ChangeWords {
            omitted: w.surfaces.vocabulary.changes_omitted.clone(),
            unread: w.surfaces.vocabulary.changes_unread.clone(),
        })
        .returns_to_exit(from_exit);
        cx.open(d, LayerTag::new("inspect"));
        Outcome::Changed
    }

    fn enter_prefix(&mut self, w: &World) {
        self.prefix_until = Some(
            self.prefix_timeout_ms
                .map_or(i64::MAX, |t| w.now_ms().saturating_add(t)),
        );
    }

    fn spawn_flow(&mut self, intent: Intent, w: &World, cx: &mut Cx) {
        self.pending_intent = Some(intent);
        let title = match intent {
            Intent::NewTab => "New tab".to_owned(),
            Intent::Split(SplitDir::Horizontal, false) => "Split → Right".into(),
            Intent::Split(SplitDir::Horizontal, true) => "Split ← Left".into(),
            Intent::Split(SplitDir::Vertical, false) => "Split ↓ Below".into(),
            Intent::Split(SplitDir::Vertical, true) => "Split ↑ Above".into(),
        };
        let mut p = Picker::new(WidgetId::of("capsule.agent"), &title);
        p.set_width(72);
        let ws = w
            .instance(&self.instance)
            .and_then(|i| i.workspace)
            .and_then(|x| w.workspace(x));
        let role = w.instance(&self.instance).and_then(|i| i.role.clone());
        let mut items = vec![];
        self.picker_agents.clear();

        let record = w.instance(&self.instance);
        let refused = |kind: PaneKind| {
            record.and_then(|i| {
                w.backend
                    .spawnable(w, i, &kind)
                    .err()
                    .map(|r| w.surfaces.vocabulary.pane.refusal(&r))
            })
        };
        for (a, mut offer) in w.offered_agents(ws, role.as_deref()) {
            if !w.surfaces.tuples.is_empty()
                && !w.surfaces.tuples.iter().any(|offer| {
                    offer.agent == a
                        && offer.provider == a.provider()
                        && offer.state == crate::domain::tuple::TupleState::Supported
                })
            {
                continue;
            }
            if offer.blocked.is_none() {
                offer.blocked = refused(PaneKind::Agent {
                    agent: a,
                    account: None,
                });
            }
            let detail = match (&offer.blocked, offer.accounts.len()) {
                (Some(why), _) => why.clone(),
                (None, 1) => offer
                    .accounts
                    .first()
                    .and_then(|id| w.accounts.get(id))
                    .map(|x| x.display_name.clone())
                    .unwrap_or_default(),
                (None, n) => format!("{n} accounts · choose at start"),
            };
            items.push(Item {
                tag: (offer.blocked.as_ref().map(|_| "blocked")).map(Into::into),
                ..Item::new(a.label())
                    .detail(detail)
                    .glyph("▪")
                    .group("agents")
                    .disabled(!offer.has_usable_account())
            });
            self.picker_agents.push(Some(a));
        }
        let shell_refused = refused(PaneKind::Shell);
        items.push(
            Item::new("Shell")
                .detail(shell_refused.clone().unwrap_or_else(|| "zsh".to_owned()))
                .glyph("$")
                .group("shells")
                .disabled(shell_refused.is_some()),
        );
        self.picker_agents.push(None);
        p.set_items(items)
            .unwrap_or_else(|error| p.report_item_error(error));
        cx.open(p, LayerTag::new("agent"));
    }

    fn continue_spawn(&mut self, agent: Option<Agent>, w: &World, cx: &mut Cx) {
        let Some(a) = agent else {
            self.spawn(None, None, w, cx);
            return;
        };
        let ws = w
            .instance(&self.instance)
            .and_then(|i| i.workspace)
            .and_then(|x| w.workspace(x));
        let role = w.instance(&self.instance).and_then(|i| i.role.clone());
        let offer = w.offer_for(a, ws, role.as_deref());
        if !offer.has_usable_account() {
            self.spawn_failed(a, offer.blocked, cx);
        } else if offer.accounts.len() == 1 {
            self.spawn(Some(a), offer.accounts.first().cloned(), w, cx);
        } else {
            self.pending_agent = Some(a);
            let (p, ids) = super::modals::account_picker(
                WidgetId::of("capsule.provider"),
                w,
                a,
                a.provider(),
                ws,
                &offer,
            );
            self.picker_accounts = ids;
            cx.open(p, LayerTag::new("provider"));
        }
    }

    fn spawn_failed(&mut self, a: Agent, blocked: Option<String>, cx: &mut Cx) {
        let why = blocked.unwrap_or_else(|| {
            format!(
                "No {} account is active for this Workspace. Enable one in the Workspace's Accounts tab or register one in Account & Usage Center.",
                a.provider().short()
            )
        });
        let mut d = Dialog::confirm(
            WidgetId::of("capsule.spawnfail"),
            &format!("{} could not start", a.label()),
            &why,
            "OK",
        );
        d.actions.remove(0);
        d.cancel_index = Some(0);
        d.initial_focus = d.actions[0].id;
        cx.open(d, LayerTag::new("spawnfail"));
    }

    fn spawn(&mut self, agent: Option<Agent>, account: Option<AccountId>, w: &World, cx: &mut Cx) {
        let intent = self.pending_intent.take().unwrap_or(Intent::NewTab);
        let now = w.now_ms();
        let ws = self
            .daemon(w)
            .map(|d| d.workspace.clone())
            .unwrap_or_default();

        if let Intent::Split(dir, _) = intent
            && let Some(f) = self.focused_pane(w)
            && let Some((_, r)) = self.pane_rects.iter().find(|(id, _)| *id == f)
        {
            let inner = self.inner_of(w, *r);
            let (need_c, need_r, have_c, have_r) = match dir {
                SplitDir::Horizontal => (
                    MIN_PANE_COLS,
                    MIN_PANE_ROWS,
                    inner.width / 2 - 2,
                    inner.height.saturating_sub(2),
                ),
                SplitDir::Vertical => (
                    MIN_PANE_COLS,
                    MIN_PANE_ROWS,
                    inner.width.saturating_sub(2),
                    inner.height / 2 - 2,
                ),
            };
            if have_c < need_c {
                cx.error(format!(
                    "Cannot split: pane would be {have_c} columns, needs {need_c}"
                ));
                return;
            }
            if have_r < need_r {
                cx.error(format!(
                    "Cannot split: pane would be {have_r} rows, needs {need_r}"
                ));
                return;
            }
        }
        if let Some(t) = self.daemon(w).and_then(|d| d.active_tab())
            && let Intent::Split(..) = intent
            && t.zoomed.is_some()
            && t.zoomed != Some(t.focused)
        {
            cx.error("Cannot split: unzoom first");
            return;
        }
        let d = w.daemons.get(&self.instance);
        let _ = d;

        self.pending_spawn = Some((intent, agent, account, ws, now));
        cx.status(w.surfaces.vocabulary.pane_starting.as_str());
    }
}

impl CapsuleScreen {
    fn apply_pending_spawn(&mut self, w: &mut World, cx: &mut Cx) {
        let Some((intent, agent, account, _ws, now)) = self.pending_spawn.take() else {
            return;
        };
        let backend = std::sync::Arc::clone(&w.backend);
        let request = PaneRequest::new(&self.instance, agent, account.clone());
        let Some(d) = self.daemon_mut(w) else { return };
        match intent {
            Intent::NewTab => {
                d.new_tab(&request, now, |spawn| backend.spawn_pane(&request, spawn));
            }
            Intent::Split(dir, first) => {
                if d.split(dir, first, &request, now, |spawn| {
                    backend.spawn_pane(&request, spawn)
                })
                .is_none()
                {
                    cx.error("Cannot split: focused pane not found");
                    return;
                }
            }
        }
        let label = agent
            .map(|a| a.label().to_owned())
            .unwrap_or("Shell".into());
        let notice =
            match account.and_then(|id| w.accounts.get(&id).map(|a| a.display_name.clone())) {
                Some(acc) => format!("Started {label} · account {acc}"),
                None => format!("Started {label}"),
            };
        cx.status(notice);
        crate::world::refresh_snapshots(w);
        w.refresh_now();
    }

    fn move_focus(&mut self, dir: Direction, w: &mut World, cx: &mut Cx) {
        let Some(from) = self.focused_pane(w) else {
            return;
        };
        if self
            .daemon(w)
            .and_then(|d| d.active_tab())
            .is_some_and(|t| t.zoomed.is_some())
        {
            cx.status("Unzoom to move focus");
            return;
        }
        if let Some(to) = nearest(&self.pane_rects, from, dir)
            && let Some(t) = self.daemon_mut(w).and_then(|d| d.active_tab_mut())
        {
            t.focused = to;
        }
    }

    fn resize(&mut self, dir: Direction, w: &mut World, cx: &mut Cx) {
        let Some(from) = self.focused_pane(w) else {
            return;
        };
        let Some(d) = self.daemon_mut(w) else { return };
        let Some(tab) = d.active_tab_mut() else {
            return;
        };
        if tab.zoomed.is_some() {
            cx.status("Unzoom to resize");
            return;
        }
        let mut path = vec![];
        if !tab.root.path_to(from, &mut path) {
            return;
        }

        let want_dir = match dir {
            Direction::Left | Direction::Right => SplitDir::Horizontal,
            Direction::Up | Direction::Down => SplitDir::Vertical,
        };
        let mut chosen: Option<(Vec<u8>, i16)> = None;
        for k in (0..path.len()).rev() {
            let node_path = &path[..k];
            let side = path[k];
            if let Some(PaneNode::Split { dir: sd, .. }) = tab.root.node_at_mut(node_path)
                && *sd == want_dir
            {
                let grows_first = matches!(dir, Direction::Right | Direction::Down);
                let delta = if (side == 0) == grows_first { 5 } else { -5 };
                chosen = Some((node_path.to_vec(), delta));
                break;
            }
        }
        let Some((p, delta)) = chosen else {
            cx.status("Nothing to resize in that direction");
            return;
        };
        let seam = self.seams.iter().find(|s| s.path == p).cloned();
        if let Some(PaneNode::Split { split, dir: sd, .. }) = tab.root.node_at_mut(&p) {
            let before = split.percent;
            split.percent = (split.percent as i16 + delta).clamp(5, 95) as u16;
            if let Some(s) = seam {
                let (a, b) = split.layout(*sd, s.container, 1);
                let too_small = match sd {
                    SplitDir::Horizontal => {
                        a.width < MIN_PANE_COLS + 2 || b.width < MIN_PANE_COLS + 2
                    }
                    SplitDir::Vertical => {
                        a.height < MIN_PANE_ROWS + 2 || b.height < MIN_PANE_ROWS + 2
                    }
                };
                if too_small {
                    split.percent = before;
                    cx.error("Cannot resize: pane at minimum size");
                }
            }
        }
    }

    fn toggle_zoom(&mut self, w: &mut World, cx: &mut Cx) {
        let Some(t) = self.daemon_mut(w).and_then(|d| d.active_tab_mut()) else {
            return;
        };
        if t.zoomed.is_some() {
            t.zoomed = None;
            cx.status("Unzoomed");
        } else if t.leaves().len() > 1 {
            t.zoomed = Some(t.focused);
            cx.status("Zoomed · z restores the layout");
        } else {
            cx.status("Nothing to zoom: one pane");
        }
    }

    fn open_palette(&mut self, w: &World, cx: &mut Cx) {
        let mut p = Picker::new(WidgetId::of("capsule.palette"), "Command palette");
        p.set_width(60);
        p.placeholder = "Type to filter commands…".into();
        p.empty_text = "No matching commands".into();
        self.palette_cmds = PALETTE.to_vec();
        p.set_items(self.palette_items("", w))
            .unwrap_or_else(|error| p.report_item_error(error));
        p.scope = Some(format!("{} of 20", PALETTE.len()));
        cx.open(p, LayerTag::new("palette"));
    }

    fn palette_items(&mut self, query: &str, w: &World) -> Vec<Item> {
        let single = self
            .daemon(w)
            .and_then(|d| d.active_tab())
            .is_some_and(|t| t.leaves().len() == 1);
        let has_sel = self
            .focused_pane(w)
            .and_then(|p| self.daemon(w).and_then(|d| d.pane(p)))
            .is_some_and(|p| p.body.viewport().has_selection());
        let q = query.to_lowercase();
        let mut out = vec![];
        self.palette_cmds.clear();
        for c in PALETTE {
            let label = if c == Command::Close && single {
                "Close tab"
            } else {
                c.label()
            };
            if !q.is_empty() && !label.to_lowercase().contains(&q) {
                continue;
            }
            let reveal_open = w
                .surfaces
                .fixed
                .as_ref()
                .map(|f| f.reveal_open.short_or_text())
                .filter(|_| c.reveals() || c.opens());
            let (detail, disabled) = match c {
                Command::NewTab => ("Ctrl+B c", false),
                Command::SplitPane => ("Ctrl+B \" %", false),
                Command::ZoomPane => ("Ctrl+B z", false),
                Command::ExportFile | Command::ExportFileReveal | Command::ExportFileOpen => {
                    ("host", false)
                }
                Command::ExportUnderCursor
                | Command::ExportUnderCursorReveal
                | Command::ExportUnderCursorOpen => ("no file under cursor", true),
                Command::ExportSelected
                | Command::ExportSelectedReveal
                | Command::ExportSelectedOpen => {
                    if has_sel {
                        ("selection", false)
                    } else {
                        ("no selection", true)
                    }
                }
                Command::StageClipboardPath
                | Command::PasteHostImage
                | Command::StageWithoutPasting => {
                    (w.surfaces.vocabulary.detail_unavailable.as_str(), true)
                }
                Command::OpenLink => ("no link under cursor", true),
                Command::ClearPane => ("Ctrl+B Ctrl+L", false),
                Command::Usage => ("Ctrl+B u", false),
                Command::Close => ("Ctrl+B x", false),
                Command::Quit => ("Ctrl+Q", false),
            };
            let (detail, disabled) = match reveal_open {
                Some(short) => (short, true),
                None => (detail, disabled),
            };
            let matched: Vec<usize> = if q.is_empty() {
                vec![]
            } else {
                label
                    .to_lowercase()
                    .find(&q)
                    .map(|p| (p..p + q.len()).collect())
                    .unwrap_or_default()
            };
            out.push(
                Item::new(label)
                    .detail(detail)
                    .glyph(" ")
                    .matched(matched)
                    .disabled(disabled),
            );
            self.palette_cmds.push(c);
        }
        out
    }

    fn run_palette(&mut self, cmd: Command, w: &mut World, cx: &mut Cx) {
        match cmd {
            Command::NewTab => self.spawn_flow(Intent::NewTab, w, cx),
            Command::SplitPane => {
                let mut p = Picker::new(WidgetId::of("capsule.split"), "Split pane");
                p.set_searchable(false)
                    .expect("disabling picker search is valid");
                p.set_width(40);
                p.set_items(
                    ["→ Right", "← Left", "↓ Below", "↑ Above"]
                        .iter()
                        .map(|l| Item::new(*l).glyph(" "))
                        .collect(),
                )
                .unwrap_or_else(|error| p.report_item_error(error));
                cx.open(p, LayerTag::new("split"));
            }
            Command::ZoomPane => self.toggle_zoom(w, cx),
            Command::ExportFile | Command::ExportFileReveal | Command::ExportFileOpen => {
                self.export_kind = Some((cmd.reveals(), cmd.opens()));
                let input = TextInput::new(WidgetId::of("capsule.export.path"), "Path")
                    .placeholder(&w.surfaces.vocabulary.export_placeholder)
                    .help("Copies to ~/Downloads/jackin/ on the host")
                    .plain_label();
                let d =
                    Dialog::prompt(WidgetId::of("capsule.export"), cmd.label(), input, "Export");
                cx.open(d, LayerTag::new("export"));
            }
            Command::ExportSelected
            | Command::ExportSelectedReveal
            | Command::ExportSelectedOpen => {
                let text = self
                    .focused_pane(w)
                    .and_then(|p| self.daemon(w).and_then(|d| d.pane(p)))
                    .and_then(|p| p.body.viewport().selected_text())
                    .unwrap_or_default();
                let path = text.lines().next().unwrap_or("").trim().to_owned();
                self.export_kind = Some((cmd.reveals(), cmd.opens()));
                self.finish_export(&path, w, cx);
            }
            Command::ExportUnderCursor
            | Command::ExportUnderCursorReveal
            | Command::ExportUnderCursorOpen
            | Command::OpenLink => {}
            Command::StageClipboardPath
            | Command::PasteHostImage
            | Command::StageWithoutPasting => {
                cx.status(format!(
                    "{}: host clipboard has no image · nothing staged",
                    cmd.label()
                ));
            }
            Command::ClearPane => self.clear_pane(w, cx),
            Command::Usage => self.open_usage(w, cx),
            Command::Close => {
                let single = self
                    .daemon(w)
                    .and_then(|d| d.active_tab())
                    .is_some_and(|t| t.leaves().len() == 1);
                if single {
                    self.close_pane_flow(true, w, cx);
                } else {
                    let mut p = Picker::new(WidgetId::of("capsule.closetarget"), "Close");
                    p.set_searchable(false)
                        .expect("disabling picker search is valid");
                    p.set_width(40);
                    p.set_items(
                        ["Close pane", "Close tab"]
                            .iter()
                            .map(|l| Item::new(*l).glyph(" "))
                            .collect(),
                    )
                    .unwrap_or_else(|error| p.report_item_error(error));
                    cx.open(p, LayerTag::new("closetarget"));
                }
            }
            Command::Quit => self.request_exit(w, cx),
        }
    }

    fn finish_export(&mut self, path: &str, w: &World, cx: &mut Cx) {
        let (reveal, open) = self.export_kind.take().unwrap_or((false, false));
        if path.is_empty() {
            cx.error("Enter a path");
            return;
        }
        if path.contains("..") || path.starts_with("/etc") {
            cx.error("Export failed: path is outside the workspace");
            return;
        }
        if path.ends_with('/') {
            cx.error("Export failed: is a directory");
            return;
        }
        if path == "cancel" {
            cx.status("Export cancelled");
            return;
        }
        let words = &w.surfaces.vocabulary;
        let workspace = self
            .daemon(w)
            .map(|d| d.workspace.as_str())
            .unwrap_or_default();
        let mut s = fill_in(
            &words.export_done,
            &[
                ("workspace", workspace),
                ("path", path.trim_start_matches('/')),
            ],
        );
        if reveal && let Some(note) = &words.export_revealed {
            s.push_str(note);
        }
        if open && let Some(note) = &words.export_opened {
            s.push_str(note);
        }
        cx.status(s);
    }

    fn confirm_close(&mut self, tab: bool, w: &World, cx: &mut Cx) {
        let words = &w.surfaces.vocabulary;
        let d = if tab {
            Dialog::destructive(
                WidgetId::of("capsule.closetab"),
                "Close tab?",
                &words.close_tab_body,
                "Close tab",
            )
        } else {
            let runs_agent = self
                .focused_pane(w)
                .and_then(|p| self.daemon(w).and_then(|d| d.pane(p)))
                .is_some_and(|p| match p.body.kind() {
                    PaneKindTag::Agent => true,
                    PaneKindTag::Shell => false,
                    PaneKindTag::Script => p.agent.is_some(),
                });
            Dialog::destructive(
                WidgetId::of("capsule.closepane"),
                "Close pane?",
                if runs_agent {
                    &words.close_agent_body
                } else {
                    &words.close_shell_body
                },
                "Close pane",
            )
        };
        cx.open(d, LayerTag::new(if tab { "closetab" } else { "closepane" }));
    }

    fn is_dirty(&self, w: &World) -> bool {
        w.instance(&self.instance).is_some_and(|i| i.is_dirty())
            || self
                .daemon(w)
                .is_some_and(|d| !d.touched_files().is_empty())
    }

    fn request_exit(&mut self, w: &World, cx: &mut Cx) {
        if self.is_dirty(w) {
            let inst = w.instance(&self.instance);
            let touched = self
                .daemon(w)
                .map(|d| d.touched_files())
                .unwrap_or_default();
            let changed = inst.map(|i| i.uncommitted).unwrap_or(0) + touched.len();
            let unpushed = inst.map(|i| i.unpushed).unwrap_or(0);
            let words = &w.surfaces.vocabulary;
            let ws = self
                .daemon(w)
                .map(|d| d.workspace.clone())
                .unwrap_or_default();
            let c = ChoiceDialog::new(
                WidgetId::of("capsule.exitdirty"),
                "Unsaved work — quit?",
                "",
                &[
                    "Start a new agent",
                    "Inspect changes",
                    &words.quit_dirty_keep,
                    &words.quit_dirty_discard,
                ],
                0,
            )
            .line(
                format!(
                    "{ws}   • {changed} changed · {unpushed} {}",
                    words.unexported_word
                ),
                Tone::Warning,
            )
            .buttons(
                vec![
                    Button::subtle(WidgetId::of("capsule.exitdirty").sub("cancel"), "Cancel"),
                    Button::primary(WidgetId::of("capsule.exitdirty").sub("ok"), "Choose"),
                ],
                0,
            )
            .option_tones(vec![Tone::Normal, Tone::Normal, Tone::Normal, Tone::Error]);
            cx.open(c, LayerTag::new("exitdirty"));
        } else {
            let mut d = Dialog::destructive(
                WidgetId::of("capsule.exit"),
                "Quit jackin❯?",
                &w.surfaces.vocabulary.quit_lines.join("\n"),
                "Quit",
            );
            d.width = 60;
            cx.open(d, LayerTag::new("exit"));
        }
    }

    fn open_usage(&mut self, w: &World, cx: &mut Cx) {
        let mut accounts: Vec<AccountId> = vec![];
        if let Some(d) = self.daemon(w) {
            for p in &d.panes {
                if let Some(a) = &p.account
                    && !accounts.contains(a)
                {
                    accounts.push(a.clone());
                }
            }
        }
        if accounts.is_empty() {
            for a in w.accounts.sorted() {
                if a.default_for_provider {
                    accounts.push(a.id.clone());
                }
            }
        }
        let focused = self
            .focused_pane(w)
            .and_then(|p| self.daemon(w).and_then(|d| d.pane(p)))
            .and_then(|p| p.account.clone());
        let mut dlg = UsageDialog::new(accounts.clone(), w);
        if let Some(f) = focused
            && let Some(i) = accounts.iter().position(|a| *a == f)
        {
            dlg.tab = i + 1;
        }
        cx.open(dlg, LayerTag::new("usage"));
    }

    fn open_container_info(&mut self, w: &World, cx: &mut Cx) {
        let Some(i) = w.instance(&self.instance) else {
            return;
        };
        let ws = i.workspace.and_then(|x| w.workspace(x));
        let focused = self
            .focused_pane(w)
            .and_then(|p| self.daemon(w).and_then(|d| d.pane(p)));
        let agent = match focused {
            Some(p) => match p.agent {
                Some(a) => format!(
                    "{} ({})",
                    a.short(),
                    p.account
                        .as_ref()
                        .and_then(|id| w.accounts.get(id))
                        .map(|x| x.title())
                        .unwrap_or(a.label().into())
                ),
                None => "(shell)".into(),
            },
            None => "(none)".into(),
        };
        let v = &w.surfaces.vocabulary;
        let noun = &v.container_noun;
        let mut props = vec![Prop::new(noun, i.container_id()).copyable()];
        if let Some(id) = &v.sandbox_id {
            let run8 = clip(&i.run_id.replace('-', ""), 8);
            props
                .push(Prop::new(&format!("{noun} ID"), fill_in(id, &[("run8", &run8)])).copyable());
        }
        props.extend([Prop::new("Role", i.role_label()), Prop::new("Agent", agent)]);
        if let Some(title) = focused.and_then(|p| p.body.title()) {
            props.push(Prop::new(&v.pane.title_label, title.to_owned()));
        }
        props.extend([
            Prop::new("Workdir", i.workdir_label()),
            Prop::new("Instance", i.id.short_suffix()),
            Prop::new(
                w.surfaces.vocabulary.version_label.as_str(),
                w.surfaces.version.as_str(),
            ),
        ]);
        if let Some(id) = &v.invocation_id {
            props.push(Prop::new("Invocation ID", fill_in(id, &[("run", &i.run_id)])).copyable());
        }
        if let Some(log) = &v.host_log {
            props.push(Prop::new("Host log", fill_in(log, &[("run", &i.run_id)])));
        }
        if let Some(task) = &i.task {
            props.push(Prop::new("Task", task.text.clone()));
            for status in &task.statuses {
                props.push(Prop::new(status.agent.label(), status.text.clone()));
            }
        }
        let _ = ws;
        let d = InfoDialog::new(WidgetId::of("capsule.info"), "Debug info", props)
            .meta("Enter copies the row · y copies");
        cx.open(d, LayerTag::new("info"));
    }

    fn github_url(&self, w: &World) -> Option<String> {
        let i = w.instance(&self.instance)?;
        let repo = i.github.as_ref()?;
        Some(match &i.pr {
            Some((n, _)) => format!("{}/pull/{n}", repo.url),
            None => format!(
                "{}/tree/{}",
                repo.url,
                i.branch.clone().unwrap_or(i.default_branch.clone())
            ),
        })
    }

    fn open_github(&mut self, w: &World, cx: &mut Cx) {
        let Some(i) = w.instance(&self.instance) else {
            return;
        };
        let url = self.github_url(w);
        let branch = i.branch.clone().unwrap_or(i.default_branch.clone());
        let mut props = vec![Prop::new("Branch", branch.clone())];
        match &i.pr {
            Some((n, title)) => {
                props.push(Prop::new("Pull Request", format!("#{n}")));
                props.push(Prop::new("PR Title", title.clone()));
                if let Some(url) = url {
                    props.push(Prop::new("GitHub URL", url).copyable());
                }
                props.push(Prop::new("CI Status", "✓ passing").tone(Tone::Success));
            }
            None => {
                props.push(Prop::new("Pull Request", "(none)").tone(Tone::Muted));
                if let Some(url) = url {
                    props.push(Prop::new("GitHub URL", url).copyable());
                }
                props.push(Prop::new("CI Status", "(unknown)").tone(Tone::Muted));
            }
        }
        let d = InfoDialog::new(WidgetId::of("capsule.github"), "GitHub context", props)
            .action(Button::secondary(
                WidgetId::of("capsule.github").sub("open"),
                &w.surfaces.vocabulary.github_open_label,
            ))
            .meta(&w.surfaces.vocabulary.github_open_meta);
        cx.open(d, LayerTag::new("github"));
    }

    fn detach(&mut self, cx: &mut Cx) {
        cx.go(Go::Detach);
    }

    fn end_instance(&mut self, purge: bool, cx: &mut Cx) {
        cx.go(Go::InstanceEnded {
            instance: self.instance.clone(),
            purge,
        });
    }

    fn handle_prefix_cmd(&mut self, key: &Key, w: &mut World, cx: &mut Cx) -> Outcome {
        self.prefix_until = None;
        if key.ctrl_char('b') {
            self.send_prefix(key, w);
            return Outcome::Changed;
        }
        if key.ctrl_char('l') {
            if let Some(p) = self.focused_pane(w)
                && let Some(pane) = self.daemon_mut(w).and_then(|d| d.pane_mut(p))
            {
                pane.body.clear();
            }
            return Outcome::Changed;
        }
        match key.code {
            KeyCode::Char('c') => self.spawn_flow(Intent::NewTab, w, cx),
            KeyCode::Char('n') => self.next_tab(w, 1),
            KeyCode::Char('p') => self.next_tab(w, -1),
            KeyCode::Char('x') => {
                let single = self
                    .daemon(w)
                    .and_then(|d| d.active_tab())
                    .is_some_and(|t| t.leaves().len() == 1);
                self.close_pane_flow(single, w, cx);
            }
            KeyCode::Char('&') => self.confirm_close(true, w, cx),
            KeyCode::Char('"') => self.spawn_flow(Intent::Split(SplitDir::Vertical, false), w, cx),
            KeyCode::Char('%') => {
                self.spawn_flow(Intent::Split(SplitDir::Horizontal, false), w, cx)
            }
            KeyCode::Char('z') => self.toggle_zoom(w, cx),
            KeyCode::Char('h') => self.move_focus(Direction::Left, w, cx),
            KeyCode::Char('j') => self.move_focus(Direction::Down, w, cx),
            KeyCode::Char('k') => self.move_focus(Direction::Up, w, cx),
            KeyCode::Char('l') => self.move_focus(Direction::Right, w, cx),

            KeyCode::Char('H') => self.resize(Direction::Left, w, cx),
            KeyCode::Char('J') => self.resize(Direction::Down, w, cx),
            KeyCode::Char('K') => self.resize(Direction::Up, w, cx),
            KeyCode::Char('L') => self.resize(Direction::Right, w, cx),
            KeyCode::Char('d') => self.detach(cx),
            KeyCode::Char('u') => self.open_usage(w, cx),
            KeyCode::Char('i') => self.open_container_info(w, cx),
            KeyCode::Char(',') => {
                let t = self.daemon(w).map(|d| d.active).unwrap_or(0);
                self.open_rename(t, w, cx);
            }
            KeyCode::Char('m') => {
                let t = self.daemon(w).map(|d| d.active).unwrap_or(0);
                self.open_tab_menu(t, w);
            }
            KeyCode::F(10) => self.menubar.open_menu(0),
            KeyCode::Char(' ') | KeyCode::Char(':') => self.open_palette(w, cx),
            KeyCode::Char('r') => self.redraw(w, cx),
            KeyCode::Char(c) if c.is_ascii_digit() => {
                let n = if c == '0' {
                    10
                } else {
                    c as usize - '0' as usize
                };
                let count = self.daemon(w).map(|d| d.tabs.len()).unwrap_or(0);
                if n >= 1 && n <= count {
                    if let Some(d) = self.daemon_mut(w) {
                        d.active = n - 1;
                    }
                } else {
                    cx.status(format!("No tab {n}"));
                }
            }
            KeyCode::Esc => {}
            KeyCode::Char(c) => cx.status(format!(
                "Ctrl+B {c} does nothing · Ctrl+B Space lists commands"
            )),
            _ => {}
        }
        Outcome::Changed
    }

    fn next_tab(&mut self, w: &mut World, delta: isize) {
        if let Some(d) = self.daemon_mut(w)
            && !d.tabs.is_empty()
        {
            let n = d.tabs.len() as isize;
            d.active = ((d.active as isize + delta + n) % n) as usize;
        }
    }

    fn forward_key(&mut self, key: &Key, w: &mut World, cx: &mut Cx) -> Outcome {
        let Some(p) = self.focused_pane(w) else {
            return Outcome::Consumed;
        };
        let now = w.now_ms();
        self.prime(p, w);
        let Some(pane) = self.daemon_mut(w).and_then(|d| d.pane_mut(p)) else {
            return Outcome::Consumed;
        };

        let term = pane.body.viewport_mut();
        if !term.follow() {
            match key.code {
                KeyCode::Up => {
                    let _ = term.wheel(Axis::Vertical, -1);
                    return Outcome::Changed;
                }
                KeyCode::Down => {
                    let _ = term.wheel(Axis::Vertical, 1);
                    return Outcome::Changed;
                }
                KeyCode::PageUp => {
                    term.page_up();
                    return Outcome::Changed;
                }
                KeyCode::PageDown => {
                    term.page_down();
                    return Outcome::Changed;
                }
                KeyCode::Home => {
                    term.jump_start();
                    return Outcome::Changed;
                }

                KeyCode::Left if key.plain() => {
                    return term.wheel(Axis::Horizontal, -1);
                }
                KeyCode::Right if key.plain() => {
                    return term.wheel(Axis::Horizontal, 1);
                }
                KeyCode::End | KeyCode::Esc => {
                    term.set_follow(true);
                    return Outcome::Changed;
                }
                _ => {}
            }
        }
        if term.has_selection() {
            if key.is(KeyCode::Esc) {
                term.clear_selection();
                return Outcome::Changed;
            }
            if key.is_char('y')
                && key.plain()
                && term.keyboard_copy()
                && let Some(t) = term.selected_text()
            {
                cx.copy_as(t, "selection");
                return Outcome::Changed;
            }
        }
        match pane.body.key(key, now) {
            PaneKey::Forwarded => Outcome::Changed,
            PaneKey::Consumed => Outcome::Consumed,
        }
    }

    fn prime(&self, pid: PaneId, w: &mut World) {
        let Some((_, r)) = self.pane_rects.iter().find(|(p, _)| *p == pid).copied() else {
            return;
        };
        let inner = self.inner_of(w, r);
        if let Some(pane) = self.daemon_mut(w).and_then(|d| d.pane_mut(pid)) {
            pane.body.sync_size(inner);
        }
    }

    fn pane_of(&self, id: WidgetId) -> Option<PaneId> {
        self.pane_rects
            .iter()
            .find(|(pid, _)| PANES.child(*pid as usize) == id)
            .map(|(pid, _)| *pid)
    }

    fn pane_at(&self, pos: Position) -> Option<PaneId> {
        self.pane_rects
            .iter()
            .find(|(_, r)| r.contains(pos))
            .map(|(id, _)| *id)
    }

    fn draw_strip(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        let t = ctx.theme;
        let bg = t.canvas;
        fill(buf, area, t.base());
        let Some(d) = self.daemon(w) else { return };
        let items: Vec<TabItem> = d
            .tabs
            .iter()
            .enumerate()
            .map(|(i, tab)| {
                let label = d.tab_label(tab, &|p| Self::account_suffix(w, p));
                let mut it = TabItem::new(&label);
                if i < 10 {
                    it = it.prefix(&if i == 9 {
                        "0".to_owned()
                    } else {
                        (i + 1).to_string()
                    });
                }

                let g = match d.tab_state(tab) {
                    AgentState::Blocked => "●",
                    AgentState::Done => {
                        it = it.done(true);
                        ""
                    }
                    AgentState::Working => "▶",
                    AgentState::Idle => "◆",
                    AgentState::Unknown => "",
                };
                if !g.is_empty() {
                    it = it.suffix(g);
                }
                it
            })
            .collect();
        self.tabs.set_items(items);
        self.tabs.set_active(d.active);
        self.tabs.render(
            Rect::new(area.x + 1, area.y, area.width.saturating_sub(2), 2),
            buf,
            ctx,
            bg,
        );
    }

    fn draw_panes(&mut self, body: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        let t = ctx.theme;
        fill(buf, body, t.base());
        self.layout(body, w);
        let Some(d) = self.daemon(w) else { return };
        if d.tabs.is_empty() {
            let e =
                EmptyState::new("No sessions").hint("Ctrl+B c starts an agent · Ctrl+B d detaches");
            empty::render(body, buf, t, &e, t.canvas);
            return;
        }
        let framed = self.framed(w);
        let Some(tab) = d.active_tab() else { return };
        let words = &w.surfaces.vocabulary.pane;
        let rects = self.pane_rects.clone();
        let dialog = self.mode() == Mode::Dialog;
        for (pid, r) in &rects {
            let Some(pane) = d.pane(*pid) else { continue };
            let focused = tab.focused == *pid;
            let inner = if framed {
                r.inner(ratatui::layout::Margin::new(1, 1))
            } else {
                *r
            };
            if framed {
                let hovered = ctx.interaction.hovered(PANES.child(*pid as usize));
                let block = ratatui::widgets::Block::new()
                    .borders(ratatui::widgets::Borders::ALL)
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .border_style(t.border(focused).bg(t.canvas));
                ratatui::widgets::Widget::render(block, *r, buf);
                let (g, gt) = match pane.body.state() {
                    AgentState::Blocked => ("●", t.warning),
                    AgentState::Done => ("○", t.text_secondary),
                    AgentState::Working => ("▶", t.accent),
                    AgentState::Idle => ("◆", t.text_muted),
                    AgentState::Unknown => ("", t.text_muted),
                };
                let mut title = format!(" {}", pane.label);
                if let Some(s) = Self::account_suffix(w, pane) {
                    title.push_str(&format!(" ({s})"));
                }
                if let Some(s) = pane.body.suffix(words) {
                    title.push(' ');
                    title.push_str(&s);
                }
                title.push(' ');
                let ts = if focused {
                    t.title().bg(t.canvas)
                } else if hovered {
                    t.primary().bg(t.canvas)
                } else {
                    t.secondary().bg(t.canvas)
                };
                if r.width > 6 {
                    buf.set_string(
                        r.x + 2,
                        r.y,
                        junie_tui::ui::reveal::cut(
                            Rect::new(r.x + 2, r.y, r.width.saturating_sub(6), 1),
                            &title,
                        ),
                        ts,
                    );
                    if !g.is_empty() {
                        buf.set_string(
                            r.x + 2 + width(&title) as u16,
                            r.y,
                            g,
                            Style::new().fg(gt).bg(t.canvas),
                        );
                    }
                }
                let mut meta = String::new();
                let term = pane.body.viewport();
                if !term.follow() {
                    let range = term.scroll().visible_range();
                    meta = format!(
                        "{}–{} of {}",
                        junie_tui::ui::text::thousands(range.start + 1),
                        junie_tui::ui::text::thousands(range.end),
                        junie_tui::ui::text::thousands(term.scroll().content_len)
                    );
                }
                if tab.zoomed.is_some() {
                    meta = if meta.is_empty() {
                        "zoomed".into()
                    } else {
                        format!("{meta} · zoomed")
                    };
                }
                if !meta.is_empty() {
                    let m = format!(" {meta} ");
                    let mw = width(&m) as u16;
                    if r.width > mw + width(&title) as u16 + 6 {
                        buf.set_string(
                            r.right().saturating_sub(mw + 2),
                            r.y,
                            &m,
                            t.faint().bg(t.canvas),
                        );
                    }
                }
            }

            let term = pane.body.viewport();
            let caret_allowed = !dialog
                && focused
                && pane.body.accepts_input()
                && self.selecting.is_none()
                && self.drag.is_none();

            let face = pane.body.face(words);
            ctx.inert = true;
            let cursor = match &face {
                PaneFace::Empty { title, hint } => {
                    Self::draw_empty(inner, buf, t, title, hint.as_deref());
                    None
                }
                PaneFace::Grid | PaneFace::Dimmed => {
                    term.draw(inner, buf, ctx, t.canvas, caret_allowed)
                }
            };
            ctx.inert = false;
            if face == PaneFace::Dimmed {
                let grid = if term.draws_track() {
                    Rect::new(
                        inner.x,
                        inner.y,
                        inner.width.saturating_sub(1),
                        inner.height,
                    )
                } else {
                    inner
                };
                buf.set_style(grid, Style::new().add_modifier(Modifier::DIM));
            }
            if let Some(p) = cursor {
                ctx.set_cursor(p);
            }

            if !term.follow() && !term.draws_track() {
                scrollbar::render_with_fade(
                    inner,
                    buf,
                    ctx,
                    PANES.child(*pid as usize),
                    term.scroll(),
                    focused,
                );
            }
            if let Some(note) = pane.body.note(words) {
                Self::draw_note(inner, buf, t, &note);
            }
            ctx.clickable(PANES.child(*pid as usize), *r);
            ctx.scrollable(PANES.child(*pid as usize), *r);
        }

        let seams = self.seams.clone();
        for (i, s) in seams.iter().enumerate() {
            let sid = PANES.sub("seam").child(i);
            let held = self.drag.as_ref().is_some_and(|(p, _)| *p == s.path);
            Splitter::new(sid, s.dir).render_held(s.handle, buf, ctx, t.canvas, held);
        }
        ctx.control(PANES, Rect::new(body.x, body.y, 1, 1), false);
    }

    fn draw_identity(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        let t = ctx.theme;
        let Some(i) = w.instance(&self.instance) else {
            return;
        };
        let used = self
            .menubar
            .areas
            .iter()
            .map(|r| r.right())
            .max()
            .unwrap_or(area.x)
            .max(self.menubar.brand_area.right());
        let rest = Rect::new(used + 2, area.y, area.right().saturating_sub(used + 2), 1);
        let ws = self
            .daemon(w)
            .map(|d| d.workspace.clone())
            .unwrap_or_default();
        let mut right = vec![];
        if self.mode() == Mode::PrefixAwait {
            right.push(Segment::new("prefix…", Tone::Normal).bold().priority(10));
        }
        right.push(
            Segment::new(format!("{ws} › {}", i.role_label()), Tone::Normal)
                .bold()
                .priority(9),
        );
        right.push(
            Segment::new(truncate_middle(&i.container_id(), 28), Tone::Muted)
                .clickable(CONTAINER_CHIP)
                .priority(6),
        );
        let n = w.running_count();
        if n > 1 {
            right.push(Segment::new(plural(n, "instance", "instances"), Tone::Faint).priority(3));
        }
        segments::render(rest, buf, ctx, &[], &right, t.canvas);
    }

    fn draw_status(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        let Some(i) = w.instance(&self.instance) else {
            return;
        };
        let d = self.daemon(w);
        let mut bar = StatusBar::new();

        let branch = i.branch.clone().unwrap_or(i.default_branch.clone());
        let work = match &i.pr {
            Some((n, title)) => format!("PR #{n} · {}", truncate(title, 32)),
            None => truncate_middle(&branch, 36).to_string(),
        };
        bar.left.push(
            StatusItem::new(work, Tone::Normal)
                .strong()
                .clickable(CONTEXT)
                .priority(10),
        );
        let touched = d.map(|d| d.touched_files().len()).unwrap_or(0);
        let changed = i.uncommitted + touched;
        if changed > 0 || i.unpushed > 0 {
            let mut parts = vec![];
            if changed > 0 {
                parts.push(format!("• {changed} changed"));
            }
            if i.unpushed > 0 {
                parts.push(format!(
                    "{} {}",
                    i.unpushed, w.surfaces.vocabulary.unexported_word
                ));
            }
            bar.left
                .push(StatusItem::new(parts.join(" · "), Tone::Warning).priority(6));
        } else {
            bar.left
                .push(StatusItem::new("clean", Tone::Muted).priority(4));
        }

        let pane = self.focused_pane(w).and_then(|p| d.and_then(|d| d.pane(p)));
        if let Some(pane) = pane {
            let agent = pane.agent.map(|a| a.label()).unwrap_or("shell");
            let account = pane
                .account
                .as_ref()
                .and_then(|id| w.accounts.get(id))
                .map(|a| format!(" · {}", a.display_name))
                .unwrap_or_default();
            let (state, tone) = match pane.body.state() {
                AgentState::Working => (" · working", Tone::Secondary),
                AgentState::Blocked => (" · needs input", Tone::Warning),
                AgentState::Done => (" · done", Tone::Secondary),
                AgentState::Idle => (" · idle", Tone::Muted),
                AgentState::Unknown => ("", Tone::Secondary),
            };
            bar.center
                .push(StatusItem::new(format!("{agent}{account}{state}"), tone).priority(8));
        }
        if let Some(d) = d {
            let panes = d.active_tab().map(|t| t.leaves().len()).unwrap_or(0);
            bar.center.push(
                StatusItem::new(
                    format!(
                        "{} · {}",
                        plural(d.tabs.len(), "tab", "tabs"),
                        plural(panes, "pane", "panes")
                    ),
                    Tone::Faint,
                )
                .priority(2),
            );
        }

        let acc = pane
            .and_then(|p| p.account.clone())
            .and_then(|id| w.accounts.get(&id).cloned());
        match acc {
            Some(a) => {
                let fresh = a.usage.freshness.phase;
                let mut any = false;
                for (k, win) in a
                    .usage
                    .windows
                    .iter()
                    .filter(|w| w.has_meter())
                    .take(2)
                    .enumerate()
                {
                    let tone = super::accounts::quota_tone(win, fresh);
                    let label = win.label.split(' ').next().unwrap_or("Usage").to_owned();
                    bar.right.push(
                        StatusItem::new(label, Tone::Muted)
                            .meter(win.used_pct, tone.tone)
                            .meter_stale(tone.stale)
                            .clickable(USAGE_CHIP)
                            .priority(if k == 0 { 9 } else { 7 }),
                    );
                    any = true;
                }
                match fresh {
                    Freshness::Stale => bar
                        .right
                        .push(StatusItem::new("stale", Tone::Warning).chip().priority(5)),
                    Freshness::Failed => bar.right.push(
                        StatusItem::new("usage error", Tone::Error)
                            .chip()
                            .priority(5),
                    ),
                    _ => {}
                }
                if !any {
                    bar.right.push(
                        StatusItem::new(
                            format!("{} · {}", a.display_name, a.status_word()),
                            Tone::Muted,
                        )
                        .clickable(USAGE_CHIP)
                        .priority(8),
                    );
                }
            }
            None => bar.right.push(
                StatusItem::new("no account · shell", Tone::Faint)
                    .clickable(USAGE_CHIP)
                    .priority(5),
            ),
        }

        ctx.clickable(STATUS, area);
        bar.render(area, buf, ctx);
    }
}

impl CapsuleScreen {
    fn reconnect(&mut self, w: &mut World, cx: &mut Cx) {
        self.takeover = None;
        if let Some(d) = self.daemon_mut(w) {
            d.attached_by = Some("this terminal".into());
        }
        cx.status("Reconnected · the other client was displaced");
    }

    fn leave_takeover(&mut self, cx: &mut Cx) {
        self.takeover = None;
        cx.go(Go::Detach);
    }
}

impl Page<Jackin> for CapsuleScreen {
    fn enter(&mut self, w: &mut World, cx: &mut Cx) {
        cx.focus.focus(PANES);
        self.sync_focus(w);
    }

    fn primary_focus(&self) -> Option<WidgetId> {
        Some(PANES)
    }

    /// Only what counts down on its own needs a frame per tick. Output of the
    /// panes arrives through the world, and an idle pane draws nothing new.
    fn animating(&self, _w: &World) -> bool {
        (self.prefix_timeout_ms.is_some() && self.prefix_until.is_some())
            || self.redraw_flash > 0
            || self.pending_spawn.is_some()
    }

    fn is_editing(&self) -> bool {
        false
    }

    /// Changed only when the tick changed something this page shows: a prefix
    /// that timed out, the redraw flash, a spawned pane, drained pane effects,
    /// a pane or focus that moved, or a new dirty count.
    fn on_tick(&mut self, w: &mut World, cx: &mut Cx) -> Outcome {
        let mut changed = false;
        if let Some(until) = self.prefix_until
            && w.now_ms() >= until
        {
            self.prefix_until = None;
            cx.status("Prefix timed out");
            changed = true;
        }
        if self.redraw_flash > 0 {
            self.redraw_flash -= 1;
            changed = true;
        }
        if self.pending_spawn.is_some() {
            self.apply_pending_spawn(w, cx);
            changed = true;
        }
        let before = self.control_signature(w);
        self.after_input(w);
        changed |= self.control_signature(w) != before;
        changed |= self.run_effects(w, cx);

        let touched = self.daemon(w).map(|d| d.touched_files().len()).unwrap_or(0);
        if touched
            > w.instance(&self.instance)
                .map_or(usize::MAX, |i| i.uncommitted)
            && let Some(i) = w.instance_view_mut(&self.instance)
        {
            i.uncommitted = i.uncommitted.max(touched);
            changed = true;
        }
        if changed {
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    fn on_msg(&mut self, msg: &Msg, w: &mut World, cx: &mut Cx) -> Outcome {
        if let Msg::ChangesRead { ticket, result } = msg {
            return self.changes_read(*ticket, result, w, cx);
        }
        if let Msg::Takeover { instance, by } = msg
            && *instance == self.instance
        {
            self.takeover = Some(by.clone());
            cx.status(format!("{by} attached to this instance"));
            return Outcome::Changed;
        }
        Outcome::Ignored
    }

    fn on_key(&mut self, key: &Key, w: &mut World, cx: &mut Cx) -> Outcome {
        let o = self.key_event(key, w, cx);
        self.after_input(w);
        o
    }

    fn pointer(&mut self, ev: &PointerEvent, w: &mut World, cx: &mut Cx) -> Outcome {
        let o = self.pointer_event(ev, w, cx);
        self.after_input(w);
        o
    }

    fn on_paste(&mut self, text: &str, w: &mut World, cx: &mut Cx) -> Outcome {
        let o = self.host_paste(text, w, cx);
        self.after_input(w);
        o
    }

    fn on_layer_result(
        &mut self,
        tag: &LayerTag,
        result: ModalResult,
        w: &mut World,
        cx: &mut Cx,
    ) -> Outcome {
        let o = self.layer_event(tag, result, w, cx);
        self.after_input(w);
        o
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        let t = ctx.theme;
        fill(buf, area, t.base());
        if let Some(by) = &self.takeover {
            let lines = [
                ("Attach taken over", t.title()),
                ("", t.base()),
                (
                    &*format!(
                        "{by} attached to {} {}.",
                        self.daemon(w)
                            .map(|d| d.workspace.as_str())
                            .unwrap_or("this instance"),
                        w.clock.ago(w.now_secs() - 3)
                    ),
                    t.secondary(),
                ),
                ("The instance, tabs and panes keep running.", t.secondary()),
                ("", t.base()),
                (
                    "Enter Reconnect (take it back)   Esc Workspace manager",
                    t.muted(),
                ),
            ];
            let y0 = area.y + area.height.saturating_sub(lines.len() as u16 + 2) / 2;
            for (i, (text, style)) in lines.iter().enumerate() {
                let w_ = width(text) as u16;
                let x = area.x + area.width.saturating_sub(w_) / 2;
                buf.set_string(x, y0 + i as u16, text, style.bg(t.canvas));
            }

            let y = y0 + lines.len() as u16 + 1;
            if y < area.bottom() {
                let mut reconnect = Button::primary(TAKEOVER_RECONNECT, "Reconnect");
                let mut leave = Button::secondary(TAKEOVER_LEAVE, "Workspace manager");
                let gap = t.spacing.button_gap;
                let widths = [reconnect.width(), leave.width()];
                let total = widths[0] + gap + widths[1];
                let x = area.x + area.width.saturating_sub(total) / 2;
                let rects = junie_tui::widgets::button::row_layout(
                    Rect::new(x, y, total.min(area.width), 1),
                    &widths,
                    gap,
                );
                reconnect.render(rects[0], buf, ctx, t.canvas);
                leave.render(rects[1], buf, ctx, t.canvas);
            }
            return;
        }

        let menu = Rect::new(area.x, area.y, area.width, 1);
        let strip = Rect::new(area.x, area.y + 2, area.width, 2);
        let status = Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1);
        let body = Rect::new(
            area.x,
            area.y + 4,
            area.width,
            area.height.saturating_sub(5),
        );
        self.body = body;
        self.menubar.render(menu, buf, ctx, t.canvas);
        self.draw_identity(menu, buf, ctx, w);
        self.draw_strip(strip, buf, ctx, w);
        self.draw_panes(body, buf, ctx, w);
        self.draw_status(status, buf, ctx, w);
        if let Some((_, m)) = self.tab_menu.as_mut() {
            m.render(area, buf, ctx);
        }
        if self.redraw_flash > 0 {
            buf.set_string(
                area.right().saturating_sub(10),
                area.y,
                " redrawn ",
                t.faint().bg(t.canvas),
            );
        }
    }

    fn hints(&self, _focus: Option<WidgetId>, w: &World) -> Vec<Hint> {
        if self.tab_menu.is_some() {
            return vec![
                hint("↑↓", "Move"),
                hint("Enter", "Choose"),
                hint("Esc", "Close"),
            ];
        }
        if self.menubar.is_open() {
            return vec![
                hint("←→", "Menu"),
                hint("↑↓", "Move"),
                hint("Enter", "Choose"),
                hint("Esc", "Close"),
            ];
        }
        match self.mode() {
            Mode::PrefixAwait => {
                let mut hints = vec![];
                if self
                    .focused_pane(w)
                    .and_then(|p| self.daemon(w).and_then(|d| d.pane(p)))
                    .is_some_and(|p| p.body.sends_prefix())
                {
                    hints.push(Hint::new(
                        "Ctrl+B Ctrl+B",
                        w.surfaces.vocabulary.pane.send_prefix.clone(),
                    ));
                }
                hints.extend([
                    hint("c", "New tab"),
                    hint("n p", "Tabs"),
                    hint("x", "Close pane"),
                    hint("h j k l", "Move between panes"),
                    hint("H J K L", "Resize"),
                    hint("\"", "Split ↕"),
                    hint("%", "Split ↔"),
                    hint("z", "Zoom"),
                    hint("&", "Close tab"),
                    hint("Ctrl+L", "Clear"),
                    hint("d", "Detach"),
                    hint("u", "Usage"),
                    hint(", m", "Title · tab menu"),
                    hint("Space", "Palette"),
                ]);
                hints
            }
            Mode::Select => vec![hint("drag", "Select"), hint("release", "Copy")],
            Mode::Drag => vec![hint("drag", "Resize"), hint("release", "Done")],
            _ => {
                let scrolled = self
                    .focused_pane(w)
                    .and_then(|p| self.daemon(w).and_then(|d| d.pane(p)))
                    .is_some_and(|p| !p.body.viewport().follow());
                if scrolled {
                    vec![
                        hint("↑↓", "Scroll"),
                        hint("PgUp PgDn", "Page"),
                        hint("End", "Live"),
                        hint("Esc", "Exit scrollback"),
                        hint("Ctrl+\\", "Palette"),
                    ]
                } else {
                    vec![
                        hint("Ctrl+B", "Pane commands…"),
                        hint("F10", "Menu"),
                        hint("Ctrl+\\", "Palette"),
                        hint("Alt+Shift+↑↓←→", "Resize"),
                        hint("right-click", "Tab menu"),
                        hint("Ctrl+Q", "Quit"),
                    ]
                }
            }
        }
    }

    fn crumb(&self, w: &World) -> String {
        let d = self.daemon(w);
        let ws = d.map(|d| d.workspace.as_str()).unwrap_or("instance");
        match d.and_then(|d| d.active_tab().map(|t| d.tab_label(t, &|_| None))) {
            Some(tab) => format!("{} › {ws} › {tab}", w.surfaces.vocabulary.capsule),
            None => format!("{} › {ws}", w.surfaces.vocabulary.capsule),
        }
    }

    fn on_esc_top(&mut self, _w: &mut World, _cx: &mut Cx) -> Outcome {
        Outcome::Consumed
    }
}

impl CapsuleScreen {
    fn key_event(&mut self, key: &Key, w: &mut World, cx: &mut Cx) -> Outcome {
        if self.takeover.is_some() {
            match key.code {
                KeyCode::Enter => self.reconnect(w, cx),
                KeyCode::Esc => self.leave_takeover(cx),
                _ => {}
            }
            return Outcome::Changed;
        }
        if let Some((tab, mut m)) = self.tab_menu.take() {
            let (o, ev) = m.on_key(key);

            let pass = o == Outcome::Ignored && ev == Some(MenuAction::Dismissed);
            match ev {
                Some(MenuAction::Chosen(i)) => {
                    let label = m.items[i].item.label.clone();
                    let t = if tab == usize::MAX { None } else { Some(tab) };
                    self.run_menu(&label, t, w, cx);
                }
                Some(MenuAction::Dismissed) => {}
                None => self.tab_menu = Some((tab, m)),
            }
            if !pass {
                return o.or(Outcome::Changed);
            }
        }
        if self.menubar.is_open() {
            let (o, ev) = self.menubar.on_key(key);
            match ev {
                Some(MenuBarAction::Chosen(mi, ii)) => {
                    let label = self.menubar.menus[mi][ii].item.label.clone();
                    self.run_menu(&label, None, w, cx);
                }
                Some(MenuBarAction::Brand) => self.open_brand_menu(w),
                _ => {}
            }
            return o.or(Outcome::Changed);
        }
        if self.context.keys.matches(key) {
            self.prefix_until = None;
            let at = self
                .focused_pane(w)
                .and_then(|p| self.daemon(w).and_then(|d| d.pane(p)))
                .map(|p| p.body.viewport().context_point());
            match at {
                Some(at) => self.open_pane_menu(at, w),
                None => {
                    let t = self.daemon(w).map(|d| d.active).unwrap_or(0);
                    self.open_tab_menu(t, w);
                }
            }
            return Outcome::Changed;
        }
        if key.code == KeyCode::F(10) {
            self.prefix_until = None;
            self.menubar.open_menu(0);
            return Outcome::Changed;
        }
        if key.ctrl_char('q') {
            self.prefix_until = None;
            self.request_exit(w, cx);
            return Outcome::Changed;
        }
        if key.code == KeyCode::Char('\\') && key.ctrl() {
            self.prefix_until = None;
            self.open_palette(w, cx);
            return Outcome::Changed;
        }
        if self.mode() == Mode::PrefixAwait {
            return self.handle_prefix_cmd(key, w, cx);
        }
        if key.ctrl_char('b') {
            self.enter_prefix(w);
            return Outcome::Changed;
        }
        if key.alt() && key.shift() {
            match key.code {
                KeyCode::Left => {
                    self.resize(Direction::Left, w, cx);
                    return Outcome::Changed;
                }
                KeyCode::Right => {
                    self.resize(Direction::Right, w, cx);
                    return Outcome::Changed;
                }
                KeyCode::Up => {
                    self.resize(Direction::Up, w, cx);
                    return Outcome::Changed;
                }
                KeyCode::Down => {
                    self.resize(Direction::Down, w, cx);
                    return Outcome::Changed;
                }
                _ => {}
            }
        }
        if self.selecting.is_some() {
            return Outcome::Consumed;
        }
        self.forward_key(key, w, cx)
    }

    fn pointer_event(&mut self, ev: &PointerEvent, w: &mut World, cx: &mut Cx) -> Outcome {
        let (id, pos) = (ev.id, ev.pos());
        if let Some((axis, delta)) = ev.wheel() {
            if let Some(o) = self.tab_menu_pointer(ev, w, cx) {
                return o;
            }
            if let Some(r) = self.menubar.route(ev) {
                return r.outcome();
            }
            if let Some(r) = self.tabs.route(ev) {
                return r.outcome();
            }
            return self.wheel(axis, delta, pos, w);
        }
        match ev.phase() {
            Some(Phase::Press) => self.press(ev, w, cx),
            Some(Phase::Drag) => self.drag(id, pos, w),
            Some(Phase::DragEnd | Phase::Release) => self.drag_end(w, cx),
            Some(Phase::Click) => {
                let o = self.drag_end(w, cx);
                if ClickPolicy::DATA.activates(ev.count(), false) {
                    let d = self.double_click(ev, w, cx);
                    if d.consumed() {
                        return d;
                    }
                }
                o.or(self.click(ev, w, cx))
            }
            Some(Phase::Secondary) => self.secondary(ev, w),
            _ => Outcome::Ignored,
        }
    }

    fn layer_event(
        &mut self,
        tag: &LayerTag,
        result: ModalResult,
        w: &mut World,
        cx: &mut Cx,
    ) -> Outcome {
        self.dialog_open = false;
        match (tag.kind, result) {
            ("palette", ModalResult::Picked(i)) => {
                if let Some(cmd) = self.palette_cmds.get(i).copied() {
                    self.run_palette(cmd, w, cx);
                }
            }
            ("agent", ModalResult::Picked(i)) => {
                let agent = self.picker_agents.get(i).copied().flatten();
                self.continue_spawn(agent, w, cx);
                self.apply_pending_spawn(w, cx);
            }
            ("provider", ModalResult::Picked(i)) => {
                let account = self.picker_accounts.get(i).cloned();
                let agent = self.pending_agent.take();
                if let (Some(a), Some(acc)) = (agent, account.clone()) {
                    let next_id = self.daemon(w).map(|d| d.next_id).unwrap_or(0);
                    w.session_accounts
                        .insert((self.instance.clone(), next_id), acc.clone());
                    self.spawn(Some(a), Some(acc), w, cx);
                    self.apply_pending_spawn(w, cx);
                }
            }
            ("split", ModalResult::Picked(i)) => {
                let intent = match i {
                    0 => Intent::Split(SplitDir::Horizontal, false),
                    1 => Intent::Split(SplitDir::Horizontal, true),
                    2 => Intent::Split(SplitDir::Vertical, false),
                    _ => Intent::Split(SplitDir::Vertical, true),
                };
                self.spawn_flow(intent, w, cx);
            }
            ("closetarget", ModalResult::Picked(i)) => {
                if i == 1 {
                    self.confirm_close(true, w, cx);
                } else {
                    self.close_pane_flow(false, w, cx);
                }
            }
            (
                "closepane",
                ModalResult::Dialog {
                    action: Some(1), ..
                },
            ) => self.close_focused(w, cx),
            (
                "paste",
                ModalResult::Dialog {
                    action: Some(1), ..
                },
            ) => self.paste_confirmed(w, cx),
            (
                "closetab",
                ModalResult::Dialog {
                    action: Some(1), ..
                },
            ) => {
                if let Some(d) = self.daemon_mut(w) {
                    let i = d.active;
                    d.close_tab(i);
                    let empty = d.tabs.is_empty();
                    crate::world::refresh_snapshots(w);
                    if empty {
                        cx.status(w.surfaces.vocabulary.last_session_ended.clone());
                        self.end_instance(false, cx);
                    } else {
                        cx.status("Tab closed");
                    }
                }
            }
            (
                "exit",
                ModalResult::Dialog {
                    action: Some(1), ..
                },
            ) => {
                self.end_instance(false, cx);
            }
            ("exitdirty", ModalResult::Choice(Some(row))) => match row {
                0 => self.spawn_flow(Intent::NewTab, w, cx),
                1 => {
                    self.open_inspect(w, cx, true);
                }
                2 => {
                    cx.status("Exited · changes kept in the preserved instance");
                    self.end_instance(false, cx);
                }
                _ => {
                    let words = &w.surfaces.vocabulary;
                    let ws = self
                        .daemon(w)
                        .map(|d| d.workspace.clone())
                        .unwrap_or_default();
                    let facts = vec![
                        Prop::new("Action", words.quit_discard_action.as_str()).tone(Tone::Error),
                        Prop::new(
                            "Target",
                            format!("{ws} · instance {}", self.instance.short_suffix()),
                        ),
                        Prop::new("Reversible", "no").tone(Tone::Secondary),
                    ];
                    let d = Dialog::facts(
                        WidgetId::of("capsule.discard"),
                        &words.quit_discard_title,
                        facts,
                        vec![],
                        Some(&ws),
                        Button::danger(
                            WidgetId::of("capsule.discard").sub("ok"),
                            &words.quit_discard_button,
                        ),
                    );
                    cx.open(d, LayerTag::new("discard"));
                }
            },
            ("inspect", ModalResult::Custom(r)) if r == "back" => {
                self.request_exit(w, cx);
            }
            ("inspect", _) => {}
            (
                "discard",
                ModalResult::Dialog {
                    action: Some(1), ..
                },
            ) => {
                if let Some(i) = w.instance_view_mut(&self.instance) {
                    i.uncommitted = 0;
                    i.unpushed = 0;
                }
                cx.status("Changes discarded");
                self.end_instance(false, cx);
            }
            (
                "rename",
                ModalResult::Dialog {
                    action: Some(1),
                    text,
                },
            ) => {
                let label: String = text.unwrap_or_default().chars().take(MAX_LABEL).collect();
                if let Some(t) = self.daemon_mut(w).and_then(|d| d.tabs.get_mut(tag.n)) {
                    t.custom_label = if label.trim().is_empty() {
                        None
                    } else {
                        Some(label.clone())
                    };
                }
                cx.status(if label.trim().is_empty() {
                    "Tab name reset"
                } else {
                    "Tab renamed"
                });
            }
            (
                "export",
                ModalResult::Dialog {
                    action: Some(1),
                    text,
                },
            ) => {
                let p = text.unwrap_or_default();
                self.finish_export(&p, w, cx);
            }
            ("info", ModalResult::Info(InfoResult::Copy(v)))
            | ("github", ModalResult::Info(InfoResult::Copy(v))) => {
                cx.copy(v);
            }
            ("github", ModalResult::Info(InfoResult::Action(0))) => {
                let words = &w.surfaces.vocabulary;
                match self.github_url(w) {
                    Some(url) => {
                        if words.github_copies_url {
                            cx.copy(url);
                        }
                        cx.status(words.github_copied.clone());
                    }
                    None => cx.error(words.github_no_url.clone()),
                }
            }
            ("usage", _) => {}
            (_, ModalResult::Cancelled)
            | (
                _,
                ModalResult::Dialog {
                    action: Some(0), ..
                },
            )
            | (_, ModalResult::Choice(None)) => {
                self.pending_intent = None;
                self.pending_agent = None;
                self.pending_paste = None;
            }
            _ => {}
        }
        cx.focus.focus(PANES);
        Outcome::Changed
    }

    /// Gives every terminal pane the room its place in the layout has now,
    /// which starts a pane that was waiting for room.
    fn sync_all(&mut self, w: &mut World) {
        if self.body.is_empty() {
            return;
        }
        let body = self.body;
        self.layout(body, w);
        let ids: Vec<PaneId> = self.pane_rects.iter().map(|(id, _)| *id).collect();
        for pid in ids {
            let terminal = self
                .daemon(w)
                .and_then(|d| d.pane(pid))
                .is_some_and(|p| p.body.kind() != PaneKindTag::Script);
            if terminal {
                self.prime(pid, w);
            }
        }
    }

    /// Tells each pane whether it is the pane the operator is looking at: the
    /// focused pane of the active tab, with no menu, palette or dialog over
    /// it.
    pub fn sync_focus(&mut self, w: &mut World) {
        let free = !self.dialog_open
            && !self.menubar.is_open()
            && self.tab_menu.is_none()
            && self.takeover.is_none();
        let now = w.now_ms();
        let Some(d) = w.daemons.get_mut(&self.instance) else {
            return;
        };
        let active = d.focused_pane();
        let seen = if free { active } else { None };
        let copies = active
            .and_then(|id| d.pane(id))
            .is_none_or(|p| p.body.viewport().keyboard_copy());
        for p in &mut d.panes {
            p.body.focus_changed(Some(p.id) == seen, now);
        }
        for item in self.menubar.menus.iter_mut().flatten() {
            if item.item.label == "Copy selection" {
                item.shortcut = copies.then_some("y".into());
            }
        }
    }

    fn after_input(&mut self, w: &mut World) {
        self.sync_all(w);
        self.sync_focus(w);
    }

    /// Drains the effects the panes asked for. Whether there were any.
    fn run_effects(&mut self, w: &mut World, cx: &mut Cx) -> bool {
        let words = &w.surfaces.vocabulary.pane;
        let Some(d) = w.daemons.get_mut(&self.instance) else {
            return false;
        };
        let mut any = false;
        for pane in &mut d.panes {
            for effect in pane.body.take_effects(words) {
                any = true;
                match effect {
                    PaneEffect::Status { text, tone } => cx.status_in(text, tone),
                    PaneEffect::Copy { text, what } => cx.copy_as(text, what),
                }
            }
        }
        any
    }

    /// The small facts a tick of input syncing can change: which panes exist,
    /// which one has focus and what state each reports. Never the grid.
    fn control_signature(&self, w: &World) -> (usize, usize, Option<PaneId>, Vec<AgentState>) {
        self.daemon(w).map_or((0, 0, None, vec![]), |d| {
            (
                d.tabs.len(),
                d.active,
                d.focused_pane(),
                d.panes.iter().map(|p| p.body.state()).collect(),
            )
        })
    }

    fn send_prefix(&mut self, key: &Key, w: &mut World) {
        let now = w.now_ms();
        if let Some(p) = self.focused_pane(w)
            && let Some(pane) = self.daemon_mut(w).and_then(|d| d.pane_mut(p))
            && pane.body.sends_prefix()
        {
            pane.body.key(key, now);
        }
    }

    fn redraw(&mut self, w: &mut World, cx: &mut Cx) {
        self.redraw_flash = 3;
        if let Some(p) = self.focused_pane(w)
            && let Some(pane) = self.daemon_mut(w).and_then(|d| d.pane_mut(p))
        {
            pane.body.redraw();
        }
        cx.status("Redrawn");
    }

    /// Closes the focused pane, asking first only when the pane would lose
    /// something by it: a pane with no live program has nothing to lose.
    fn close_pane_flow(&mut self, single: bool, w: &mut World, cx: &mut Cx) {
        let asks = self
            .focused_pane(w)
            .and_then(|p| self.daemon(w).and_then(|d| d.pane(p)))
            .is_none_or(|p| p.body.close_needs_confirm());
        if asks {
            self.confirm_close(single, w, cx);
        } else {
            self.close_focused(w, cx);
        }
    }

    fn close_focused(&mut self, w: &mut World, cx: &mut Cx) {
        if let Some(p) = self.focused_pane(w)
            && let Some(d) = self.daemon_mut(w)
        {
            let tab_closed = d.close_pane(p);
            let empty = d.tabs.is_empty();
            crate::world::refresh_snapshots(w);
            if empty {
                cx.status(w.surfaces.vocabulary.last_session_ended.clone());
                self.end_instance(false, cx);
            } else {
                cx.status(if tab_closed {
                    "Tab closed"
                } else {
                    "Pane closed"
                });
            }
        }
    }

    fn host_paste(&mut self, text: &str, w: &mut World, cx: &mut Cx) -> Outcome {
        let now = w.now_ms();
        let Some(pid) = self.focused_pane(w) else {
            return Outcome::Consumed;
        };
        let Some(pane) = self.daemon_mut(w).and_then(|d| d.pane_mut(pid)) else {
            return Outcome::Consumed;
        };
        if pane.body.kind() == PaneKindTag::Script {
            let line: String = text.chars().filter(|c| *c != '\n' && *c != '\r').collect();
            pane.body.paste(&line, now);
            return Outcome::Changed;
        }
        let outcome = pane.body.paste(text, now);
        self.settle_paste(pid, outcome, text, false, w, cx);
        Outcome::Changed
    }

    fn paste_last_copy(&mut self, text: &str, w: &mut World, cx: &mut Cx) {
        let now = w.now_ms();
        let pasted = w.surfaces.vocabulary.pane.pasted.clone();
        let Some(pid) = self.focused_pane(w) else {
            cx.status(pasted);
            return;
        };
        let Some(pane) = self.daemon_mut(w).and_then(|d| d.pane_mut(pid)) else {
            cx.status(pasted);
            return;
        };
        if pane.body.kind() == PaneKindTag::Script {
            let head: String = text.chars().take(200).collect();
            pane.body.paste(&head, now);
            cx.status(pasted);
            return;
        }
        let outcome = pane.body.paste(text, now);
        self.settle_paste(pid, outcome, text, true, w, cx);
    }

    fn settle_paste(
        &mut self,
        pid: PaneId,
        outcome: PasteOutcome,
        text: &str,
        menu: bool,
        w: &World,
        cx: &mut Cx,
    ) {
        let words = &w.surfaces.vocabulary.pane;
        match outcome {
            PasteOutcome::Sent if menu => cx.status(words.pasted.clone()),
            PasteOutcome::Sent | PasteOutcome::Dropped => {}
            PasteOutcome::Full => cx.error(words.paste_full.clone()),
            PasteOutcome::TooLarge => cx.error(fill_in(
                &words.paste_too_large,
                &[("bytes", &text.len().to_string())],
            )),
            PasteOutcome::NeedsConfirm { lines } => {
                self.pending_paste = Some((pid, text.to_owned(), menu));
                let title = fill_in(
                    &words.paste_confirm_title,
                    &[("lines", &plural(lines, "line", "lines"))],
                );
                let mut d = Dialog::confirm(
                    WidgetId::of("capsule.paste"),
                    &title,
                    &words.paste_confirm_body,
                    &words.paste_confirm_button,
                );
                d.initial_focus = d.actions[0].id;
                cx.open(d, LayerTag::new("paste"));
            }
        }
    }

    fn paste_confirmed(&mut self, w: &mut World, cx: &mut Cx) {
        let Some((pid, text, menu)) = self.pending_paste.take() else {
            return;
        };
        let now = w.now_ms();
        if let Some(pane) = self.daemon_mut(w).and_then(|d| d.pane_mut(pid)) {
            let outcome = pane.body.paste_confirmed(&text, now);
            self.settle_paste(pid, outcome, &text, menu, w, cx);
        }
    }

    fn draw_empty(inner: Rect, buf: &mut Buffer, t: &Theme, title: &str, hint: Option<&str>) {
        fill(buf, inner, t.base());
        let mut state = EmptyState::new(title);
        if let Some(hint) = hint {
            state = state.hint(hint);
        }
        empty::render(inner, buf, t, &state, t.canvas);
    }

    fn draw_note(inner: Rect, buf: &mut Buffer, t: &Theme, note: &PaneNote) {
        if inner.height == 0 || inner.width < 2 {
            return;
        }
        let row = Rect::new(inner.x, inner.bottom() - 1, inner.width - 1, 1);
        let color = match note.tone {
            Tone::Error => t.error,
            Tone::Warning => t.warning,
            Tone::Muted => t.text_muted,
            _ => t.text_secondary,
        };
        let style = Style::new().fg(color).bg(t.surface_elevated);
        fill(buf, row, style);
        buf.set_string(
            row.x,
            row.y,
            junie_tui::ui::reveal::cut(row, &format!(" {} ", note.text)),
            style,
        );
    }
}

impl Screen for CapsuleScreen {
    fn layer_open(&self) -> bool {
        self.menubar.hides_page_focus()
            || self
                .tab_menu
                .as_ref()
                .is_some_and(|(_, m)| m.hides_page_focus())
    }

    fn picker_items(&mut self, tag: &LayerTag, query: &str, w: &World) -> Option<Vec<Item>> {
        if tag.kind == "palette" {
            return Some(self.palette_items(query, w));
        }
        None
    }

    fn strip_right(&self, w: &World) -> Vec<Segment> {
        let n = w.running_count();
        vec![Segment::new(plural(n, "instance", "instances"), Tone::Muted).priority(4)]
    }
}

pub struct UsageDialog {
    pub accounts: Vec<AccountId>,
    pub tab: usize,
    pub tabbar_focus: bool,
    pub scroll: junie_tui::core::scroll::ScrollState,
    pub refreshing_until: Option<i64>,
    pub result: Option<ModalResult>,
    pub area: Rect,
    pub tab_areas: Vec<Rect>,
}

impl UsageDialog {
    pub fn new(accounts: Vec<AccountId>, _w: &World) -> Self {
        Self {
            accounts,
            tab: 0,
            tabbar_focus: true,
            scroll: junie_tui::core::scroll::ScrollState::default(),
            refreshing_until: None,
            result: None,
            area: Rect::ZERO,
            tab_areas: vec![],
        }
    }
}

impl Layer<ModalResult, World> for UsageDialog {
    fn animating(&self) -> bool {
        true
    }
    fn spec(&self) -> LayerSpec {
        LayerSpec::modal(junie_tui::ui::popup::Dismiss::LIGHT)
            .initial_focus(WidgetId::of("capsule.usage"))
    }

    fn on_dismiss(&mut self, _: DismissReason) {
        self.result = Some(ModalResult::Custom("close".into()));
    }

    fn on_key(&mut self, key: &Key, cx: &mut LayerCx<'_, World>) -> Outcome {
        let w = cx.app;
        let n = self.accounts.len() + 1;
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                if self.tabbar_focus {
                    self.result = Some(ModalResult::Custom("close".into()));
                } else {
                    self.tabbar_focus = true;
                }
                Outcome::Changed
            }
            KeyCode::Tab => {
                self.tabbar_focus = !self.tabbar_focus;
                Outcome::Changed
            }
            KeyCode::Left | KeyCode::Char('h') => {
                self.tab = (self.tab + n - 1) % n;
                self.scroll.jump_start();
                Outcome::Changed
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.tab = (self.tab + 1) % n;
                self.scroll.jump_start();
                Outcome::Changed
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll.scroll_by(1);
                Outcome::Changed
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.scroll.scroll_by(-1);
                Outcome::Changed
            }
            KeyCode::PageDown => {
                self.scroll.page_down();
                Outcome::Changed
            }
            KeyCode::PageUp => {
                self.scroll.page_up();
                Outcome::Changed
            }
            KeyCode::Char('r') => {
                if self.refreshing_until.is_none() {
                    self.refreshing_until = Some(w.now_ms() + 800);
                }
                Outcome::Changed
            }
            _ => Outcome::Consumed,
        }
    }

    fn pointer(&mut self, ev: &PointerEvent, _: &mut LayerCx<'_, World>) -> Outcome {
        if let Some((axis, delta)) = ev.wheel() {
            if axis == Axis::Horizontal {
                return Outcome::Consumed;
            }
            self.scroll.scroll_by(delta as isize);
            return Outcome::Changed;
        }
        if !ev.is(Phase::Click) {
            return Outcome::Ignored;
        }
        for (i, _) in self.tab_areas.iter().enumerate() {
            if WidgetId::of("capsule.usage.tab").child(i) == ev.id {
                self.tab = i;
                self.tabbar_focus = true;
                return Outcome::Changed;
            }
        }
        Outcome::Consumed
    }

    fn on_tick(&mut self, w: &World) -> Outcome {
        if let Some(u) = self.refreshing_until
            && w.now_ms() >= u
        {
            self.refreshing_until = None;
            return Outcome::Changed;
        }
        if self.refreshing_until.is_some() {
            return Outcome::Changed;
        }
        Outcome::Ignored
    }

    fn take_result(&mut self) -> Option<LayerOut<ModalResult>> {
        self.result.take().map(LayerOut::Close)
    }

    fn hints(&self, _: Option<WidgetId>) -> Vec<Hint> {
        vec![
            hint("r", "Refresh"),
            hint("Tab", "Tabs / body"),
            hint("←→", "Provider"),
            hint("Esc", "Close"),
        ]
    }

    fn render(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        let wdt = 66u16.min(screen.width.saturating_sub(4));
        let h = screen.height.saturating_sub(4).min(24);
        let (area, inner) = modal_frame(
            screen,
            buf,
            ctx,
            wdt,
            h,
            "Usage",
            Some("read-only projection"),
            Layer::spec(self).dismiss,
        );
        self.area = area;
        let t: &Theme = ctx.theme;
        let bg = t.surface_elevated;

        let mut labels = vec!["Overview".to_owned()];
        for id in &self.accounts {
            if let Some(a) = w.accounts.get(id) {
                labels.push(format!(
                    "{} · {}",
                    a.agent.map(|x| x.label()).unwrap_or(a.provider.short()),
                    a.display_name
                ));
            }
        }
        let y = inner.y;

        let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();

        let mut strip = Tabs::new(WidgetId::of("capsule.usage.tab"), &label_refs)
            .passive(true)
            .passive_focused(self.tabbar_focus);
        strip.set_active(self.tab);
        let (strip_area, body) = Tabs::layout(Rect::new(
            inner.x,
            y,
            inner.width,
            inner.bottom().saturating_sub(y),
        ));
        strip.render(strip_area, buf, ctx, bg);
        self.tab_areas.clear();
        self.tab_areas.extend(strip.areas.iter().copied());
        let body = Rect {
            height: body.height.saturating_sub(2),
            ..body
        };

        enum Line {
            Text(String, Tone),
            Meter(String, u8, String, super::accounts::QuotaTone),
        }
        let mut lines: Vec<Line> = vec![];
        let refreshing = self.refreshing_until.is_some();
        if self.tab == 0 {
            for id in &self.accounts {
                if let Some(a) = w.accounts.get(id) {
                    let name = format!(
                        "{} · {}",
                        a.agent.map(|x| x.label()).unwrap_or(a.provider.short()),
                        a.display_name
                    );
                    let state = if refreshing {
                        "refreshing…".to_owned()
                    } else {
                        a.usage.freshness.phase.label().to_owned()
                    };
                    match a.usage.windows.iter().find(|w| w.used_pct.is_some()) {
                        Some(win) => lines.push(Line::Meter(
                            name,
                            win.used_pct.unwrap_or(0),
                            format!("{} · {}", win.label, state),
                            super::accounts::quota_tone(
                                win,
                                if refreshing {
                                    Freshness::Refreshing
                                } else {
                                    a.usage.freshness.phase
                                },
                            ),
                        )),
                        None => lines.push(Line::Text(
                            format!("{name}   {}", a.status_word()),
                            Tone::Muted,
                        )),
                    }
                }
            }
            if lines.is_empty() {
                lines.push(Line::Text(
                    "Providers   usage unavailable".into(),
                    Tone::Muted,
                ));
            }
        } else if let Some(a) = self
            .accounts
            .get(self.tab - 1)
            .and_then(|id| w.accounts.get(id))
        {
            lines.push(Line::Text(
                format!("Identity provider   {}", a.provider.label()),
                Tone::Normal,
            ));
            lines.push(Line::Text(
                format!(
                    "Identity account    {} ({})",
                    a.display_name,
                    a.identity.label()
                ),
                Tone::Normal,
            ));
            lines.push(Line::Text(
                format!(
                    "Identity activity   last request {}",
                    a.last_refresh_secs
                        .map(|s| w.clock.ago(s))
                        .unwrap_or("never".into())
                ),
                Tone::Normal,
            ));
            lines.push(Line::Text(String::new(), Tone::Normal));
            for win in &a.usage.windows {
                if win.has_meter() {
                    let reset = win
                        .reset_secs
                        .map(|r| w.clock.reset_label(r))
                        .unwrap_or_default();
                    lines.push(Line::Meter(
                        win.label.clone(),
                        win.used_pct.unwrap_or(0),
                        format!("{} · {reset}", win.value_label()),
                        super::accounts::quota_tone(
                            win,
                            if refreshing {
                                Freshness::Refreshing
                            } else {
                                a.usage.freshness.phase
                            },
                        ),
                    ));
                } else {
                    lines.push(Line::Text(
                        format!("{}   {}", win.label, win.value_label()),
                        Tone::Muted,
                    ));
                }
            }
            lines.push(Line::Text(String::new(), Tone::Normal));
            let fresh = if refreshing {
                format!("{} refreshing…", spinner_frame(w.now_ms() as u64 / 80))
            } else {
                match a.usage.freshness.phase {
                    Freshness::Current => format!(
                        "current · refreshed {}",
                        a.last_refresh_secs
                            .map(|s| w.clock.ago(s))
                            .unwrap_or("just now".into())
                    ),
                    Freshness::Stale => format!(
                        "stale · refreshed {}",
                        a.last_refresh_secs
                            .map(|s| w.clock.ago(s))
                            .unwrap_or("?".into())
                    ),
                    Freshness::Failed => format!(
                        "! error: {}",
                        a.issue
                            .as_ref()
                            .map(|i| w.surfaces.vocabulary.reason(&i.reason))
                            .unwrap_or("refresh failed".into())
                    ),
                    Freshness::Refreshing => "refreshing…".into(),
                }
            };
            lines.push(Line::Text(
                fresh,
                if a.usage.freshness.phase == Freshness::Failed && !refreshing {
                    Tone::Error
                } else {
                    Tone::Muted
                },
            ));
        }
        self.scroll.set_content(lines.len());
        self.scroll.set_viewport(body.height as usize);
        ctx.scrollable(WidgetId::of("capsule.usage"), body);
        for (k, i) in self.scroll.visible_range().enumerate() {
            let yy = body.y + k as u16;
            match &lines[i] {
                Line::Text(s, tone) => buf.set_string(
                    body.x,
                    yy,
                    junie_tui::ui::reveal::cut(Rect::new(body.x, yy, body.width, 1), s),
                    Style::new().fg(t.tone(*tone)).bg(bg),
                ),
                Line::Meter(label, pct, value, tone) => {
                    MeterList::new(vec![MeterRow::new(
                        label.as_str(),
                        tone.on(Meter::new(Some(*pct)))
                            .value(format!("{pct}%"))
                            .visual(MeterVisual::Block),
                    )])
                    .label_width(20)
                    .min_meter(12)
                    .render(
                        Rect::new(body.x, yy, body.width, 1),
                        buf,
                        ctx,
                        bg,
                    );
                    let _ = value;
                }
            }
        }
        if self.scroll.overflows() {
            scrollbar::render_with_fade(
                Rect::new(
                    area.x,
                    body.y,
                    (area.right() - 1).saturating_sub(area.x),
                    body.height,
                ),
                buf,
                ctx,
                WidgetId::of("capsule.usage"),
                &self.scroll,
                true,
            );
        }
        let hint = "r refresh  Tab tabs/body  ←→ provider  Esc close";
        buf.set_string(
            inner.x,
            inner.bottom().saturating_sub(1),
            junie_tui::ui::reveal::cut(
                Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
                hint,
            ),
            t.faint().bg(bg),
        );
        let _ = props::render;
    }
}

impl CapsuleScreen {
    fn vbar_of(term: &dyn ViewportOps) -> Bar {
        Bar {
            id: scrollbar::id_for(term.id()),
            owner: term.id(),
            axis: Axis::Vertical,
            track: term.vbar(),
        }
    }

    fn hbar_of(term: &dyn ViewportOps) -> Bar {
        Bar {
            id: scrollbar::h_id_for(term.id()),
            owner: term.id(),
            axis: Axis::Horizontal,
            track: term.hbar(),
        }
    }

    fn tab_menu_pointer(
        &mut self,
        ev: &PointerEvent,
        w: &mut World,
        cx: &mut Cx,
    ) -> Option<Outcome> {
        let (tab, mut m) = self.tab_menu.take()?;
        let r = match m.route(ev) {
            Some(r) if r.outcome() != Outcome::Ignored => r,
            _ if ev.is(Phase::Click) => layer::dismiss(&mut m, DismissReason::OutsideClick)
                .unwrap_or_else(Response::consumed),
            _ => {
                self.tab_menu = Some((tab, m));
                return None;
            }
        };
        let (o, act) = r.into_parts();
        match act {
            Some(MenuAction::Chosen(i)) => {
                let label = m.items[i].item.label.clone();
                let t = if tab == usize::MAX { None } else { Some(tab) };
                self.run_menu(&label, t, w, cx);
            }
            Some(MenuAction::Dismissed) => {}
            None => self.tab_menu = Some((tab, m)),
        }
        Some(o.or(Outcome::Changed))
    }

    fn click(&mut self, ev: &PointerEvent, w: &mut World, cx: &mut Cx) -> Outcome {
        let (id, pos) = (ev.id, ev.pos());
        if self.takeover.is_some() {
            if id == TAKEOVER_RECONNECT {
                self.reconnect(w, cx);
                return Outcome::Changed;
            }
            if id == TAKEOVER_LEAVE {
                self.leave_takeover(cx);
                return Outcome::Changed;
            }
            return Outcome::Consumed;
        }
        self.prefix_until = None;

        if let Some(o) = self.tab_menu_pointer(ev, w, cx) {
            return o;
        }
        if let Some(r) = self.menubar.route(ev) {
            let (o, act) = r.into_parts();
            match act {
                Some(MenuBarAction::Chosen(mi, ii)) => {
                    let label = self.menubar.menus[mi][ii].item.label.clone();
                    self.run_menu(&label, None, w, cx);
                }
                Some(MenuBarAction::Brand) => self.open_brand_menu(w),
                _ => {}
            }
            return o.or(Outcome::Changed);
        }
        if self.menubar.is_open() {
            self.menubar.close();
            return Outcome::Changed;
        }
        if id == USAGE_CHIP {
            self.open_usage(w, cx);
            return Outcome::Changed;
        }
        if id == CONTAINER_CHIP {
            self.open_container_info(w, cx);
            return Outcome::Changed;
        }
        if id == CONTEXT {
            self.open_github(w, cx);
            return Outcome::Changed;
        }
        if let Some(r) = self.tabs.route(ev) {
            let (o, act) = r.into_parts();
            if let Some(TabAction::Activated(i)) = act
                && let Some(d) = self.daemon_mut(w)
            {
                d.active = i;
            }
            cx.focus.focus(PANES);
            return o.or(Outcome::Changed);
        }
        if let Some(pid) = self.pane_at(pos) {
            cx.focus.focus(PANES);
            if let Some(t) = self.daemon_mut(w).and_then(|d| d.active_tab_mut()) {
                t.focused = pid;
            }
            self.prime(pid, w);
            let now = w.now_ms();
            if let Some(pane) = self.daemon_mut(w).and_then(|d| d.pane_mut(pid))
                && !pane.body.viewport().hbar().contains(pos)
                && !pane.body.viewport().vbar().contains(pos)
            {
                pane.body.gesture(now);
                pane.body.viewport_mut().on_click_with(pos, ev.count());
            }
            return Outcome::Changed;
        }
        Outcome::Consumed
    }

    fn press(&mut self, ev: &PointerEvent, w: &mut World, cx: &mut Cx) -> Outcome {
        if let Some(r) = self.menubar.route(ev)
            && r.outcome() != Outcome::Ignored
        {
            return Outcome::Changed;
        }
        let pos = ev.pos();

        if let Some(pid) = self.pane_at(pos) {
            cx.focus.focus(PANES);
            if let Some(t) = self.daemon_mut(w).and_then(|d| d.active_tab_mut()) {
                t.focused = pid;
            }
            self.prime(pid, w);
            let now = w.now_ms();
            if let Some(pane) = self.daemon_mut(w).and_then(|d| d.pane_mut(pid)) {
                pane.body.gesture(now);
                if pane.body.viewport().hbar().contains(pos) {
                    self.hbar_held = Some(pid);
                    let bar = Self::hbar_of(pane.body.viewport());
                    return pane
                        .body
                        .viewport_mut()
                        .bar_input(&bar, pos, true)
                        .or(Outcome::Changed);
                }
                if pane.body.viewport().vbar().contains(pos) {
                    let bar = Self::vbar_of(pane.body.viewport());
                    self.vbar_held = Some((pid, bar));
                    return pane
                        .body
                        .viewport_mut()
                        .bar_input(&bar, pos, true)
                        .or(Outcome::Changed);
                }
                pane.body
                    .viewport_mut()
                    .on_press_with(pos, ev.count(), ev.mods());
            }
            return Outcome::Changed;
        }
        Outcome::Ignored
    }

    fn secondary(&mut self, ev: &PointerEvent, w: &mut World) -> Outcome {
        if self.takeover.is_some() {
            return Outcome::Consumed;
        }
        if let Some(i) = ev.on(STRIP).then(|| ev.index_of(Part::TAB)).flatten() {
            if let Some(d) = self.daemon_mut(w) {
                d.active = i;
            }
            self.open_tab_menu(i, w);
            return Outcome::Changed;
        }
        let pos = ev.pos();
        if let Some(pid) = self.pane_at(pos) {
            if self.context.moves_cursor
                && let Some(t) = self.daemon_mut(w).and_then(|d| d.active_tab_mut())
            {
                t.focused = pid;
            }
            self.open_pane_menu(pos, w);
            return Outcome::Changed;
        }
        Outcome::Ignored
    }

    fn double_click(&mut self, ev: &PointerEvent, w: &mut World, cx: &mut Cx) -> Outcome {
        if let Some(i) = ev.on(STRIP).then(|| ev.index_of(Part::TAB)).flatten() {
            self.open_rename(i, w, cx);
            return Outcome::Changed;
        }
        let pos = ev.pos();
        if let Some(pid) = self.pane_at(pos) {
            self.prime(pid, w);
        }
        if let Some(pid) = self.pane_at(pos)
            && let Some(pane) = self.daemon_mut(w).and_then(|d| d.pane_mut(pid))
        {
            let view = pane.body.viewport();
            if view.hbar().contains(pos) || view.vbar().contains(pos) {
                return Outcome::Ignored;
            }
            let o = pane.body.viewport_mut().select_word_at(pos);
            if let Some(text) = pane.body.viewport().selected_text() {
                cx.copy_as(text, "selection");
            }
            return o;
        }
        Outcome::Ignored
    }

    fn drag(&mut self, pressed: WidgetId, pos: Position, w: &mut World) -> Outcome {
        for i in 0..self.seams.len() {
            if PANES.sub("seam").child(i) == pressed {
                let s = self.seams[i].clone();
                self.drag = Some((s.path.clone(), s.container));
                if let Some(t) = self.daemon_mut(w).and_then(|d| d.active_tab_mut())
                    && let Some(PaneNode::Split { split, dir, .. }) = t.root.node_at_mut(&s.path)
                {
                    split.drag_to(*dir, s.container, 1, pos);
                }
                return Outcome::Changed;
            }
        }

        if let Some((pid, bar)) = self.vbar_held {
            self.prime(pid, w);
            if let Some(pane) = self.daemon_mut(w).and_then(|d| d.pane_mut(pid)) {
                return pane.body.viewport_mut().bar_input(&bar, pos, false);
            }
        }

        if let Some(pid) = self.hbar_held {
            self.prime(pid, w);
            if let Some(pane) = self.daemon_mut(w).and_then(|d| d.pane_mut(pid)) {
                let bar = Self::hbar_of(pane.body.viewport());
                return pane.body.viewport_mut().bar_input(&bar, pos, false);
            }
        }

        let pressed_pane = self.pane_of(pressed);
        if let Some(pid) = pressed_pane {
            self.prime(pid, w);
        }
        if let Some(pid) = pressed_pane
            && let Some(pane) = self.daemon_mut(w).and_then(|d| d.pane_mut(pid))
        {
            self.selecting = Some(pid);
            return pane.body.viewport_mut().on_drag(pos);
        }
        Outcome::Ignored
    }

    fn drag_end(&mut self, w: &mut World, cx: &mut Cx) -> Outcome {
        if self.hbar_held.take().is_some() || self.vbar_held.take().is_some() {
            return Outcome::Changed;
        }
        if self.drag.take().is_some() {
            cx.status("Split resized");
            return Outcome::Changed;
        }
        if let Some(pid) = self.selecting.take() {
            if let Some(pane) = self.daemon(w).and_then(|d| d.pane(pid))
                && let Some(text) = pane.body.viewport().selected_text()
                && !text.trim().is_empty()
            {
                cx.copy_as(text, "selection");
            }
            return Outcome::Changed;
        }
        Outcome::Ignored
    }

    fn wheel(&mut self, axis: Axis, delta: i32, pos: Position, w: &mut World) -> Outcome {
        if let Some(pid) = self.pane_at(pos) {
            self.prime(pid, w);
        }
        if let Some(pid) = self.pane_at(pos)
            && let Some(pane) = self.daemon_mut(w).and_then(|d| d.pane_mut(pid))
        {
            if let Some(taken) = pane.body.wheel(axis, delta) {
                return taken;
            }
            return pane.body.viewport_mut().wheel(axis, delta);
        }
        Outcome::Ignored
    }
}
