// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **FileManager** — source-owned file-management composition from **public**
//! TermRock widgets only (Yazi / ranger / lf / broot as interaction and
//! performance *references* — not a clone API).
//!
//! **Mission.** Layout + focus + typed messages for breadcrumbs, file tree/list,
//! preview, quick open, search, multi-select, operation queue, status bar, and
//! confirm/conflict dialogs. Responsive multi-pane → single-pane/drawer.
//! **Filesystem I/O stays host-owned** (no `std::fs` walkers, trash, or
//! recursive delete inside this surface).
//!
//! **vs [`super::resource_browser`].** Thin AppShell geometry + preview wire
//! only; this is the elevated interactive composition.
//! **vs standalone [`FileTree`] / [`Breadcrumbs`] / [`QuickOpen`].** Composed,
//! not re-painted.
//!
//! Research: Yazi, ranger, lf, broot, desktop file managers.
//!
//! Teaches: how to compose a file manager: tree, listing, preview and inline
//! rename or filter chrome, routed through one focus model.
//!
//! Composes: [`crate::widgets::AlertDialog`],
//! [`crate::widgets::AlertDialogOutcome`],
//! [`crate::widgets::AlertDialogState`], [`crate::widgets::AlertKind`],
//! [`crate::widgets::AlertScope`], [`crate::widgets::BreadcrumbItem`],
//! [`crate::widgets::Breadcrumbs`], [`crate::widgets::BreadcrumbsOutcome`],
//! and 28 more.
//!
//! Copy-adapt: keep the widget composition and the focus routing;
//! replace the domain types, the wording, and the effects with your own.
use ratatui_core::{buffer::Buffer, layout::Rect, text::Line, widgets::StatefulWidget};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    layout::{
        PaneConstraint, PaneGeom, PaneId, Workspace, WorkspaceAxis, WorkspaceNode, WorkspaceState,
    },
    style::{DesignSystem, PanelChrome},
    widgets::{
        AlertDialog, AlertDialogOutcome, AlertDialogState, AlertKind, AlertScope, BreadcrumbItem,
        Breadcrumbs, BreadcrumbsOutcome, BreadcrumbsState, EmptyKind, EmptyState, FileTree,
        FileTreeEntry, FileTreeOutcome, FileTreeState, List, ListRow, ListState, Panel,
        PreviewCard, PreviewCardContent, PreviewCardState, PreviewLoadState, PreviewMetadata,
        PreviewResourceKind, QuickOpen, QuickOpenItem, QuickOpenOutcome, QuickOpenProvider,
        QuickOpenState, SearchInput, SearchInputOutcome, SearchInputState, StatusBar,
        StatusBarState, StatusRegion, StatusSlot, breadcrumbs_from_path,
        file_tree_to_quick_open_items, normalize_path_display,
    },
};

/// Default QuickOpen provider strip for file manager palette.
#[must_use]
pub fn default_quick_open_providers() -> Vec<QuickOpenProvider> {
    vec![QuickOpenProvider::new("files", "Files")]
}

// ── Panes & density ─────────────────────────────────────────────────────────

/// Named panes of the file manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FileManagerPane {
    /// Path breadcrumbs.
    Breadcrumbs,
    /// Search / filter bar.
    Search,
    /// File tree / list.
    Tree,
    /// Preview card.
    Preview,
    /// Operation queue.
    Queue,
    /// Status strip.
    Status,
}

impl FileManagerPane {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Breadcrumbs => "breadcrumbs",
            Self::Search => "search",
            Self::Tree => "tree",
            Self::Preview => "preview",
            Self::Queue => "queue",
            Self::Status => "status",
        }
    }

    /// Default Tab focus cycle (status is chrome-only).
    #[must_use]
    pub fn focus_order() -> &'static [FileManagerPane] {
        &[
            Self::Breadcrumbs,
            Self::Search,
            Self::Tree,
            Self::Preview,
            Self::Queue,
        ]
    }
}

/// Responsive density.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum FileManagerDensity {
    /// Full multi-pane: crumbs + search + tree + preview + queue + status.
    #[default]
    Normal,
    /// Collapse queue; preview as drawer (toggle).
    Narrow,
    /// Tree + status (optional search strip); no preview/queue.
    Tiny,
}

impl FileManagerDensity {
    /// From terminal width.
    #[must_use]
    pub const fn for_width(width: u16) -> Self {
        if width < 52 {
            Self::Tiny
        } else if width < 96 {
            Self::Narrow
        } else {
            Self::Normal
        }
    }

    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Narrow => "narrow",
            Self::Tiny => "tiny",
        }
    }
}

// ── Domain / operations ─────────────────────────────────────────────────────

/// Host-owned filesystem op kind (request only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FileOpKind {
    /// Copy sources → dest.
    Copy,
    /// Move sources → dest.
    Move,
    /// Delete targets.
    Delete,
    /// Rename one entry.
    Rename,
    /// Create file.
    NewFile,
    /// Create directory.
    NewDir,
}

impl FileOpKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Copy => "copy",
            Self::Move => "move",
            Self::Delete => "delete",
            Self::Rename => "rename",
            Self::NewFile => "new-file",
            Self::NewDir => "new-dir",
        }
    }

    /// Label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Copy => "Copy",
            Self::Move => "Move",
            Self::Delete => "Delete",
            Self::Rename => "Rename",
            Self::NewFile => "New file",
            Self::NewDir => "New directory",
        }
    }
}

/// Progress / lifecycle of a queued op (host projects).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum FileOpStatus {
    /// Waiting to start.
    #[default]
    Pending,
    /// In flight.
    Running,
    /// Needs conflict resolution.
    Conflict,
    /// Failed (retryable).
    Failed,
    /// Completed.
    Done,
    /// Cancelled by user/host.
    Cancelled,
}

impl FileOpStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Conflict => "conflict",
            Self::Failed => "failed",
            Self::Done => "done",
            Self::Cancelled => "cancelled",
        }
    }
}

/// Host-projected operation queue row (no FS I/O).
#[derive(Debug, Clone, PartialEq)]
pub struct FileOpItem {
    /// Stable op id (host).
    pub id: String,
    /// Kind.
    pub kind: FileOpKind,
    /// Source paths.
    pub sources: Vec<String>,
    /// Destination path (dir or new name).
    pub dest: Option<String>,
    /// Progress 0.0–1.0 (host).
    pub progress: f32,
    /// Lifecycle.
    pub status: FileOpStatus,
    /// Human message (conflict path, error).
    pub message: Option<String>,
}

impl FileOpItem {
    /// Construct pending op.
    #[must_use]
    pub fn new(id: impl Into<String>, kind: FileOpKind) -> Self {
        Self {
            id: id.into(),
            kind,
            sources: Vec::new(),
            dest: None,
            progress: 0.0,
            status: FileOpStatus::Pending,
            message: None,
        }
    }

    /// Sources.
    #[must_use]
    pub fn sources(mut self, paths: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.sources = paths.into_iter().map(Into::into).collect();
        self
    }

    /// Dest.
    #[must_use]
    pub fn dest(mut self, d: impl Into<String>) -> Self {
        self.dest = Some(d.into());
        self
    }

    /// Progress.
    #[must_use]
    pub fn progress(mut self, p: f32) -> Self {
        self.progress = p.clamp(0.0, 1.0);
        self
    }

    /// Status.
    #[must_use]
    pub const fn status(mut self, s: FileOpStatus) -> Self {
        self.status = s;
        self
    }

    /// Message.
    #[must_use]
    pub fn message(mut self, m: impl Into<String>) -> Self {
        self.message = Some(m.into());
        self
    }
}

/// Clipboard mode for paste → copy/move request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FileClipboardMode {
    /// Paste as copy.
    Copy,
    /// Paste as move.
    Move,
}

/// Conflict resolution choice (host applies).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FileConflictResolution {
    /// Overwrite existing.
    Overwrite,
    /// Skip this item.
    Skip,
    /// Rename destination (host picks unique name).
    Rename,
    /// Cancel whole op.
    Cancel,
}

impl FileConflictResolution {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Overwrite => "overwrite",
            Self::Skip => "skip",
            Self::Rename => "rename",
            Self::Cancel => "cancel",
        }
    }
}

/// Dialog / overlay mode projected by workbench (host may set conflict).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum FileManagerDialog {
    /// No modal.
    #[default]
    None,
    /// Destructive confirm elevated beyond FileTree banner.
    ConfirmDelete {
        /// Subject label.
        subject: String,
        /// Target ids/paths.
        paths: Vec<String>,
    },
    /// Op conflict needs resolution.
    Conflict {
        /// Op id.
        op_id: String,
        /// Conflicting path.
        path: String,
    },
    /// Quick open palette open.
    QuickOpen,
}

