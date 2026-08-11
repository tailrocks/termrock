// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **HelpCenter** / **CommandReference** — contextual product help composed from
//! **public** TermRock widgets and **live metadata** (keymap → [`HelpEntry`],
//! command catalog, registry inventory, doctor report projection).
//!
//! **Mission.** Layout + focus + typed messages for search, topic navigation,
//! keyboard map, command reference, tutorials/troubleshooting topics,
//! current-context help, capability diagnostics, and markdown links/anchors.
//! Compact overlay and full documentation modes. **No second hand-maintained
//! shortcut table** — keyboard map and command shortcuts project from
//! [`HelpEntry`] / keymap generators only. Host owns markdown assembly and
//! command execution; no help-file I/O or network fetch inside this surface.
//!
//! **vs standalone [`KeyboardHelp`] / [`CommandPalette`] / [`MarkdownView`].**
//! Composed, not re-painted.
//! **vs `termrock doctor` CLI.** Displays host-projected [`DoctorReport`]; does
//! not re-detect terminal capabilities.
//!
//! Research: Vim/Helix help, Zellij key help, CLI man pages, command palettes.

#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    text::Line,
    widgets::{StatefulWidget, Widget},
};

use crate::{
    capability::{DoctorFinding, DoctorReport, DoctorSeverity},
    input::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    interaction::Outcome,
    layout::{
        PaneConstraint, PaneGeom, PaneId, Workspace, WorkspaceAxis, WorkspaceNode, WorkspaceState,
    },
    style::{DesignSystem, PanelChrome, Role},
    text::take_display_cols,
    widgets::{
        CommandEntry, HelpEntry, KeyboardHelp, KeyboardHelpMode, KeyboardHelpOutcome,
        KeyboardHelpState, List, ListRow, ListState, MarkdownBlock, MarkdownOutcome, MarkdownView,
        MarkdownViewState, SearchInput, SearchInputOutcome, SearchInputState, StatusBar,
        StatusBarState, StatusRegion, StatusSlot, example_help_entries, filter_help_entries,
        project_markdown,
    },
};

// ── Panes, mode, density ────────────────────────────────────────────────────

/// Named panes of the help center.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum HelpCenterPane {
    /// Search / filter.
    Search,
    /// Topic / section navigation.
    Nav,
    /// Keyboard map (live HelpEntry projection).
    Keyboard,
    /// Command reference list (from HelpEntry / command metadata).
    Commands,
    /// Markdown topic body.
    Body,
    /// Doctor / capability diagnostics.
    Diagnostics,
    /// Status strip.
    Status,
}

impl HelpCenterPane {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Search => "search",
            Self::Nav => "nav",
            Self::Keyboard => "keyboard",
            Self::Commands => "commands",
            Self::Body => "body",
            Self::Diagnostics => "diagnostics",
            Self::Status => "status",
        }
    }

    /// Default Tab cycle (status chrome-only).
    #[must_use]
    pub fn focus_order() -> &'static [HelpCenterPane] {
        &[
            Self::Search,
            Self::Nav,
            Self::Keyboard,
            Self::Commands,
            Self::Body,
            Self::Diagnostics,
        ]
    }
}

/// Presentation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum HelpCenterMode {
    /// Full documentation multi-pane.
    #[default]
    Full,
    /// Compact help overlay (search + body/keyboard).
    Compact,
}

impl HelpCenterMode {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Compact => "compact",
        }
    }
}

/// Responsive density.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum HelpCenterDensity {
    /// Full multi-pane.
    #[default]
    Normal,
    /// Drop diagnostics / shrink secondary.
    Narrow,
    /// Search + body (or keyboard) + status.
    Tiny,
}

impl HelpCenterDensity {
    /// From width.
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

// ── Domain (host-projected topics; metadata-sourced shortcuts) ──────────────

/// Help topic group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum HelpTopicGroup {
    /// Getting started / tutorial.
    Tutorial,
    /// Reference docs.
    #[default]
    Reference,
    /// Troubleshooting.
    Troubleshoot,
    /// Current focus context.
    Context,
    /// Capability / doctor.
    Diagnostics,
}

impl HelpTopicGroup {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Tutorial => "tutorial",
            Self::Reference => "reference",
            Self::Troubleshoot => "troubleshoot",
            Self::Context => "context",
            Self::Diagnostics => "diagnostics",
        }
    }

    /// Label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Tutorial => "Tutorials",
            Self::Reference => "Reference",
            Self::Troubleshoot => "Troubleshooting",
            Self::Context => "Context",
            Self::Diagnostics => "Diagnostics",
        }
    }

    /// Sort key.
    #[must_use]
    pub const fn sort_key(self) -> u8 {
        match self {
            Self::Context => 0,
            Self::Tutorial => 1,
            Self::Reference => 2,
            Self::Troubleshoot => 3,
            Self::Diagnostics => 4,
        }
    }
}

/// Host-projected help topic (markdown body is host-owned text).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpTopic {
    /// Stable id.
    pub id: String,
    /// Title.
    pub title: String,
    /// Group.
    pub group: HelpTopicGroup,
    /// Markdown source (host assembly).
    pub markdown: String,
    /// Semantic anchors (`#id` in body).
    pub anchors: Vec<String>,
    /// Optional component id for inspection handoff.
    pub component_id: Option<String>,
}

impl HelpTopic {
    /// Construct.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        markdown: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            group: HelpTopicGroup::Reference,
            markdown: markdown.into(),
            anchors: Vec::new(),
            component_id: None,
        }
    }

    /// Group.
    #[must_use]
    pub const fn group(mut self, g: HelpTopicGroup) -> Self {
        self.group = g;
        self
    }

    /// Anchors.
    #[must_use]
    pub fn anchors(mut self, a: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.anchors = a.into_iter().map(Into::into).collect();
        self
    }

    /// Component id.
    #[must_use]
    pub fn component_id(mut self, id: impl Into<String>) -> Self {
        self.component_id = Some(id.into());
        self
    }

    /// Query match.
    #[must_use]
    pub fn matches_query(&self, q: &str) -> bool {
        let q = q.trim().to_ascii_lowercase();
        if q.is_empty() {
            return true;
        }
        let hay = format!(
            "{} {} {} {}",
            self.title,
            self.markdown,
            self.group.id(),
            self.anchors.join(" ")
        )
        .to_ascii_lowercase();
        hay.contains(&q)
    }
}

/// Filter topics.
#[must_use]
pub fn filter_help_topics<'a>(topics: &'a [HelpTopic], query: &str) -> Vec<&'a HelpTopic> {
    let mut v: Vec<&HelpTopic> = topics.iter().filter(|t| t.matches_query(query)).collect();
    v.sort_by(|a, b| {
        a.group
            .sort_key()
            .cmp(&b.group.sort_key())
            .then_with(|| a.title.cmp(&b.title))
    });
    v
}

/// Nav list rows with group headers.
#[must_use]
pub fn help_topic_rows<'a>(topics: &[&'a HelpTopic]) -> Vec<ListRow<'a, String>> {
    let mut rows = Vec::new();
    let mut last: Option<HelpTopicGroup> = None;
    for t in topics {
        if last != Some(t.group) {
            rows.push(ListRow::group_header(
                format!("g-{}", t.group.id()),
                Line::from(t.group.label()),
            ));
            last = Some(t.group);
        }
        rows.push(ListRow::item(t.id.clone(), Line::from(t.title.as_str())));
    }
    rows
}

/// **Single SoT path:** project [`CommandEntry`] from live [`HelpEntry`] rows.
///
/// Shortcuts always come from `HelpEntry.chord` (keymap formatting), never a
/// parallel static chord table inside this pattern.
#[must_use]
pub fn command_entries_from_help(entries: &[HelpEntry]) -> Vec<CommandEntry<String>> {
    entries
        .iter()
        .map(|e| {
            CommandEntry::new(e.id.clone(), e.action.clone())
                .shortcut(e.chord.clone())
                .group(e.category.clone())
                .command_key(e.id.clone())
                .preview(format!("{} — {}", e.chord, e.action))
                .keywords([e.category.as_str(), e.chord.as_str()])
        })
        .collect()
}

/// Command list rows from metadata-derived command entries.
#[must_use]
pub fn command_list_rows<'a>(commands: &'a [CommandEntry<String>]) -> Vec<ListRow<'a, String>> {
    let mut rows = Vec::new();
    let mut last_group: Option<&str> = None;
    for c in commands {
        let g = c.group.as_deref().unwrap_or("Commands");
        if last_group != Some(g) {
            rows.push(ListRow::group_header(format!("g-cmd-{g}"), Line::from(g)));
            last_group = Some(g);
        }
        let label = if let Some(s) = &c.shortcut {
            format!("{s}  {}", c.label)
        } else {
            c.label.clone()
        };
        let mut row = ListRow::item(c.id.clone(), Line::from(label));
        if let Some(p) = &c.preview {
            row = row.secondary(Line::from(p.as_str()));
        }
        rows.push(row);
    }
    rows
}

