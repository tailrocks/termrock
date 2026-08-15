// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! File and directory browser built from PathInput, list navigation, and overlays.
//!
//! **Mission.** Setup flows and PathInput browse need a reusable picker for
//! files/directories with filters, breadcrumbs, multi-select, preview, and
//! path entry — **without** embedding filesystem I/O. Hosts supply listings,
//! previews, and cancellation.
//!
//! **vs [`PathInput`](super::PathInput).** Path field only; FilePicker is the
//! full browser PathInput requests via [`PathInputOutcome::BrowseRequested`].
//! **vs [`Picker`](super::Picker).** Domain-neutral query+list; FilePicker is
//! path/FS-shaped with breadcrumbs and entry kinds.
//!
//! Research: Yazi, ranger, lf, broot, desktop dialogs, fuzzy finders.

#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Modifier,
    widgets::{StatefulWidget, Widget},
};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    interaction::{
        CollectionItem, CollectionOutcome, CollectionState, OverlayId, OverlayOutcome, OverlaySize,
        OverlaySpec, OverlayStack, SemanticNode, SemanticRole, SemanticScene, SemanticState,
        UiIntent,
    },
    style::{DesignSystem, ListRowVisualState, Role},
    text::{display_cols, take_display_cols},
};

use super::{
    Panel, PanelChrome, PathExpect, PathFsStatus, PathInput, PathInputOutcome, PathInputState,
    PathStyle, Selection, Validation, join_path, normalize_separators,
};

/// Overlay id for modal file pickers.
pub const FILE_PICKER_OVERLAY_ID: &str = "termrock.file-picker";
/// Width under which layout prefers fullscreen / stacked no-preview.
pub const FILE_PICKER_FULLSCREEN_MAX_WIDTH: u16 = 48;
/// Height under which preview is dropped.
pub const FILE_PICKER_PREVIEW_MIN_HEIGHT: u16 = 12;

// ── Entry model (host-provided; no FS) ───────────────────────────────────────

/// Kind of filesystem entry (host classification).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum FileEntryKind {
    /// Regular file.
    #[default]
    File,
    /// Directory.
    Directory,
    /// Symlink to file.
    SymlinkFile,
    /// Symlink to directory.
    SymlinkDir,
    /// Other (socket, device, …).
    Other,
}

impl FileEntryKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Directory => "directory",
            Self::SymlinkFile => "symlink-file",
            Self::SymlinkDir => "symlink-dir",
            Self::Other => "other",
        }
    }

    /// Whether this is directory-like for navigation.
    #[must_use]
    pub const fn is_dir(self) -> bool {
        matches!(self, Self::Directory | Self::SymlinkDir)
    }
}

/// One host-projected entry in the current directory listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    /// Stable id (usually full path or relative name unique in cwd).
    pub id: String,
    /// Display name (basename).
    pub name: String,
    /// Full path for selection / open.
    pub path: String,
    /// Entry kind.
    pub kind: FileEntryKind,
    /// Hidden (dotfile / host flag).
    pub hidden: bool,
    /// Optional size in bytes.
    pub size: Option<u64>,
    /// Optional host-formatted modified time.
    pub modified: Option<String>,
    /// Permission / access error on this entry.
    pub error: Option<String>,
    /// Whether selectable under current mode.
    pub selectable: bool,
}

impl FileEntry {
    /// File entry.
    #[must_use]
    pub fn file(id: impl Into<String>, name: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            path: path.into(),
            kind: FileEntryKind::File,
            hidden: false,
            size: None,
            modified: None,
            error: None,
            selectable: true,
        }
    }

    /// Directory entry.
    #[must_use]
    pub fn directory(
        id: impl Into<String>,
        name: impl Into<String>,
        path: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            path: path.into(),
            kind: FileEntryKind::Directory,
            hidden: false,
            size: None,
            modified: None,
            error: None,
            selectable: true,
        }
    }

    /// Hidden flag.
    #[must_use]
    pub const fn hidden(mut self, on: bool) -> Self {
        self.hidden = on;
        self
    }

    /// Size.
    #[must_use]
    pub const fn size(mut self, size: u64) -> Self {
        self.size = Some(size);
        self
    }

    /// Modified display string.
    #[must_use]
    pub fn modified(mut self, s: impl Into<String>) -> Self {
        self.modified = Some(s.into());
        self
    }

    /// Permission error.
    #[must_use]
    pub fn error(mut self, msg: impl Into<String>) -> Self {
        self.error = Some(msg.into());
        self
    }

    /// Selectable.
    #[must_use]
    pub const fn selectable(mut self, on: bool) -> Self {
        self.selectable = on;
        self
    }

    /// Kind.
    #[must_use]
    pub const fn kind(mut self, kind: FileEntryKind) -> Self {
        self.kind = kind;
        self
    }
}

/// Breadcrumb segment (path → label).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileBreadcrumb {
    /// Absolute path for this segment.
    pub path: String,
    /// Short label.
    pub label: String,
}

impl FileBreadcrumb {
    /// Segment.
    #[must_use]
    pub fn new(path: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            label: label.into(),
        }
    }
}

/// Host-projected preview payload (never loaded inside TermRock).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FilePreview {
    /// Title line.
    pub title: String,
    /// Body lines (already truncated by host).
    pub lines: Vec<String>,
    /// Preview error.
    pub error: Option<String>,
}

impl FilePreview {
    /// Empty.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Title + lines.
    #[must_use]
    pub fn text(title: impl Into<String>, lines: impl IntoIterator<Item = String>) -> Self {
        Self {
            title: title.into(),
            lines: lines.into_iter().collect(),
            error: None,
        }
    }

    /// Error preview.
    #[must_use]
    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            title: String::new(),
            lines: Vec::new(),
            error: Some(msg.into()),
        }
    }
}

// ── Mode / sort / status ────────────────────────────────────────────────────

/// What the picker accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum FilePickerMode {
    /// Files only (dirs for navigation).
    #[default]
    OpenFile,
    /// Directories only.
    OpenDirectory,
    /// File or directory.
    OpenAny,
    /// Save path (path entry primary; may create).
    SaveFile,
}

impl FilePickerMode {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::OpenFile => "open-file",
            Self::OpenDirectory => "open-directory",
            Self::OpenAny => "open-any",
            Self::SaveFile => "save-file",
        }
    }
}

/// Entry sort keys (applied client-side on applied listing).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum FileSortKey {
    /// Name (dirs first).
    #[default]
    Name,
    /// Size (dirs first).
    Size,
    /// Modified string lexicographic (host-formatted).
    Modified,
    /// Kind then name.
    Kind,
}

impl FileSortKey {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Size => "size",
            Self::Modified => "modified",
            Self::Kind => "kind",
        }
    }
}