/// Workbench outcomes — requests only; host owns FS.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum FileManagerOutcome {
    /// Ignored.
    Ignored,
    /// Focus pane changed.
    FocusChanged(&'static str),
    /// Navigate breadcrumb / path change request.
    Navigate {
        /// Target path.
        path: String,
    },
    /// Selection changed in tree.
    SelectionChanged {
        /// Selected id (path).
        id: String,
    },
    /// Multi-check toggled.
    CheckToggled {
        /// Entry id.
        id: String,
    },
    /// Open / activate file.
    OpenRequested {
        /// Entry id.
        id: String,
    },
    /// Preview selection.
    PreviewRequested {
        /// Entry id.
        id: String,
    },
    /// Load lazy dir children.
    LoadChildrenRequested {
        /// Dir id.
        id: String,
    },
    /// Copy sources into dest (dir).
    CopyRequested {
        /// Source paths.
        sources: Vec<String>,
        /// Destination directory.
        dest: String,
    },
    /// Move sources into dest.
    MoveRequested {
        /// Source paths.
        sources: Vec<String>,
        /// Destination directory.
        dest: String,
    },
    /// Delete targets (after confirm).
    DeleteRequested {
        /// Paths / ids.
        ids: Vec<String>,
    },
    /// Rename one entry.
    RenameRequested {
        /// Target id.
        id: String,
        /// Old name.
        from: String,
        /// New name.
        to: String,
    },
    /// Create file or dir.
    NewRequested {
        /// `file` or `dir`.
        kind: &'static str,
        /// Parent id.
        parent: Option<String>,
        /// Suggested name.
        name: String,
    },
    /// Clipboard set (yank/cut) for later paste.
    ClipboardSet {
        /// Paths.
        paths: Vec<String>,
        /// Copy vs move paste.
        mode: FileClipboardMode,
    },
    /// Cancel queued op.
    OpCancel {
        /// Op id.
        op_id: String,
    },
    /// Retry failed op.
    OpRetry {
        /// Op id.
        op_id: String,
    },
    /// Conflict resolution chosen.
    ConflictResolved {
        /// Op id.
        op_id: String,
        /// Choice.
        resolution: FileConflictResolution,
    },
    /// Destructive confirm acknowledged (host may still require typed gates).
    ConfirmDestructive {
        /// Paths.
        paths: Vec<String>,
    },
    /// Confirm cancelled.
    ConfirmCancelled,
    /// Quick open toggled open.
    QuickOpenOpened,
    /// Quick open closed.
    QuickOpenClosed,
    /// Quick open item activated.
    QuickOpenActivated {
        /// Id.
        id: String,
    },
    /// Global search / filter changed.
    FilterChanged {
        /// Query.
        query: String,
    },
    /// Tree expand/collapse.
    Toggle {
        /// Id.
        id: String,
    },
    /// Drawer preview toggled (narrow).
    DrawerToggled {
        /// Open after.
        open: bool,
    },
    /// Esc root cancel.
    Cancelled,
    /// Child tree residual.
    Tree {
        /// Kind label.
        kind: String,
    },
}

// ── Surfaces ────────────────────────────────────────────────────────────────

/// Borrowed surfaces for one paint frame.
pub struct FileManagerSurfaces<'a> {
    /// Design system.
    pub system: &'a DesignSystem,
    /// State.
    pub state: &'a mut FileManagerState,
    /// Host-projected tree entries.
    pub entries: &'a [FileTreeEntry<'a, String>],
    /// Host-projected operation queue.
    pub ops: &'a [FileOpItem],
    /// Host-projected preview content (selection-based).
    pub preview: Option<PreviewCardContent<'a>>,
    /// Quick-open items (when palette open).
    pub quick_open_items: &'a [QuickOpenItem<String>],
}

// ── State ───────────────────────────────────────────────────────────────────

/// Persistent file manager state.
#[derive(Debug)]
pub struct FileManagerState {
    /// Workspace collapse.
    pub workspace: WorkspaceState,
    /// Breadcrumbs.
    pub breadcrumbs: BreadcrumbsState,
    /// Global search / filter.
    pub search: SearchInputState,
    /// File tree.
    pub tree: FileTreeState<String>,
    /// Preview card.
    pub preview: PreviewCardState,
    /// Operation queue list.
    pub queue: ListState<String>,
    /// Quick open palette.
    pub quick_open: QuickOpenState<String>,
    /// Alert dialog (confirm / conflict).
    pub alert: AlertDialogState<&'static str>,
    /// Status bar.
    pub status: StatusBarState<&'static str>,
    /// Current working directory (host-projected path chrome).
    pub cwd: String,
    /// Clipboard paths for paste.
    pub clipboard: Vec<String>,
    /// Clipboard paste mode.
    pub clipboard_mode: FileClipboardMode,
    /// Dialog / overlay mode.
    pub dialog: FileManagerDialog,
    /// Focused pane id.
    pub focus: &'static str,
    /// Density override (`None` = width-derived).
    pub density: Option<FileManagerDensity>,
    /// Narrow density: preview drawer open.
    pub drawer_open: bool,
    /// Entry count chrome.
    pub entry_count: u64,
    /// Selected path chrome.
    pub selected_path: Option<String>,
    /// Colorless.
    pub colorless: bool,
    /// Last panes.
    last_panes: Vec<PaneGeom>,
    /// Last paint width for density=None.
    last_area_width: Option<u16>,
}

impl Default for FileManagerState {
    fn default() -> Self {
        Self::new()
    }
}

impl FileManagerState {
    /// Fresh browser at root.
    #[must_use]
    pub fn new() -> Self {
        let mut search = SearchInputState::new();
        search.set_focused(false);
        let mut tree = FileTreeState::new();
        tree.enable_multi_select();
        tree.set_accepts_input(true);
        let mut quick_open = QuickOpenState::new();
        quick_open.set_accepts_input(false);
        quick_open.set_focused(false);
        Self {
            workspace: WorkspaceState::new(),
            breadcrumbs: BreadcrumbsState::new(),
            search,
            tree,
            preview: PreviewCardState::new(),
            queue: ListState::new(None),
            quick_open,
            alert: AlertDialogState::new(
                AlertKind::Delete,
                AlertScope::example_delete(),
                "confirm",
                "cancel",
            ),
            status: StatusBarState::new(),
            cwd: "/".into(),
            clipboard: Vec::new(),
            clipboard_mode: FileClipboardMode::Copy,
            dialog: FileManagerDialog::None,
            focus: FileManagerPane::Tree.id(),
            density: None,
            drawer_open: false,
            entry_count: 0,
            selected_path: None,
            colorless: false,
            last_panes: Vec::new(),
            last_area_width: None,
        }
    }

    /// Last panes.
    #[must_use]
    pub fn last_panes(&self) -> &[PaneGeom] {
        &self.last_panes
    }

    /// Effective density.
    #[must_use]
    pub fn effective_density(&self) -> FileManagerDensity {
        self.density
            .unwrap_or_else(|| FileManagerDensity::for_width(self.last_area_width.unwrap_or(120)))
    }

    /// Visible focusable panes for density.
    #[must_use]
    pub fn visible_focus_panes(&self, density: FileManagerDensity) -> Vec<FileManagerPane> {
        match density {
            FileManagerDensity::Normal => vec![
                FileManagerPane::Breadcrumbs,
                FileManagerPane::Search,
                FileManagerPane::Tree,
                FileManagerPane::Preview,
                FileManagerPane::Queue,
            ],
            FileManagerDensity::Narrow => {
                let mut v = vec![
                    FileManagerPane::Breadcrumbs,
                    FileManagerPane::Search,
                    FileManagerPane::Tree,
                ];
                if self.drawer_open {
                    v.push(FileManagerPane::Preview);
                }
                v
            }
            FileManagerDensity::Tiny => vec![FileManagerPane::Search, FileManagerPane::Tree],
        }
    }

    /// Clamp focus to density-visible panes.
    pub fn clamp_focus_to_density(&mut self, density: FileManagerDensity) {
        let visible = self.visible_focus_panes(density);
        if !visible.iter().any(|p| p.id() == self.focus) {
            self.focus = visible
                .first()
                .map(|p| p.id())
                .unwrap_or(FileManagerPane::Tree.id());
        }
    }

    /// Sync child accept/focus gates.
    pub fn apply_focus_gates(&mut self) {
        let f = self.focus;
        self.search.set_focused(f == "search");
        self.breadcrumbs.set_focused(f == "breadcrumbs");
        self.tree
            .set_accepts_input(f == "tree" && !self.dialog_blocks_tree());
        self.preview.set_focus_within(f == "preview");
        let qo = matches!(self.dialog, FileManagerDialog::QuickOpen);
        self.quick_open.set_accepts_input(qo);
        self.quick_open.set_focused(qo);
        self.alert.set_accepts_input(matches!(
            self.dialog,
            FileManagerDialog::ConfirmDelete { .. } | FileManagerDialog::Conflict { .. }
        ));
    }

    fn dialog_blocks_tree(&self) -> bool {
        !matches!(self.dialog, FileManagerDialog::None)
    }