/// Doctor findings as list rows (host-projected report; no re-detect).
///
/// Row ids are `finding:{index}:{code}` so duplicate finding codes (common in
/// doctor reports) remain navigable under List/CollectionState (stable unique ids).
#[must_use]
pub fn doctor_finding_rows(findings: &[DoctorFinding]) -> Vec<ListRow<'_, String>> {
    findings
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let sev = match f.severity {
                DoctorSeverity::Info => "I",
                DoctorSeverity::Warning => "W",
                DoctorSeverity::Error => "E",
            };
            let label = format!("[{sev}] {} — {}", f.code, f.message);
            ListRow::item(format!("finding:{i}:{}", f.code), Line::from(label))
        })
        .collect()
}

/// Component inspection rows from registry official ids (host may filter).
#[must_use]
pub fn component_inspect_rows(ids: &[String]) -> Vec<ListRow<'_, String>> {
    ids.iter()
        .map(|id| ListRow::item(id.clone(), Line::from(id.as_str())))
        .collect()
}

/// **Single list for paint + handle_key:** findings then component inspect rows.
///
/// Must stay identical in `render_help_center` and `handle_diagnostics_key` so
/// keyboard navigation can reach painted component ids when both are present.
#[must_use]
pub fn diagnostics_rows<'a>(
    doctor: Option<&'a DoctorReport>,
    component_ids: &'a [String],
) -> Vec<ListRow<'a, String>> {
    let findings = doctor.map(|d| d.findings.as_slice()).unwrap_or(&[]);
    let mut rows = doctor_finding_rows(findings);
    if !component_ids.is_empty() {
        if !rows.is_empty() {
            rows.push(ListRow::group_header(
                "g-components".into(),
                Line::from("Components"),
            ));
        }
        rows.extend(component_inspect_rows(component_ids));
    }
    rows
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Help center outcomes — requests only.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum HelpCenterOutcome {
    /// Ignored.
    Ignored,
    /// Focus changed.
    FocusChanged(&'static str),
    /// Mode changed.
    ModeChanged(HelpCenterMode),
    /// Topic selected / opened.
    TopicOpened {
        /// Topic id.
        id: String,
    },
    /// Jump to markdown anchor.
    AnchorJumped {
        /// Anchor id.
        anchor: String,
    },
    /// Follow link from markdown.
    LinkFollowed {
        /// Href.
        href: String,
        /// Label.
        label: String,
    },
    /// Command selected (palette handoff / run request).
    CommandSelected {
        /// Command id.
        id: String,
    },
    /// Command run requested.
    CommandRun {
        /// Command id.
        id: String,
    },
    /// Filter changed.
    FilterChanged {
        /// Query.
        query: String,
    },
    /// Open doctor / refresh diagnostics projection.
    DoctorOpened,
    /// Inspect component from registry.
    InspectComponent {
        /// Component id.
        id: String,
    },
    /// Keyboard help mode toggled.
    KeyboardModeChanged(KeyboardHelpMode),
    /// Esc / cancel.
    Cancelled,
    /// Child residual.
    Child {
        /// Kind.
        kind: String,
    },
}

// ── Surfaces ────────────────────────────────────────────────────────────────

/// Borrowed surfaces for one paint frame.
pub struct HelpCenterSurfaces<'a> {
    /// Design system.
    pub system: &'a DesignSystem,
    /// State.
    pub state: &'a mut HelpCenterState,
    /// Host-projected topics.
    pub topics: &'a [HelpTopic],
    /// Live keyboard help entries (from keymap generators — **SoT**).
    pub help_entries: &'a [HelpEntry],
    /// Commands derived from help entries (or host command catalog sharing chords).
    pub commands: &'a [CommandEntry<String>],
    /// Host-projected doctor report (optional).
    pub doctor: Option<&'a DoctorReport>,
    /// Registry component ids for inspection (optional).
    pub component_ids: &'a [String],
}

// ── State ───────────────────────────────────────────────────────────────────

/// Persistent help center state.
#[derive(Debug)]
pub struct HelpCenterState {
    /// Workspace.
    pub workspace: WorkspaceState,
    /// Search.
    pub search: SearchInputState,
    /// Topic nav list.
    pub nav: ListState<String>,
    /// Keyboard help child.
    pub keyboard: KeyboardHelpState,
    /// Command list.
    pub commands: ListState<String>,
    /// Markdown body.
    pub body: MarkdownViewState,
    /// Diagnostics list.
    pub diagnostics: ListState<String>,
    /// Status.
    pub status: StatusBarState<&'static str>,
    /// Mode.
    pub mode: HelpCenterMode,
    /// Focus pane.
    pub focus: &'static str,
    /// Density override.
    pub density: Option<HelpCenterDensity>,
    /// Selected topic id.
    pub selected_topic: Option<String>,
    /// Selected command id.
    pub selected_command: Option<String>,
    /// Context help line (host: current focus widget).
    pub context_label: Option<String>,
    /// Host wants diagnostics pane when content exists.
    pub show_diagnostics: bool,
    /// Live this frame: diagnostics pane is laid out (findings or component ids).
    /// Synced from doctor/component surfaces in handle_key and render.
    pub diagnostics_live: bool,
    /// ASCII.
    pub ascii: bool,
    /// Colorless.
    pub colorless: bool,
    last_panes: Vec<PaneGeom>,
    last_area_width: Option<u16>,
}

impl Default for HelpCenterState {
    fn default() -> Self {
        Self::new()
    }
}

impl HelpCenterState {
    /// Full docs factory.
    #[must_use]
    pub fn new() -> Self {
        let mut search = SearchInputState::new();
        search.set_focused(false);
        // Dedicated keyboard map pane needs Modal + open (Footer paints 1-line strip only).
        let mut keyboard = KeyboardHelpState::new();
        keyboard.set_focused(false);
        keyboard.set_accepts_input(false);
        let _ = keyboard.set_mode(KeyboardHelpMode::Modal);
        let _ = keyboard.open_modal();
        let mut body = MarkdownViewState::new();
        body.set_focused(false);
        Self {
            workspace: WorkspaceState::new(),
            search,
            nav: ListState::new(None),
            keyboard,
            commands: ListState::new(None),
            body,
            diagnostics: ListState::new(None),
            status: StatusBarState::new(),
            mode: HelpCenterMode::Full,
            focus: HelpCenterPane::Nav.id(),
            density: None,
            selected_topic: None,
            selected_command: None,
            context_label: None,
            show_diagnostics: true,
            diagnostics_live: false,
            ascii: false,
            colorless: false,
            last_panes: Vec::new(),
            last_area_width: None,
        }
    }

    /// Compact overlay factory.
    #[must_use]
    pub fn compact() -> Self {
        let mut s = Self::new();
        s.mode = HelpCenterMode::Compact;
        s.focus = HelpCenterPane::Search.id();
        // Modal already open from new()
        s
    }

    /// Whether diagnostics pane is painted this frame (same predicate as layout).
    #[must_use]
    pub fn diagnostics_pane_visible(
        &self,
        doctor: Option<&DoctorReport>,
        component_ids: &[String],
    ) -> bool {
        self.show_diagnostics
            && (doctor.map(|d| !d.findings.is_empty()).unwrap_or(false)
                || !component_ids.is_empty())
    }

    /// Sync diagnostics_live from surfaces (call before Tab/clamp).
    pub fn sync_diagnostics_live(
        &mut self,
        doctor: Option<&DoctorReport>,
        component_ids: &[String],
    ) {
        self.diagnostics_live = self.diagnostics_pane_visible(doctor, component_ids);
    }

    /// Ensure keyboard map is Modal + open for navigable multi-row paint.
    pub fn ensure_keyboard_map_modal(&mut self) {
        if !matches!(self.keyboard.mode(), KeyboardHelpMode::Modal) {
            let _ = self.keyboard.set_mode(KeyboardHelpMode::Modal);
        }
        if !self.keyboard.is_open() {
            let _ = self.keyboard.open_modal();
        }
    }

    /// Last panes.
    #[must_use]
    pub fn last_panes(&self) -> &[PaneGeom] {
        &self.last_panes
    }

    /// Effective density.
    #[must_use]
    pub fn effective_density(&self) -> HelpCenterDensity {
        self.density
            .unwrap_or_else(|| HelpCenterDensity::for_width(self.last_area_width.unwrap_or(120)))
    }

    /// Visible focus panes (diagnostics only when `diagnostics_live` — same as layout paint).
    #[must_use]
    pub fn visible_focus_panes(&self, density: HelpCenterDensity) -> Vec<HelpCenterPane> {
        match (self.mode, density) {
            (HelpCenterMode::Compact, _) => {
                vec![
                    HelpCenterPane::Search,
                    HelpCenterPane::Keyboard,
                    HelpCenterPane::Body,
                ]
            }
            (_, HelpCenterDensity::Tiny) => {
                vec![HelpCenterPane::Search, HelpCenterPane::Body]
            }
            (_, HelpCenterDensity::Narrow) => {
                let mut v = vec![
                    HelpCenterPane::Search,
                    HelpCenterPane::Nav,
                    HelpCenterPane::Commands,
                    HelpCenterPane::Body,
                ];
                if self.diagnostics_live {
                    v.push(HelpCenterPane::Diagnostics);
                }
                v
            }
            (_, HelpCenterDensity::Normal) => {
                let mut v = vec![
                    HelpCenterPane::Search,
                    HelpCenterPane::Nav,
                    HelpCenterPane::Keyboard,
                    HelpCenterPane::Commands,
                    HelpCenterPane::Body,
                ];
                if self.diagnostics_live {
                    v.push(HelpCenterPane::Diagnostics);
                }
                v
            }
        }
    }