/// Listing fetch status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum FileListingStatus {
    /// Idle / initial.
    #[default]
    Idle,
    /// Host loading (cancellable).
    Loading,
    /// Listing applied.
    Ready,
    /// Permission / IO error for cwd.
    Error,
}

impl FileListingStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Loading => "loading",
            Self::Ready => "ready",
            Self::Error => "error",
        }
    }
}

/// Layout presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum FilePickerPresentation {
    /// In-place panel (embedded).
    #[default]
    Embedded,
    /// Modal overlay preferred.
    Modal,
    /// Fullscreen (tiny terminal / host force).
    Fullscreen,
}

impl FilePickerPresentation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Embedded => "embedded",
            Self::Modal => "modal",
            Self::Fullscreen => "fullscreen",
        }
    }
}

/// Focused pane inside the picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum FilePickerPane {
    /// Entry list.
    #[default]
    List,
    /// Path entry field.
    Path,
    /// Preview pane (when visible).
    Preview,
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// FilePicker outcomes. Host owns FS.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FilePickerOutcome {
    /// No effect.
    Ignored,
    /// Chrome / cursor changed.
    Changed,
    /// The pointer moved onto (or off) a row.
    HoverChanged,
    /// Host should list `path` for `generation` (cancellable).
    ListRequested {
        /// Directory to list.
        path: String,
        /// Race generation.
        generation: u64,
    },
    /// Host should load preview for path.
    PreviewRequested {
        /// Path.
        path: String,
        /// Generation.
        generation: u64,
    },
    /// Highlight moved.
    HighlightChanged {
        /// Entry id.
        id: Option<String>,
    },
    /// Selection membership changed.
    SelectionChanged,
    /// Confirmed selection (Enter / Open).
    Confirmed {
        /// Selected paths.
        paths: Vec<String>,
    },
    /// Cancelled (Esc).
    Cancelled,
    /// Navigate into directory (highlight or path).
    OpenDirectory {
        /// Directory path.
        path: String,
    },
    /// Filter / hidden / sort UI changed (host may re-list).
    FilterChanged,
    /// Presentation hint changed.
    PresentationChanged {
        /// Presentation.
        presentation: FilePickerPresentation,
    },
}

// ── State ───────────────────────────────────────────────────────────────────

/// Runtime state for [`FilePicker`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePickerState {
    cwd: String,
    breadcrumbs: Vec<FileBreadcrumb>,
    /// Host listing as applied (pre filter/sort).
    raw_entries: Vec<FileEntry>,
    /// Visible entries after filter/sort.
    entries: Vec<FileEntry>,
    collection: CollectionState<String>,
    selection: Selection<String>,
    multi: bool,
    mode: FilePickerMode,
    path_style: PathStyle,
    path: PathInputState,
    show_hidden: bool,
    /// Extension/name filter substring (client-side on applied listing).
    name_filter: String,
    sort: FileSortKey,
    sort_dirs_first: bool,
    status: FileListingStatus,
    error_message: Option<String>,
    generation: u64,
    applied_generation: u64,
    preview: Option<FilePreview>,
    preview_enabled: bool,
    presentation: FilePickerPresentation,
    pane: FilePickerPane,
    focused: bool,
    enabled: bool,
    /// Double-click open (mouse).
    last_click: Option<(String, u64)>,
    click_seq: u64,
    // geometry
    breadcrumb_hits: Vec<(String, Rect)>,
    entry_hits: Vec<(String, Rect)>,
    /// Entry the pointer is over (hover wash; never a commit).
    hovered: Option<String>,
    list_area: Rect,
    path_area: Rect,
    preview_area: Rect,
    root: Rect,
}

impl Default for FilePickerState {
    fn default() -> Self {
        Self::new("/")
    }
}

impl FilePickerState {
    /// Picker rooted at `cwd`.
    #[must_use]
    pub fn new(cwd: impl Into<String>) -> Self {
        let cwd = cwd.into();
        let mut path = PathInputState::new()
            .with_style(PathStyle::Unix)
            .with_path(&cwd)
            .with_expect(PathExpect::Any);
        path.set_fs_status(PathFsStatus::Directory);
        Self {
            cwd: cwd.clone(),
            breadcrumbs: vec![FileBreadcrumb::new(cwd, "/")],
            raw_entries: Vec::new(),
            entries: Vec::new(),
            collection: CollectionState::new().wrap(true),
            selection: Selection::new(),
            multi: false,
            mode: FilePickerMode::OpenFile,
            path_style: PathStyle::Unix,
            path,
            show_hidden: false,
            name_filter: String::new(),
            sort: FileSortKey::Name,
            sort_dirs_first: true,
            status: FileListingStatus::Idle,
            error_message: None,
            generation: 0,
            applied_generation: 0,
            preview: None,
            preview_enabled: true,
            presentation: FilePickerPresentation::Embedded,
            pane: FilePickerPane::List,
            focused: false,
            enabled: true,
            last_click: None,
            click_seq: 0,
            breadcrumb_hits: Vec::new(),
            entry_hits: Vec::new(),
            hovered: None,
            list_area: Rect::default(),
            path_area: Rect::default(),
            preview_area: Rect::default(),
            root: Rect::default(),
        }
    }

    /// Multi-select.
    #[must_use]
    pub const fn with_multi(mut self, on: bool) -> Self {
        self.multi = on;
        self
    }

    /// Mode.
    #[must_use]
    pub fn with_mode(mut self, mode: FilePickerMode) -> Self {
        self.mode = mode;
        let expect = match mode {
            FilePickerMode::OpenDirectory => PathExpect::Directory,
            FilePickerMode::OpenFile | FilePickerMode::SaveFile => PathExpect::File,
            FilePickerMode::OpenAny => PathExpect::Any,
        };
        let mut path = PathInputState::new()
            .with_style(self.path_style)
            .with_path(self.cwd.clone())
            .with_expect(expect);
        path.set_fs_status(PathFsStatus::Directory);
        self.path = path;
        self
    }

    /// Path style (Unix / Windows).
    #[must_use]
    pub fn with_path_style(mut self, style: PathStyle) -> Self {
        self.path_style = style;
        self.path = self.path.with_style(style);
        self
    }

    /// Preview pane enabled.
    #[must_use]
    pub const fn with_preview(mut self, on: bool) -> Self {
        self.preview_enabled = on;
        self
    }

    /// Show hidden entries.
    #[must_use]
    pub const fn with_show_hidden(mut self, on: bool) -> Self {
        self.show_hidden = on;
        self
    }

    /// Sort key.
    #[must_use]
    pub const fn with_sort(mut self, sort: FileSortKey) -> Self {
        self.sort = sort;
        self
    }

    /// Presentation.
    #[must_use]
    pub const fn with_presentation(mut self, p: FilePickerPresentation) -> Self {
        self.presentation = p;
        self
    }