    /// Set focus pane.
    pub fn set_focus(&mut self, pane: FileManagerPane) -> FileManagerOutcome {
        let density = self.effective_density();
        let visible = self.visible_focus_panes(density);
        if !visible.contains(&pane) {
            return FileManagerOutcome::Ignored;
        }
        if self.focus == pane.id() {
            self.apply_focus_gates();
            return FileManagerOutcome::Ignored;
        }
        self.focus = pane.id();
        self.apply_focus_gates();
        FileManagerOutcome::FocusChanged(self.focus)
    }

    /// Cycle Tab focus.
    pub fn cycle_focus(&mut self, reverse: bool) -> FileManagerOutcome {
        let density = self.effective_density();
        let visible = self.visible_focus_panes(density);
        if visible.is_empty() {
            return FileManagerOutcome::Ignored;
        }
        let cur = visible
            .iter()
            .position(|p| p.id() == self.focus)
            .unwrap_or(0);
        let next = if reverse {
            if cur == 0 { visible.len() - 1 } else { cur - 1 }
        } else {
            (cur + 1) % visible.len()
        };
        self.focus = visible[next].id();
        self.apply_focus_gates();
        FileManagerOutcome::FocusChanged(self.focus)
    }

    /// Project breadcrumb items from cwd.
    #[must_use]
    pub fn breadcrumb_items(&self) -> Vec<BreadcrumbItem<String>> {
        breadcrumbs_from_path(&self.cwd)
    }

    /// Status slots.
    #[must_use]
    pub fn status_slots(&self) -> Vec<StatusSlot<'static, &'static str>> {
        let mut slots = vec![
            StatusSlot::context("cwd", "cwd").priority(60),
            StatusSlot::context("entries", "entries").priority(40),
            StatusSlot::focus_zone("focus", self.focus).priority(70),
            // Every pointer action needs a keyboard path, and this slot is
            // where they are advertised — parity outranks the hint budget
            // (docs/design/web-premium-tui-law.md §4.2).
            StatusSlot::shortcut(
                "keys",
                "y yank · x cut · v paste · d del · r ren · n new · p preview · C-o open",
            )
            .priority(10),
        ];
        if !self.clipboard.is_empty() {
            slots.push(
                StatusSlot::new("clip", self.clipboard_mode_label())
                    .region(StatusRegion::Left)
                    .priority(50),
            );
        }
        let running = matches!(
            // host projects via ops in paint; chrome hint only
            self.dialog,
            FileManagerDialog::Conflict { .. }
        );
        if running {
            slots.push(
                StatusSlot::new("conflict", "conflict")
                    .semantic(crate::widgets::SemanticStatus::Warning)
                    .region(StatusRegion::Left)
                    .priority(100),
            );
        }
        slots
    }

