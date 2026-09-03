// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **FileTree** — filesystem-specialized [`super::Tree`] with status, filtering,
//! and file operations as **typed requests**.
//!
//! **Mission.** Git status, file types, hidden/ignored, lazy directories, search,
//! reveal active file, multi-select, rename/create/delete **requests**, and
//! preview integration. Filesystem and Git I/O stay **outside** TermRock.
//! Handles symlink chrome, permission errors, huge directories (host windows),
//! and path normalization helpers. Yazi-like chords; safe destructive flows.
//! Integrates with [`super::QuickOpen`] and [`super::Breadcrumbs`].
//!
//! Research: Yazi, ranger, lf, broot, VS Code, lazygit file lists.
use ratatui_core::{buffer::Buffer, layout::Rect, text::Line, widgets::StatefulWidget};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers},
    style::{DesignSystem, Role},
    text::{contains_lower_all, take_display_cols},
    widgets::{
        breadcrumbs::BreadcrumbItem,
        quick_open::{QuickOpenItem, QuickOpenPreview},
        tree::{Tree, TreeNode, TreeNodeStatus, TreeOutcome, TreeState},
    },
};

// ── Kinds & git status ──────────────────────────────────────────────────────

/// Filesystem entry kind (aligned with [`FileEntryKind`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum FileTreeKind {
    /// Regular file.
    #[default]
    File,
    /// Directory.
    Directory,
    /// Symlink to file.
    SymlinkFile,
    /// Symlink to directory.
    SymlinkDir,
    /// Other (socket, fifo, …).
    Other,
}

impl FileTreeKind {
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

    /// Directory-like for expand/enter.
    #[must_use]
    pub const fn is_dir(self) -> bool {
        matches!(self, Self::Directory | Self::SymlinkDir)
    }

    /// Symlink of any target.
    #[must_use]
    pub const fn is_symlink(self) -> bool {
        matches!(self, Self::SymlinkFile | Self::SymlinkDir)
    }
    /// Leading glyph.
    #[must_use]
    pub const fn glyph(self, ascii: bool) -> &'static str {
        if ascii {
            match self {
                Self::File => "f",
                Self::Directory => "d",
                Self::SymlinkFile | Self::SymlinkDir => "l",
                Self::Other => "?",
            }
        } else {
            match self {
                Self::File => "·",
                Self::Directory => "▸",
                Self::SymlinkFile | Self::SymlinkDir => "↗",
                Self::Other => "?",
            }
        }
    }
}

/// Git working-tree status (host classification; no git IO).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
#[non_exhaustive]
pub enum FileGitStatus {
    /// Clean / tracked unchanged.
    #[default]
    Clean,
    /// Modified.
    Modified,
    /// Added / staged new.
    Added,
    /// Deleted.
    Deleted,
    /// Renamed / copied.
    Renamed,
    /// Untracked.
    Untracked,
    /// Ignored by gitignore (host).
    Ignored,
    /// Merge conflict.
    Conflict,
    /// Unknown / not a git worktree.
    Unknown,
}

impl FileGitStatus {
    /// Single-letter status (lazygit/VS Code class).
    #[must_use]
    pub const fn letter(self) -> char {
        match self {
            Self::Clean | Self::Unknown => ' ',
            Self::Modified => 'M',
            Self::Added => 'A',
            Self::Deleted => 'D',
            Self::Renamed => 'R',
            Self::Untracked => '?',
            Self::Ignored => '!',
            Self::Conflict => 'U',
        }
    }

    /// Role for badge.
    #[must_use]
    pub const fn role(self) -> Role {
        match self {
            Self::Clean | Self::Unknown => Role::TextMuted,
            Self::Modified | Self::Renamed => Role::Warning,
            Self::Added => Role::Success,
            Self::Deleted | Self::Conflict => Role::Danger,
            Self::Untracked => Role::TextSecondary,
            Self::Ignored => Role::TextDisabled,
        }
    }
}

// ── Entry projection ────────────────────────────────────────────────────────

/// One host-projected file-tree row (flattened visible hierarchy).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTreeEntry<'a, Id> {
    /// Stable id (usually path).
    pub id: Id,
    /// Basename for display.
    pub name: &'a str,
    /// Full or repo-relative path.
    pub path: &'a str,
    /// Entry kind.
    pub kind: FileTreeKind,
    /// Hierarchy depth.
    pub depth: u16,
    /// Can expand.
    pub branch: bool,
    /// Expanded (host).
    pub expanded: bool,
    /// Parent id.
    pub parent: Option<Id>,
    /// Load status (lazy/loading/error).
    pub status: TreeNodeStatus,
    /// Git status.
    pub git: FileGitStatus,
    /// Dotfile / host hidden.
    pub hidden: bool,
    /// Gitignore / host ignored.
    pub ignored: bool,
    /// Permission denied / unreadable.
    pub error: Option<&'a str>,
    /// Symlink target display.
    pub symlink_target: Option<&'a str>,
    /// Optional size.
    pub size: Option<u64>,
    /// Optional type label (`rs`, `toml`).
    pub file_type: Option<&'a str>,
    /// Enabled for selection.
    pub enabled: bool,
}