    /// Cwd.
    #[must_use]
    pub fn cwd(&self) -> &str {
        &self.cwd
    }

    /// Visible entries (after filter/sort).
    #[must_use]
    pub fn entries(&self) -> &[FileEntry] {
        &self.entries
    }

    /// Selected paths (from entry paths by id).
    #[must_use]
    pub fn selected_paths(&self) -> Vec<String> {
        self.selection
            .checked()
            .iter()
            .filter_map(|id| {
                self.entries
                    .iter()
                    .find(|e| &e.id == id)
                    .map(|e| e.path.clone())
            })
            .collect()
    }

    /// Highlight entry.
    #[must_use]
    pub fn highlight(&self) -> Option<&FileEntry> {
        let id = self.collection.active()?;
        self.entries.iter().find(|e| &e.id == id)
    }

    /// Status.
    #[must_use]
    pub const fn listing_status(&self) -> FileListingStatus {
        self.status
    }

    /// Generation for list requests.
    #[must_use]
    pub const fn listing_generation(&self) -> u64 {
        self.generation
    }

    /// Applied generation.
    #[must_use]
    pub const fn applied_generation(&self) -> u64 {
        self.applied_generation
    }

    /// Mode.
    #[must_use]
    pub const fn mode(&self) -> FilePickerMode {
        self.mode
    }

    /// Multi.
    #[must_use]
    pub const fn is_multi(&self) -> bool {
        self.multi
    }

    /// Show hidden.
    #[must_use]
    pub const fn show_hidden(&self) -> bool {
        self.show_hidden
    }

    /// Pane.
    #[must_use]
    pub const fn pane(&self) -> FilePickerPane {
        self.pane
    }

    /// Presentation.
    #[must_use]
    pub const fn presentation(&self) -> FilePickerPresentation {
        self.presentation
    }

    /// Path field.
    #[must_use]
    pub const fn path_state(&self) -> &PathInputState {
        &self.path
    }