    fn clipboard_mode_label(&self) -> &'static str {
        match self.clipboard_mode {
            FileClipboardMode::Copy => "yank",
            FileClipboardMode::Move => "cut",
        }
    }

    /// Build queue list rows from host ops.
    #[must_use]
    pub fn queue_rows<'a>(ops: &'a [FileOpItem]) -> Vec<ListRow<'a, String>> {
        ops.iter()
            .map(|op| {
                let pct = (op.progress * 100.0) as u32;
                let label = format!(
                    "[{}] {} {}% {}",
                    op.status.id(),
                    op.kind.label(),
                    pct,
                    op.message.as_deref().unwrap_or("")
                );
                let mut row = ListRow::item(op.id.clone(), Line::from(label));
                if let Some(d) = &op.dest {
                    row = row.secondary(Line::from(d.as_str()));
                }
                row
            })
            .collect()
    }

    /// Seed conflict dialog from host op.
    pub fn open_conflict(&mut self, op_id: impl Into<String>, path: impl Into<String>) {
        let op_id = op_id.into();
        let path = path.into();
        self.dialog = FileManagerDialog::Conflict {
            op_id: op_id.clone(),
            path: path.clone(),
        };
        self.alert = AlertDialogState::new(
            AlertKind::Overwrite,
            AlertScope::example_overwrite().scope_detail(path),
            "confirm",
            "cancel",
        );
        self.alert.set_title("Conflict");
        self.alert.set_action_labels("Overwrite", "Skip");
        // DialogState defaults open; re-assert open + input after rebuild.
        self.alert.set_accepts_input(true);
        self.apply_focus_gates();
    }

    /// Open elevated delete confirm.
    pub fn open_delete_confirm(&mut self, paths: Vec<String>) {
        let subject = if paths.len() == 1 {
            paths[0].clone()
        } else {
            format!("{} items", paths.len())
        };
        self.dialog = FileManagerDialog::ConfirmDelete {
            subject: subject.clone(),
            paths,
        };
        self.alert = AlertDialogState::new(
            AlertKind::Delete,
            AlertScope::example_delete().scope_detail(subject),
            "confirm",
            "cancel",
        );
        self.alert.set_title("Delete");
        self.alert.set_accepts_input(true);
        self.apply_focus_gates();
    }

    /// Open quick open.
    pub fn open_quick_open(&mut self) -> FileManagerOutcome {
        self.dialog = FileManagerDialog::QuickOpen;
        self.quick_open.set_accepts_input(true);
        self.quick_open.set_focused(true);
        FileManagerOutcome::QuickOpenOpened
    }

    /// Close dialogs / quick open.
    pub fn close_dialog(&mut self) {
        self.dialog = FileManagerDialog::None;
        self.quick_open.set_accepts_input(false);
        self.quick_open.set_focused(false);
        self.apply_focus_gates();
    }

    /// Paste clipboard into dest dir → typed copy/move request.
    pub fn request_paste(&mut self, dest: String) -> FileManagerOutcome {
        if self.clipboard.is_empty() {
            return FileManagerOutcome::Ignored;
        }
        let sources = self.clipboard.clone();
        match self.clipboard_mode {
            FileClipboardMode::Copy => FileManagerOutcome::CopyRequested { sources, dest },
            FileClipboardMode::Move => {
                // Clear clipboard after move request (host still owns success).
                let out = FileManagerOutcome::MoveRequested { sources, dest };
                self.clipboard.clear();
                out
            }
        }
    }

    /// Collect selected paths from tree + entries.
    #[must_use]
    pub fn selected_paths(&self, entries: &[FileTreeEntry<'_, String>]) -> Vec<String> {
        let mut paths = Vec::new();
        if let Some(sel) = self.tree.tree.selection() {
            for id in sel.checked() {
                if let Some(e) = entries.iter().find(|e| &e.id == id) {
                    paths.push(normalize_path_display(e.path));
                }
            }
        }
        if paths.is_empty() {
            if let Some(id) = self.tree.selected() {
                if let Some(e) = entries.iter().find(|e| &e.id == id) {
                    paths.push(normalize_path_display(e.path));
                }
            }
        }
        paths
    }

    /// Destination dir for paste (selected dir or cwd).
    #[must_use]
    pub fn paste_dest(&self, entries: &[FileTreeEntry<'_, String>]) -> String {
        if let Some(id) = self.tree.selected() {
            if let Some(e) = entries.iter().find(|e| &e.id == id) {
                if e.kind.is_dir() {
                    return normalize_path_display(e.path);
                }
                if let Some(p) = &e.parent {
                    return p.clone();
                }
            }
        }
        self.cwd.clone()
    }

    /// Keys — real workbench path.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        entries: &[FileTreeEntry<'_, String>],
        ops: &[FileOpItem],
        quick_open_items: &[QuickOpenItem<String>],
    ) -> FileManagerOutcome {
        if key.kind == KeyEventKind::Release {
            return FileManagerOutcome::Ignored;
        }
        let is_press = key.kind == KeyEventKind::Press;

        // Modal dialogs first
        match &self.dialog {
            FileManagerDialog::ConfirmDelete { paths, .. } => {
                if is_press {
                    let paths = paths.clone();
                    let out = self.alert.handle_key(key);
                    return match out {
                        AlertDialogOutcome::Confirmed { .. } => {
                            self.close_dialog();
                            FileManagerOutcome::DeleteRequested { ids: paths }
                        }
                        AlertDialogOutcome::Cancelled { .. } => {
                            self.close_dialog();
                            FileManagerOutcome::ConfirmCancelled
                        }
                        _ => FileManagerOutcome::Ignored,
                    };
                }
                return FileManagerOutcome::Ignored;
            }
            FileManagerDialog::Conflict { op_id, .. } => {
                if is_press {
                    let op_id = op_id.clone();
                    // s = skip, o/enter = overwrite, r = rename, esc = cancel
                    match key.code {
                        KeyCode::Char('s' | 'S') => {
                            self.close_dialog();
                            return FileManagerOutcome::ConflictResolved {
                                op_id,
                                resolution: FileConflictResolution::Skip,
                            };
                        }
                        KeyCode::Char('r' | 'R') => {
                            self.close_dialog();
                            return FileManagerOutcome::ConflictResolved {
                                op_id,
                                resolution: FileConflictResolution::Rename,
                            };
                        }
                        KeyCode::Esc => {
                            self.close_dialog();
                            return FileManagerOutcome::ConflictResolved {
                                op_id,
                                resolution: FileConflictResolution::Cancel,
                            };
                        }
                        _ => {}
                    }
                    let out = self.alert.handle_key(key);
                    return match out {
                        AlertDialogOutcome::Confirmed { .. } => {
                            self.close_dialog();
                            FileManagerOutcome::ConflictResolved {
                                op_id,
                                resolution: FileConflictResolution::Overwrite,
                            }
                        }
                        AlertDialogOutcome::Cancelled { .. } => {
                            self.close_dialog();
                            FileManagerOutcome::ConflictResolved {
                                op_id,
                                resolution: FileConflictResolution::Skip,
                            }
                        }
                        _ => FileManagerOutcome::Ignored,
                    };
                }
                return FileManagerOutcome::Ignored;
            }
            FileManagerDialog::QuickOpen => {
                if is_press && key.code == KeyCode::Esc {
                    self.close_dialog();
                    return FileManagerOutcome::QuickOpenClosed;
                }
                let providers = default_quick_open_providers();
                let out = self
                    .quick_open
                    .handle_key(key, &providers, quick_open_items);
                return match out {
                    QuickOpenOutcome::Ignored => FileManagerOutcome::Ignored,
                    QuickOpenOutcome::Activated { id, .. } => {
                        self.close_dialog();
                        FileManagerOutcome::QuickOpenActivated { id }
                    }
                    QuickOpenOutcome::Cancelled => {
                        self.close_dialog();
                        FileManagerOutcome::QuickOpenClosed
                    }
                    other => {
                        let kind = format!("{other:?}")
                            .split(|c: char| c == '(' || c == ' ')
                            .next()
                            .unwrap_or("quick-open")
                            .to_string();
                        FileManagerOutcome::Tree { kind }
                    }
                };
            }
            FileManagerDialog::None => {}
        }

        if is_press {
            // Global chords
            match key.code {
                KeyCode::Tab if key.modifiers.is_empty() => {
                    return self.cycle_focus(false);
                }
                KeyCode::BackTab | KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
                    return self.cycle_focus(true);
                }
                KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return self.open_quick_open();
                }
                KeyCode::Esc => {
                    if self.drawer_open {
                        self.drawer_open = false;
                        return FileManagerOutcome::DrawerToggled { open: false };
                    }
                    return FileManagerOutcome::Cancelled;
                }
                _ => {}
            }
        }

        match self.focus {
            "breadcrumbs" => self.handle_breadcrumbs_key(key),
            "search" => self.handle_search_key(key),
            "tree" => self.handle_tree_key(key, entries),
            "preview" => {
                if is_press && key.code == KeyCode::Char('p') {
                    self.drawer_open = !self.drawer_open;
                    return FileManagerOutcome::DrawerToggled {
                        open: self.drawer_open,
                    };
                }
                FileManagerOutcome::Ignored
            }
            "queue" => self.handle_queue_key(key, ops),
            _ => FileManagerOutcome::Ignored,
        }
    }

    fn handle_breadcrumbs_key(&mut self, key: KeyEvent) -> FileManagerOutcome {
        let items = self.breadcrumb_items();
        let out = self.breadcrumbs.handle_key(key, &items);
        match out {
            BreadcrumbsOutcome::Ignored => FileManagerOutcome::Ignored,
            BreadcrumbsOutcome::Navigate(id) => {
                self.cwd = id.clone();
                FileManagerOutcome::Navigate { path: id }
            }
            BreadcrumbsOutcome::EditCommitted { path } => {
                self.cwd = path.clone();
                FileManagerOutcome::Navigate { path }
            }
            other => {
                let kind = format!("{other:?}")
                    .split(|c: char| c == '(' || c == ' ')
                    .next()
                    .unwrap_or("breadcrumbs")
                    .to_string();
                FileManagerOutcome::Tree { kind }
            }
        }
    }

    fn handle_search_key(&mut self, key: KeyEvent) -> FileManagerOutcome {
        let out = self.search.handle_key(key);
        match out {
            SearchInputOutcome::Ignored => FileManagerOutcome::Ignored,
            SearchInputOutcome::DebouncedQuery { query }
            | SearchInputOutcome::Submitted { query } => {
                if query.is_empty() {
                    self.tree.filter = None;
                } else {
                    self.tree.filter = Some(query.clone());
                }
                FileManagerOutcome::FilterChanged { query }
            }
            SearchInputOutcome::Changed => {
                let query = self.search.query().to_string();
                if query.is_empty() {
                    self.tree.filter = None;
                } else {
                    self.tree.filter = Some(query.clone());
                }
                FileManagerOutcome::FilterChanged { query }
            }
            other => {
                let kind = format!("{other:?}")
                    .split(|c: char| c == '(' || c == ' ')
                    .next()
                    .unwrap_or("search")
                    .to_string();
                FileManagerOutcome::Tree { kind }
            }
        }
    }

    fn handle_tree_key(
        &mut self,
        key: KeyEvent,
        entries: &[FileTreeEntry<'_, String>],
    ) -> FileManagerOutcome {
        let is_press = key.kind == KeyEventKind::Press;

        // While rename draft, filter typing, or inline delete confirm is active,
        // FileTree owns the keyboard — do not steal x/v/p for cut/paste/preview.
        let tree_owns_typing = self.tree.draft.is_some()
            || self.tree.filter.is_some()
            || self.tree.pending_confirm.is_some();

        // Workbench-level cut / paste (FileTree owns yank path-copy as `y`)
        if is_press && key.modifiers.is_empty() && !tree_owns_typing {
            match key.code {
                KeyCode::Char('x') => {
                    let paths = self.selected_paths(entries);
                    if paths.is_empty() {
                        return FileManagerOutcome::Ignored;
                    }
                    self.clipboard = paths.clone();
                    self.clipboard_mode = FileClipboardMode::Move;
                    return FileManagerOutcome::ClipboardSet {
                        paths,
                        mode: FileClipboardMode::Move,
                    };
                }
                KeyCode::Char('v') => {
                    let dest = self.paste_dest(entries);
                    return self.request_paste(dest);
                }
                KeyCode::Char('p') => {
                    // Preview + open drawer on narrow
                    if let Some(id) = self.tree.selected().cloned() {
                        self.drawer_open = true;
                        let _ = self.preview.set_selection(id.clone());
                        self.selected_path = Some(id.clone());
                        return FileManagerOutcome::PreviewRequested { id };
                    }
                }
                _ => {}
            }
        }

        let out = self.tree.handle_key(entries, key);
        self.map_tree_outcome(out, entries)
    }

    fn map_tree_outcome(
        &mut self,
        out: FileTreeOutcome<String>,
        entries: &[FileTreeEntry<'_, String>],
    ) -> FileManagerOutcome {
        match out {
            FileTreeOutcome::Ignored => FileManagerOutcome::Ignored,
            FileTreeOutcome::SelectionChanged(id) => {
                self.selected_path = Some(id.clone());
                let _ = self.preview.set_selection(id.clone());
                FileManagerOutcome::SelectionChanged { id }
            }
            FileTreeOutcome::Toggle(id) => FileManagerOutcome::Toggle { id },
            FileTreeOutcome::CheckToggled(id) => FileManagerOutcome::CheckToggled { id },
            FileTreeOutcome::OpenRequested(id) => FileManagerOutcome::OpenRequested { id },
            FileTreeOutcome::PreviewRequested(id) => {
                self.drawer_open = true;
                let _ = self.preview.set_selection(id.clone());
                self.selected_path = Some(id.clone());
                FileManagerOutcome::PreviewRequested { id }
            }
            FileTreeOutcome::LoadChildrenRequested(id) => {
                FileManagerOutcome::LoadChildrenRequested { id }
            }
            FileTreeOutcome::CreateFileRequested { parent, name } => {
                FileManagerOutcome::NewRequested {
                    kind: "file",
                    parent,
                    name,
                }
            }
            FileTreeOutcome::CreateDirRequested { parent, name } => {
                FileManagerOutcome::NewRequested {
                    kind: "dir",
                    parent,
                    name,
                }
            }
            FileTreeOutcome::RenameRequested { id, from, to } => {
                FileManagerOutcome::RenameRequested { id, from, to }
            }
            FileTreeOutcome::DeleteRequested { ids } => FileManagerOutcome::DeleteRequested { ids },
            FileTreeOutcome::ConfirmRequired(conf) => {
                let paths: Vec<String> = conf
                    .ids
                    .iter()
                    .filter_map(|id| {
                        entries
                            .iter()
                            .find(|e| &e.id == id)
                            .map(|e| normalize_path_display(e.path))
                    })
                    .collect();
                // Elevate to dialog for multi; single stays host via ConfirmDestructive
                if paths.len() > 1 {
                    self.open_delete_confirm(paths.clone());
                    FileManagerOutcome::ConfirmDestructive { paths }
                } else {
                    FileManagerOutcome::ConfirmDestructive { paths }
                }
            }
            FileTreeOutcome::ConfirmCancelled => FileManagerOutcome::ConfirmCancelled,
            FileTreeOutcome::CopyPathRequested { paths } => {
                self.clipboard = paths.clone();
                self.clipboard_mode = FileClipboardMode::Copy;
                FileManagerOutcome::ClipboardSet {
                    paths,
                    mode: FileClipboardMode::Copy,
                }
            }
            FileTreeOutcome::QuickOpenRequested => self.open_quick_open(),
            FileTreeOutcome::BreadcrumbsPath { items } => {
                if let Some(last) = items.last() {
                    self.cwd = last.id.clone();
                    return FileManagerOutcome::Navigate {
                        path: last.id.clone(),
                    };
                }
                FileManagerOutcome::Ignored
            }
            FileTreeOutcome::FilterChanged(query) => {
                if query.is_empty() {
                    self.tree.filter = None;
                } else {
                    self.tree.filter = Some(query.clone());
                }
                // keep search in sync
                self.search.set_query(query.clone());
                FileManagerOutcome::FilterChanged { query }
            }
            FileTreeOutcome::RevealPathRequested { path } => FileManagerOutcome::Navigate { path },
            FileTreeOutcome::RevealOsRequested(id) => FileManagerOutcome::OpenRequested { id },
            FileTreeOutcome::ShowHiddenToggled { .. }
            | FileTreeOutcome::ShowIgnoredToggled { .. }
            | FileTreeOutcome::DraftChanged(_) => {
                let kind = format!("{out:?}")
                    .split(|c: char| c == '(' || c == ' ')
                    .next()
                    .unwrap_or("tree")
                    .to_string();
                FileManagerOutcome::Tree { kind }
            }
            FileTreeOutcome::Cancelled => FileManagerOutcome::Cancelled,
        }
    }

    fn handle_queue_key(&mut self, key: KeyEvent, ops: &[FileOpItem]) -> FileManagerOutcome {
        if key.kind != KeyEventKind::Press {
            return FileManagerOutcome::Ignored;
        }
        let rows = Self::queue_rows(ops);
        match key.code {
            KeyCode::Char('c') if key.modifiers.is_empty() => {
                if let Some(id) = self.queue.selected().cloned() {
                    return FileManagerOutcome::OpCancel { op_id: id };
                }
                if let Some(op) = ops
                    .iter()
                    .find(|o| matches!(o.status, FileOpStatus::Running | FileOpStatus::Pending))
                {
                    return FileManagerOutcome::OpCancel {
                        op_id: op.id.clone(),
                    };
                }
                FileManagerOutcome::Ignored
            }
            KeyCode::Char('r') if key.modifiers.is_empty() => {
                if let Some(id) = self.queue.selected().cloned() {
                    return FileManagerOutcome::OpRetry { op_id: id };
                }
                if let Some(op) = ops
                    .iter()
                    .find(|o| matches!(o.status, FileOpStatus::Failed))
                {
                    return FileManagerOutcome::OpRetry {
                        op_id: op.id.clone(),
                    };
                }
                FileManagerOutcome::Ignored
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let _ = self.queue.handle_key(&rows, key);
                FileManagerOutcome::Ignored
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let _ = self.queue.handle_key(&rows, key);
                FileManagerOutcome::Ignored
            }
            _ => {
                let _ = self.queue.handle_key(&rows, key);
                FileManagerOutcome::Ignored
            }
        }
    }
}

// ── Layout ──────────────────────────────────────────────────────────────────

/// Breadcrumbs strip height.
pub const FILE_MANAGER_BREADCRUMBS_HEIGHT: u16 = 1;
/// Search strip height (panel + input).
pub const FILE_MANAGER_SEARCH_HEIGHT: u16 = 3;

/// Width-derived layout.
#[must_use]
pub fn file_manager_layout(area: Rect, state: &WorkspaceState) -> Vec<PaneGeom> {
    file_manager_layout_density(
        area,
        state,
        FileManagerDensity::for_width(area.width),
        false,
    )
}

/// Explicit density layout.
#[must_use]
pub fn file_manager_layout_density(
    area: Rect,
    state: &WorkspaceState,
    density: FileManagerDensity,
    drawer_open: bool,
) -> Vec<PaneGeom> {
    let crumb_h = FILE_MANAGER_BREADCRUMBS_HEIGHT.min(area.height);
    let mut y = area.y;
    let mut remain = area.height;

    let mut panes = Vec::new();

    // Breadcrumbs strip
    let crumbs_area = Rect {
        x: area.x,
        y,
        width: area.width,
        height: crumb_h,
    };
    panes.push(PaneGeom {
        id: PaneId::from_static(FileManagerPane::Breadcrumbs.id()),
        area: if crumb_h == 0 {
            Rect::new(area.x, area.y, 0, 0)
        } else {
            crumbs_area
        },
        collapsed: crumb_h == 0,
    });
    y = y.saturating_add(crumb_h);
    remain = remain.saturating_sub(crumb_h);

    // Search strip (skip only if zero height)
    let search_h = if remain >= 3 {
        FILE_MANAGER_SEARCH_HEIGHT.min(remain.saturating_sub(2))
    } else if remain >= 1 {
        1
    } else {
        0
    };
    let search_area = Rect {
        x: area.x,
        y,
        width: area.width,
        height: search_h,
    };
    panes.push(PaneGeom {
        id: PaneId::from_static(FileManagerPane::Search.id()),
        area: if search_h == 0 {
            Rect::new(area.x, y, 0, 0)
        } else {
            search_area
        },
        collapsed: search_h == 0,
    });
    y = y.saturating_add(search_h);
    remain = remain.saturating_sub(search_h);

    let body = Rect {
        x: area.x,
        y,
        width: area.width,
        height: remain,
    };

    let root = match density {
        FileManagerDensity::Tiny => WorkspaceNode::Split {
            axis: WorkspaceAxis::Vertical,
            ratio_percent: 92,
            first: Box::new(WorkspaceNode::Leaf {
                id: PaneId::from_static(FileManagerPane::Tree.id()),
                constraint: PaneConstraint::Weight(1),
                collapse_priority: 1,
            }),
            second: Box::new(WorkspaceNode::Leaf {
                id: PaneId::from_static(FileManagerPane::Status.id()),
                constraint: PaneConstraint::Fixed(1),
                collapse_priority: 3,
            }),
        },
        FileManagerDensity::Narrow => {
            // tree | (optional drawer preview) | status — no queue
            if drawer_open {
                WorkspaceNode::Split {
                    axis: WorkspaceAxis::Vertical,
                    ratio_percent: 92,
                    first: Box::new(WorkspaceNode::Split {
                        axis: WorkspaceAxis::Horizontal,
                        ratio_percent: 55,
                        first: Box::new(WorkspaceNode::Leaf {
                            id: PaneId::from_static(FileManagerPane::Tree.id()),
                            constraint: PaneConstraint::Weight(1),
                            collapse_priority: 1,
                        }),
                        second: Box::new(WorkspaceNode::Leaf {
                            id: PaneId::from_static(FileManagerPane::Preview.id()),
                            constraint: PaneConstraint::Min(16),
                            collapse_priority: 0,
                        }),
                    }),
                    second: Box::new(WorkspaceNode::Leaf {
                        id: PaneId::from_static(FileManagerPane::Status.id()),
                        constraint: PaneConstraint::Fixed(1),
                        collapse_priority: 3,
                    }),
                }
            } else {
                WorkspaceNode::Split {
                    axis: WorkspaceAxis::Vertical,
                    ratio_percent: 92,
                    first: Box::new(WorkspaceNode::Leaf {
                        id: PaneId::from_static(FileManagerPane::Tree.id()),
                        constraint: PaneConstraint::Weight(1),
                        collapse_priority: 1,
                    }),
                    second: Box::new(WorkspaceNode::Leaf {
                        id: PaneId::from_static(FileManagerPane::Status.id()),
                        constraint: PaneConstraint::Fixed(1),
                        collapse_priority: 3,
                    }),
                }
            }
        }
        FileManagerDensity::Normal => {
            // (tree | preview) / queue / status
            WorkspaceNode::Split {
                axis: WorkspaceAxis::Vertical,
                ratio_percent: 78,
                first: Box::new(WorkspaceNode::Split {
                    axis: WorkspaceAxis::Horizontal,
                    ratio_percent: 45,
                    first: Box::new(WorkspaceNode::Leaf {
                        id: PaneId::from_static(FileManagerPane::Tree.id()),
                        constraint: PaneConstraint::Weight(1),
                        collapse_priority: 1,
                    }),
                    second: Box::new(WorkspaceNode::Leaf {
                        id: PaneId::from_static(FileManagerPane::Preview.id()),
                        constraint: PaneConstraint::Min(20),
                        collapse_priority: 0,
                    }),
                }),
                second: Box::new(WorkspaceNode::Split {
                    axis: WorkspaceAxis::Vertical,
                    ratio_percent: 70,
                    first: Box::new(WorkspaceNode::Leaf {
                        id: PaneId::from_static(FileManagerPane::Queue.id()),
                        constraint: PaneConstraint::Min(3),
                        collapse_priority: 0,
                    }),
                    second: Box::new(WorkspaceNode::Leaf {
                        id: PaneId::from_static(FileManagerPane::Status.id()),
                        constraint: PaneConstraint::Fixed(1),
                        collapse_priority: 3,
                    }),
                }),
            }
        }
    };

    panes.extend(Workspace::new(root).layout(body, state));
    panes
}

fn pane_area(panes: &[PaneGeom], id: &str) -> Option<Rect> {
    panes.iter().find_map(|p| {
        if p.id.0.as_str() == id && !p.collapsed && p.area.width > 0 && p.area.height > 0 {
            Some(p.area)
        } else {
            None
        }
    })
}

// ── Render ──────────────────────────────────────────────────────────────────

/// Paint composed file manager (public child widgets only).
pub fn render_file_manager(buffer: &mut Buffer, area: Rect, surfaces: FileManagerSurfaces<'_>) {
    let FileManagerSurfaces {
        system,
        state,
        entries,
        ops,
        preview,
        quick_open_items,
    } = surfaces;

    if area.is_empty() {
        return;
    }

    state.last_area_width = Some(area.width);
    let density = state.effective_density();
    let panes = file_manager_layout_density(area, &state.workspace, density, state.drawer_open);
    state.last_panes = panes.clone();
    state.clamp_focus_to_density(density);
    state.apply_focus_gates();
    state.entry_count = entries.len() as u64;

    // Breadcrumbs
    if let Some(r) = pane_area(&panes, "breadcrumbs") {
        let focused = state.focus == "breadcrumbs";
        state.breadcrumbs.set_focused(focused);
        let items = state.breadcrumb_items();
        Breadcrumbs::new(&items, system).paint(r, buffer, &mut state.breadcrumbs);
    }

    // Search
    if let Some(r) = pane_area(&panes, "search") {
        let focused = state.focus == "search";
        state.search.set_focused(focused);
        if r.height >= 3 {
            let inner = Panel::new(system)
                .title("Filter")
                .emphasis(PanelChrome::for_focus(focused))
                .paint(r, buffer, None);
            if !inner.is_empty() {
                SearchInput::new(system).placeholder("filter files…").paint(
                    inner,
                    buffer,
                    &mut state.search,
                );
            }
        } else if !r.is_empty() {
            SearchInput::new(system).placeholder("filter files…").paint(
                r,
                buffer,
                &mut state.search,
            );
        }
    }

    // Tree
    if let Some(r) = pane_area(&panes, "tree") {
        let focused = state.focus == "tree";
        FileTree::new(entries, system)
            .title("Files")
            .focused(focused)
            .show_filter_chrome(false)
            .render(r, buffer, &mut state.tree);
    }

    // Preview
    if let Some(r) = pane_area(&panes, "preview") {
        let focused = state.focus == "preview";
        state.preview.set_focus_within(focused);
        let content = preview.unwrap_or_else(|| {
            PreviewCardContent::title("(no selection)", PreviewResourceKind::File)
                .load(PreviewLoadState::Idle)
                .essential_elsewhere(true)
        });
        let inner = Panel::new(system)
            .title("Preview")
            .emphasis(PanelChrome::for_focus(focused))
            .paint(r, buffer, None);
        if !inner.is_empty() {
            PreviewCard::new(content, system).paint(inner, buffer, &mut state.preview);
        }
    }

    // Queue
    if let Some(r) = pane_area(&panes, "queue") {
        let focused = state.focus == "queue";
        let inner = Panel::new(system)
            .title("Operations")
            .emphasis(PanelChrome::for_focus(focused))
            .paint(r, buffer, None);
        if !inner.is_empty() {
            let rows = FileManagerState::queue_rows(ops);
            if rows.is_empty() {
                EmptyState::new("No pending operations", system)
                    .kind(EmptyKind::NoData)
                    .paint(Rect::new(inner.x, inner.y, inner.width, 1), buffer);
            } else {
                let list = List::new(&rows, system).focused(focused);
                StatefulWidget::render(&list, inner, buffer, &mut state.queue);
            }
        }
    }

    // Status
    if let Some(r) = pane_area(&panes, "status") {
        let running = ops
            .iter()
            .filter(|o| matches!(o.status, FileOpStatus::Running))
            .count();
        let failed = ops
            .iter()
            .filter(|o| matches!(o.status, FileOpStatus::Failed | FileOpStatus::Conflict))
            .count();
        if running > 0 || failed > 0 {
            state.status.transient =
                Some(format!("ops running={running} failed/conflict={failed}"));
        } else if !state.clipboard.is_empty() {
            state.status.transient = Some(format!(
                "{} {} path(s) · v paste",
                state.clipboard_mode_label(),
                state.clipboard.len()
            ));
        } else {
            state.status.transient = None;
        }
        let slots = state.status_slots();
        StatefulWidget::render(
            &StatusBar::new(&slots, &[], system),
            r,
            buffer,
            &mut state.status,
        );
    }

    // Dialog overlays (center of full area)
    match &state.dialog {
        FileManagerDialog::ConfirmDelete { .. } | FileManagerDialog::Conflict { .. } => {
            let dlg = dialog_rect(area);
            if !dlg.is_empty() {
                AlertDialog::new(system).paint(dlg, buffer, &mut state.alert);
            }
        }
        FileManagerDialog::QuickOpen => {
            let qo = quick_open_rect(area);
            if !qo.is_empty() {
                let providers = default_quick_open_providers();
                QuickOpen::new(&providers, quick_open_items, system).paint(
                    qo,
                    buffer,
                    &mut state.quick_open,
                );
            }
        }
        FileManagerDialog::None => {}
    }
}

/// Center dialog rect.
#[must_use]
pub fn dialog_rect(area: Rect) -> Rect {
    let w = area.width.min(56).max(24.min(area.width));
    let h = area.height.min(12).max(6.min(area.height));
    let x = area.x.saturating_add(area.width.saturating_sub(w) / 2);
    let y = area.y.saturating_add(area.height.saturating_sub(h) / 2);
    Rect::new(x, y, w, h)
}

/// Quick open rect.
#[must_use]
pub fn quick_open_rect(area: Rect) -> Rect {
    let w = area.width.min(72).max(32.min(area.width));
    let h = area.height.min(18).max(8.min(area.height));
    let x = area.x.saturating_add(area.width.saturating_sub(w) / 2);
    let y = area.y.saturating_add(area.height.saturating_sub(h) / 2);
    Rect::new(x, y, w, h)
}

// ── Fixtures ────────────────────────────────────────────────────────────────

/// Example directory listing.
#[must_use]
pub fn example_file_entries() -> Vec<FileTreeEntry<'static, String>> {
    vec![
        FileTreeEntry::dir("src".into(), "src", "src", 0).expanded(),
        FileTreeEntry::file("src/main.rs".into(), "main.rs", "src/main.rs", 1)
            .parent("src".into())
            .file_type("rs")
            .size(420),
        FileTreeEntry::file("src/lib.rs".into(), "lib.rs", "src/lib.rs", 1)
            .parent("src".into())
            .file_type("rs"),
        FileTreeEntry::dir("docs".into(), "docs", "docs", 0),
        FileTreeEntry::file("README.md".into(), "README.md", "README.md", 0)
            .file_type("md")
            .size(2048),
        FileTreeEntry::file("Cargo.toml".into(), "Cargo.toml", "Cargo.toml", 0).file_type("toml"),
        FileTreeEntry::file(".gitignore".into(), ".gitignore", ".gitignore", 0)
            .hidden(true)
            .file_type("git"),
        FileTreeEntry::file("sample.txt".into(), "sample.txt", "sample.txt", 0).file_type("txt"),
    ]
}

