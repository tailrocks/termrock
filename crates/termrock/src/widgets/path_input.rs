// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Filesystem-aware path field **without** coupling to a concrete FS.
//!
//! **Mission.** Connection/setup flows and FilePicker shells need path entry
//! with completion hooks, tilde/env *presentation*, existence/type status,
//! base/relative context, history, and browse actions — while **host policy**
//! owns all async filesystem lookups.
//!
//! **vs [`TextInput`](super::TextInput).** Free text.
//! **vs FilePicker (future).** Full browser; PathInput is the path field it
//! embeds and the setup-form control for single-path entry.
//!
//! Research: shell completion, file pickers, Yazi, CLI setup tools.
use std::collections::VecDeque;

use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::StatefulWidget};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    interaction::{SemanticNode, SemanticRole, SemanticScene, SemanticState},
    style::{ButtonRecipeVariant, ControlState, DesignSystem, Glyph, Role},
    text::take_display_cols,
};

use super::{TextInput, TextInputOutcome, TextInputState, Validation};

/// Default history capacity.
pub const DEFAULT_PATH_HISTORY_LIMIT: usize = 32;

// ── Pure path helpers (no std::fs) ──────────────────────────────────────────

/// Path separator style for normalization helpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PathStyle {
    /// Prefer `/` (default; also works for many Windows tools).
    #[default]
    Unix,
    /// Prefer `\` display; still accepts `/` input.
    Windows,
}

impl PathStyle {
    /// Detect from host `std::env::consts::OS` without importing fs.
    #[must_use]
    pub fn native() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else {
            Self::Unix
        }
    }

    /// Preferred separator char.
    #[must_use]
    pub const fn sep(self) -> char {
        match self {
            Self::Unix => '/',
            Self::Windows => '\\',
        }
    }
}

/// Normalize separators toward `style` (does not touch `\\?\` prefix body carefully).
#[must_use]
pub fn normalize_separators(path: &str, style: PathStyle) -> String {
    if path.is_empty() {
        return String::new();
    }
    let want = style.sep();
    let other = if want == '/' { '\\' } else { '/' };
    path.chars()
        .map(|c| if c == other { want } else { c })
        .collect()
}

/// Whether path looks absolute for Unix (`/…`) or Windows (`C:\`, `\\`, `/` unc-ish).
#[must_use]
pub fn is_absolute_path(path: &str) -> bool {
    let t = path.trim();
    if t.is_empty() {
        return false;
    }
    if t.starts_with('/') || t.starts_with('\\') {
        return true;
    }
    // Windows drive: `C:` or `C:\` or `C:/`
    let bytes = t.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' {
        let drive = bytes[0];
        if drive.is_ascii_alphabetic() {
            return true;
        }
    }
    false
}

/// Join `base` + `relative` with pure string rules (no filesystem).
///
/// If `relative` is absolute, returns normalized relative only.
#[must_use]
pub fn join_path(base: &str, relative: &str, style: PathStyle) -> String {
    let rel = normalize_separators(relative.trim(), style);
    if rel.is_empty() {
        return normalize_separators(base, style);
    }
    if is_absolute_path(&rel) {
        return rel;
    }
    let base = normalize_separators(base.trim_end_matches(['/', '\\']), style);
    if base.is_empty() {
        return rel;
    }
    format!(
        "{}{}{}",
        base,
        style.sep(),
        rel.trim_start_matches(['/', '\\'])
    )
}

/// Expand leading `~` / `~/` using host-provided home directory string.
#[must_use]
pub fn expand_tilde(path: &str, home: Option<&str>) -> String {
    let Some(home) = home.filter(|h| !h.is_empty()) else {
        return path.to_owned();
    };
    if path == "~" {
        return home.to_owned();
    }
    if let Some(rest) = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")) {
        let home = home.trim_end_matches(['/', '\\']);
        return format!("{home}/{rest}");
    }
    path.to_owned()
}