    /// Focus.
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on;
        if matches!(self.pane, FilePickerPane::Path) {
            self.path.set_focused(on);
        } else {
            self.path.set_focused(false);
        }
    }

    /// Enabled.
    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
        self.path.set_enabled(on);
    }

    /// Toggle hidden and rebuild visible projection from raw listing.
    pub fn set_show_hidden(&mut self, on: bool) {
        self.show_hidden = on;
        self.reprocess_visible();
    }

    /// Name filter (client-side) and rebuild visible projection.
    pub fn set_name_filter(&mut self, filter: impl Into<String>) {
        self.name_filter = filter.into();
        self.reprocess_visible();
    }

    /// Sort and rebuild visible projection.
    pub fn set_sort(&mut self, sort: FileSortKey) {
        self.sort = sort;
        self.reprocess_visible();
    }

    /// Presentation.
    pub const fn set_presentation(&mut self, p: FilePickerPresentation) {
        self.presentation = p;
    }

    /// Auto presentation from bounds.
    #[must_use]
    pub fn presentation_for_bounds(bounds: Rect) -> FilePickerPresentation {
        if bounds.width < FILE_PICKER_FULLSCREEN_MAX_WIDTH
            || bounds.height < FILE_PICKER_PREVIEW_MIN_HEIGHT
        {
            FilePickerPresentation::Fullscreen
        } else {
            FilePickerPresentation::Embedded
        }
    }

    fn bump_generation(&mut self) -> u64 {
        self.generation = self.generation.saturating_add(1);
        self.status = FileListingStatus::Loading;
        self.error_message = None;
        self.generation
    }

    /// Request listing for `cwd` (or set path and request).
    pub fn request_list(&mut self, path: impl Into<String>) -> FilePickerOutcome {
        let path = normalize_separators(&path.into(), self.path_style);
        self.cwd = path.clone();
        self.path.set_path(&path);
        self.path.set_fs_status(PathFsStatus::Directory);
        self.rebuild_breadcrumbs();
        self.selection.clear();
        self.preview = None;
        let generation = self.bump_generation();
        FilePickerOutcome::ListRequested { path, generation }
    }

    fn rebuild_breadcrumbs(&mut self) {
        let sep = self.path_style.sep();
        let norm = normalize_separators(&self.cwd, self.path_style);
        let mut crumbs = Vec::new();
        if self.path_style == PathStyle::Windows {
            // simplistic: split on sep, keep drive
            let parts: Vec<&str> = norm.split(sep).filter(|p| !p.is_empty()).collect();
            if norm.chars().nth(1) == Some(':') {
                let mut acc = String::new();
                for (i, p) in parts.iter().enumerate() {
                    if i == 0 {
                        acc = format!("{p}{sep}");
                        crumbs.push(FileBreadcrumb::new(acc.clone(), (*p).to_owned()));
                    } else {
                        acc = join_path(&acc, p, self.path_style);
                        crumbs.push(FileBreadcrumb::new(acc.clone(), (*p).to_owned()));
                    }
                }
            } else {
                crumbs.push(FileBreadcrumb::new(norm.clone(), norm.clone()));
            }
        } else {
            crumbs.push(FileBreadcrumb::new("/", "/"));
            let rest = norm.trim_start_matches('/');
            if !rest.is_empty() {
                let mut acc = String::from("/");
                for p in rest.split('/').filter(|s| !s.is_empty()) {
                    acc = join_path(&acc, p, PathStyle::Unix);
                    crumbs.push(FileBreadcrumb::new(acc.clone(), p.to_owned()));
                }
            }
        }
        if crumbs.is_empty() {
            crumbs.push(FileBreadcrumb::new(norm, "."));
        }
        self.breadcrumbs = crumbs;
    }

    /// Apply listing for generation (race-safe).
    pub fn apply_listing(
        &mut self,
        generation: u64,
        cwd: impl Into<String>,
        entries: Vec<FileEntry>,
        breadcrumbs: Option<Vec<FileBreadcrumb>>,
    ) -> bool {
        if generation != self.generation {
            return false;
        }
        self.applied_generation = generation;
        self.cwd = normalize_separators(&cwd.into(), self.path_style);
        self.path.set_path(&self.cwd);
        if let Some(b) = breadcrumbs {
            self.breadcrumbs = b;
        } else {
            self.rebuild_breadcrumbs();
        }
        self.raw_entries = entries;
        self.status = FileListingStatus::Ready;
        self.error_message = None;
        self.reprocess_visible();
        true
    }

    /// Apply listing error.
    pub fn apply_listing_error(&mut self, generation: u64, message: impl Into<String>) -> bool {
        if generation != self.generation {
            return false;
        }
        self.applied_generation = generation;
        self.status = FileListingStatus::Error;
        self.error_message = Some(message.into());
        self.raw_entries.clear();
        self.entries.clear();
        true
    }

    /// Apply preview (generation-gated optionally by list gen).
    pub fn apply_preview(&mut self, preview: FilePreview) {
        self.preview = Some(preview);
    }

    fn process_entries(&self, mut entries: Vec<FileEntry>) -> Vec<FileEntry> {
        if !self.show_hidden {
            entries.retain(|e| !e.hidden && !e.name.starts_with('.'));
        }
        if !self.name_filter.is_empty() {
            let q = self.name_filter.to_ascii_lowercase();
            entries.retain(|e| e.name.to_ascii_lowercase().contains(&q));
        }
        // mode: non-selectable files in directory-only mode still shown for nav? dirs always nav
        for e in &mut entries {
            e.selectable = match self.mode {
                FilePickerMode::OpenFile | FilePickerMode::SaveFile => !e.kind.is_dir(),
                FilePickerMode::OpenDirectory => e.kind.is_dir(),
                FilePickerMode::OpenAny => true,
            };
        }
        entries.sort_by(|a, b| {
            if self.sort_dirs_first {
                match (a.kind.is_dir(), b.kind.is_dir()) {
                    (true, false) => return std::cmp::Ordering::Less,
                    (false, true) => return std::cmp::Ordering::Greater,
                    _ => {}
                }
            }
            match self.sort {
                FileSortKey::Name => a
                    .name
                    .to_ascii_lowercase()
                    .cmp(&b.name.to_ascii_lowercase()),
                FileSortKey::Size => a.size.unwrap_or(0).cmp(&b.size.unwrap_or(0)),
                FileSortKey::Modified => a
                    .modified
                    .as_deref()
                    .unwrap_or("")
                    .cmp(b.modified.as_deref().unwrap_or("")),
                FileSortKey::Kind => a
                    .kind
                    .id()
                    .cmp(b.kind.id())
                    .then_with(|| a.name.cmp(&b.name)),
            }
        });
        entries
    }

    fn reprocess_visible(&mut self) {
        self.entries = self.process_entries(self.raw_entries.clone());
        let items: Vec<CollectionItem<String>> = self
            .entries
            .iter()
            .map(|e| {
                CollectionItem::new(e.id.clone(), e.name.clone())
                    .enabled(e.error.is_none() && (e.selectable || e.kind.is_dir()))
            })
            .collect();
        let _ = self.collection.reconcile(&items);
        let valid: Vec<String> = self.entries.iter().map(|e| e.id.clone()).collect();
        self.selection.reconcile(&valid);
    }

    /// Confirm selection.
    pub fn confirm(&mut self) -> FilePickerOutcome {
        let paths = if self.multi {
            let mut p = self.selected_paths();
            if p.is_empty() {
                if let Some(h) = self.highlight() {
                    if h.selectable && h.error.is_none() {
                        p.push(h.path.clone());
                    }
                }
            }
            p
        } else if let Some(h) = self.highlight() {
            if h.selectable && h.error.is_none() {
                vec![h.path.clone()]
            } else if matches!(self.mode, FilePickerMode::SaveFile) {
                vec![self.path.path().to_owned()]
            } else {
                Vec::new()
            }
        } else if matches!(self.mode, FilePickerMode::SaveFile) {
            vec![self.path.path().to_owned()]
        } else {
            Vec::new()
        };
        if paths.is_empty() {
            return FilePickerOutcome::Ignored;
        }
        FilePickerOutcome::Confirmed { paths }
    }

    /// Open highlighted directory or path.
    pub fn open_highlight(&mut self) -> FilePickerOutcome {
        let Some(h) = self.highlight().cloned() else {
            return FilePickerOutcome::Ignored;
        };
        if h.kind.is_dir() {
            return self.request_list(h.path);
        }
        if h.selectable {
            if !self.multi {
                self.selection.clear();
            }
            if !self.selection.is_checked(&h.id) {
                let _ = self.selection.toggle(&h.id);
            }
            return self.confirm();
        }
        FilePickerOutcome::Ignored
    }

    /// Key adapter.
    pub fn handle_key(&mut self, key: KeyEvent) -> FilePickerOutcome {
        if key.kind == KeyEventKind::Release || !self.enabled {
            return FilePickerOutcome::Ignored;
        }
        if !self.focused {
            return FilePickerOutcome::Ignored;
        }

        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);

        // Esc: leave path/preview first; else cancel picker
        if key.code == KeyCode::Esc && key.modifiers.is_empty() {
            if matches!(self.pane, FilePickerPane::Path | FilePickerPane::Preview) {
                self.pane = FilePickerPane::List;
                self.path.set_focused(false);
                return FilePickerOutcome::Changed;
            }
            return FilePickerOutcome::Cancelled;
        }

        // Pane focus: Tab cycles List → Path → Preview
        if matches!(key.code, KeyCode::Tab) && !ctrl && !alt {
            self.pane = match self.pane {
                FilePickerPane::List => FilePickerPane::Path,
                FilePickerPane::Path => {
                    if self.preview_enabled {
                        FilePickerPane::Preview
                    } else {
                        FilePickerPane::List
                    }
                }
                FilePickerPane::Preview => FilePickerPane::List,
            };
            self.path
                .set_focused(matches!(self.pane, FilePickerPane::Path));
            return FilePickerOutcome::Changed;
        }

        // Ctrl+H toggle hidden (client-side reprocess from raw)
        if ctrl && matches!(key.code, KeyCode::Char('h' | 'H')) {
            self.show_hidden = !self.show_hidden;
            self.reprocess_visible();
            return FilePickerOutcome::FilterChanged;
        }

        // Ctrl+L focus path
        if ctrl && matches!(key.code, KeyCode::Char('l' | 'L')) {
            self.pane = FilePickerPane::Path;
            self.path.set_focused(true);
            return FilePickerOutcome::Changed;
        }

        match self.pane {
            FilePickerPane::Path => self.handle_path_key(key),
            FilePickerPane::Preview => {
                // arrows go back to list
                if matches!(key.code, KeyCode::Left | KeyCode::Esc) {
                    self.pane = FilePickerPane::List;
                    return FilePickerOutcome::Changed;
                }
                FilePickerOutcome::Ignored
            }
            FilePickerPane::List => self.handle_list_key(key),
        }
    }

    fn handle_path_key(&mut self, key: KeyEvent) -> FilePickerOutcome {
        match self.path.handle_key(key) {
            PathInputOutcome::Submitted { path } => {
                // if directory navigate; else set selection path
                self.request_list(path)
            }
            PathInputOutcome::Cancelled => {
                self.pane = FilePickerPane::List;
                self.path.set_focused(false);
                FilePickerOutcome::Changed
            }
            PathInputOutcome::Changed | PathInputOutcome::Cleared => FilePickerOutcome::Changed,
            PathInputOutcome::BrowseRequested => FilePickerOutcome::Ignored,
            PathInputOutcome::CompletionRequested { .. } => FilePickerOutcome::Changed,
            _ => FilePickerOutcome::Ignored,
        }
    }

    fn handle_list_key(&mut self, key: KeyEvent) -> FilePickerOutcome {
        let items: Vec<CollectionItem<String>> = self
            .entries
            .iter()
            .map(|e| {
                CollectionItem::new(e.id.clone(), e.name.clone())
                    .enabled(e.error.is_none() && (e.selectable || e.kind.is_dir()))
            })
            .collect();

        // Enter open / confirm
        if key.code == KeyCode::Enter && key.modifiers.is_empty() {
            return self.open_highlight();
        }

        // Space toggle multi
        if matches!(key.code, KeyCode::Char(' ')) && self.multi {
            if let Some(id) = self.collection.active().cloned() {
                if let Some(e) = self.entries.iter().find(|e| e.id == id) {
                    if e.selectable && e.error.is_none() {
                        let _ = self.selection.toggle(&id);
                        return FilePickerOutcome::SelectionChanged;
                    }
                }
            }
            return FilePickerOutcome::Ignored;
        }

        // Backspace / Left parent
        if matches!(key.code, KeyCode::Backspace | KeyCode::Left)
            && key.modifiers.is_empty()
            && self.breadcrumbs.len() > 1
        {
            let parent = self.breadcrumbs[self.breadcrumbs.len() - 2].path.clone();
            return self.request_list(parent);
        }

        // Right open dir
        if key.code == KeyCode::Right && key.modifiers.is_empty() {
            if let Some(h) = self.highlight() {
                if h.kind.is_dir() {
                    return self.request_list(h.path.clone());
                }
            }
        }

        match self.collection.handle_key(key, &items) {
            CollectionOutcome::ActiveChanged { to, .. } => {
                let mut out = FilePickerOutcome::HighlightChanged { id: to.clone() };
                if let Some(id) = to {
                    if let Some(e) = self.entries.iter().find(|e| e.id == id) {
                        if !e.kind.is_dir() && self.preview_enabled {
                            out = FilePickerOutcome::PreviewRequested {
                                path: e.path.clone(),
                                generation: self.generation,
                            };
                        }
                    }
                }
                out
            }
            CollectionOutcome::Scrolled => FilePickerOutcome::Changed,
            CollectionOutcome::Ignored => FilePickerOutcome::Ignored,
        }
    }

    /// Intent path.
    pub fn handle_intent(&mut self, intent: UiIntent) -> FilePickerOutcome {
        if !self.enabled || !self.focused {
            return FilePickerOutcome::Ignored;
        }
        match intent {
            UiIntent::Cancel | UiIntent::Close => FilePickerOutcome::Cancelled,
            UiIntent::Submit | UiIntent::Activate => self.open_highlight(),
            UiIntent::Fullscreen => {
                self.presentation = FilePickerPresentation::Fullscreen;
                FilePickerOutcome::PresentationChanged {
                    presentation: FilePickerPresentation::Fullscreen,
                }
            }
            other if matches!(self.pane, FilePickerPane::List) => {
                let items: Vec<CollectionItem<String>> = self
                    .entries
                    .iter()
                    .map(|e| {
                        CollectionItem::new(e.id.clone(), e.name.clone())
                            .enabled(e.error.is_none() && (e.selectable || e.kind.is_dir()))
                    })
                    .collect();
                match self.collection.handle_intent(other, &items) {
                    CollectionOutcome::ActiveChanged { to, .. } => {
                        FilePickerOutcome::HighlightChanged { id: to }
                    }
                    CollectionOutcome::Scrolled => FilePickerOutcome::Changed,
                    CollectionOutcome::Ignored => FilePickerOutcome::Ignored,
                }
            }
            _ => FilePickerOutcome::Ignored,
        }
    }

    /// Mouse.
    pub fn handle_mouse(&mut self, event: MouseEvent) -> FilePickerOutcome {
        if !self.enabled {
            return FilePickerOutcome::Ignored;
        }
        let click = matches!(
            event.kind,
            MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Up(MouseButton::Left)
        );
        if !click && !matches!(event.kind, MouseEventKind::Down(MouseButton::Left)) {
            // only left down for most
        }
        if matches!(event.kind, MouseEventKind::Moved) {
            // Hover is stated every event, so leaving the list clears it.
            let was = self.hovered.clone();
            self.hovered = self
                .entry_hits
                .iter()
                .find(|(_, rect)| rect.contains(event.position))
                .map(|(id, _)| id.clone());
            return if was == self.hovered {
                FilePickerOutcome::Ignored
            } else {
                FilePickerOutcome::HoverChanged
            };
        }
        if !matches!(event.kind, MouseEventKind::Down(MouseButton::Left)) {
            return FilePickerOutcome::Ignored;
        }
        self.focused = true;
        self.click_seq = self.click_seq.saturating_add(1);

        // breadcrumbs
        for (path, rect) in &self.breadcrumb_hits {
            if rect.contains(event.position) {
                return self.request_list(path.clone());
            }
        }

        // path field
        if self.path_area.contains(event.position) {
            self.pane = FilePickerPane::Path;
            self.path.set_focused(true);
            let _ = self.path.handle_mouse(event);
            return FilePickerOutcome::Changed;
        }

        // entries
        for (id, rect) in &self.entry_hits {
            if rect.contains(event.position) {
                self.pane = FilePickerPane::List;
                self.path.set_focused(false);
                self.collection.set_active(Some(id.clone()));
                // double-click detection (same id within 2 clicks sequential)
                let is_double = self.last_click.as_ref().is_some_and(|(prev, seq)| {
                    prev == id && self.click_seq.saturating_sub(*seq) <= 2
                });
                self.last_click = Some((id.clone(), self.click_seq));
                if is_double {
                    return self.open_highlight();
                }
                if self.multi {
                    if let Some(e) = self.entries.iter().find(|e| &e.id == id) {
                        if e.selectable {
                            let _ = self.selection.toggle(id);
                            return FilePickerOutcome::SelectionChanged;
                        }
                    }
                }
                if let Some(e) = self.entries.iter().find(|e| &e.id == id) {
                    if !e.kind.is_dir() && self.preview_enabled {
                        return FilePickerOutcome::PreviewRequested {
                            path: e.path.clone(),
                            generation: self.generation,
                        };
                    }
                }
                return FilePickerOutcome::HighlightChanged {
                    id: Some(id.clone()),
                };
            }
        }
        FilePickerOutcome::Ignored
    }

    /// Open as overlay helper.
    pub fn open_overlay<FocusId: Clone>(
        stack: &mut OverlayStack<FocusId>,
        bounds: Rect,
        size: OverlaySize,
        opener: Option<FocusId>,
    ) -> OverlayOutcome<FocusId> {
        stack.open(
            bounds,
            OverlaySpec::dialog(FILE_PICKER_OVERLAY_ID, size, opener),
        )
    }

    /// Dismiss overlay.
    pub fn dismiss_overlay<FocusId: Clone>(
        stack: &mut OverlayStack<FocusId>,
    ) -> OverlayOutcome<FocusId> {
        stack.dismiss(&OverlayId::from_static(FILE_PICKER_OVERLAY_ID))
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// File picker chrome.
#[derive(Debug, Clone, Copy)]
pub struct FilePicker<'a> {
    system: &'a DesignSystem,
    title: &'a str,
    ascii: bool,
    show_preview: bool,
}