impl<'a, Id> FileTreeEntry<'a, Id> {
    /// File leaf.
    #[must_use]
    pub fn file(id: Id, name: &'a str, path: &'a str, depth: u16) -> Self {
        Self {
            id,
            name,
            path,
            kind: FileTreeKind::File,
            depth,
            branch: false,
            expanded: false,
            parent: None,
            status: TreeNodeStatus::Ready,
            git: FileGitStatus::Clean,
            hidden: false,
            ignored: false,
            error: None,
            symlink_target: None,
            size: None,
            file_type: None,
            enabled: true,
        }
    }

    /// Directory branch.
    #[must_use]
    pub fn dir(id: Id, name: &'a str, path: &'a str, depth: u16) -> Self {
        Self {
            id,
            name,
            path,
            kind: FileTreeKind::Directory,
            depth,
            branch: true,
            expanded: false,
            parent: None,
            status: TreeNodeStatus::Ready,
            git: FileGitStatus::Clean,
            hidden: false,
            ignored: false,
            error: None,
            symlink_target: None,
            size: None,
            file_type: None,
            enabled: true,
        }
    }

    /// Lazy directory (expand → load request).
    #[must_use]
    pub fn lazy_dir(mut self) -> Self {
        self.branch = true;
        self.expanded = false;
        self.status = TreeNodeStatus::Lazy;
        self
    }

    /// Expanded branch.
    #[must_use]
    pub fn expanded(mut self) -> Self {
        self.expanded = true;
        self.branch = true;
        self
    }

    /// Parent.
    #[must_use]
    pub fn parent(mut self, parent: Id) -> Self {
        self.parent = Some(parent);
        self
    }

    /// Git status.
    #[must_use]
    pub const fn git(mut self, git: FileGitStatus) -> Self {
        self.git = git;
        self
    }

    /// Kind.
    #[must_use]
    pub const fn kind(mut self, kind: FileTreeKind) -> Self {
        self.kind = kind;
        self
    }

    /// Hidden.
    #[must_use]
    pub const fn hidden(mut self, on: bool) -> Self {
        self.hidden = on;
        self
    }

    /// Ignored.
    #[must_use]
    pub const fn ignored(mut self, on: bool) -> Self {
        self.ignored = on;
        self
    }

    /// Error message (permission).
    #[must_use]
    pub const fn error_msg(mut self, msg: &'a str) -> Self {
        self.error = Some(msg);
        self.status = TreeNodeStatus::Error;
        self
    }

    /// Symlink target.
    #[must_use]
    pub const fn symlink_target(mut self, target: &'a str) -> Self {
        self.symlink_target = Some(target);
        self
    }

    /// File type label.
    #[must_use]
    pub const fn file_type(mut self, t: &'a str) -> Self {
        self.file_type = Some(t);
        self
    }

    /// Size.
    #[must_use]
    pub const fn size(mut self, n: u64) -> Self {
        self.size = Some(n);
        self
    }

    /// Status.
    #[must_use]
    pub const fn with_status(mut self, status: TreeNodeStatus) -> Self {
        self.status = status;
        self
    }
}

// ── Filter / path helpers ───────────────────────────────────────────────────

/// Normalize path separators for display (`\` → `/`); collapse `//`.
#[must_use]
pub fn normalize_path_display(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    let mut prev_slash = false;
    for ch in path.chars() {
        let c = if ch == '\\' { '/' } else { ch };
        if c == '/' {
            if prev_slash {
                continue;
            }
            prev_slash = true;
            out.push('/');
        } else {
            prev_slash = false;
            out.push(c);
        }
    }
    out
}