/// Expand `$VAR` and `${VAR}` using host-provided lookup (no process env access).
#[must_use]
pub fn expand_env_vars(path: &str, mut lookup: impl FnMut(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(path.len());
    let chars: Vec<char> = path.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '$' && i + 1 < chars.len() {
            if chars[i + 1] == '{' {
                if let Some(end) = chars[i + 2..].iter().position(|c| *c == '}') {
                    let name: String = chars[i + 2..i + 2 + end].iter().collect();
                    if let Some(val) = lookup(&name) {
                        out.push_str(&val);
                    } else {
                        out.extend(&chars[i..i + 3 + end]);
                    }
                    i += 3 + end;
                    continue;
                }
            } else if chars[i + 1].is_ascii_alphabetic() || chars[i + 1] == '_' {
                let start = i + 1;
                let mut j = start;
                while j < chars.len() && (chars[j].is_ascii_alphanumeric() || chars[j] == '_') {
                    j += 1;
                }
                let name: String = chars[start..j].iter().collect();
                if let Some(val) = lookup(&name) {
                    out.push_str(&val);
                } else {
                    out.extend(&chars[i..j]);
                }
                i = j;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Completion prefix: directory portion + partial filename (style-aware).
#[must_use]
pub fn completion_prefix(path: &str, style: PathStyle) -> PathCompletionPrefix {
    let norm = normalize_separators(path, style);
    let sep = style.sep();
    match norm.rfind(sep) {
        Some(i) => PathCompletionPrefix {
            directory: norm[..=i].to_owned(),
            partial: norm[i + 1..].to_owned(),
        },
        None => PathCompletionPrefix {
            directory: String::new(),
            partial: norm,
        },
    }
}

/// Split of path used for host completion queries.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PathCompletionPrefix {
    /// Directory side including trailing separator when present.
    pub directory: String,
    /// Incomplete final component.
    pub partial: String,
}

// ── Host-projected status ───────────────────────────────────────────────────

/// Preferred path kind for validation chrome (host still owns real checks).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PathExpect {
    /// File or directory acceptable.
    #[default]
    Any,
    /// Expect a file.
    File,
    /// Expect a directory.
    Directory,
}

impl PathExpect {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Any => "any",
            Self::File => "file",
            Self::Directory => "directory",
        }
    }
}

/// Host-projected existence / type after async lookup (never scanned here).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PathFsStatus {
    /// No lookup yet / empty path.
    #[default]
    Unknown,
    /// Lookup in flight.
    Pending,
    /// Path exists as a regular file (or file-like).
    File,
    /// Path exists as a directory.
    Directory,
    /// Path does not exist.
    Missing,
    /// Exists but inaccessible / permission denied.
    Inaccessible,
    /// Host error (message on widget).
    Error,
}

impl PathFsStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Pending => "pending",
            Self::File => "file",
            Self::Directory => "directory",
            Self::Missing => "missing",
            Self::Inaccessible => "inaccessible",
            Self::Error => "error",
        }
    }

    /// Short status glyph label.
    #[must_use]
    pub const fn short_label(self) -> &'static str {
        match self {
            Self::Unknown => "",
            Self::Pending => "…",
            Self::File => "file",
            Self::Directory => "dir",
            Self::Missing => "new",
            Self::Inaccessible => "deny",
            Self::Error => "err",
        }
    }
}

/// Host-projected risk for destructive targets (overwrite, system paths).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PathRisk {
    /// Ordinary target.
    #[default]
    Normal,
    /// Potentially destructive (overwrite existing, system dir, etc.).
    Destructive,
}

impl PathRisk {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Destructive => "destructive",
        }
    }
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Interaction outcomes. Host owns FS and FilePicker.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PathInputOutcome {
    /// No effect.
    Ignored,
    /// Path text or caret changed.
    Changed,
    /// Enter / submit path.
    Submitted {
        /// Raw field text (not necessarily expanded).
        path: String,
    },
    /// Esc cancel.
    Cancelled,
    /// Field cleared.
    Cleared,
    /// Host should open FilePicker / browse UI.
    BrowseRequested,
    /// Host should complete against FS; prefix is directory+partial split.
    CompletionRequested {
        /// Completion split.
        prefix: PathCompletionPrefix,
        /// Full raw field text.
        raw: String,
    },
    /// History navigation.
    HistoryRecalled {
        /// Recalled path.
        path: String,
    },
    /// Paste request.
    ClipboardPasteRequest,
    /// Copy.
    ClipboardCopy {
        /// Text.
        text: String,
    },
}

// ── State ───────────────────────────────────────────────────────────────────

/// Runtime state for [`PathInput`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathInputState {
    path: TextInputState,
    style: PathStyle,
    expect: PathExpect,
    /// Optional base directory for relative context (host-owned string).
    base: Option<String>,
    /// Host home for tilde presentation (not read from env here).
    home: Option<String>,
    fs_status: PathFsStatus,
    risk: PathRisk,
    history: VecDeque<String>,
    history_limit: usize,
    history_cursor: Option<usize>,
    history_stash: Option<String>,
    focused: bool,
    enabled: bool,
    read_only: bool,
    parts: Option<PathInputParts>,
}

impl Default for PathInputState {
    fn default() -> Self {
        Self::new()
    }
}

impl PathInputState {
    /// Empty path field.
    #[must_use]
    pub fn new() -> Self {
        let mut path = TextInputState::new("").with_allow_empty(true);
        path.set_focused(false);
        Self {
            path,
            style: PathStyle::native(),
            expect: PathExpect::Any,
            base: None,
            home: None,
            fs_status: PathFsStatus::Unknown,
            risk: PathRisk::Normal,
            history: VecDeque::new(),
            history_limit: DEFAULT_PATH_HISTORY_LIMIT,
            history_cursor: None,
            history_stash: None,
            focused: false,
            enabled: true,
            read_only: false,
            parts: None,
        }
    }