impl<'a> FilePicker<'a> {
    /// Create picker chrome.
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self {
            system,
            title: "Open",
            ascii: false,
            show_preview: true,
        }
    }

    /// Title.
    #[must_use]
    pub const fn title(mut self, title: &'a str) -> Self {
        self.title = title;
        self
    }

    /// ASCII glyphs.
    #[must_use]
    pub const fn ascii(mut self, on: bool) -> Self {
        self.ascii = on;
        self
    }

    /// Show preview column when space allows.
    #[must_use]
    pub const fn show_preview(mut self, on: bool) -> Self {
        self.show_preview = on;
        self
    }

    /// Paint full picker into `area`.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut FilePickerState) {
        state.breadcrumb_hits.clear();
        state.entry_hits.clear();
        state.root = area;
        if area.is_empty() {
            return;
        }

        // Auto-contract presentation
        if area.width < FILE_PICKER_FULLSCREEN_MAX_WIDTH {
            state.presentation = FilePickerPresentation::Fullscreen;
        }

        let panel = Panel::new(self.system)
            .overlay(true)
            .emphasis(if state.focused {
                PanelChrome::Focused
            } else {
                PanelChrome::Normal
            });
        let inner = panel.inner(area);
        Widget::render(&panel.title(self.title), area, buffer);
        if inner.is_empty() {
            return;
        }

        let mut y = inner.y;
        // Breadcrumbs
        if inner.height >= 1 {
            let mut x = inner.x;
            for (i, crumb) in state.breadcrumbs.iter().enumerate() {
                if x >= inner.right() {
                    break;
                }
                if i > 0 {
                    let sep = if self.ascii { ">" } else { "›" };
                    buffer.set_stringn(x, y, sep, 1, self.system.style(Role::TextMuted));
                    x = x.saturating_add(2);
                }
                let label = take_display_cols(&crumb.label, 12);
                let w = display_cols(&label) as u16;
                let rect = Rect::new(x, y, w.min(inner.right().saturating_sub(x)), 1);
                buffer.set_stringn(
                    rect.x,
                    rect.y,
                    &label,
                    usize::from(rect.width),
                    self.system.style(if state.focused {
                        Role::Focus
                    } else {
                        Role::Text
                    }),
                );
                state.breadcrumb_hits.push((crumb.path.clone(), rect));
                x = x.saturating_add(w).saturating_add(1);
            }
            y = y.saturating_add(1);
        }

        // Path input row
        if y < inner.bottom() {
            let path_row = Rect::new(inner.x, y, inner.width, 1);
            state.path_area = path_row;
            let _ = PathInput::new(self.system)
                .placeholder("Path…")
                .show_browse(false)
                .ascii(self.ascii)
                .paint(path_row, buffer, &mut state.path);
            y = y.saturating_add(1);
        }

        // Status / error
        if y < inner.bottom()
            && (matches!(
                state.status,
                FileListingStatus::Loading | FileListingStatus::Error
            ) || state.error_message.is_some())
        {
            let msg = match state.status {
                FileListingStatus::Loading => "Loading…",
                FileListingStatus::Error => state
                    .error_message
                    .as_deref()
                    .unwrap_or("Permission denied"),
                _ => "",
            };
            if !msg.is_empty() {
                buffer.set_stringn(
                    inner.x,
                    y,
                    take_display_cols(msg, usize::from(inner.width)),
                    usize::from(inner.width),
                    self.system
                        .style(if matches!(state.status, FileListingStatus::Error) {
                            Role::Danger
                        } else {
                            Role::TextMuted
                        }),
                );
                y = y.saturating_add(1);
            }
        }

        // Reserve the footer row before the body claims the space, so the
        // hints have somewhere to go instead of being computed and dropped
        // (plans/009 Step 3).
        let footer_h = u16::from(inner.bottom().saturating_sub(y) > 2);
        let body = Rect::new(
            inner.x,
            y,
            inner.width,
            inner.bottom().saturating_sub(y).saturating_sub(footer_h),
        );
        if body.is_empty() {
            return;
        }

        // Preview needs width and is dropped in fullscreen / host-disabled.
        let show_preview = self.show_preview
            && state.preview_enabled
            && body.width >= 40
            && body.height >= 4
            && !matches!(state.presentation, FilePickerPresentation::Fullscreen);

        let (list_area, preview_area) = if show_preview {
            let lw = body.width * 3 / 5;
            (
                Rect::new(body.x, body.y, lw, body.height),
                Rect::new(
                    body.x.saturating_add(lw).saturating_add(1),
                    body.y,
                    body.width.saturating_sub(lw).saturating_sub(1),
                    body.height,
                ),
            )
        } else {
            (body, Rect::default())
        };
        state.list_area = list_area;
        state.preview_area = preview_area;

        self.paint_list(list_area, buffer, state);
        if !preview_area.is_empty() {
            self.paint_preview(preview_area, buffer, state);
        }

        // Footer: selection count / hints
        if footer_h > 0 {
            let n = state.selection.checked().len();
            let join = self.system.glyphs.meta_join();
            let hint = format!("{n} selected{join}enter open{join}space multi{join}esc close");
            let fy = inner.bottom().saturating_sub(1);
            buffer.set_stringn(
                inner.x,
                fy,
                take_display_cols(&hint, usize::from(inner.width)),
                usize::from(inner.width),
                self.system.style(Role::TextMuted),
            );
        }
    }

    fn paint_list(&self, area: Rect, buffer: &mut Buffer, state: &mut FilePickerState) {
        if area.is_empty() {
            return;
        }
        let items: Vec<CollectionItem<String>> = state
            .entries
            .iter()
            .map(|e| {
                CollectionItem::new(e.id.clone(), e.name.clone())
                    .enabled(e.error.is_none() && (e.selectable || e.kind.is_dir()))
            })
            .collect();
        let vp = usize::from(area.height).max(1);
        state
            .collection
            .set_viewport(state.collection.offset(), vp, items.len());
        let _ = state.collection.reconcile(&items);
        let _ = state.collection.ensure_active_visible(&items);
        let offset = state.collection.offset();

        for (row, entry) in state.entries.iter().skip(offset).enumerate() {
            if row >= vp {
                break;
            }
            let y = area.y.saturating_add(row as u16);
            let rect = Rect::new(area.x, y, area.width, 1);
            let is_hi = state.collection.active() == Some(&entry.id);
            let is_sel = state.selection.is_checked(&entry.id);
            let active = is_hi && matches!(state.pane, FilePickerPane::List);
            let recipe = self.system.resolve_list_row(ListRowVisualState {
                selected: active,
                focused: active && state.focused,
                hovered: state.hovered.as_deref() == Some(entry.id.as_str()),
                enabled: entry.error.is_none(),
                loading: false,
                checked: is_sel,
            });
            if recipe.use_fill {
                buffer.set_style(rect, recipe.label);
            } else if recipe.use_tint {
                buffer.set_style(rect, recipe.tint);
            }
            let kind_mark = if self.ascii {
                match entry.kind {
                    FileEntryKind::Directory | FileEntryKind::SymlinkDir => "d",
                    FileEntryKind::File | FileEntryKind::SymlinkFile => "f",
                    FileEntryKind::Other => "?",
                }
            } else {
                match entry.kind {
                    FileEntryKind::Directory | FileEntryKind::SymlinkDir => "📁",
                    _ => " ",
                }
            };
            let kind_mark = if self.ascii {
                kind_mark
            } else if entry.kind.is_dir() {
                "/"
            } else {
                " "
            };
            let check = if state.multi {
                if is_sel {
                    if self.ascii { "[x]" } else { "[✓]" }
                } else {
                    "[ ]"
                }
            } else if is_sel {
                "*"
            } else {
                " "
            };
            let err = entry.error.as_deref().unwrap_or("");
            let line = if err.is_empty() {
                format!("{check}{kind_mark} {}", entry.name)
            } else {
                format!("{check}! {} ({err})", entry.name)
            };
            let style = if entry.error.is_some() {
                self.system.style(Role::Danger)
            } else if active {
                recipe.label
            } else if is_sel {
                self.system.style(Role::TextStrong)
            } else {
                self.system.style(Role::Text)
            };
            buffer.set_stringn(
                rect.x,
                rect.y,
                take_display_cols(&line, usize::from(rect.width)),
                usize::from(rect.width),
                style,
            );
            state.entry_hits.push((entry.id.clone(), rect));
        }

        if state.entries.is_empty() && matches!(state.status, FileListingStatus::Ready) {
            super::EmptyState::new("Empty folder", self.system)
                .inline()
                .paint(Rect::new(area.x, area.y, area.width, 1), buffer);
        }
    }

    fn paint_preview(&self, area: Rect, buffer: &mut Buffer, state: &FilePickerState) {
        if area.is_empty() {
            return;
        }
        buffer.set_style(area, self.system.style(Role::Elevated));
        let Some(preview) = &state.preview else {
            buffer.set_stringn(
                area.x,
                area.y,
                take_display_cols("No preview", usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
            return;
        };
        if let Some(err) = &preview.error {
            buffer.set_stringn(
                area.x,
                area.y,
                take_display_cols(err, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::Danger),
            );
            return;
        }
        let mut y = area.y;
        if !preview.title.is_empty() {
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&preview.title, usize::from(area.width)),
                usize::from(area.width),
                self.system
                    .style(Role::TextStrong)
                    .add_modifier(Modifier::BOLD),
            );
            y = y.saturating_add(1);
        }
        for line in &preview.lines {
            if y >= area.bottom() {
                break;
            }
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(line, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::Text),
            );
            y = y.saturating_add(1);
        }
    }

    /// Semantic registration.
    pub fn register_semantic<Id, Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        id: Id,
        area: Rect,
        state: &FilePickerState,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if area.is_empty() {
            return;
        }
        let desc = format!(
            "file-picker {} {} entries={}",
            state.mode.id(),
            state.status.id(),
            state.entries.len()
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Dialog)
                .label(self.title)
                .description(desc)
                .focusable(state.enabled)
                .disabled(!state.enabled)
                .state(SemanticState {
                    selected: state.focused,
                    busy: matches!(state.status, FileListingStatus::Loading),
                    invalid: matches!(state.status, FileListingStatus::Error),
                    expanded: true,
                    ..Default::default()
                }),
        );
    }
}