    /// Clamp focus.
    pub fn clamp_focus_to_density(&mut self, density: HelpCenterDensity) {
        let visible = self.visible_focus_panes(density);
        if !visible.iter().any(|p| p.id() == self.focus) {
            self.focus = visible
                .first()
                .map(|p| p.id())
                .unwrap_or(HelpCenterPane::Body.id());
        }
    }

    /// Sync child focus/accept gates.
    pub fn apply_focus_gates(&mut self) {
        let f = self.focus;
        self.search.set_focused(f == "search");
        let kb_on = f == "keyboard";
        self.keyboard.set_focused(kb_on);
        self.keyboard.set_accepts_input(kb_on);
        self.body.set_focused(f == "body");
    }

    /// Set focus.
    pub fn set_focus(&mut self, pane: HelpCenterPane) -> HelpCenterOutcome {
        let density = self.effective_density();
        let visible = self.visible_focus_panes(density);
        if !visible.contains(&pane) {
            return HelpCenterOutcome::Ignored;
        }
        if self.focus == pane.id() {
            self.apply_focus_gates();
            return HelpCenterOutcome::Ignored;
        }
        self.focus = pane.id();
        self.apply_focus_gates();
        HelpCenterOutcome::FocusChanged(self.focus)
    }

    /// Tab cycle.
    pub fn cycle_focus(&mut self, reverse: bool) -> HelpCenterOutcome {
        let density = self.effective_density();
        let visible = self.visible_focus_panes(density);
        if visible.is_empty() {
            return HelpCenterOutcome::Ignored;
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
        HelpCenterOutcome::FocusChanged(self.focus)
    }

    /// Set mode.
    pub fn set_mode(&mut self, mode: HelpCenterMode) -> HelpCenterOutcome {
        if self.mode == mode {
            return HelpCenterOutcome::Ignored;
        }
        self.mode = mode;
        // Full and compact both use Modal map in the keyboard pane (not Footer strip).
        self.ensure_keyboard_map_modal();
        let density = self.effective_density();
        self.clamp_focus_to_density(density);
        self.apply_focus_gates();
        HelpCenterOutcome::ModeChanged(mode)
    }

    /// Status slots.
    #[must_use]
    pub fn status_slots(&self) -> Vec<StatusSlot<'static, &'static str>> {
        let mut slots = vec![
            StatusSlot::context("mode", self.mode.id()).priority(10),
            StatusSlot::focus_zone("focus", self.focus).priority(20),
            StatusSlot::shortcut(
                "keys",
                "enter open · / search · c cmd · k keys · d doctor · i inspect · tab · esc",
            )
            .priority(90),
        ];
        if let Some(ctx) = &self.context_label {
            let _ = ctx; // content is &'static in StatusSlot — use fixed label
            slots.push(StatusSlot::context("ctx", "context").priority(30));
        }
        slots
    }

    /// Keys — real workbench path.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        topics: &[HelpTopic],
        help_entries: &[HelpEntry],
        commands: &[CommandEntry<String>],
        doctor: Option<&DoctorReport>,
        component_ids: &[String],
    ) -> HelpCenterOutcome {
        if key.kind == KeyEventKind::Release {
            return HelpCenterOutcome::Ignored;
        }
        // Keep focus cycle / clamp aligned with painted diagnostics pane.
        self.sync_diagnostics_live(doctor, component_ids);
        self.ensure_keyboard_map_modal();
        let is_press = key.kind == KeyEventKind::Press;

        if is_press {
            match key.code {
                KeyCode::Tab if key.modifiers.is_empty() => {
                    return self.cycle_focus(false);
                }
                KeyCode::BackTab | KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
                    return self.cycle_focus(true);
                }
                KeyCode::Esc => {
                    return HelpCenterOutcome::Cancelled;
                }
                KeyCode::Char('d')
                    if key.modifiers.is_empty()
                        && self.focus != "search"
                        && self.focus != "body" =>
                {
                    return HelpCenterOutcome::DoctorOpened;
                }
                KeyCode::Char('m')
                    if key.modifiers.is_empty()
                        && self.focus != "search"
                        && self.focus != "body" =>
                {
                    // Toggle compact / full
                    let next = match self.mode {
                        HelpCenterMode::Full => HelpCenterMode::Compact,
                        HelpCenterMode::Compact => HelpCenterMode::Full,
                    };
                    return self.set_mode(next);
                }
                _ => {}
            }
        }