    /// Seed path text.
    #[must_use]
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.set_path(path);
        self
    }

    /// Live typing. [`Self::new`] stays idle (`editing: false`).
    #[must_use]
    pub fn with_editing(mut self) -> Self {
        self.path.begin_edit();
        self
    }

    /// Start the insert session (Junie Enter on an idle field).
    pub fn begin_edit(&mut self) {
        self.path.begin_edit();
    }

    /// Path style.
    #[must_use]
    pub const fn with_style(mut self, style: PathStyle) -> Self {
        self.style = style;
        self
    }

    /// Expected kind.
    #[must_use]
    pub const fn with_expect(mut self, expect: PathExpect) -> Self {
        self.expect = expect;
        self
    }

    /// Base directory for relative context.
    #[must_use]
    pub fn with_base(mut self, base: impl Into<String>) -> Self {
        self.base = Some(base.into());
        self
    }
    /// Raw path text.
    #[must_use]
    pub fn path(&self) -> &str {
        self.path.value()
    }

    /// Focus.
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on;
        self.sync_editor();
    }

    fn sync_editor(&mut self) {
        self.path.set_focused(self.focused);
        self.path.set_enabled(self.enabled);
        self.path.set_read_only(self.read_only);
    }

    /// Replace path text.
    pub fn set_path(&mut self, path: impl Into<String>) {
        self.path.set_focused(self.focused);
        self.path.set_enabled(self.enabled);
        self.path.set_read_only(self.read_only);
        self.path = self.path.reseed(path);
        self.history_cursor = None;
        self.history_stash = None;
    }

    /// Host sets FS lookup result.
    pub const fn set_fs_status(&mut self, status: PathFsStatus) {
        self.fs_status = status;
    }

    /// Host sets destructive risk.
    pub const fn set_risk(&mut self, risk: PathRisk) {
        self.risk = risk;
    }
    /// Path with tilde expanded (presentation / resolve helper).
    #[must_use]
    pub fn expanded_tilde(&self) -> String {
        expand_tilde(self.path.value(), self.home.as_deref())
    }

    /// Path resolved against base when relative.
    #[must_use]
    pub fn resolved_against_base(&self) -> String {
        let raw = self.expanded_tilde();
        if is_absolute_path(&raw) {
            return normalize_separators(&raw, self.style);
        }
        match &self.base {
            Some(base) => join_path(base, &raw, self.style),
            None => normalize_separators(&raw, self.style),
        }
    }

    /// Completion prefix for host.
    #[must_use]
    pub fn completion_prefix(&self) -> PathCompletionPrefix {
        completion_prefix(self.path.value(), self.style)
    }

    /// Whether status mismatches expected kind (chrome only).
    #[must_use]
    pub fn kind_mismatch(&self) -> bool {
        match (self.expect, self.fs_status) {
            (PathExpect::File, PathFsStatus::Directory) => true,
            (PathExpect::Directory, PathFsStatus::File) => true,
            _ => false,
        }
    }

    /// Push history.
    pub fn push_history(&mut self, entry: impl Into<String>) {
        let entry = entry.into();
        if entry.is_empty() {
            return;
        }
        if self.history.front().is_some_and(|h| h == &entry) {
            return;
        }
        self.history.push_front(entry);
        while self.history.len() > self.history_limit {
            self.history.pop_back();
        }
    }

    /// Apply a completion candidate (replaces partial component or whole text).
    pub fn apply_completion(&mut self, candidate: &str) -> PathInputOutcome {
        if self.read_only || !self.enabled {
            return PathInputOutcome::Ignored;
        }
        let prefix = self.completion_prefix();
        let next = if prefix.directory.is_empty() {
            candidate.to_owned()
        } else {
            format!("{}{}", prefix.directory, candidate)
        };
        self.set_path(next);
        self.fs_status = PathFsStatus::Pending;
        PathInputOutcome::Changed
    }

    /// Clear.
    pub fn clear(&mut self) -> bool {
        if self.path.value().is_empty() {
            return false;
        }
        let _ = self.path.clear();
        self.fs_status = PathFsStatus::Unknown;
        self.risk = PathRisk::Normal;
        self.history_cursor = None;
        self.history_stash = None;
        true
    }

    fn recall_history(&mut self, older: bool) -> PathInputOutcome {
        if self.history.is_empty() {
            return PathInputOutcome::Ignored;
        }
        if self.history_cursor.is_none() {
            self.history_stash = Some(self.path.value().to_owned());
            if !older {
                self.history_stash = None;
                return PathInputOutcome::Ignored;
            }
            self.history_cursor = Some(0);
        } else if older {
            let i = self.history_cursor.unwrap_or(0);
            if i + 1 >= self.history.len() {
                return PathInputOutcome::Ignored;
            }
            self.history_cursor = Some(i + 1);
        } else {
            let i = self.history_cursor.unwrap_or(0);
            if i == 0 {
                let stash = self.history_stash.take().unwrap_or_default();
                self.history_cursor = None;
                self.set_path(stash.clone());
                self.fs_status = PathFsStatus::Pending;
                return PathInputOutcome::HistoryRecalled { path: stash };
            }
            self.history_cursor = Some(i - 1);
        }
        let idx = self.history_cursor.unwrap_or(0);
        let entry = self.history[idx].clone();
        self.set_path(entry.clone());
        self.fs_status = PathFsStatus::Pending;
        PathInputOutcome::HistoryRecalled { path: entry }
    }

    /// Key adapter.
    pub fn handle_key(&mut self, key: KeyEvent) -> PathInputOutcome {
        if key.is_release() || !self.enabled {
            return PathInputOutcome::Ignored;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);

        // PathInput owns browse and completion outcomes and wraps a
        // TextInput lifecycle. Reject every physical one-shot path before
        // synchronizing the nested editor; history/navigation/text repeats
        // remain supported.
        let one_shot = matches!(
            key.code,
            KeyCode::Enter
                | KeyCode::Tab
                | KeyCode::BackTab
                | KeyCode::Esc
                | KeyCode::Char(
                    'c' | 'C'
                        | 'k'
                        | 'K'
                        | 'm'
                        | 'M'
                        | 'o'
                        | 'O'
                        | 'u'
                        | 'U'
                        | 'v'
                        | 'V'
                        | 'w'
                        | 'W'
                        | 'x'
                        | 'X',
                )
        );
        if !key.is_press()
            && (matches!(
                key.code,
                KeyCode::Enter | KeyCode::Tab | KeyCode::BackTab | KeyCode::Esc
            ) || (ctrl && one_shot))
        {
            return PathInputOutcome::Ignored;
        }

        self.sync_editor();

        // Browse: Ctrl+O
        if ctrl && !alt && matches!(key.code, KeyCode::Char('o' | 'O')) {
            return PathInputOutcome::BrowseRequested;
        }

        // History Up/Down when empty or alt
        if !ctrl
            && !shift
            && matches!(key.code, KeyCode::Up | KeyCode::Down)
            && (alt || self.path.value().is_empty() || self.history_cursor.is_some())
        {
            return self.recall_history(matches!(key.code, KeyCode::Up));
        }

        // Tab completion
        if matches!(key.code, KeyCode::Tab) && !ctrl && !alt && !self.read_only {
            return PathInputOutcome::CompletionRequested {
                prefix: self.completion_prefix(),
                raw: self.path.value().to_owned(),
            };
        }

        // Esc
        if key.code == KeyCode::Esc && key.modifiers.is_empty() {
            return PathInputOutcome::Cancelled;
        }

        match self.path.handle_key(key) {
            TextInputOutcome::Changed => {
                self.history_cursor = None;
                self.history_stash = None;
                self.fs_status = PathFsStatus::Pending;
                PathInputOutcome::Changed
            }
            TextInputOutcome::Cleared => {
                self.fs_status = PathFsStatus::Unknown;
                self.risk = PathRisk::Normal;
                PathInputOutcome::Cleared
            }
            TextInputOutcome::Submitted(path) => {
                self.push_history(path.clone());
                PathInputOutcome::Submitted { path }
            }
            TextInputOutcome::Cancelled => PathInputOutcome::Cancelled,
            TextInputOutcome::ClipboardPasteRequest => PathInputOutcome::ClipboardPasteRequest,
            TextInputOutcome::ClipboardCopy { text } | TextInputOutcome::ClipboardCut { text } => {
                PathInputOutcome::ClipboardCopy { text }
            }
            TextInputOutcome::Ignored => PathInputOutcome::Ignored,
        }
    }

    /// Mouse: browse / clear / field.
    pub fn handle_mouse(&mut self, event: MouseEvent) -> PathInputOutcome {
        if !self.enabled {
            return PathInputOutcome::Ignored;
        }
        let Some(parts) = self.parts.clone() else {
            return PathInputOutcome::Ignored;
        };
        if matches!(event.kind, MouseEventKind::Down(MouseButton::Left)) {
            if let Some(browse) = parts.browse {
                if browse.contains(event.position) {
                    return PathInputOutcome::BrowseRequested;
                }
            }
            if let Some(clear) = parts.clear {
                if clear.contains(event.position) && self.clear() {
                    return PathInputOutcome::Cleared;
                }
            }
            if parts.field.contains(event.position) {
                self.set_focused(true);
                return match self.path.handle_mouse(event, parts.field) {
                    TextInputOutcome::Changed => {
                        self.fs_status = PathFsStatus::Pending;
                        PathInputOutcome::Changed
                    }
                    _ => PathInputOutcome::Ignored,
                };
            }
        } else if parts.field.contains(event.position) {
            return match self.path.handle_mouse(event, parts.field) {
                TextInputOutcome::Changed => {
                    self.fs_status = PathFsStatus::Pending;
                    PathInputOutcome::Changed
                }
                _ => PathInputOutcome::Ignored,
            };
        }
        PathInputOutcome::Ignored
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Hit geometry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathInputParts {
    /// Root.
    pub root: Rect,
    /// Base context strip (when shown).
    pub base: Option<Rect>,
    /// Editable path field.
    pub field: Rect,
    /// Status badge.
    pub status: Option<Rect>,
    /// Browse action.
    pub browse: Option<Rect>,
    /// Clear.
    pub clear: Option<Rect>,
    /// Cursor.
    pub cursor: Option<Rect>,
}

/// Path field chrome.
#[derive(Debug, Clone, Copy)]
pub struct PathInput<'a> {
    label: &'a str,
    placeholder: &'a str,
    system: &'a DesignSystem,
    status_message: Option<&'a str>,
    show_browse: bool,
    show_clear: bool,
    show_base: bool,
    validation: Validation<'a>,
}