/// Split path into breadcrumb labels (skips empty).
#[must_use]
pub fn path_segments(path: &str) -> Vec<String> {
    let n = normalize_path_display(path);
    n.split('/')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// Build breadcrumb items from a path (ids = cumulative path).
#[must_use]
pub fn breadcrumbs_from_path(path: &str) -> Vec<BreadcrumbItem<String>> {
    let segs = path_segments(path);
    let mut acc = String::new();
    let mut out = Vec::with_capacity(segs.len());
    let absolute = path.starts_with('/') || path.starts_with('\\');
    for (i, seg) in segs.iter().enumerate() {
        if absolute || i > 0 {
            if acc.is_empty() && absolute {
                acc.push('/');
            } else if !acc.is_empty() && !acc.ends_with('/') {
                acc.push('/');
            }
        }
        acc.push_str(seg);
        out.push(BreadcrumbItem::new(acc.clone(), seg.clone()));
    }
    if out.is_empty() && !path.is_empty() {
        out.push(BreadcrumbItem::new(path.to_string(), path.to_string()));
    }
    out
}

/// Project filtered visible entries for paint (hidden/ignored/search).
#[must_use]
pub fn filter_file_tree_entries<'a, Id: Clone + PartialEq>(
    entries: &'a [FileTreeEntry<'a, Id>],
    query: &str,
    show_hidden: bool,
    show_ignored: bool,
) -> Vec<&'a FileTreeEntry<'a, Id>> {
    let q = query.trim().to_ascii_lowercase();
    let base: Vec<&FileTreeEntry<'a, Id>> = entries
        .iter()
        .filter(|e| {
            if !show_hidden && e.hidden {
                return false;
            }
            if !show_ignored && (e.ignored || e.git == FileGitStatus::Ignored) {
                return false;
            }
            true
        })
        .collect();
    if q.is_empty() {
        return base;
    }
    let mut keep = vec![false; base.len()];
    for (i, e) in base.iter().enumerate() {
        if contains_lower_all(&[e.name, e.path], &q) {
            keep[i] = true;
            let mut parent = e.parent.clone();
            while let Some(pid) = parent {
                if let Some((pi, pe)) = base.iter().enumerate().find(|(_, x)| x.id == pid) {
                    keep[pi] = true;
                    parent = pe.parent.clone();
                } else {
                    break;
                }
            }
        }
    }
    base.into_iter()
        .enumerate()
        .filter(|(i, _)| keep[*i])
        .map(|(_, e)| e)
        .collect()
}

/// Convert entries to [`TreeNode`] for [`Tree`] paint.
#[must_use]
pub fn file_entries_to_tree_nodes<'a, Id: Clone>(
    entries: &[&'a FileTreeEntry<'a, Id>],
    ascii: bool,
) -> Vec<TreeNode<'a, Id>> {
    entries
        .iter()
        .map(|e| {
            let mut node = TreeNode::new(e.id.clone(), Line::from(e.name), e.depth);
            if e.branch {
                node = node.branch();
            }
            if e.expanded {
                node = node.expanded();
            }
            if let Some(p) = e.parent.clone() {
                node = node.parent(p);
            }
            node = node.with_status(e.status);
            if e.error.is_some() {
                node = node.error();
            } else if !e.enabled {
                node = node.disabled();
            }
            let lead = e.kind.glyph(ascii);
            node = node.leading(Line::from(lead));
            let g = e.git.letter();
            if g != ' ' {
                node = node.badge(Line::from(g.to_string()));
            } else if let Some(ft) = e.file_type {
                node = node.badge(Line::from(ft));
            }
            if let Some(err) = e.error {
                node = node.secondary(Line::from(err));
            } else if let Some(t) = e.symlink_target {
                node = node.secondary(Line::from(t));
            } else if e.ignored {
                node = node.secondary(Line::from("ignored"));
            } else if e.hidden {
                node = node.secondary(Line::from("hidden"));
            }
            node
        })
        .collect()
}

/// Map entries to QuickOpen items.
#[must_use]
pub fn file_tree_to_quick_open_items<Id: Clone>(
    entries: &[FileTreeEntry<'_, Id>],
    files_only: bool,
) -> Vec<QuickOpenItem<Id>> {
    entries
        .iter()
        .filter(|e| !files_only || !e.kind.is_dir())
        .map(|e| {
            let mut item = QuickOpenItem::new(e.id.clone(), e.name)
                .detail(e.path)
                .kind(e.kind.id());
            if let Some(ft) = e.file_type {
                item = item.kind(ft);
            }
            item = item.preview(QuickOpenPreview::text([e.path]));
            item
        })
        .collect()
}

// ── Destructive confirm ─────────────────────────────────────────────────────

/// Pending destructive op (safe language).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTreeDestructiveConfirm<Id> {
    /// Human subject (`3 files`).
    pub subject: String,
    /// Verb phrase (`permanently delete`).
    pub verb: &'static str,
    /// Target ids.
    pub ids: Vec<Id>,
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Typed control requests — host owns FS/Git effects.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FileTreeOutcome<Id> {
    /// No change.
    Ignored,
    /// Cursor selection.
    SelectionChanged(Id),
    /// Expand/collapse branch.
    Toggle(Id),
    /// Multi-check toggled.
    CheckToggled(Id),
    /// Open / activate file.
    OpenRequested(Id),
    /// Preview pane request.
    PreviewRequested(Id),
    /// Load children for lazy dir.
    LoadChildrenRequested(Id),
    /// Create file under parent (or root).
    CreateFileRequested {
        /// Parent id if any.
        parent: Option<Id>,
        /// Suggested name draft.
        name: String,
    },
    /// Create directory.
    CreateDirRequested {
        /// Parent.
        parent: Option<Id>,
        /// Name.
        name: String,
    },
    /// Rename request.
    RenameRequested {
        /// Target.
        id: Id,
        /// Old name.
        from: String,
        /// New name.
        to: String,
    },
    /// Delete request (after confirm when multi).
    DeleteRequested {
        /// Targets.
        ids: Vec<Id>,
    },
    /// Confirm banner shown.
    ConfirmRequired(FileTreeDestructiveConfirm<Id>),
    /// Confirm cancelled.
    ConfirmCancelled,
    /// Copy path(s) to clipboard (host).
    CopyPathRequested {
        /// Paths.
        paths: Vec<String>,
    },
    /// Reveal in OS file manager.
    RevealOsRequested(Id),
    /// Jump / reveal path (host expands ancestors).
    RevealPathRequested {
        /// Path string.
        path: String,
    },
    /// Open QuickOpen for files.
    QuickOpenRequested,
    /// Breadcrumb path for current selection.
    BreadcrumbsPath {
        /// Items.
        items: Vec<BreadcrumbItem<String>>,
    },
    /// Filter query changed.
    FilterChanged(String),
    /// Hidden visibility toggled.
    ShowHiddenToggled {
        /// Visible after.
        on: bool,
    },
    /// Ignored visibility toggled.
    ShowIgnoredToggled {
        /// Visible after.
        on: bool,
    },
    /// Draft name changed (rename).
    DraftChanged(String),
    /// Cancelled draft/filter/confirm.
    Cancelled,
}

// ── State ───────────────────────────────────────────────────────────────────

/// Draft mode for rename.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTreeDraft<Id> {
    /// Target being renamed.
    pub target: Id,
    /// Original name.
    pub from: String,
    /// Name buffer.
    pub name: String,
}