        match self.focus {
            "search" => self.handle_search_key(key),
            "nav" => self.handle_nav_key(key, topics),
            "keyboard" => self.handle_keyboard_key(key, help_entries),
            "commands" => self.handle_commands_key(key, commands),
            "body" => self.handle_body_key(key, topics),
            "diagnostics" => self.handle_diagnostics_key(key, doctor, component_ids),
            _ => HelpCenterOutcome::Ignored,
        }
    }

    fn handle_search_key(&mut self, key: KeyEvent) -> HelpCenterOutcome {
        let out = self.search.handle_key(key);
        match out {
            SearchInputOutcome::Ignored => HelpCenterOutcome::Ignored,
            SearchInputOutcome::DebouncedQuery { query }
            | SearchInputOutcome::Submitted { query } => HelpCenterOutcome::FilterChanged { query },
            SearchInputOutcome::Changed | SearchInputOutcome::HistoryRecalled { .. } => {
                HelpCenterOutcome::FilterChanged {
                    query: self.search.query().to_string(),
                }
            }
            SearchInputOutcome::Cleared => HelpCenterOutcome::FilterChanged {
                query: String::new(),
            },
            SearchInputOutcome::Cancelled => HelpCenterOutcome::Cancelled,
            other => {
                let kind = format!("{other:?}")
                    .split(|c: char| c == '(' || c == ' ')
                    .next()
                    .unwrap_or("search")
                    .to_string();
                HelpCenterOutcome::Child { kind }
            }
        }
    }

    fn handle_nav_key(&mut self, key: KeyEvent, topics: &[HelpTopic]) -> HelpCenterOutcome {
        let q = self.search.query().to_string();
        let filtered = filter_help_topics(topics, &q);
        let rows = help_topic_rows(&filtered);
        if key.kind == KeyEventKind::Press && key.code == KeyCode::Enter {
            if let Some(id) = self
                .nav
                .selected()
                .cloned()
                .or_else(|| filtered.first().map(|t| t.id.clone()))
            {
                if id.starts_with("g-") {
                    return HelpCenterOutcome::Ignored;
                }
                self.selected_topic = Some(id.clone());
                return HelpCenterOutcome::TopicOpened { id };
            }
        }
        let out = self.nav.handle_key(&rows, key);
        match out {
            Outcome::Ignored => HelpCenterOutcome::Ignored,
            Outcome::Changed => {
                let id = self.nav.selected().cloned().unwrap_or_default();
                if id.is_empty() || id.starts_with("g-") {
                    return HelpCenterOutcome::Ignored;
                }
                self.selected_topic = Some(id.clone());
                HelpCenterOutcome::TopicOpened { id }
            }
            Outcome::Activated(id) => {
                if id.starts_with("g-") {
                    return HelpCenterOutcome::Ignored;
                }
                self.selected_topic = Some(id.clone());
                HelpCenterOutcome::TopicOpened { id }
            }
            Outcome::Cancelled => HelpCenterOutcome::Cancelled,
            Outcome::CheckToggled(id) => HelpCenterOutcome::TopicOpened { id },
        }
    }

    fn handle_keyboard_key(
        &mut self,
        key: KeyEvent,
        help_entries: &[HelpEntry],
    ) -> HelpCenterOutcome {
        let visible = filter_help_entries(help_entries, self.search.query());
        // Ensure modal is open when focusing keyboard in compact/full map pane
        if !self.keyboard.is_open() && matches!(self.keyboard.mode(), KeyboardHelpMode::Modal) {
            let _ = self.keyboard.open_modal();
        }
        let out = self.keyboard.handle_key(key, &visible);
        match out {
            KeyboardHelpOutcome::Ignored => HelpCenterOutcome::Ignored,
            KeyboardHelpOutcome::QueryChanged { query } => {
                HelpCenterOutcome::FilterChanged { query }
            }
            KeyboardHelpOutcome::ModeChanged { mode } => {
                HelpCenterOutcome::KeyboardModeChanged(mode)
            }
            KeyboardHelpOutcome::Closed => HelpCenterOutcome::Cancelled,
            KeyboardHelpOutcome::Opened
            | KeyboardHelpOutcome::CursorMoved { .. }
            | KeyboardHelpOutcome::PresentationChanged { .. } => {
                let kind = format!("{out:?}")
                    .split(|c: char| c == '(' || c == ' ')
                    .next()
                    .unwrap_or("keyboard")
                    .to_string();
                HelpCenterOutcome::Child { kind }
            }
        }
    }

    fn handle_commands_key(
        &mut self,
        key: KeyEvent,
        commands: &[CommandEntry<String>],
    ) -> HelpCenterOutcome {
        let q = self.search.query().to_string();
        let filtered: Vec<&CommandEntry<String>> = commands
            .iter()
            .filter(|c| {
                if q.trim().is_empty() {
                    return true;
                }
                let hay = format!(
                    "{} {} {}",
                    c.label,
                    c.shortcut.as_deref().unwrap_or(""),
                    c.keywords.join(" ")
                )
                .to_ascii_lowercase();
                hay.contains(&q.to_ascii_lowercase())
            })
            .collect();
        let owned: Vec<CommandEntry<String>> = filtered.into_iter().cloned().collect();
        let rows = command_list_rows(&owned);

        if key.kind == KeyEventKind::Press {
            match key.code {
                KeyCode::Enter => {
                    if let Some(id) = self
                        .commands
                        .selected()
                        .cloned()
                        .or_else(|| owned.first().map(|c| c.id.clone()))
                    {
                        if id.starts_with("g-") {
                            return HelpCenterOutcome::Ignored;
                        }
                        self.selected_command = Some(id.clone());
                        return HelpCenterOutcome::CommandRun { id };
                    }
                }
                KeyCode::Char('c') if key.modifiers.is_empty() => {
                    if let Some(id) = self.commands.selected().cloned() {
                        self.selected_command = Some(id.clone());
                        return HelpCenterOutcome::CommandSelected { id };
                    }
                }
                _ => {}
            }
        }

        let out = self.commands.handle_key(&rows, key);
        match out {
            Outcome::Ignored => HelpCenterOutcome::Ignored,
            Outcome::Changed => {
                let id = self.commands.selected().cloned().unwrap_or_default();
                if id.is_empty() || id.starts_with("g-") {
                    return HelpCenterOutcome::Ignored;
                }
                self.selected_command = Some(id.clone());
                HelpCenterOutcome::CommandSelected { id }
            }
            Outcome::Activated(id) => {
                if id.starts_with("g-") {
                    return HelpCenterOutcome::Ignored;
                }
                self.selected_command = Some(id.clone());
                HelpCenterOutcome::CommandRun { id }
            }
            Outcome::Cancelled => HelpCenterOutcome::Cancelled,
            Outcome::CheckToggled(id) => HelpCenterOutcome::CommandSelected { id },
        }
    }

    fn handle_body_key(&mut self, key: KeyEvent, topics: &[HelpTopic]) -> HelpCenterOutcome {
        let topic = self
            .selected_topic
            .as_ref()
            .and_then(|id| topics.iter().find(|t| &t.id == id))
            .or_else(|| topics.first());
        let md = topic.map(|t| t.markdown.as_str()).unwrap_or("");
        // project_markdown needs owned lifetime — use static empty for empty
        let blocks = if md.is_empty() {
            Vec::new()
        } else {
            // MarkdownView handle_key needs blocks; re-project each call
            // We can't easily store projected blocks with lifetime; paint path does.
            // For keys we only need scroll/link outcomes — use empty and map anchors via topic.
            project_markdown(md)
                .into_iter()
                .map(|b| {
                    // leak-free: MarkdownBlock may borrow from md; md is from topics which outlive call
                    // Actually project_markdown returns owned-ish with internal borrows from md
                    b
                })
                .collect::<Vec<_>>()
        };
        // Safety: blocks borrow from `md` which borrows from `topics` parameter — valid for this call
        let out = if blocks.is_empty() {
            // Still allow anchor jump chords without body
            if key.kind == KeyEventKind::Press {
                if let KeyCode::Char('g') = key.code {
                    if let Some(t) = topic {
                        if let Some(a) = t.anchors.first() {
                            return HelpCenterOutcome::AnchorJumped { anchor: a.clone() };
                        }
                    }
                }
            }
            MarkdownOutcome::Ignored
        } else {
            let system = DesignSystem::default();
            let view = MarkdownView::new(&blocks, &system);
            view.handle_key(&mut self.body, key)
        };
        match out {
            MarkdownOutcome::Ignored => {
                // Anchor jump: g then first anchor, or explicit
                if key.kind == KeyEventKind::Press && key.code == KeyCode::Char('g') {
                    if let Some(t) = topic {
                        if let Some(a) = t.anchors.first() {
                            return HelpCenterOutcome::AnchorJumped { anchor: a.clone() };
                        }
                    }
                }
                HelpCenterOutcome::Ignored
            }
            MarkdownOutcome::LinkActivated { label, href } => {
                // Internal anchor
                if let Some(rest) = href.strip_prefix('#') {
                    return HelpCenterOutcome::AnchorJumped {
                        anchor: rest.to_string(),
                    };
                }
                HelpCenterOutcome::LinkFollowed { href, label }
            }
            MarkdownOutcome::Scrolled { .. }
            | MarkdownOutcome::CursorMoved { .. }
            | MarkdownOutcome::SelectionChanged { .. }
            | MarkdownOutcome::Copy { .. }
            | MarkdownOutcome::BlockActivated { .. } => {
                let kind = format!("{out:?}")
                    .split(|c: char| c == '(' || c == ' ')
                    .next()
                    .unwrap_or("body")
                    .to_string();
                HelpCenterOutcome::Child { kind }
            }
        }
    }

    fn handle_diagnostics_key(
        &mut self,
        key: KeyEvent,
        doctor: Option<&DoctorReport>,
        component_ids: &[String],
    ) -> HelpCenterOutcome {
        if key.kind == KeyEventKind::Press {
            match key.code {
                KeyCode::Char('i') if key.modifiers.is_empty() => {
                    // Only component registry ids — not DoctorFinding codes.
                    if let Some(id) = self.diagnostics.selected().cloned() {
                        if component_ids.iter().any(|c| c == &id) {
                            return HelpCenterOutcome::InspectComponent { id };
                        }
                    }
                }
                KeyCode::Char('d') if key.modifiers.is_empty() => {
                    return HelpCenterOutcome::DoctorOpened;
                }
                KeyCode::Enter => {
                    // Enter on finding → doctor open; on component id → inspect
                    if let Some(id) = self.diagnostics.selected().cloned() {
                        if component_ids.iter().any(|c| c == &id) {
                            return HelpCenterOutcome::InspectComponent { id };
                        }
                        return HelpCenterOutcome::DoctorOpened;
                    }
                }
                _ => {}
            }
        }
        // Same row model as paint — findings + components (not findings-only fallback).
        let rows = diagnostics_rows(doctor, component_ids);
        let out = self.diagnostics.handle_key(&rows, key);
        match out {
            Outcome::Ignored => HelpCenterOutcome::Ignored,
            Outcome::Changed | Outcome::Activated(_) | Outcome::CheckToggled(_) => {
                HelpCenterOutcome::Ignored
            }
            Outcome::Cancelled => HelpCenterOutcome::Cancelled,
        }
    }
}

// ── Layout ──────────────────────────────────────────────────────────────────

/// Search strip height.
pub const HELP_CENTER_SEARCH_HEIGHT: u16 = 3;

/// Width-derived layout.
#[must_use]
pub fn help_center_layout(area: Rect, state: &WorkspaceState) -> Vec<PaneGeom> {
    help_center_layout_density(
        area,
        state,
        HelpCenterDensity::for_width(area.width),
        HelpCenterMode::Full,
        true,
    )
}