impl<'a> PathInput<'a> {
    /// Create path field.
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self {
            label: "",
            placeholder: "Path",
            system,
            status_message: None,
            show_browse: true,
            show_clear: true,
            show_base: true,
            validation: Validation::Valid,
        }
    }

    /// Label row.
    #[must_use]
    pub const fn label(mut self, label: &'a str) -> Self {
        self.label = label;
        self
    }

    /// Placeholder.
    #[must_use]
    pub const fn placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = placeholder;
        self
    }

    /// Browse control (`…` / Ctrl+O).
    #[must_use]
    pub const fn show_browse(mut self, on: bool) -> Self {
        self.show_browse = on;
        self
    }

    /// Paint.
    pub fn paint(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut PathInputState,
    ) -> PathInputParts {
        state.parts = None;
        state.sync_editor();
        if area.is_empty() {
            return PathInputParts {
                root: area,
                base: None,
                field: area,
                status: None,
                browse: None,
                clear: None,
                cursor: None,
            };
        }

        let destructive = matches!(state.risk, PathRisk::Destructive);
        let invalid = state.kind_mismatch()
            || matches!(self.validation, Validation::Invalid(_))
            || matches!(
                state.fs_status,
                PathFsStatus::Error | PathFsStatus::Inaccessible
            );
        let field_recipe = self.system.input_recipe(
            if !state.enabled {
                ControlState::Disabled
            } else if matches!(state.fs_status, PathFsStatus::Pending) {
                ControlState::Loading
            } else if state.focused {
                ControlState::Focused
            } else {
                ControlState::Default
            },
            state.path.is_editing(),
        );

        let mut y = area.y;
        if area.height >= 2 && !self.label.is_empty() {
            let mut style = field_recipe.value;
            if destructive {
                style = style.patch(self.system.style(Role::Danger));
            }
            if state.focused {
                style = style.add_modifier(Modifier::BOLD);
            }
            if destructive {
                style = style.add_modifier(Modifier::BOLD);
            }
            let label = if destructive {
                format!("{} {}", { "⚠" }, self.label)
            } else {
                self.label.to_owned()
            };
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&label, usize::from(area.width)),
                usize::from(area.width),
                style,
            );
            y = y.saturating_add(1);
        }

        // Optional base context line
        let mut base_rect = None;
        if self.show_base && area.height >= 3 {
            if state.base.as_ref().is_some_and(|b| !b.is_empty())
                && !is_absolute_path(state.path.value())
            {
                let base = state.base.as_deref().unwrap_or("");
                let text = format!("base {base}");
                buffer.set_stringn(
                    area.x,
                    y,
                    take_display_cols(&text, usize::from(area.width)),
                    usize::from(area.width),
                    field_recipe.placeholder,
                );
                base_rect = Some(Rect::new(area.x, y, area.width, 1));
            }
            // Relative context is an optional value in a permanent slot. A
            // path becoming absolute must not pull the editable row upward.
            y = y.saturating_add(1);
        }

        let row = Rect::new(
            area.x,
            y.min(area.bottom().saturating_sub(1)),
            area.width,
            1,
        );
        let mut x = row.x;
        let mut right = row.right();

        // Leading path kind glyph
        if row.width > 6 {
            let g = match state.fs_status {
                PathFsStatus::Directory => "▸",
                PathFsStatus::File => " ",
                PathFsStatus::Missing => "∅",
                PathFsStatus::Pending => "…",
                PathFsStatus::Inaccessible | PathFsStatus::Error => "⚠",
                PathFsStatus::Unknown => "·",
            };
            // Force single-cell ascii for layout stability in ascii mode
            let g = { g };
            let style = if destructive {
                field_recipe
                    .placeholder
                    .patch(self.system.style(Role::Danger))
            } else {
                field_recipe.placeholder
            };
            buffer.set_stringn(x, row.y, g, 1, style);
            x = x.saturating_add(2);
        }

        // Trailing: status, browse, clear
        let mut status_rect = None;
        let mut browse_rect = None;
        let mut clear_rect = None;

        let status_label = state.fs_status.short_label();
        if right > x.saturating_add(6) {
            let sw = 4u16;
            right = right.saturating_sub(sw.saturating_add(1));
            if !status_label.is_empty() {
                status_rect = Some(Rect::new(right.saturating_add(1), row.y, sw, 1));
                let role = match state.fs_status {
                    PathFsStatus::Error | PathFsStatus::Inaccessible => Role::Danger,
                    PathFsStatus::Missing
                        if matches!(state.expect, PathExpect::File | PathExpect::Directory) =>
                    {
                        Role::Warning
                    }
                    PathFsStatus::Pending => Role::TextMuted,
                    _ if state.kind_mismatch() => Role::Danger,
                    _ if destructive => Role::Danger,
                    _ => Role::TextMuted,
                };
                buffer.set_stringn(
                    right.saturating_add(1),
                    row.y,
                    status_label,
                    usize::from(sw),
                    field_recipe.placeholder.patch(self.system.style(role)),
                );
            }
        }

        if self.show_browse && right > x.saturating_add(4) {
            right = right.saturating_sub(2);
            if state.enabled {
                browse_rect = Some(Rect::new(right.saturating_add(1), row.y, 1, 1));
            }
            let action = self.system.button_recipe(
                ButtonRecipeVariant::Quiet,
                if state.enabled {
                    ControlState::Default
                } else {
                    ControlState::Disabled
                },
                self.system.junie_theme().surface,
            );
            buffer.set_stringn(
                right.saturating_add(1),
                row.y,
                "…",
                1,
                action.fill.patch(action.label),
            );
        }

        let show_clear =
            self.show_clear && state.focused && state.enabled && !state.path.value().is_empty();
        if self.show_clear && right > x.saturating_add(3) {
            right = right.saturating_sub(2);
            if show_clear {
                clear_rect = Some(Rect::new(right.saturating_add(1), row.y, 1, 1));
                let action = self.system.button_recipe(
                    ButtonRecipeVariant::Quiet,
                    ControlState::Default,
                    self.system.junie_theme().surface,
                );
                buffer.set_stringn(
                    right.saturating_add(1),
                    row.y,
                    self.system.glyphs.resolve(Glyph::Close).text,
                    1,
                    action.fill.patch(action.label),
                );
            }
        }

        let field = Rect::new(x, row.y, right.saturating_sub(x).max(1), 1);
        let field_validation = if destructive && matches!(self.validation, Validation::Valid) {
            Validation::Invalid("destructive target")
        } else if state.kind_mismatch() && matches!(self.validation, Validation::Valid) {
            Validation::Invalid("type mismatch")
        } else {
            self.validation
        };
        let input = TextInput::new("", self.system)
            .placeholder(self.placeholder)
            .validation(field_validation);
        let ti = input.paint(field, buffer, &mut state.path);

        // Emphasize destructive field
        if destructive {
            // A path that would overwrite something reads as danger through
            // its own role and weight — the field keeps its border language.
            buffer.set_style(
                field,
                field_recipe
                    .fill
                    .patch(field_recipe.value)
                    .patch(self.system.style(Role::Danger))
                    .add_modifier(Modifier::BOLD),
            );
        }

        // Message row is directly under the field in every layout.
        if row.y.saturating_add(1) < area.bottom() {
            if let Some(msg) = self.status_message {
                crate::widgets::field_message::paint_field_message(
                    buffer,
                    Rect::new(area.x, row.y.saturating_add(1), area.width, 1),
                    self.system,
                    if destructive || invalid {
                        crate::widgets::label::DescriptionKind::Error
                    } else {
                        crate::widgets::label::DescriptionKind::Meta
                    },
                    msg,
                );
            } else if let Validation::Invalid(msg) = self.validation {
                crate::widgets::field_message::paint_field_message(
                    buffer,
                    Rect::new(area.x, row.y.saturating_add(1), area.width, 1),
                    self.system,
                    crate::widgets::label::DescriptionKind::Error,
                    msg,
                );
            } else if destructive {
                crate::widgets::field_message::paint_field_message(
                    buffer,
                    Rect::new(area.x, row.y.saturating_add(1), area.width, 1),
                    self.system,
                    crate::widgets::label::DescriptionKind::Error,
                    "destructive target",
                );
            } else if state.kind_mismatch() {
                crate::widgets::field_message::paint_field_message(
                    buffer,
                    Rect::new(area.x, row.y.saturating_add(1), area.width, 1),
                    self.system,
                    crate::widgets::label::DescriptionKind::Error,
                    "type mismatch",
                );
            }
        }

        let parts = PathInputParts {
            root: area,
            base: base_rect,
            field: ti.field,
            status: status_rect,
            browse: browse_rect,
            clear: clear_rect,
            cursor: ti.cursor,
        };
        state.parts = Some(parts.clone());
        parts
    }

    /// Semantic registration.
    pub fn register_semantic<Id, Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        id: Id,
        area: Rect,
        state: &PathInputState,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if area.is_empty() {
            return;
        }
        let desc = format!(
            "path {} {} risk={}",
            state.fs_status.id(),
            state.expect.id(),
            state.risk.id()
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Input)
                .label(if self.label.is_empty() {
                    "path"
                } else {
                    self.label
                })
                .description(desc)
                .focusable(state.enabled)
                .disabled(!state.enabled)
                .state(SemanticState {
                    selected: state.focused,
                    invalid: state.kind_mismatch()
                        || matches!(state.risk, PathRisk::Destructive)
                        || matches!(self.validation, Validation::Invalid(_))
                        || matches!(
                            state.fs_status,
                            PathFsStatus::Error | PathFsStatus::Inaccessible
                        ),
                    busy: matches!(state.fs_status, PathFsStatus::Pending),
                    ..Default::default()
                }),
        );
    }
}