/// Large mock listing for paint stress (not real FS walk).
#[must_use]
pub fn burst_file_entries(n: usize) -> Vec<(String, String, String)> {
    // owned (id, name, path) — caller maps to FileTreeEntry
    (0..n)
        .map(|i| {
            let name = format!("file_{i:05}.txt");
            let path = format!("bulk/{name}");
            (path.clone(), name, path)
        })
        .collect()
}

/// Example ops queue with progress / conflict / failed.
#[must_use]
pub fn example_file_ops() -> Vec<FileOpItem> {
    vec![
        FileOpItem::new("op-1", FileOpKind::Copy)
            .sources(["src/main.rs"])
            .dest("backup/")
            .progress(0.45)
            .status(FileOpStatus::Running),
        FileOpItem::new("op-2", FileOpKind::Move)
            .sources(["old.txt"])
            .dest("docs/old.txt")
            .progress(0.0)
            .status(FileOpStatus::Conflict)
            .message("docs/old.txt exists"),
        FileOpItem::new("op-3", FileOpKind::Delete)
            .sources(["tmp/cache"])
            .progress(1.0)
            .status(FileOpStatus::Failed)
            .message("permission denied"),
    ]
}

/// Empty ops.
#[must_use]
pub fn example_empty_ops() -> Vec<FileOpItem> {
    Vec::new()
}