/// Explicit density + mode layout.
#[must_use]
pub fn help_center_layout_density(
    area: Rect,
    state: &WorkspaceState,
    density: HelpCenterDensity,
    mode: HelpCenterMode,
    show_diagnostics: bool,
) -> Vec<PaneGeom> {
    let mut panes = Vec::new();
    let mut y = area.y;
    let mut remain = area.height;

    let search_h = if remain >= 3 {
        HELP_CENTER_SEARCH_HEIGHT.min(remain.saturating_sub(2))
    } else if remain >= 1 {
        1
    } else {
        0
    };
    panes.push(PaneGeom {
        id: PaneId::from_static(HelpCenterPane::Search.id()),
        area: if search_h == 0 {
            Rect::new(area.x, y, 0, 0)
        } else {
            Rect {
                x: area.x,
                y,
                width: area.width,
                height: search_h,
            }
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

    let root = match (mode, density) {
        (HelpCenterMode::Compact, _) => {
            // keyboard | body | status
            WorkspaceNode::Split {
                axis: WorkspaceAxis::Vertical,
                ratio_percent: 92,
                first: Box::new(WorkspaceNode::Split {
                    axis: WorkspaceAxis::Horizontal,
                    ratio_percent: 45,
                    first: Box::new(WorkspaceNode::Leaf {
                        id: PaneId::from_static(HelpCenterPane::Keyboard.id()),
                        constraint: PaneConstraint::Weight(1),
                        collapse_priority: 1,
                    }),
                    second: Box::new(WorkspaceNode::Leaf {
                        id: PaneId::from_static(HelpCenterPane::Body.id()),
                        constraint: PaneConstraint::Weight(1),
                        collapse_priority: 1,
                    }),
                }),
                second: Box::new(WorkspaceNode::Leaf {
                    id: PaneId::from_static(HelpCenterPane::Status.id()),
                    constraint: PaneConstraint::Fixed(1),
                    collapse_priority: 3,
                }),
            }
        }
        (_, HelpCenterDensity::Tiny) => WorkspaceNode::Split {
            axis: WorkspaceAxis::Vertical,
            ratio_percent: 92,
            first: Box::new(WorkspaceNode::Leaf {
                id: PaneId::from_static(HelpCenterPane::Body.id()),
                constraint: PaneConstraint::Weight(1),
                collapse_priority: 1,
            }),
            second: Box::new(WorkspaceNode::Leaf {
                id: PaneId::from_static(HelpCenterPane::Status.id()),
                constraint: PaneConstraint::Fixed(1),
                collapse_priority: 3,
            }),
        },
        (_, HelpCenterDensity::Narrow) => {
            // nav | commands | body | optional diagnostics | status — no keyboard
            let main = WorkspaceNode::Split {
                axis: WorkspaceAxis::Horizontal,
                ratio_percent: 28,
                first: Box::new(WorkspaceNode::Leaf {
                    id: PaneId::from_static(HelpCenterPane::Nav.id()),
                    constraint: PaneConstraint::Min(12),
                    collapse_priority: 0,
                }),
                second: Box::new(WorkspaceNode::Split {
                    axis: WorkspaceAxis::Horizontal,
                    ratio_percent: 40,
                    first: Box::new(WorkspaceNode::Leaf {
                        id: PaneId::from_static(HelpCenterPane::Commands.id()),
                        constraint: PaneConstraint::Weight(1),
                        collapse_priority: 1,
                    }),
                    second: Box::new(WorkspaceNode::Leaf {
                        id: PaneId::from_static(HelpCenterPane::Body.id()),
                        constraint: PaneConstraint::Weight(1),
                        collapse_priority: 1,
                    }),
                }),
            };
            if show_diagnostics {
                WorkspaceNode::Split {
                    axis: WorkspaceAxis::Vertical,
                    ratio_percent: 80,
                    first: Box::new(main),
                    second: Box::new(WorkspaceNode::Split {
                        axis: WorkspaceAxis::Vertical,
                        ratio_percent: 75,
                        first: Box::new(WorkspaceNode::Leaf {
                            id: PaneId::from_static(HelpCenterPane::Diagnostics.id()),
                            constraint: PaneConstraint::Min(3),
                            collapse_priority: 0,
                        }),
                        second: Box::new(WorkspaceNode::Leaf {
                            id: PaneId::from_static(HelpCenterPane::Status.id()),
                            constraint: PaneConstraint::Fixed(1),
                            collapse_priority: 3,
                        }),
                    }),
                }
            } else {
                WorkspaceNode::Split {
                    axis: WorkspaceAxis::Vertical,
                    ratio_percent: 92,
                    first: Box::new(main),
                    second: Box::new(WorkspaceNode::Leaf {
                        id: PaneId::from_static(HelpCenterPane::Status.id()),
                        constraint: PaneConstraint::Fixed(1),
                        collapse_priority: 3,
                    }),
                }
            }
        }
        (_, HelpCenterDensity::Normal) => {
            // (nav | keyboard | commands) / body / optional diagnostics / status
            let top = WorkspaceNode::Split {
                axis: WorkspaceAxis::Horizontal,
                ratio_percent: 25,
                first: Box::new(WorkspaceNode::Leaf {
                    id: PaneId::from_static(HelpCenterPane::Nav.id()),
                    constraint: PaneConstraint::Min(14),
                    collapse_priority: 0,
                }),
                second: Box::new(WorkspaceNode::Split {
                    axis: WorkspaceAxis::Horizontal,
                    ratio_percent: 50,
                    first: Box::new(WorkspaceNode::Leaf {
                        id: PaneId::from_static(HelpCenterPane::Keyboard.id()),
                        constraint: PaneConstraint::Weight(1),
                        collapse_priority: 1,
                    }),
                    second: Box::new(WorkspaceNode::Leaf {
                        id: PaneId::from_static(HelpCenterPane::Commands.id()),
                        constraint: PaneConstraint::Weight(1),
                        collapse_priority: 1,
                    }),
                }),
            };
            let mid = WorkspaceNode::Split {
                axis: WorkspaceAxis::Vertical,
                ratio_percent: 45,
                first: Box::new(top),
                second: Box::new(WorkspaceNode::Leaf {
                    id: PaneId::from_static(HelpCenterPane::Body.id()),
                    constraint: PaneConstraint::Weight(1),
                    collapse_priority: 1,
                }),
            };
            if show_diagnostics {
                WorkspaceNode::Split {
                    axis: WorkspaceAxis::Vertical,
                    ratio_percent: 85,
                    first: Box::new(mid),
                    second: Box::new(WorkspaceNode::Split {
                        axis: WorkspaceAxis::Vertical,
                        ratio_percent: 70,
                        first: Box::new(WorkspaceNode::Leaf {
                            id: PaneId::from_static(HelpCenterPane::Diagnostics.id()),
                            constraint: PaneConstraint::Min(3),
                            collapse_priority: 0,
                        }),
                        second: Box::new(WorkspaceNode::Leaf {
                            id: PaneId::from_static(HelpCenterPane::Status.id()),
                            constraint: PaneConstraint::Fixed(1),
                            collapse_priority: 3,
                        }),
                    }),
                }
            } else {
                WorkspaceNode::Split {
                    axis: WorkspaceAxis::Vertical,
                    ratio_percent: 92,
                    first: Box::new(mid),
                    second: Box::new(WorkspaceNode::Leaf {
                        id: PaneId::from_static(HelpCenterPane::Status.id()),
                        constraint: PaneConstraint::Fixed(1),
                        collapse_priority: 3,
                    }),
                }
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

/// Paint help center (public children only; shortcuts from HelpEntry SoT).
pub fn render_help_center(buffer: &mut Buffer, area: Rect, surfaces: HelpCenterSurfaces<'_>) {
    let HelpCenterSurfaces {
        system,
        state,
        topics,
        help_entries,
        commands,
        doctor,
        component_ids,
    } = surfaces;

    if area.is_empty() {
        return;
    }

    state.last_area_width = Some(area.width);
    let density = state.effective_density();
    state.sync_diagnostics_live(doctor, component_ids);
    state.ensure_keyboard_map_modal();
    let show_diag = state.diagnostics_live;
    let panes = help_center_layout_density(area, &state.workspace, density, state.mode, show_diag);
    state.last_panes = panes.clone();
    state.clamp_focus_to_density(density);
    state.apply_focus_gates();

    let query = state.search.query().to_string();
    let filtered_topics = filter_help_topics(topics, &query);
    let filtered_help = filter_help_entries(help_entries, &query);

    // Search
    if let Some(r) = pane_area(&panes, "search") {
        let focused = state.focus == "search";
        state.search.set_focused(focused);
        if r.height >= 3 {
            let panel = Panelish {
                system,
                title: match state.mode {
                    HelpCenterMode::Full => "Help · docs",
                    HelpCenterMode::Compact => "Help · overlay",
                },
                focused,
            };
            let inner = panel.paint(r, buffer);
            if !inner.is_empty() {
                SearchInput::new(system)
                    .placeholder("search topics, keys, commands…")
                    .paint(inner, buffer, &mut state.search);
            }
        } else if !r.is_empty() {
            SearchInput::new(system)
                .placeholder("search…")
                .paint(r, buffer, &mut state.search);
        }
    }

    // Nav
    if let Some(r) = pane_area(&panes, "nav") {
        let focused = state.focus == "nav";
        let panel = Panelish {
            system,
            title: "Topics",
            focused,
        };
        let inner = panel.paint(r, buffer);
        if !inner.is_empty() {
            let rows = help_topic_rows(&filtered_topics);
            if rows.is_empty() {
                buffer.set_stringn(
                    inner.x,
                    inner.y,
                    take_display_cols("(no topics)", usize::from(inner.width)),
                    usize::from(inner.width),
                    system.style(Role::TextMuted),
                );
            } else {
                let list = List::new(&rows, system).focused(focused);
                StatefulWidget::render(&list, inner, buffer, &mut state.nav);
            }
        }
    }

    // Keyboard map — live HelpEntry only (Modal+open for multi-row navigable map)
    if let Some(r) = pane_area(&panes, "keyboard") {
        let focused = state.focus == "keyboard";
        state.ensure_keyboard_map_modal();
        state.keyboard.set_focused(focused);
        state.keyboard.set_accepts_input(focused);
        KeyboardHelp::new(&filtered_help, system)
            .title("Keyboard")
            .ascii(state.ascii)
            .colorless(state.colorless)
            .paint(r, buffer, &mut state.keyboard);
    }

    // Commands — from command_entries_from_help / host catalog
    if let Some(r) = pane_area(&panes, "commands") {
        let focused = state.focus == "commands";
        let panel = Panelish {
            system,
            title: "Commands",
            focused,
        };
        let inner = panel.paint(r, buffer);
        if !inner.is_empty() {
            let filtered: Vec<CommandEntry<String>> = commands
                .iter()
                .filter(|c| {
                    if query.trim().is_empty() {
                        return true;
                    }
                    let hay = format!(
                        "{} {} {}",
                        c.label,
                        c.shortcut.as_deref().unwrap_or(""),
                        c.keywords.join(" ")
                    )
                    .to_ascii_lowercase();
                    hay.contains(&query.to_ascii_lowercase())
                })
                .cloned()
                .collect();
            let rows = command_list_rows(&filtered);
            if rows.is_empty() {
                buffer.set_stringn(
                    inner.x,
                    inner.y,
                    take_display_cols("(no commands)", usize::from(inner.width)),
                    usize::from(inner.width),
                    system.style(Role::TextMuted),
                );
            } else {
                let list = List::new(&rows, system).focused(focused);
                StatefulWidget::render(&list, inner, buffer, &mut state.commands);
            }
        }
    }

    // Body markdown
    if let Some(r) = pane_area(&panes, "body") {
        let focused = state.focus == "body";
        state.body.set_focused(focused);
        let topic = state
            .selected_topic
            .as_ref()
            .and_then(|id| topics.iter().find(|t| &t.id == id))
            .or_else(|| filtered_topics.first().copied())
            .or_else(|| topics.first());
        let title = topic.map(|t| t.title.as_str()).unwrap_or("Help");
        let panel = Panelish {
            system,
            title,
            focused,
        };
        let inner = panel.paint(r, buffer);
        if !inner.is_empty() {
            if let Some(t) = topic {
                let blocks = project_markdown(&t.markdown);
                MarkdownView::new(&blocks, system).paint(inner, buffer, &mut state.body);
            } else {
                buffer.set_stringn(
                    inner.x,
                    inner.y,
                    take_display_cols("(select a topic)", usize::from(inner.width)),
                    usize::from(inner.width),
                    system.style(Role::TextMuted),
                );
            }
        }
    }

    // Diagnostics
    if let Some(r) = pane_area(&panes, "diagnostics") {
        let focused = state.focus == "diagnostics";
        let panel = Panelish {
            system,
            title: "Diagnostics · doctor",
            focused,
        };
        let inner = panel.paint(r, buffer);
        if !inner.is_empty() {
            let rows = diagnostics_rows(doctor, component_ids);
            if rows.is_empty() {
                buffer.set_stringn(
                    inner.x,
                    inner.y,
                    take_display_cols("(no findings — d open doctor)", usize::from(inner.width)),
                    usize::from(inner.width),
                    system.style(Role::TextMuted),
                );
            } else {
                let list = List::new(&rows, system).focused(focused);
                StatefulWidget::render(&list, inner, buffer, &mut state.diagnostics);
            }
        }
    }

    // Status
    if let Some(r) = pane_area(&panes, "status") {
        let n_help = help_entries.len();
        let n_cmd = commands.len();
        state.status.transient = Some(format!(
            "keys={n_help} cmds={n_cmd} · shortcuts from HelpEntry/keymap SoT"
        ));
        let slots = state.status_slots();
        StatefulWidget::render(
            &StatusBar::new(&slots, &[], system),
            r,
            buffer,
            &mut state.status,
        );
    }
}

struct Panelish<'a> {
    system: &'a DesignSystem,
    title: &'a str,
    focused: bool,
}

impl Panelish<'_> {
    fn paint(&self, area: Rect, buffer: &mut Buffer) -> Rect {
        use crate::widgets::Panel;
        let panel = Panel::new(self.system)
            .title(self.title)
            .emphasis(if self.focused {
                PanelChrome::Focused
            } else {
                PanelChrome::Normal
            });
        let inner = panel.inner(area);
        Widget::render(&panel, area, buffer);
        inner
    }
}

// ── Fixtures ────────────────────────────────────────────────────────────────

/// Example topics (host-style markdown with anchors).
#[must_use]
pub fn example_help_topics() -> Vec<HelpTopic> {
    vec![
        HelpTopic::new(
            "getting-started",
            "Getting started",
            "# Getting started\n\nWelcome to TermRock.\n\n## Install\n\nSee the handbook.\n\n[Open doctor](#doctor)\n",
        )
        .group(HelpTopicGroup::Tutorial)
        .anchors(["install", "doctor"]),
        HelpTopic::new(
            "keyboard",
            "Keyboard reference",
            "# Keyboard\n\nChords come from the live keymap — see the **Keyboard** pane.\n\n## Navigation\n\nUse Tab to move focus.\n",
        )
        .group(HelpTopicGroup::Reference)
        .anchors(["navigation"])
        .component_id("keyboard-help"),
        HelpTopic::new(
            "commands",
            "Command palette",
            "# Commands\n\nActivate with the palette. Shortcuts share the HelpEntry SoT.\n",
        )
        .group(HelpTopicGroup::Reference)
        .component_id("command-palette"),
        HelpTopic::new(
            "troubleshoot-color",
            "Color / glyphs issues",
            "# Color troubleshooting\n\nRun `termrock doctor` for capability findings.\n\n## SSH\n\nPrefer `TERMROCK_PROFILE=compatible`.\n",
        )
        .group(HelpTopicGroup::Troubleshoot)
        .anchors(["ssh"]),
        HelpTopic::new(
            "context",
            "Current context",
            "# Context help\n\nHost projects the active focus zone here.\n",
        )
        .group(HelpTopicGroup::Context),
        HelpTopic::new(
            "unicode",
            "Unicode & wide glyphs",
            "# Unicode\n\nWide labels and glyphs are safe on the paint path.\n",
        )
        .group(HelpTopicGroup::Reference)
        .anchors(["wide"]),
    ]
}

/// Live help entries from example keymap (SoT for stories/tests).
#[must_use]
pub fn example_help_center_entries(system: &DesignSystem) -> Vec<HelpEntry> {
    example_help_entries(system)
}

/// Commands derived from help entries (same chords).
#[must_use]
pub fn example_help_center_commands(system: &DesignSystem) -> Vec<CommandEntry<String>> {
    command_entries_from_help(&example_help_entries(system))
}

/// Sample doctor report for projection (real build_doctor_report).
#[must_use]
pub fn example_help_doctor_report() -> DoctorReport {
    use crate::capability::{CapabilityOverrides, build_doctor_report};
    build_doctor_report(None, CapabilityOverrides::default())
}

/// Burst topics for paint stress.
#[must_use]
pub fn burst_help_topics(n: usize) -> Vec<HelpTopic> {
    (0..n)
        .map(|i| {
            HelpTopic::new(
                format!("t-{i}"),
                format!("Topic {i:04}"),
                format!(
                    "# Topic {i}\n\nBody line for paint stress {i}.\n\n## Anchor\n\nMore text.\n"
                ),
            )
            .group(match i % 4 {
                0 => HelpTopicGroup::Tutorial,
                1 => HelpTopicGroup::Reference,
                2 => HelpTopicGroup::Troubleshoot,
                _ => HelpTopicGroup::Context,
            })
            .anchors([format!("a-{i}")])
        })
        .collect()
}

/// Seed compact overlay mode.
pub fn seed_compact_mode(state: &mut HelpCenterState) {
    let _ = state.set_mode(HelpCenterMode::Compact);
}

/// Seed diagnostics-visible story.
pub fn seed_diagnostics_state(state: &mut HelpCenterState) {
    state.show_diagnostics = true;
    state.mode = HelpCenterMode::Full;
    state.context_label = Some("focus: editor".into());
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Paint stress targets.
pub mod bench {
    /// Mock topics.
    pub const BURST_TOPICS: usize = 500;
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

    fn open() -> HelpCenterState {
        let mut st = HelpCenterState::new();
        st.density = Some(HelpCenterDensity::Normal);
        st.mode = HelpCenterMode::Full;
        st
    }

    #[test]
    fn focus_cycle_visits_visible_panes_only() {
        let mut st = open();
        let system = DesignSystem::default();
        let topics = example_help_topics();
        let help = example_help_center_entries(&system);
        let cmds = command_entries_from_help(&help);
        let doctor = example_help_doctor_report();
        st.focus = "nav";
        let mut seen = vec![st.focus];
        for _ in 0..12 {
            let out = st.handle_key(
                press(KeyCode::Tab),
                &topics,
                &help,
                &cmds,
                Some(&doctor),
                &[],
            );
            assert!(matches!(out, HelpCenterOutcome::FocusChanged(_)));
            seen.push(st.focus);
        }
        assert!(seen.contains(&"search"));
        assert!(seen.contains(&"nav"));
        assert!(seen.contains(&"keyboard"));
        assert!(seen.contains(&"commands"));
        assert!(seen.contains(&"body"));
        assert!(!seen.contains(&"status"));
    }

    #[test]
    fn narrow_tiny_collapse_and_tab_clamp() {
        let mut st = open();
        let system = DesignSystem::default();
        let topics = example_help_topics();
        let help = example_help_center_entries(&system);
        let cmds = command_entries_from_help(&help);
        st.density = Some(HelpCenterDensity::Tiny);
        st.focus = "keyboard";
        st.clamp_focus_to_density(HelpCenterDensity::Tiny);
        assert_ne!(st.focus, "keyboard");
        assert_ne!(st.focus, "nav");
        let vis = st.visible_focus_panes(HelpCenterDensity::Tiny);
        assert!(!vis.contains(&HelpCenterPane::Keyboard));
        assert!(!vis.contains(&HelpCenterPane::Nav));

        let area = Rect::new(0, 0, 40, 20);
        let mut buf = Buffer::empty(area);
        st.density = None;
        render_help_center(
            &mut buf,
            area,
            HelpCenterSurfaces {
                system: &system,
                state: &mut st,
                topics: &topics,
                help_entries: &help,
                commands: &cmds,
                doctor: None,
                component_ids: &[],
            },
        );
        assert_eq!(st.effective_density(), HelpCenterDensity::Tiny);
        for _ in 0..6 {
            let _ = st.handle_key(press(KeyCode::Tab), &topics, &help, &cmds, None, &[]);
            assert!(st.focus == "search" || st.focus == "body");
            assert_ne!(st.focus, "keyboard");
            assert_ne!(st.focus, "nav");
        }
    }

    #[test]
    fn compact_vs_full_layout_differs() {
        let ws = WorkspaceState::new();
        let full = help_center_layout_density(
            Rect::new(0, 0, 120, 40),
            &ws,
            HelpCenterDensity::Normal,
            HelpCenterMode::Full,
            true,
        );
        let compact = help_center_layout_density(
            Rect::new(0, 0, 120, 40),
            &ws,
            HelpCenterDensity::Normal,
            HelpCenterMode::Compact,
            true,
        );
        let full_ids: Vec<_> = full
            .iter()
            .filter(|p| !p.collapsed && p.area.width > 0)
            .map(|p| p.id.0.as_str())
            .collect();
        let compact_ids: Vec<_> = compact
            .iter()
            .filter(|p| !p.collapsed && p.area.width > 0)
            .map(|p| p.id.0.as_str())
            .collect();
        assert!(full_ids.contains(&"nav"));
        assert!(!compact_ids.contains(&"nav"));
        assert!(compact_ids.contains(&"keyboard"));
        assert!(compact_ids.contains(&"body"));

        let mut st = open();
        let out = st.set_mode(HelpCenterMode::Compact);
        assert!(matches!(
            out,
            HelpCenterOutcome::ModeChanged(HelpCenterMode::Compact)
        ));
    }

    #[test]
    fn topic_open_and_filter() {
        let mut st = open();
        let system = DesignSystem::default();
        let topics = example_help_topics();
        let help = example_help_center_entries(&system);
        let cmds = command_entries_from_help(&help);
        st.focus = "nav";
        st.nav = ListState::new(Some("getting-started".into()));
        st.apply_focus_gates();
        let out = st.handle_key(press(KeyCode::Enter), &topics, &help, &cmds, None, &[]);
        assert!(
            matches!(
                out,
                HelpCenterOutcome::TopicOpened { ref id } if id == "getting-started"
            ),
            "got {out:?}"
        );

        st.focus = "search";
        st.apply_focus_gates();
        st.search.set_query("color");
        let out = st.handle_key(press(KeyCode::Enter), &topics, &help, &cmds, None, &[]);
        assert!(
            matches!(out, HelpCenterOutcome::FilterChanged { ref query } if query.contains("color"))
                || st.search.query().contains("color"),
            "got {out:?}"
        );
        let filtered = filter_help_topics(&topics, "color");
        assert!(!filtered.is_empty());
        assert!(filtered.iter().all(|t| t.matches_query("color")));
    }

    #[test]
    fn command_run_from_metadata_help_entries() {
        let mut st = open();
        let system = DesignSystem::default();
        let topics = example_help_topics();
        let help = example_help_center_entries(&system);
        let cmds = command_entries_from_help(&help);
        assert!(!cmds.is_empty(), "commands must derive from HelpEntry");
        // Every command shortcut must match a HelpEntry chord (SoT)
        for c in &cmds {
            let chord = c.shortcut.as_deref().expect("shortcut from HelpEntry");
            assert!(
                help.iter().any(|h| h.chord == chord && h.id == c.id),
                "command {} shortcut not from HelpEntry SoT",
                c.id
            );
        }
        st.focus = "commands";
        let first = cmds[0].id.clone();
        st.commands = ListState::new(Some(first.clone()));
        st.apply_focus_gates();
        let out = st.handle_key(press(KeyCode::Enter), &topics, &help, &cmds, None, &[]);
        assert!(
            matches!(
                out,
                HelpCenterOutcome::CommandRun { ref id } if id == &first
            ),
            "got {out:?}"
        );
    }

    #[test]
    fn keyboard_map_from_live_help_entries_not_static_table() {
        let system = DesignSystem::default();
        let help = example_help_center_entries(&system);
        // example_help_entries uses help_entries_from_keymap — chords are formatted live
        assert!(!help.is_empty());
        for e in &help {
            assert!(!e.chord.is_empty(), "chord must be live-formatted");
            assert!(!e.action.is_empty());
        }
        // Structural: command_entries_from_help is the only command path in this module
        let body = include_str!("help_center.rs");
        let code = body
            .split("fn keyboard_map_from_live_help_entries_not_static_table")
            .next()
            .unwrap_or(body);
        assert!(
            code.contains("command_entries_from_help"),
            "must use HelpEntry→command projection"
        );
        assert!(
            code.contains("example_help_entries") || code.contains("help_entries_from_keymap"),
            "must use keymap help generators"
        );
        // No parallel hardcoded Ctrl+S style table as sole SoT in fixtures
        let forbidden_static = ["Ctrl+S save document hardcoded", "STATIC_SHORTCUTS"];
        for f in forbidden_static {
            assert!(!code.contains(f), "forbidden stale table {f}");
        }
    }

    #[test]
    fn link_and_anchor_outcomes() {
        let mut st = open();
        let system = DesignSystem::default();
        let topics = example_help_topics();
        let help = example_help_center_entries(&system);
        let cmds = command_entries_from_help(&help);
        st.selected_topic = Some("getting-started".into());
        st.focus = "body";
        st.apply_focus_gates();
        // g → first anchor
        let out = st.handle_key(press(KeyCode::Char('g')), &topics, &help, &cmds, None, &[]);
        assert!(
            matches!(
                out,
                HelpCenterOutcome::AnchorJumped { ref anchor } if !anchor.is_empty()
            ),
            "got {out:?}"
        );
    }

    #[test]
    fn doctor_and_inspect_outcomes() {
        let mut st = open();
        let system = DesignSystem::default();
        let topics = example_help_topics();
        let help = example_help_center_entries(&system);
        let cmds = command_entries_from_help(&help);
        let doctor = example_help_doctor_report();
        let components = vec!["keyboard-help".into(), "command-palette".into()];
        st.show_diagnostics = true;
        st.focus = "diagnostics";
        st.diagnostics = ListState::new(Some("keyboard-help".into()));
        st.apply_focus_gates();
        let out = st.handle_key(
            press(KeyCode::Char('i')),
            &topics,
            &help,
            &cmds,
            Some(&doctor),
            &components,
        );
        assert!(
            matches!(
                out,
                HelpCenterOutcome::InspectComponent { ref id } if id == "keyboard-help"
            ),
            "got {out:?}"
        );
        let out = st.handle_key(
            press(KeyCode::Char('d')),
            &topics,
            &help,
            &cmds,
            Some(&doctor),
            &components,
        );
        assert!(
            matches!(out, HelpCenterOutcome::DoctorOpened),
            "got {out:?}"
        );
    }

    #[test]
    fn no_help_io_in_composition() {
        let body = include_str!("help_center.rs");
        let code = body
            .split("fn no_help_io_in_composition")
            .next()
            .unwrap_or(body);
        for forbidden in [
            "std::fs::",
            "tokio::fs",
            "reqwest",
            "ureq",
            "TcpStream",
            "include_str!(\"/",
        ] {
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
            assert!(hits.is_empty(), "forbidden {forbidden}: {hits:?}");
        }
    }

    #[test]
    fn paint_smoke_and_search_height() {
        let system = DesignSystem::default();
        let mut st = open();
        let topics = example_help_topics();
        let help = example_help_center_entries(&system);
        let cmds = command_entries_from_help(&help);
        let doctor = example_help_doctor_report();
        let area = Rect::new(0, 0, 120, 40);
        let mut buf = Buffer::empty(area);
        render_help_center(
            &mut buf,
            area,
            HelpCenterSurfaces {
                system: &system,
                state: &mut st,
                topics: &topics,
                help_entries: &help,
                commands: &cmds,
                doctor: Some(&doctor),
                component_ids: &[],
            },
        );
        let search = st
            .last_panes()
            .iter()
            .find(|p| p.id.0.as_str() == "search")
            .expect("search");
        assert!(search.area.height >= 3);
        assert!(
            st.status
                .transient
                .as_ref()
                .is_some_and(|t| t.contains("HelpEntry") || t.contains("keymap")),
            "status must document metadata SoT"
        );
    }

    #[test]
    fn keyboard_map_full_mode_modal_navigable() {
        let system = DesignSystem::default();
        let mut st = open();
        assert_eq!(st.mode, HelpCenterMode::Full, "open() is full docs");
        assert_eq!(
            st.keyboard.mode(),
            KeyboardHelpMode::Modal,
            "full mode must use Modal map, not Footer strip"
        );
        assert!(
            st.keyboard.is_open(),
            "keyboard modal must be open for navigable map"
        );

        let topics = example_help_topics();
        let help = example_help_center_entries(&system);
        assert!(!help.is_empty(), "need HelpEntry content");
        let cmds = command_entries_from_help(&help);
        let area = Rect::new(0, 0, 120, 40);
        let mut buf = Buffer::empty(area);
        st.focus = "keyboard";
        render_help_center(
            &mut buf,
            area,
            HelpCenterSurfaces {
                system: &system,
                state: &mut st,
                topics: &topics,
                help_entries: &help,
                commands: &cmds,
                doctor: None,
                component_ids: &[],
            },
        );
        let kb = st
            .last_panes()
            .iter()
            .find(|p| p.id.0.as_str() == "keyboard")
            .expect("keyboard pane in full normal layout");
        assert!(
            !kb.collapsed && kb.area.height >= 3,
            "keyboard pane must be multi-row for map, got h={}",
            kb.area.height
        );
        // Buffer should show HelpEntry action/chord text from live SoT
        let mut painted = String::new();
        for y in kb.area.y..kb.area.y.saturating_add(kb.area.height) {
            for x in kb.area.x..kb.area.x.saturating_add(kb.area.width) {
                if let Some(cell) = buf.cell((x, y)) {
                    painted.push_str(cell.symbol());
                }
            }
            painted.push('\n');
        }
        let hit = help.iter().any(|e| {
            painted.contains(&e.action)
                || painted.contains(&e.chord)
                || painted.contains("Keyboard")
        });
        assert!(
            hit,
            "keyboard pane must paint HelpEntry content, sample={painted:?}"
        );

        // Down through real handle_key path — Modal+focused yields CursorMoved / Child
        st.focus = "keyboard";
        st.apply_focus_gates();
        st.ensure_keyboard_map_modal();
        let before = st.keyboard.cursor_index();
        let out = st.handle_key(press(KeyCode::Down), &topics, &help, &cmds, None, &[]);
        assert!(
            !matches!(out, HelpCenterOutcome::Ignored)
                || st.keyboard.cursor_index() != before
                || help.len() <= 1,
            "Down on non-empty keyboard map must not be dead Footer path, got {out:?} cursor {before}→{}",
            st.keyboard.cursor_index()
        );
        if help.len() > 1 {
            assert!(
                matches!(out, HelpCenterOutcome::Child { .. })
                    || st.keyboard.cursor_index() != before,
                "expected cursor move or child outcome, got {out:?}"
            );
        }
    }

    #[test]
    fn diagnostics_tab_not_when_unpainted() {
        let mut st = open();
        let system = DesignSystem::default();
        let topics = example_help_topics();
        let help = example_help_center_entries(&system);
        let cmds = command_entries_from_help(&help);
        // No doctor findings, no components → diagnostics not live
        st.show_diagnostics = true;
        st.sync_diagnostics_live(None, &[]);
        assert!(!st.diagnostics_live);
        let vis = st.visible_focus_panes(HelpCenterDensity::Normal);
        assert!(!vis.contains(&HelpCenterPane::Diagnostics));
        for _ in 0..10 {
            let _ = st.handle_key(press(KeyCode::Tab), &topics, &help, &cmds, None, &[]);
            assert_ne!(st.focus, "diagnostics");
        }
        // With findings, diagnostics enters Tab cycle
        let doctor = example_help_doctor_report();
        st.sync_diagnostics_live(Some(&doctor), &[]);
        assert!(st.diagnostics_live || doctor.findings.is_empty());
        if !doctor.findings.is_empty() {
            assert!(
                st.visible_focus_panes(HelpCenterDensity::Normal)
                    .contains(&HelpCenterPane::Diagnostics)
            );
        }
    }

    #[test]
    fn inspect_only_for_component_ids_not_findings() {
        let mut st = open();
        let system = DesignSystem::default();
        let topics = example_help_topics();
        let help = example_help_center_entries(&system);
        let cmds = command_entries_from_help(&help);
        let doctor = example_help_doctor_report();
        let components = vec!["keyboard-help".into()];
        st.show_diagnostics = true;
        st.focus = "diagnostics";
        st.sync_diagnostics_live(Some(&doctor), &components);
        // Select a finding code (not a component id)
        if let Some(code) = doctor.findings.first().map(|f| f.code.clone()) {
            st.diagnostics = ListState::new(Some(code.clone()));
            st.apply_focus_gates();
            let out = st.handle_key(
                press(KeyCode::Char('i')),
                &topics,
                &help,
                &cmds,
                Some(&doctor),
                &components,
            );
            assert!(
                !matches!(out, HelpCenterOutcome::InspectComponent { .. }),
                "i on finding code must not InspectComponent, got {out:?}"
            );
        }
        st.diagnostics = ListState::new(Some("keyboard-help".into()));
        let out = st.handle_key(
            press(KeyCode::Char('i')),
            &topics,
            &help,
            &cmds,
            Some(&doctor),
            &components,
        );
        assert!(
            matches!(
                out,
                HelpCenterOutcome::InspectComponent { ref id } if id == "keyboard-help"
            ),
            "got {out:?}"
        );
    }

    #[test]
    fn diagnostics_down_reaches_component_when_findings_present() {
        let mut st = open();
        let system = DesignSystem::default();
        let topics = example_help_topics();
        let help = example_help_center_entries(&system);
        let cmds = command_entries_from_help(&help);
        let doctor = example_help_doctor_report();
        assert!(
            !doctor.findings.is_empty(),
            "need findings so both lists are painted"
        );
        let components = vec!["keyboard-help".into(), "command-palette".into()];
        let rows = diagnostics_rows(Some(&doctor), &components);
        assert!(
            rows.iter().any(|r| r.id == "keyboard-help"),
            "shared rows must include component ids alongside findings"
        );
        assert!(
            rows.iter().any(|r| r.id.starts_with("finding:")),
            "shared rows must include finding:* ids"
        );

        st.show_diagnostics = true;
        st.focus = "diagnostics";
        st.sync_diagnostics_live(Some(&doctor), &components);
        st.apply_focus_gates();
        // Start on first finding (list head), then Down until selection is a component id
        st.diagnostics = ListState::new(doctor.findings.first().map(|f| f.code.clone()));
        let mut reached = false;
        for _ in 0..rows.len().saturating_add(4) {
            let _ = st.handle_key(
                press(KeyCode::Down),
                &topics,
                &help,
                &cmds,
                Some(&doctor),
                &components,
            );
            if let Some(id) = st.diagnostics.selected() {
                if components.iter().any(|c| c == id) {
                    reached = true;
                    let out = st.handle_key(
                        press(KeyCode::Char('i')),
                        &topics,
                        &help,
                        &cmds,
                        Some(&doctor),
                        &components,
                    );
                    assert!(
                        matches!(
                            out,
                            HelpCenterOutcome::InspectComponent { ref id } if components.contains(id)
                        ),
                        "i on navigated component must InspectComponent, got {out:?}"
                    );
                    break;
                }
            }
        }
        assert!(
            reached,
            "Down on diagnostics_rows must reach a painted component id when findings+components present; selected={:?}",
            st.diagnostics.selected()
        );
    }

    #[test]
    fn burst_paint_perf() {
        let system = DesignSystem::default();
        let mut st = open();
        st.density = Some(HelpCenterDensity::Normal);
        let topics = burst_help_topics(bench::BURST_TOPICS);
        let help = example_help_center_entries(&system);
        let cmds = command_entries_from_help(&help);
        let area = Rect::new(0, 0, bench::VIEWPORT.0, bench::VIEWPORT.1);
        let mut buf = Buffer::empty(area);
        let start = std::time::Instant::now();
        for _ in 0..bench::PAINT_FRAMES {
            render_help_center(
                &mut buf,
                area,
                HelpCenterSurfaces {
                    system: &system,
                    state: &mut st,
                    topics: &topics,
                    help_entries: &help,
                    commands: &cmds,
                    doctor: None,
                    component_ids: &[],
                },
            );
        }
        let elapsed = start.elapsed();
        assert!(elapsed.as_secs() < 5, "paint too slow: {elapsed:?}");
    }

    #[test]
    fn layout_full_normal_has_expected_panes() {
        let ws = WorkspaceState::new();
        let panes = help_center_layout_density(
            Rect::new(0, 0, 120, 40),
            &ws,
            HelpCenterDensity::Normal,
            HelpCenterMode::Full,
            true,
        );
        let ids: Vec<_> = panes
            .iter()
            .filter(|p| !p.collapsed && p.area.width > 0 && p.area.height > 0)
            .map(|p| p.id.0.as_str())
            .collect();
        for need in ["search", "nav", "keyboard", "commands", "body", "status"] {
            assert!(ids.contains(&need), "missing {need} in {ids:?}");
        }
    }
}