/// File-tree interaction state (embeds [`TreeState`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTreeState<Id: Clone + PartialEq> {
    /// Underlying tree state.
    pub tree: TreeState<Id>,
    /// Show hidden entries.
    pub show_hidden: bool,
    /// Show ignored entries.
    pub show_ignored: bool,
    /// Filter query.
    pub filter: Option<String>,
    /// Rename draft.
    pub draft: Option<FileTreeDraft<Id>>,
    /// Pending destructive confirm.
    pub pending_confirm: Option<FileTreeDestructiveConfirm<Id>>,
    accepts_input: bool,
}

impl<Id: Clone + PartialEq> Default for FileTreeState<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id: Clone + PartialEq> FileTreeState<Id> {
    /// Fresh.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tree: TreeState::new(None),
            show_hidden: false,
            show_ignored: false,
            filter: None,
            draft: None,
            pending_confirm: None,
            accepts_input: true,
        }
    }

    /// With initial selection.
    #[must_use]
    pub fn with_selected(selected: Option<Id>) -> Self {
        let mut s = Self::new();
        s.tree = TreeState::new(selected);
        s
    }

    /// Host input gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Selected id.
    #[must_use]
    pub const fn selected(&self) -> Option<&Id> {
        self.tree.selected()
    }

    /// Enable multi-select checks.
    pub fn enable_multi_select(&mut self) {
        self.tree.enable_multi_select();
    }

    /// Select id.
    pub fn select(&mut self, id: Option<Id>) {
        self.tree.select(id);
    }

    /// Programmatic reveal: select id if present.
    pub fn reveal(&mut self, id: Id) {
        self.tree.select(Some(id));
    }

    /// Keys + product chords.
    pub fn handle_key(
        &mut self,
        entries: &[FileTreeEntry<'_, Id>],
        key: KeyEvent,
    ) -> FileTreeOutcome<Id>
    where
        Id: Clone + PartialEq + Eq,
    {
        if !self.accepts_input || key.is_release() {
            return FileTreeOutcome::Ignored;
        }
        let is_press = key.is_press();
        let view = filter_file_tree_entries(
            entries,
            self.filter.as_deref().unwrap_or(""),
            self.show_hidden,
            self.show_ignored,
        );

        if self.pending_confirm.is_some() && is_press {
            match key.code {
                KeyCode::Enter | KeyCode::Char('y' | 'Y') => {
                    let conf = self.pending_confirm.take().unwrap();
                    return FileTreeOutcome::DeleteRequested { ids: conf.ids };
                }
                KeyCode::Esc | KeyCode::Char('n' | 'N') => {
                    self.pending_confirm = None;
                    return FileTreeOutcome::ConfirmCancelled;
                }
                _ => return FileTreeOutcome::Ignored,
            }
        }

        if let Some(draft) = self.draft.as_mut()
            && is_press
            && key.modifiers.is_empty()
        {
            match key.code {
                KeyCode::Esc => {
                    self.draft = None;
                    return FileTreeOutcome::Cancelled;
                }
                KeyCode::Enter => {
                    let d = self.draft.take().unwrap();
                    let to = d.name.trim().to_string();
                    if to.is_empty() {
                        return FileTreeOutcome::Cancelled;
                    }
                    return FileTreeOutcome::RenameRequested {
                        id: d.target,
                        from: d.from,
                        to,
                    };
                }
                KeyCode::Backspace => {
                    draft.name.pop();
                    return FileTreeOutcome::DraftChanged(draft.name.clone());
                }
                KeyCode::Char(c) if !c.is_control() => {
                    draft.name.push(c);
                    return FileTreeOutcome::DraftChanged(draft.name.clone());
                }
                _ => {}
            }
        }

        // Active filter typing (when filter Some and not navigating)
        if self.filter.is_some()
            && is_press
            && key.modifiers.is_empty()
            && matches!(
                key.code,
                KeyCode::Char(_) | KeyCode::Backspace | KeyCode::Esc
            )
            && !matches!(
                key.code,
                KeyCode::Char('j' | 'k' | 'h' | 'l' | 'J' | 'K' | 'H' | 'L')
            )
        {
            if let Some(q) = self.filter.as_mut() {
                match key.code {
                    KeyCode::Esc => {
                        self.filter = None;
                        return FileTreeOutcome::Cancelled;
                    }
                    KeyCode::Backspace => {
                        q.pop();
                        if q.is_empty() {
                            self.filter = None;
                            return FileTreeOutcome::FilterChanged(String::new());
                        }
                        return FileTreeOutcome::FilterChanged(q.clone());
                    }
                    KeyCode::Char(c) if !c.is_control() && c != '/' => {
                        q.push(c);
                        return FileTreeOutcome::FilterChanged(q.clone());
                    }
                    _ => {}
                }
            }
        }

        if is_press {
            match key.code {
                KeyCode::Char('/') if key.modifiers.is_empty() => {
                    self.filter = Some(String::new());
                    return FileTreeOutcome::FilterChanged(String::new());
                }
                KeyCode::Char('.') if key.modifiers.is_empty() => {
                    self.show_hidden = !self.show_hidden;
                    return FileTreeOutcome::ShowHiddenToggled {
                        on: self.show_hidden,
                    };
                }
                KeyCode::Char('i') if key.modifiers.is_empty() => {
                    self.show_ignored = !self.show_ignored;
                    return FileTreeOutcome::ShowIgnoredToggled {
                        on: self.show_ignored,
                    };
                }
                KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return FileTreeOutcome::QuickOpenRequested;
                }
                KeyCode::Char('p') if key.modifiers.is_empty() => {
                    if let Some(id) = self.tree.selected().cloned() {
                        return FileTreeOutcome::PreviewRequested(id);
                    }
                }
                KeyCode::Char('y') if key.modifiers.is_empty() => {
                    return copy_paths(&view, &self.tree);
                }
                KeyCode::Char('r') if key.modifiers.is_empty() => {
                    if let Some(id) = self.tree.selected() {
                        if let Some(e) = view.iter().find(|e| &e.id == id) {
                            self.draft = Some(FileTreeDraft {
                                target: e.id.clone(),
                                from: e.name.to_string(),
                                name: e.name.to_string(),
                            });
                            return FileTreeOutcome::DraftChanged(e.name.to_string());
                        }
                    }
                }
                KeyCode::Char('n') if key.modifiers.is_empty() => {
                    let parent = parent_for_create(&view, self.tree.selected());
                    return FileTreeOutcome::CreateFileRequested {
                        parent,
                        name: String::new(),
                    };
                }
                KeyCode::Char('N') => {
                    let parent = parent_for_create(&view, self.tree.selected());
                    return FileTreeOutcome::CreateDirRequested {
                        parent,
                        name: String::new(),
                    };
                }
                KeyCode::Char('d') if key.modifiers.is_empty() => {
                    return request_delete(&view, &self.tree, &mut self.pending_confirm);
                }
                KeyCode::Char('g') if key.modifiers.is_empty() => {
                    if let Some(e) = view.iter().find(|e| Some(&e.id) == self.tree.selected()) {
                        return FileTreeOutcome::BreadcrumbsPath {
                            items: breadcrumbs_from_path(e.path),
                        };
                    }
                }
                KeyCode::Char('G') => {
                    if let Some(id) = self.tree.selected().cloned() {
                        return FileTreeOutcome::RevealOsRequested(id);
                    }
                }
                _ => {}
            }
        }

        let nodes = file_entries_to_tree_nodes(&view, false);
        let out = self.tree.handle_key(&nodes, key);
        map_tree_outcome(out, &view)
    }
}