impl StatefulWidget for &FilePicker<'_> {
    type State = FilePickerState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl StatefulWidget for FilePicker<'_> {
    type State = FilePickerState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::RolePalette;

    fn sample_entries(cwd: &str) -> Vec<FileEntry> {
        vec![
            FileEntry::directory("d1", "src", format!("{cwd}/src")),
            FileEntry::file("f1", "README.md", format!("{cwd}/README.md")).size(100),
            FileEntry::file("f2", ".hidden", format!("{cwd}/.hidden")).hidden(true),
            FileEntry::file("f3", "secret.env", format!("{cwd}/secret.env"))
                .error("permission denied"),
        ]
    }

    #[test]
    fn list_request_and_apply_race() {
        let mut state = FilePickerState::new("/home/u");
        state.set_focused(true);
        match state.request_list("/home/u/proj") {
            FilePickerOutcome::ListRequested { path, generation } => {
                assert_eq!(path, "/home/u/proj");
                assert_eq!(generation, 1);
                // stale
                assert!(!state.apply_listing(0, "/old", sample_entries("/old"), None));
                assert!(state.apply_listing(
                    generation,
                    "/home/u/proj",
                    sample_entries("/home/u/proj"),
                    None
                ));
                assert_eq!(state.listing_status(), FileListingStatus::Ready);
                // hidden filtered by default
                assert!(!state.entries().iter().any(|e| e.name == ".hidden"));
                assert!(state.entries().iter().any(|e| e.name == "src"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn open_directory_and_confirm_file() {
        let mut state = FilePickerState::new("/proj").with_mode(FilePickerMode::OpenFile);
        state.set_focused(true);
        let g = 1;
        state.generation = 1;
        assert!(state.apply_listing(g, "/proj", sample_entries("/proj"), None));
        // highlight first (src dir)
        assert_eq!(state.highlight().map(|e| e.name.as_str()), Some("src"));
        // open dir
        assert!(matches!(
            state.open_highlight(),
            FilePickerOutcome::ListRequested { .. }
        ));
        // re-apply as file list (Name sort → lib.rs first)
        state.generation = 2;
        let files = vec![
            FileEntry::file("a", "main.rs", "/proj/src/main.rs"),
            FileEntry::file("b", "lib.rs", "/proj/src/lib.rs"),
        ];
        assert!(state.apply_listing(2, "/proj/src", files, None));
        assert_eq!(state.highlight().map(|e| e.name.as_str()), Some("lib.rs"));
        assert!(matches!(
            state.open_highlight(),
            FilePickerOutcome::Confirmed { paths } if paths[0].ends_with("lib.rs")
        ));
    }

    #[test]
    fn multi_select_space() {
        let mut state = FilePickerState::new("/p")
            .with_mode(FilePickerMode::OpenAny)
            .with_multi(true);
        state.set_focused(true);
        state.generation = 1;
        assert!(state.apply_listing(1, "/p", sample_entries("/p"), None));
        // move to README
        let items: Vec<_> = state
            .entries()
            .iter()
            .map(|e| CollectionItem::new(e.id.clone(), e.name.clone()).enabled(true))
            .collect();
        let _ = state.collection.move_next(&items);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)),
            FilePickerOutcome::SelectionChanged
        ));
        assert!(!state.selected_paths().is_empty());
    }

    #[test]
    fn show_hidden_filter_change() {
        let mut state = FilePickerState::new("/p");
        state.set_focused(true);
        state.generation = 1;
        assert!(state.apply_listing(1, "/p", sample_entries("/p"), None));
        assert!(!state.entries().iter().any(|e| e.name == ".hidden"));
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL)),
            FilePickerOutcome::FilterChanged
        ));
        assert!(state.show_hidden());
        assert!(state.entries().iter().any(|e| e.name == ".hidden"));
    }

    #[test]
    fn esc_leaves_path_before_cancel() {
        let mut state = FilePickerState::new("/p");
        state.set_focused(true);
        // Tab → Path pane
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
            FilePickerOutcome::Changed
        ));
        assert_eq!(state.pane(), FilePickerPane::Path);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            FilePickerOutcome::Changed
        ));
        assert_eq!(state.pane(), FilePickerPane::List);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            FilePickerOutcome::Cancelled
        ));
    }

    #[test]
    fn breadcrumbs_unix() {
        let mut state = FilePickerState::new("/");
        state.cwd = "/a/b".into();
        state.rebuild_breadcrumbs();
        assert!(state.breadcrumbs.len() >= 3);
        assert_eq!(state.breadcrumbs[0].path, "/");
    }

    #[test]
    fn windows_style_paths() {
        let mut state = FilePickerState::new(r"C:\Users").with_path_style(PathStyle::Windows);
        state.cwd = r"C:\Users\me".into();
        state.rebuild_breadcrumbs();
        assert!(!state.breadcrumbs.is_empty());
    }

    #[test]
    fn listing_error() {
        let mut state = FilePickerState::new("/root");
        state.generation = 3;
        assert!(state.apply_listing_error(3, "permission denied"));
        assert_eq!(state.listing_status(), FileListingStatus::Error);
        assert!(!state.apply_listing_error(1, "stale"));
    }

    #[test]
    fn presentation_tiny() {
        let tiny = Rect::new(0, 0, 30, 8);
        assert_eq!(
            FilePickerState::presentation_for_bounds(tiny),
            FilePickerPresentation::Fullscreen
        );
    }

    #[test]
    fn paint_unix_story_shape() {
        let system = DesignSystem::from_palette(RolePalette::default());
        let mut state = FilePickerState::new("/home/u")
            .with_mode(FilePickerMode::OpenFile)
            .with_preview(true);
        state.set_focused(true);
        state.generation = 1;
        assert!(state.apply_listing(1, "/home/u", sample_entries("/home/u"), None));
        state.apply_preview(FilePreview::text("README.md", ["# hi".into()]));
        let area = Rect::new(0, 0, 80, 20);
        let mut buf = Buffer::empty(area);
        FilePicker::new(&system)
            .title("Open file")
            .ascii(true)
            .paint(area, &mut buf, &mut state);
        assert!(!state.list_area.is_empty());
        assert!(!state.entry_hits.is_empty());
    }

    #[test]
    fn paint_no_preview() {
        let system = DesignSystem::default();
        let mut state = FilePickerState::new("/tmp").with_preview(false);
        state.set_focused(true);
        state.generation = 1;
        let _ = state.apply_listing(1, "/tmp", sample_entries("/tmp"), None);
        let area = Rect::new(0, 0, 60, 16);
        let mut buf = Buffer::empty(area);
        FilePicker::new(&system)
            .show_preview(false)
            .ascii(true)
            .paint(area, &mut buf, &mut state);
        assert!(state.preview_area.is_empty() || state.preview_area.width == 0);
    }

    #[test]
    fn mouse_breadcrumb_and_double_click() {
        let system = DesignSystem::default();
        let mut state = FilePickerState::new("/a/b");
        state.set_focused(true);
        state.generation = 1;
        let _ = state.apply_listing(1, "/a/b", sample_entries("/a/b"), None);
        let area = Rect::new(0, 0, 70, 18);
        let mut buf = Buffer::empty(area);
        FilePicker::new(&system)
            .ascii(true)
            .paint(area, &mut buf, &mut state);
        assert!(!state.breadcrumb_hits.is_empty());
        let (path, rect) = state.breadcrumb_hits[0].clone();
        assert!(matches!(
            state.handle_mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: Position::new(rect.x, rect.y),
                modifiers: KeyModifiers::NONE,
            }),
            FilePickerOutcome::ListRequested { path: p, .. } if p == path
        ));
    }

    #[test]
    fn fuzz_keys() {
        let mut state = FilePickerState::new("/p").with_multi(true);
        state.set_focused(true);
        state.generation = 1;
        let _ = state.apply_listing(1, "/p", sample_entries("/p"), None);
        let keys = [
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL),
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
        ];
        for key in keys.iter().cycle().take(30) {
            let _ = state.handle_key(*key);
        }
    }

    #[test]
    fn paint_hot_path() {
        let system = DesignSystem::default();
        let mut state = FilePickerState::new("/hot");
        state.set_focused(true);
        state.generation = 1;
        let mut entries = Vec::new();
        for i in 0..40 {
            entries.push(FileEntry::file(
                format!("f{i}"),
                format!("file{i}.txt"),
                format!("/hot/file{i}.txt"),
            ));
        }
        let _ = state.apply_listing(1, "/hot", entries, None);
        let area = Rect::new(0, 0, 72, 20);
        let mut buf = Buffer::empty(area);
        let w = FilePicker::new(&system).ascii(true);
        for _ in 0..50 {
            w.paint(area, &mut buf, &mut state);
        }
    }

    #[test]
    fn semantic() {
        let system = DesignSystem::default();
        let state = FilePickerState::new("/x");
        let mut scene = SemanticScene::<&str, ()>::default();
        FilePicker::new(&system).register_semantic(
            &mut scene,
            "fp",
            Rect::new(0, 0, 40, 12),
            &state,
        );
        assert!(scene.get(&"fp").is_some());
    }

    #[test]
    fn overlay_helpers() {
        let mut stack = OverlayStack::<&str>::default();
        let bounds = Rect::new(0, 0, 80, 24);
        let _ = FilePickerState::open_overlay(
            &mut stack,
            bounds,
            OverlaySize::dialog(60, 20),
            Some("opener"),
        );
        let _ = FilePickerState::dismiss_overlay(&mut stack);
    }
}