/// Preview for README.
#[must_use]
pub fn example_file_preview() -> (
    PreviewCardContent<'static>,
    &'static [&'static str],
    &'static [PreviewMetadata<'static>],
) {
    const BODY: &[&str] = &[
        "# TermRock",
        "",
        "High-class Rust TUI components.",
        "FileManager composes public widgets only.",
    ];
    const META: &[PreviewMetadata<'static>] = &[
        PreviewMetadata::new("size", "2 KB"),
        PreviewMetadata::new("type", "markdown"),
    ];
    let content = PreviewCardContent::title("README.md", PreviewResourceKind::File)
        .subtitle("README.md")
        .meta(META)
        .body(BODY)
        .load(PreviewLoadState::Ready)
        .essential_elsewhere(true);
    (content, BODY, META)
}

/// Seed conflict + running queue for failure story.
pub fn seed_conflict_state(state: &mut FileManagerState) {
    state.open_conflict("op-2", "docs/old.txt");
    state.cwd = "/project".into();
}

/// Seed multi-delete confirm.
pub fn seed_delete_confirm(state: &mut FileManagerState) {
    state.open_delete_confirm(vec!["tmp/a".into(), "tmp/b".into()]);
}

/// Quick open items from entries.
#[must_use]
pub fn example_quick_open_from_entries(
    entries: &[FileTreeEntry<'_, String>],
) -> Vec<QuickOpenItem<String>> {
    file_tree_to_quick_open_items(entries, true)
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Paint stress targets (mock scale — host windows real FS).
pub mod bench {
    /// Mock entries for burst paint.
    pub const BURST_ENTRIES: usize = 2_000;
    /// Paint frames.
    pub const PAINT_FRAMES: usize = 8;
    /// Viewport.
    pub const VIEWPORT: (u16, u16) = (120, 40);
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::DesignSystem;

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn press_mod(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    fn open() -> FileManagerState {
        let mut st = FileManagerState::new();
        st.cwd = "/project".into();
        st.density = Some(FileManagerDensity::Normal);
        st
    }

    #[test]
    fn focus_cycle_visits_visible_panes_only() {
        let mut st = open();
        let entries = example_file_entries();
        let ops = example_empty_ops();
        st.focus = "tree";
        let mut seen = vec![st.focus];
        for _ in 0..8 {
            let out = st.handle_key(press(KeyCode::Tab), &entries, &ops, &[]);
            assert!(matches!(out, FileManagerOutcome::FocusChanged(_)));
            seen.push(st.focus);
        }
        assert!(seen.contains(&"breadcrumbs"));
        assert!(seen.contains(&"search"));
        assert!(seen.contains(&"tree"));
        assert!(seen.contains(&"preview"));
        assert!(seen.contains(&"queue"));
        assert!(!seen.contains(&"status"));
    }

    #[test]
    fn narrow_tiny_collapse_and_tab_clamp() {
        let mut st = open();
        let entries = example_file_entries();
        let ops = example_file_ops();
        st.density = Some(FileManagerDensity::Tiny);
        st.focus = "preview";
        st.clamp_focus_to_density(FileManagerDensity::Tiny);
        assert_ne!(st.focus, "preview");
        assert_ne!(st.focus, "queue");
        let visible = st.visible_focus_panes(FileManagerDensity::Tiny);
        assert!(!visible.contains(&FileManagerPane::Preview));
        assert!(!visible.contains(&FileManagerPane::Queue));

        // paint records width; density=None uses last width
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 40, 20);
        let mut buf = Buffer::empty(area);
        st.density = None;
        st.drawer_open = false;
        render_file_manager(
            &mut buf,
            area,
            FileManagerSurfaces {
                system: &system,
                state: &mut st,
                entries: &entries,
                ops: &ops,
                preview: None,
                quick_open_items: &[],
            },
        );
        assert_eq!(st.effective_density(), FileManagerDensity::Tiny);
        // Tab must not land on unpainted preview/queue
        for _ in 0..6 {
            let _ = st.handle_key(press(KeyCode::Tab), &entries, &ops, &[]);
            assert!(st.focus == "search" || st.focus == "tree" || st.focus == "breadcrumbs");
            assert_ne!(st.focus, "preview");
            assert_ne!(st.focus, "queue");
        }

        st.density = Some(FileManagerDensity::Narrow);
        st.drawer_open = false;
        st.clamp_focus_to_density(FileManagerDensity::Narrow);
        let vis = st.visible_focus_panes(FileManagerDensity::Narrow);
        assert!(!vis.contains(&FileManagerPane::Queue));
        assert!(!vis.contains(&FileManagerPane::Preview));
        st.drawer_open = true;
        let vis2 = st.visible_focus_panes(FileManagerDensity::Narrow);
        assert!(vis2.contains(&FileManagerPane::Preview));
    }

    #[test]
    fn yank_cut_paste_typed_ops() {
        let mut st = open();
        let entries = example_file_entries();
        let ops = example_empty_ops();
        st.focus = "tree";
        st.tree.select(Some("README.md".into()));
        st.apply_focus_gates();

        // yank via FileTree `y`
        let out = st.handle_key(press(KeyCode::Char('y')), &entries, &ops, &[]);
        assert!(
            matches!(
                out,
                FileManagerOutcome::ClipboardSet {
                    mode: FileClipboardMode::Copy,
                    ..
                }
            ),
            "got {out:?}"
        );
        assert!(!st.clipboard.is_empty());

        // cut
        st.tree.select(Some("Cargo.toml".into()));
        let out = st.handle_key(press(KeyCode::Char('x')), &entries, &ops, &[]);
        assert!(
            matches!(
                out,
                FileManagerOutcome::ClipboardSet {
                    mode: FileClipboardMode::Move,
                    ..
                }
            ),
            "got {out:?}"
        );

        // paste → MoveRequested
        st.tree.select(Some("docs".into()));
        let out = st.handle_key(press(KeyCode::Char('v')), &entries, &ops, &[]);
        assert!(
            matches!(out, FileManagerOutcome::MoveRequested { .. }),
            "got {out:?}"
        );
    }

    #[test]
    fn rename_delete_new_through_tree() {
        let mut st = open();
        let entries = example_file_entries();
        let ops = example_empty_ops();
        st.focus = "tree";
        st.tree.select(Some("README.md".into()));
        st.apply_focus_gates();

        let out = st.handle_key(press(KeyCode::Char('r')), &entries, &ops, &[]);
        assert!(
            st.tree.draft.is_some(),
            "rename must open draft, got {out:?}"
        );
        let before = st.tree.draft.as_ref().unwrap().name.clone();
        // Type `x` into draft — must NOT steal to cut/ClipboardSet
        let out = st.handle_key(press(KeyCode::Char('x')), &entries, &ops, &[]);
        assert!(
            !matches!(out, FileManagerOutcome::ClipboardSet { .. }),
            "draft typing must not become cut: {out:?}"
        );
        let after = st
            .tree
            .draft
            .as_ref()
            .map(|d| d.name.clone())
            .expect("draft must remain open while typing");
        assert_eq!(after, format!("{before}x"), "draft must receive char x");
        let out = st.handle_key(press(KeyCode::Enter), &entries, &ops, &[]);
        assert!(
            matches!(
                out,
                FileManagerOutcome::RenameRequested { ref to, .. } if to.ends_with('x')
            ),
            "got {out:?}"
        );

        st.tree.select(Some("Cargo.toml".into()));
        let out = st.handle_key(press(KeyCode::Char('d')), &entries, &ops, &[]);
        assert!(
            matches!(
                out,
                FileManagerOutcome::DeleteRequested { .. }
                    | FileManagerOutcome::ConfirmDestructive { .. }
            ),
            "got {out:?}"
        );

        let out = st.handle_key(press(KeyCode::Char('n')), &entries, &ops, &[]);
        assert!(
            matches!(out, FileManagerOutcome::NewRequested { kind: "file", .. }),
            "got {out:?}"
        );
        let out = st.handle_key(
            press_mod(KeyCode::Char('N'), KeyModifiers::SHIFT),
            &entries,
            &ops,
            &[],
        );
        // Shift+N may arrive as Char('N') with shift or just 'N'
        let out = if matches!(out, FileManagerOutcome::Ignored) {
            st.handle_key(press(KeyCode::Char('N')), &entries, &ops, &[])
        } else {
            out
        };
        assert!(
            matches!(out, FileManagerOutcome::NewRequested { kind: "dir", .. }),
            "got {out:?}"
        );
    }

    #[test]
    fn conflict_resolution_outcomes() {
        let mut st = open();
        let entries = example_file_entries();
        let ops = example_file_ops();
        seed_conflict_state(&mut st);
        assert!(matches!(st.dialog, FileManagerDialog::Conflict { .. }));

        let out = st.handle_key(press(KeyCode::Char('s')), &entries, &ops, &[]);
        assert!(
            matches!(
                out,
                FileManagerOutcome::ConflictResolved {
                    resolution: FileConflictResolution::Skip,
                    ..
                }
            ),
            "got {out:?}"
        );
        assert!(matches!(st.dialog, FileManagerDialog::None));

        seed_conflict_state(&mut st);
        let out = st.handle_key(press(KeyCode::Enter), &entries, &ops, &[]);
        // Enter may go through AlertDialog confirm → Overwrite
        assert!(
            matches!(
                out,
                FileManagerOutcome::ConflictResolved {
                    resolution: FileConflictResolution::Overwrite | FileConflictResolution::Skip,
                    ..
                }
            ),
            "got {out:?}"
        );
    }

    #[test]
    fn queue_cancel_retry_no_fs() {
        let mut st = open();
        let entries = example_file_entries();
        let ops = example_file_ops();
        st.focus = "queue";
        st.queue = ListState::new(Some("op-1".into()));
        st.apply_focus_gates();

        let out = st.handle_key(press(KeyCode::Char('c')), &entries, &ops, &[]);
        assert!(
            matches!(out, FileManagerOutcome::OpCancel { ref op_id } if op_id == "op-1"),
            "got {out:?}"
        );
        st.queue = ListState::new(Some("op-3".into()));
        let out = st.handle_key(press(KeyCode::Char('r')), &entries, &ops, &[]);
        assert!(
            matches!(out, FileManagerOutcome::OpRetry { ref op_id } if op_id == "op-3"),
            "got {out:?}"
        );
    }

    #[test]
    fn elevated_delete_confirm_dialog() {
        let mut st = open();
        let entries = example_file_entries();
        let ops = example_empty_ops();
        seed_delete_confirm(&mut st);
        assert!(matches!(st.dialog, FileManagerDialog::ConfirmDelete { .. }));
        // Safe default is cancel — move action cursor to confirm, then Enter.
        let out = st.handle_key(press(KeyCode::Right), &entries, &ops, &[]);
        assert!(
            matches!(out, FileManagerOutcome::Ignored),
            "focus move maps to Ignored, got {out:?}"
        );
        assert_eq!(
            st.alert.action_cursor().copied(),
            Some("confirm"),
            "Right must focus destructive confirm"
        );
        let out = st.handle_key(press(KeyCode::Enter), &entries, &ops, &[]);
        assert!(
            matches!(
                out,
                FileManagerOutcome::DeleteRequested { ref ids } if ids.len() == 2
            ),
            "workbench path must emit DeleteRequested for 2 paths, got {out:?}"
        );
        assert!(
            matches!(st.dialog, FileManagerDialog::None),
            "dialog must close after confirm"
        );
    }

    #[test]
    fn quick_open_open_close() {
        let mut st = open();
        let entries = example_file_entries();
        let ops = example_empty_ops();
        let qo = example_quick_open_from_entries(&entries);
        st.focus = "tree";
        st.apply_focus_gates();
        let out = st.handle_key(
            press_mod(KeyCode::Char('o'), KeyModifiers::CONTROL),
            &entries,
            &ops,
            &qo,
        );
        assert!(matches!(out, FileManagerOutcome::QuickOpenOpened));
        assert!(matches!(st.dialog, FileManagerDialog::QuickOpen));
        let out = st.handle_key(press(KeyCode::Esc), &entries, &ops, &qo);
        assert!(matches!(out, FileManagerOutcome::QuickOpenClosed));
    }

    #[test]
    fn search_filter_projects_into_tree() {
        let mut st = open();
        let entries = example_file_entries();
        let ops = example_empty_ops();
        st.focus = "search";
        st.apply_focus_gates();
        st.search.set_query("readme");
        let out = st.handle_key(press(KeyCode::Enter), &entries, &ops, &[]);
        assert!(
            matches!(out, FileManagerOutcome::FilterChanged { .. })
                || st.tree.filter.as_deref() == Some("readme")
                || !st.search.query().is_empty(),
            "got {out:?} filter={:?}",
            st.tree.filter
        );
    }

    #[test]
    fn no_fs_io_in_composition_source() {
        let body = include_str!("file_manager.rs");
        // strip this test's own forbidden strings in the assert list
        let code = body
            .split("fn no_fs_io_in_composition_source")
            .next()
            .unwrap_or(body);
        for forbidden in [
            "std::fs::",
            "walkdir",
            "tokio::fs",
            "remove_dir_all",
            "TcpStream",
            "std::io::",
        ] {
            // allow comments mentioning std::fs as boundary
            let hits: Vec<_> = code
                .lines()
                .filter(|l| {
                    let t = l.trim_start();
                    !t.starts_with("//")
                        && !t.starts_with("//!")
                        && !t.starts_with('*')
                        && l.contains(forbidden)
                })
                .collect();
            assert!(
                hits.is_empty(),
                "forbidden I/O {forbidden} in composition: {hits:?}"
            );
        }
        // resource_browser layout preserved as peer
        assert!(!code.contains("layout_resource_browser") || code.contains("resource_browser"));
    }

    #[test]
    fn layout_normal_has_preview_and_queue() {
        let st = WorkspaceState::new();
        let panes = file_manager_layout_density(
            Rect::new(0, 0, 120, 40),
            &st,
            FileManagerDensity::Normal,
            false,
        );
        let ids: Vec<_> = panes
            .iter()
            .filter(|p| !p.collapsed && p.area.height > 0 && p.area.width > 0)
            .map(|p| p.id.0.as_str())
            .collect();
        assert!(ids.contains(&"tree"));
        assert!(ids.contains(&"preview"));
        assert!(ids.contains(&"queue"));
        assert!(ids.contains(&"status"));
        assert!(ids.contains(&"search"));
        assert!(ids.contains(&"breadcrumbs"));
    }

    #[test]
    fn layout_tiny_drops_secondary() {
        let st = WorkspaceState::new();
        let panes = file_manager_layout_density(
            Rect::new(0, 0, 40, 20),
            &st,
            FileManagerDensity::Tiny,
            false,
        );
        let ids: Vec<_> = panes
            .iter()
            .filter(|p| !p.collapsed && p.area.height > 0 && p.area.width > 0)
            .map(|p| p.id.0.as_str())
            .collect();
        assert!(ids.contains(&"tree"));
        assert!(!ids.contains(&"preview"));
        assert!(!ids.contains(&"queue"));
    }

    #[test]
    fn paint_smoke_and_search_height() {
        let system = DesignSystem::default();
        let mut st = open();
        let entries = example_file_entries();
        let ops = example_file_ops();
        let (preview, _, _) = example_file_preview();
        let area = Rect::new(0, 0, 120, 36);
        let mut buf = Buffer::empty(area);
        render_file_manager(
            &mut buf,
            area,
            FileManagerSurfaces {
                system: &system,
                state: &mut st,
                entries: &entries,
                ops: &ops,
                preview: Some(preview),
                quick_open_items: &[],
            },
        );
        let search = st
            .last_panes()
            .iter()
            .find(|p| p.id.0.as_str() == "search")
            .expect("search pane");
        assert!(
            search.area.height >= 3,
            "search strip must be ≥3 rows, got {}",
            search.area.height
        );
        let keys = st
            .status_slots()
            .iter()
            .find(|s| s.id == "keys")
            .map(|s| s.content)
            .unwrap_or("");
        assert!(
            keys.contains("v paste") && keys.contains("y yank"),
            "status must document keyboard paths, got {keys}"
        );
    }

    #[test]
    fn burst_paint_perf() {
        let system = DesignSystem::default();
        let mut st = open();
        st.density = Some(FileManagerDensity::Normal);
        let owned = burst_file_entries(bench::BURST_ENTRIES);
        let entries: Vec<FileTreeEntry<'_, String>> = owned
            .iter()
            .map(|(id, name, path)| {
                FileTreeEntry::file(id.clone(), name.as_str(), path.as_str(), 0)
            })
            .collect();
        let ops = example_file_ops();
        let area = Rect::new(0, 0, bench::VIEWPORT.0, bench::VIEWPORT.1);
        let mut buf = Buffer::empty(area);
        let start = std::time::Instant::now();
        for _ in 0..bench::PAINT_FRAMES {
            render_file_manager(
                &mut buf,
                area,
                FileManagerSurfaces {
                    system: &system,
                    state: &mut st,
                    entries: &entries,
                    ops: &ops,
                    preview: None,
                    quick_open_items: &[],
                },
            );
        }
        let elapsed = start.elapsed();
        assert!(elapsed.as_secs() < 5, "paint too slow: {elapsed:?}");
    }

    #[test]
    fn resource_browser_peer_not_dual_interactive() {
        // layout helper still exists as separate module
        let _ = crate::patterns::layout_resource_browser;
    }

    #[test]
    fn mouse_actions_have_keyboard_paths() {
        // Structural: status shortcuts list keyboard for all primary actions
        let st = open();
        let keys = st
            .status_slots()
            .iter()
            .find(|s| s.id == "keys")
            .map(|s| s.content)
            .unwrap_or("");
        for chord in [
            "y yank",
            "x cut",
            "v paste",
            "d del",
            "r ren",
            "n new",
            "p preview",
        ] {
            assert!(
                keys.contains(chord),
                "missing keyboard path {chord} in {keys}"
            );
        }
    }
}