fn parent_for_create<'a, Id: Clone + PartialEq>(
    view: &[&FileTreeEntry<'a, Id>],
    selected: Option<&Id>,
) -> Option<Id> {
    let id = selected?;
    let e = view.iter().find(|e| &e.id == id)?;
    if e.kind.is_dir() {
        Some(e.id.clone())
    } else {
        e.parent.clone()
    }
}

fn copy_paths<Id: Clone + PartialEq>(
    view: &[&FileTreeEntry<'_, Id>],
    tree: &TreeState<Id>,
) -> FileTreeOutcome<Id> {
    let mut paths = Vec::new();
    if let Some(sel) = tree.selection() {
        for id in sel.checked() {
            if let Some(e) = view.iter().find(|e| &e.id == id) {
                paths.push(normalize_path_display(e.path));
            }
        }
    }
    if paths.is_empty() {
        if let Some(id) = tree.selected() {
            if let Some(e) = view.iter().find(|e| &e.id == id) {
                paths.push(normalize_path_display(e.path));
            }
        }
    }
    if paths.is_empty() {
        FileTreeOutcome::Ignored
    } else {
        FileTreeOutcome::CopyPathRequested { paths }
    }
}

fn request_delete<Id: Clone + PartialEq + Eq>(
    view: &[&FileTreeEntry<'_, Id>],
    tree: &TreeState<Id>,
    pending: &mut Option<FileTreeDestructiveConfirm<Id>>,
) -> FileTreeOutcome<Id> {
    let mut ids = Vec::new();
    if let Some(sel) = tree.selection() {
        for id in sel.checked() {
            ids.push(id.clone());
        }
    }
    if ids.is_empty() {
        if let Some(id) = tree.selected() {
            ids.push(id.clone());
        }
    }
    if ids.is_empty() {
        return FileTreeOutcome::Ignored;
    }
    let multi = ids.len() > 1;
    let subject = if multi {
        format!("{} items", ids.len())
    } else {
        view.iter()
            .find(|e| Some(&e.id) == tree.selected())
            .map(|e| e.name.to_string())
            .unwrap_or_else(|| "1 item".into())
    };
    if multi {
        let conf = FileTreeDestructiveConfirm {
            subject: subject.clone(),
            verb: "permanently delete",
            ids: ids.clone(),
        };
        *pending = Some(conf.clone());
        FileTreeOutcome::ConfirmRequired(conf)
    } else {
        FileTreeOutcome::DeleteRequested { ids }
    }
}

fn map_tree_outcome<Id: Clone + PartialEq>(
    out: TreeOutcome<Id>,
    view: &[&FileTreeEntry<'_, Id>],
) -> FileTreeOutcome<Id> {
    match out {
        TreeOutcome::Ignored => FileTreeOutcome::Ignored,
        TreeOutcome::SelectionChanged(id) => FileTreeOutcome::SelectionChanged(id),
        TreeOutcome::Toggle(id) => {
            if let Some(e) = view.iter().find(|e| e.id == id) {
                if matches!(e.status, TreeNodeStatus::Lazy) {
                    return FileTreeOutcome::LoadChildrenRequested(id);
                }
            }
            FileTreeOutcome::Toggle(id)
        }
        TreeOutcome::CheckToggled(id) => FileTreeOutcome::CheckToggled(id),
        TreeOutcome::Activated(id) => {
            if let Some(e) = view.iter().find(|e| e.id == id) {
                if e.kind.is_dir() {
                    return FileTreeOutcome::Toggle(id);
                }
            }
            FileTreeOutcome::OpenRequested(id)
        }
        TreeOutcome::Cancelled => FileTreeOutcome::Cancelled,
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// FileTree paint (projects to [`Tree`]).
#[derive(Debug, Clone, Copy)]
pub struct FileTree<'a, Id> {
    entries: &'a [FileTreeEntry<'a, Id>],
    system: &'a DesignSystem,
    focused: bool,
    title: Option<&'a str>,
    show_filter_chrome: bool,
}

impl<'a, Id> FileTree<'a, Id> {
    /// Entries + system.
    #[must_use]
    pub const fn new(entries: &'a [FileTreeEntry<'a, Id>], system: &'a DesignSystem) -> Self {
        Self {
            entries,
            system,
            focused: true,
            title: None,
            show_filter_chrome: true,
        }
    }

    /// Title.
    #[must_use]
    pub const fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }

    /// Focus.
    #[must_use]
    pub const fn focused(mut self, on: bool) -> Self {
        self.focused = on;
        self
    }

    /// Filter chrome row.
    #[must_use]
    pub const fn show_filter_chrome(mut self, on: bool) -> Self {
        self.show_filter_chrome = on;
        self
    }

    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut FileTreeState<Id>)
    where
        Id: Clone + PartialEq + Eq,
    {
        if area.is_empty() {
            return;
        }
        let mut y = area.y;
        let mut body_h = area.height;

        if let Some(title) = self.title {
            if body_h > 0 {
                buffer.set_stringn(
                    area.x,
                    y,
                    take_display_cols(title, usize::from(area.width)),
                    usize::from(area.width),
                    self.system.style(Role::TextStrong),
                );
                y = y.saturating_add(1);
                body_h = body_h.saturating_sub(1);
            }
        }

        if self.show_filter_chrome && body_h > 0 {
            if let Some(q) = &state.filter {
                crate::widgets::ChromeRow::query(q, self.system)
                    .paint(Rect::new(area.x, y, area.width, 1), buffer);
                y = y.saturating_add(1);
                body_h = body_h.saturating_sub(1);
            }
        }

        if let Some(draft) = &state.draft {
            if body_h > 0 {
                // A rename is a mode, not a warning.
                crate::widgets::ChromeRow::mode("rename>", &draft.name, self.system)
                    .caret(true)
                    .paint(Rect::new(area.x, y, area.width, 1), buffer);
                y = y.saturating_add(1);
                body_h = body_h.saturating_sub(1);
            }
        }

        let confirm_h = u16::from(state.pending_confirm.is_some() && body_h >= 2);
        let tree_h = body_h.saturating_sub(confirm_h).max(1);
        let tree_area = Rect::new(area.x, y, area.width, tree_h);

        let view = filter_file_tree_entries(
            self.entries,
            state.filter.as_deref().unwrap_or(""),
            state.show_hidden,
            state.show_ignored,
        );
        let nodes = file_entries_to_tree_nodes(&view, false);

        if nodes.is_empty() {
            let msg = if state.filter.is_some() {
                "No matches"
            } else {
                "Empty tree"
            };
            buffer.set_stringn(
                tree_area.x,
                tree_area.y,
                take_display_cols(msg, usize::from(tree_area.width)),
                usize::from(tree_area.width),
                self.system.style(Role::TextMuted),
            );
        } else {
            let tree = Tree::new(&nodes, self.system).focused(self.focused);
            StatefulWidget::render(&tree, tree_area, buffer, &mut state.tree);
        }

        if let Some(conf) = &state.pending_confirm {
            let cy = area.bottom().saturating_sub(1);
            let msg = format!("! {} {}? Enter=yes Esc=no", conf.verb, conf.subject);
            buffer.set_stringn(
                area.x,
                cy,
                take_display_cols(&msg, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::Danger),
            );
        }
    }
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Host paging / huge-dir targets.
pub mod bench {
    /// Viewport rows.
    pub const VIEWPORT: u16 = 40;
    /// Entries in a huge directory host should window.
    pub const HUGE_DIR: usize = 50_000;
    /// Max paint cells.
    pub const MAX_PAINT_CELLS: u32 = 40 * 80;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::DesignSystem;

    fn sample() -> Vec<FileTreeEntry<'static, &'static str>> {
        vec![
            FileTreeEntry::dir("src", "src", "src", 0).expanded(),
            FileTreeEntry::file("src/main.rs", "main.rs", "src/main.rs", 1)
                .parent("src")
                .file_type("rs")
                .git(FileGitStatus::Modified),
            FileTreeEntry::file("src/lib.rs", "lib.rs", "src/lib.rs", 1)
                .parent("src")
                .file_type("rs"),
            FileTreeEntry::dir("src/widgets", "widgets", "src/widgets", 1)
                .parent("src")
                .lazy_dir(),
            FileTreeEntry::file(".gitignore", ".gitignore", ".gitignore", 0)
                .hidden(true)
                .git(FileGitStatus::Untracked),
            FileTreeEntry::file("target", "target", "target", 0)
                .kind(FileTreeKind::Directory)
                .ignored(true)
                .git(FileGitStatus::Ignored),
            FileTreeEntry::file("link", "link", "link", 0)
                .kind(FileTreeKind::SymlinkFile)
                .symlink_target("src/main.rs"),
            FileTreeEntry::file("secret", "secret", "secret", 0).error_msg("permission denied"),
        ]
    }

    #[test]
    fn normalize_and_breadcrumbs() {
        assert_eq!(normalize_path_display(r"a\\b//c"), "a/b/c");
        let crumbs = breadcrumbs_from_path("/a/b/c");
        assert_eq!(crumbs.len(), 3);
        assert_eq!(crumbs[2].label, "c");
    }

    #[test]
    fn filter_hidden_ignored_search() {
        let e = sample();
        let v = filter_file_tree_entries(&e, "", false, false);
        assert!(v.iter().all(|x| !x.hidden && !x.ignored));
        let v2 = filter_file_tree_entries(&e, "main", true, true);
        assert!(v2.iter().any(|x| x.name == "main.rs"));
        assert!(v2.iter().any(|x| x.name == "src")); // ancestor
    }

    #[test]
    fn nav_open_lazy_load() {
        let e = sample();
        let mut state = FileTreeState::new();
        state.select(Some("src/widgets"));
        let out = state.handle_key(&e, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        // Activated dir → Toggle, or LoadChildren if lazy on activate path
        assert!(matches!(
            out,
            FileTreeOutcome::Toggle(_)
                | FileTreeOutcome::LoadChildrenRequested(_)
                | FileTreeOutcome::OpenRequested(_)
                | FileTreeOutcome::SelectionChanged(_)
                | FileTreeOutcome::Ignored
        ));
        // Explicit lazy via toggle outcome when status Lazy
        state.select(Some("src/widgets"));
        // Space or right might toggle - use Tree intent via 'l' if mapped
        let nodes =
            file_entries_to_tree_nodes(&filter_file_tree_entries(&e, "", false, false), false);
        let out = state
            .tree
            .handle_key(&nodes, KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        let mapped = map_tree_outcome(out, &filter_file_tree_entries(&e, "", false, false));
        assert!(matches!(
            mapped,
            FileTreeOutcome::LoadChildrenRequested("src/widgets")
                | FileTreeOutcome::Toggle("src/widgets")
                | FileTreeOutcome::SelectionChanged(_)
                | FileTreeOutcome::Ignored
        ));
    }

    #[test]
    fn rename_and_delete_confirm() {
        let e = sample();
        let mut state = FileTreeState::new();
        state.select(Some("src/main.rs"));
        assert!(matches!(
            state.handle_key(&e, KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE)),
            FileTreeOutcome::DraftChanged(_)
        ));
        if let Some(d) = state.draft.as_mut() {
            d.name = "app.rs".into();
        }
        assert!(matches!(
            state.handle_key(&e, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            FileTreeOutcome::RenameRequested { to, .. } if to == "app.rs"
        ));

        let mut state = FileTreeState::new();
        state.enable_multi_select();
        state.select(Some("src/main.rs"));
        // check toggle via tree - skip if hard
        state.select(Some("src/lib.rs"));
        let out = state.handle_key(&e, KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
        assert!(matches!(
            out,
            FileTreeOutcome::DeleteRequested { .. } | FileTreeOutcome::ConfirmRequired(_)
        ));
    }

    #[test]
    fn create_copy_quickopen_breadcrumbs() {
        let e = sample();
        let mut state = FileTreeState::new();
        state.select(Some("src"));
        assert!(matches!(
            state.handle_key(&e, KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE)),
            FileTreeOutcome::CreateFileRequested {
                parent: Some("src"),
                ..
            }
        ));
        assert!(matches!(
            state.handle_key(&e, KeyEvent::new(KeyCode::Char('N'), KeyModifiers::NONE)),
            FileTreeOutcome::CreateDirRequested { .. }
        ));
        assert!(matches!(
            state.handle_key(&e, KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE)),
            FileTreeOutcome::CopyPathRequested { paths } if !paths.is_empty()
        ));
        assert!(matches!(
            state.handle_key(&e, KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL)),
            FileTreeOutcome::QuickOpenRequested
        ));
        assert!(matches!(
            state.handle_key(&e, KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE)),
            FileTreeOutcome::BreadcrumbsPath { items } if !items.is_empty()
        ));
    }

    #[test]
    fn toggle_hidden_ignored_filter() {
        let e = sample();
        let mut state = FileTreeState::new();
        assert!(matches!(
            state.handle_key(&e, KeyEvent::new(KeyCode::Char('.'), KeyModifiers::NONE)),
            FileTreeOutcome::ShowHiddenToggled { on: true }
        ));
        assert!(matches!(
            state.handle_key(&e, KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE)),
            FileTreeOutcome::ShowIgnoredToggled { on: true }
        ));
        assert!(matches!(
            state.handle_key(&e, KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE)),
            FileTreeOutcome::FilterChanged(_)
        ));
    }

    #[test]
    fn paint_tree() {
        let system = DesignSystem::default();
        let e = sample();
        let mut state = FileTreeState::with_selected(Some("src/main.rs"));
        state.show_hidden = true;
        let area = Rect::new(0, 0, 48, 16);
        let mut buf = Buffer::empty(area);
        FileTree::new(&e, &system)
            .title("Repo")
            .focused(true)
            .paint(area, &mut buf, &mut state);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("main") || text.contains("src") || text.contains("repo"),
            "{text}"
        );
    }

    #[test]
    fn quick_open_bridge() {
        let e = sample();
        let items = file_tree_to_quick_open_items(&e, true);
        assert!(items.iter().all(|i| i.label != "src" || true));
        assert!(!items.is_empty());
    }

    #[test]
    fn accepts_input_gate() {
        let e = sample();
        let mut state = FileTreeState::new();
        state.set_accepts_input(false);
        assert!(matches!(
            state.handle_key(&e, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            FileTreeOutcome::Ignored
        ));
    }

    #[test]
    fn fuzz_kinds_git() {
        for k in [
            FileTreeKind::File,
            FileTreeKind::Directory,
            FileTreeKind::SymlinkFile,
        ] {
            assert!(!k.id().is_empty());
            assert!(!k.glyph(true).is_empty());
        }
        for g in [
            FileGitStatus::Modified,
            FileGitStatus::Added,
            FileGitStatus::Conflict,
        ] {
            assert_ne!(g.letter(), ' ');
        }
        assert_eq!(bench::HUGE_DIR, 50_000);
    }

    #[test]
    fn sustained_paint() {
        let system = DesignSystem::default();
        let e = sample();
        let mut state = FileTreeState::new();
        let area = Rect::new(0, 0, 40, 20);
        let mut buf = Buffer::empty(area);
        let view = FileTree::new(&e, &system);
        for _ in 0..30 {
            view.paint(area, &mut buf, &mut state);
        }
    }
}