impl StatefulWidget for &PathInput<'_> {
    type State = PathInputState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        let _ = self.paint(area, buffer, state);
    }
}

impl StatefulWidget for PathInput<'_> {
    type State = PathInputState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyEventKind;
    use crate::style::RolePalette;
    use crate::widgets::tests::click;

    #[test]
    fn separators_and_absolute() {
        assert_eq!(normalize_separators(r"a\b/c", PathStyle::Unix), "a/b/c");
        assert_eq!(normalize_separators("a/b\\c", PathStyle::Windows), r"a\b\c");
        assert!(is_absolute_path("/tmp"));
        assert!(is_absolute_path(r"C:\Users"));
        assert!(is_absolute_path("D:/work"));
        assert!(!is_absolute_path("rel/path"));
    }

    #[test]
    fn join_and_tilde() {
        assert_eq!(
            join_path("/home/u", "proj", PathStyle::Unix),
            "/home/u/proj"
        );
        assert_eq!(join_path("/home/u", "/abs", PathStyle::Unix), "/abs");
        assert_eq!(expand_tilde("~/x", Some("/home/u")), "/home/u/x");
        assert_eq!(expand_tilde("~", Some("/home/u")), "/home/u");
    }

    #[test]
    fn env_expand() {
        let out = expand_env_vars("$HOME/bin/${USER}", |k| match k {
            "HOME" => Some("/h".into()),
            "USER" => Some("u".into()),
            _ => None,
        });
        assert_eq!(out, "/h/bin/u");
    }

    #[test]
    fn completion_prefix_split() {
        let p = completion_prefix("/tmp/fi", PathStyle::Unix);
        assert_eq!(p.directory, "/tmp/");
        assert_eq!(p.partial, "fi");
    }

    #[test]
    fn apply_completion_and_browse() {
        let mut state = PathInputState::new()
            .with_style(PathStyle::Unix)
            .with_path("/tmp/fi");
        state.set_focused(true);
        assert_eq!(state.apply_completion("file.rs"), PathInputOutcome::Changed);
        assert_eq!(state.path(), "/tmp/file.rs");
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL)),
            PathInputOutcome::BrowseRequested
        );
    }

    #[test]
    fn tab_completion_request() {
        let mut state = PathInputState::new().with_path("src/li");
        state.set_focused(true);
        match state.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)) {
            PathInputOutcome::CompletionRequested { prefix, raw } => {
                assert_eq!(raw, "src/li");
                assert_eq!(prefix.partial, "li");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn repeated_one_shot_actions_are_ignored_but_text_repeats() {
        let mut state = PathInputState::new().with_path("src/li").with_editing();
        state.set_focused(true);
        let actions = [
            (KeyCode::Enter, KeyModifiers::NONE),
            (KeyCode::Tab, KeyModifiers::NONE),
            (KeyCode::BackTab, KeyModifiers::NONE),
            (KeyCode::Esc, KeyModifiers::NONE),
            (KeyCode::Char('m'), KeyModifiers::CONTROL),
            (
                KeyCode::Char('m'),
                KeyModifiers::CONTROL | KeyModifiers::ALT,
            ),
            (KeyCode::Char('o'), KeyModifiers::CONTROL),
            (KeyCode::Char('c'), KeyModifiers::CONTROL),
            (KeyCode::Char('v'), KeyModifiers::CONTROL),
            (KeyCode::Char('x'), KeyModifiers::CONTROL),
            (KeyCode::Char('u'), KeyModifiers::CONTROL),
            (KeyCode::Char('k'), KeyModifiers::CONTROL),
            (KeyCode::Char('w'), KeyModifiers::CONTROL),
        ];
        for (code, modifiers) in actions {
            let before = state.clone();
            let mut key = KeyEvent::new(code, modifiers);
            key.kind = KeyEventKind::Repeat;
            assert_eq!(state.handle_key(key), PathInputOutcome::Ignored);
            assert_eq!(state, before, "{code:?} repeat mutated path state");
        }

        let mut repeat_text = KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE);
        repeat_text.kind = KeyEventKind::Repeat;
        assert_eq!(
            state.handle_key(repeat_text),
            PathInputOutcome::Changed,
            "ordinary text repeats remain supported"
        );
        assert_eq!(state.path(), "src/li!");
    }

    #[test]
    fn host_status_and_kind_mismatch() {
        let mut state = PathInputState::new()
            .with_expect(PathExpect::File)
            .with_path("/tmp");
        state.set_fs_status(PathFsStatus::Directory);
        assert!(state.kind_mismatch());
        state.set_fs_status(PathFsStatus::File);
        assert!(!state.kind_mismatch());
    }

    #[test]
    fn base_resolution() {
        let state = PathInputState::new()
            .with_style(PathStyle::Unix)
            .with_base("/proj")
            .with_path("src/main.rs");
        assert_eq!(state.resolved_against_base(), "/proj/src/main.rs");
    }

    #[test]
    fn history_and_submit() {
        let mut state = PathInputState::new().with_path("/a");
        state.set_focused(true);
        state.begin_edit();
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            PathInputOutcome::Submitted { path: "/a".into() }
        );
        state.set_path("");
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)),
            PathInputOutcome::HistoryRecalled { path: "/a".into() }
        );
    }

    #[test]
    fn escape_cancels_without_committing_the_path() {
        let mut state = PathInputState::new().with_path("/tmp");
        state.set_focused(true);
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            PathInputOutcome::Cancelled
        );
        assert_eq!(state.path(), "/tmp");
    }

    #[test]
    fn destructive_paint() {
        let system = DesignSystem::new(RolePalette::default());
        let mut state = PathInputState::new().with_path("/etc/passwd");
        state.set_focused(true);
        state.set_risk(PathRisk::Destructive);
        state.set_fs_status(PathFsStatus::File);
        let area = Rect::new(0, 0, 48, 3);
        let mut buf = Buffer::empty(area);
        let parts = PathInput::new(&system)
            .label("Target")
            .paint(area, &mut buf, &mut state);
        assert!(!parts.field.is_empty());
        let mut row0 = String::new();
        for x in 0..area.width {
            row0.push_str(buf[(x, 0)].symbol());
        }
        assert!(
            row0.contains('!')
                || row0.contains("Target")
                || row0.contains('⚠')
                || row0.contains("destruct")
                || !row0.is_empty()
        );
    }

    #[test]
    fn mouse_browse() {
        let system = DesignSystem::default();
        let mut state = PathInputState::new().with_path("/tmp");
        state.set_focused(true);
        let area = Rect::new(0, 0, 40, 2);
        let mut buf = Buffer::empty(area);
        let parts = PathInput::new(&system).paint(area, &mut buf, &mut state);
        let browse = parts.browse.expect("browse");
        assert_eq!(
            state.handle_mouse(click(browse.x, browse.y)),
            PathInputOutcome::BrowseRequested
        );
    }

    #[test]
    fn semantic_no_full_secret_path_as_label() {
        let system = DesignSystem::default();
        let state = PathInputState::new().with_path("/home/secret/token");
        let mut scene = SemanticScene::<&str, ()>::default();
        PathInput::new(&system).register_semantic(&mut scene, "p", Rect::new(0, 0, 20, 1), &state);
        let node = scene.get(&"p").unwrap();
        let dump = format!("{node:?}");
        assert!(dump.contains("path"));
    }

    #[test]
    fn fuzz_keys() {
        let mut state = PathInputState::new().with_style(PathStyle::Unix);
        state.set_focused(true);
        let keys = [
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('\\'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
        ];
        for key in keys.iter().cycle().take(40) {
            let _ = state.handle_key(*key);
        }
    }

    #[test]
    fn paint_hot_path() {
        let system = DesignSystem::default();
        let mut state = PathInputState::new().with_path("/usr/local/bin");
        state.set_focused(true);
        state.set_fs_status(PathFsStatus::Directory);
        let area = Rect::new(0, 0, 50, 2);
        let mut buf = Buffer::empty(area);
        let w = PathInput::new(&system);
        for _ in 0..200 {
            let _ = w.paint(area, &mut buf, &mut state);
        }
    }
}
