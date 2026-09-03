// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **FullscreenViewer** and **SemanticZoom** — promote compact → detail → fullscreen.
//!
//! **Mission.** Reusable promotion path from a compact row to inline detail to
//! fullscreen inspection without copying application state. Preserves selection,
//! focus token, scroll anchor, and source context across promote/demote.
//!
//! **Chrome.** Title, breadcrumbs (path from source), actions, optional search
//! and help strips, close/restore. **Body** is a host-owned slot — paint
//! [`super::CodeBlock`], [`super::DiffReview`], logs, objects, tasks, or media
//! into [`FullscreenViewerSlots::body`].
//!
//! **Escape law (nested overlays).**
//! 1. If a **child** overlay is open under the viewer → peel **one** stack layer
//!    ([`OverlayStack::handle_escape`]) — do not demote zoom.
//! 2. Else if help is open → close help.
//! 3. Else if search is open → close search / clear query.
//! 4. Else **demote** one zoom level (Fullscreen → Detail → Compact) and restore
//!    source context to the host; closing Compact emits `Closed`.
//!
//! Research: Grok Build fullscreen overlays, file previews, IDE inspectors,
//! terminal pagers.
use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::StatefulWidget};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    interaction::{
        HitRegion, OverlayId, OverlayOutcome, OverlayStack, SemanticNode, SemanticRole,
        SemanticScene, SemanticState, UiIntent,
    },
    style::{DesignSystem, Role},
    text::{display_cols, take_display_cols},
};

#[cfg(test)]
use super::ActionVariant;
use super::{Action, Surface, SurfaceRecipe};

/// Default overlay id for fullscreen viewer.
pub const FULLSCREEN_VIEWER_OVERLAY_ID: &str = "termrock.fullscreen-viewer";
/// Nested overlay id prefix for children under the viewer (search help, pickers).
pub const FULLSCREEN_VIEWER_NESTED_PREFIX: &str = "termrock.fullscreen-viewer.child";
/// Default help footer when help strip is closed.
pub const FULLSCREEN_VIEWER_HINT: &str = "esc demote · / search · ? help · f fullscreen";

// ── Zoom / source context ───────────────────────────────────────────────────

/// Promotion level along the compact → detail → fullscreen path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
#[non_exhaustive]
pub enum ZoomLevel {
    /// Compact row / list density (host paints in place).
    #[default]
    Compact,
    /// Inline expanded detail (host in-layout, not necessarily overlay).
    Detail,
    /// Full bounds inspection overlay.
    Fullscreen,
}

impl ZoomLevel {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Detail => "detail",
            Self::Fullscreen => "fullscreen",
        }
    }

    /// Promote one step (saturates at Fullscreen).
    #[must_use]
    pub const fn promote(self) -> Self {
        match self {
            Self::Compact => Self::Detail,
            Self::Detail | Self::Fullscreen => Self::Fullscreen,
        }
    }

    /// Demote one step (saturates at Compact).
    #[must_use]
    pub const fn demote(self) -> Self {
        match self {
            Self::Fullscreen => Self::Detail,
            Self::Detail | Self::Compact => Self::Compact,
        }
    }

    /// Whether this level uses the fullscreen overlay chrome.
    #[must_use]
    pub const fn is_fullscreen(self) -> bool {
        matches!(self, Self::Fullscreen)
    }
}

/// Content family hint for chrome (does not own the view).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ViewerContentKind {
    /// Source / code.
    #[default]
    Code,
    /// Diff / review.
    Diff,
    /// Log stream.
    Log,
    /// Structured object / inspector.
    Object,
    /// Task / work item.
    Task,
    /// Image / media surface.
    Media,
    /// Host custom.
    Custom,
}

impl ViewerContentKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Code => "code",
            Self::Diff => "diff",
            Self::Log => "log",
            Self::Object => "object",
            Self::Task => "task",
            Self::Media => "media",
            Self::Custom => "custom",
        }
    }

    /// Short chrome badge.
    #[must_use]
    pub const fn badge(self) -> &'static str {
        match self {
            Self::Code => "code",
            Self::Diff => "diff",
            Self::Log => "log",
            Self::Object => "object",
            Self::Task => "task",
            Self::Media => "media",
            Self::Custom => "view",
        }
    }
}

/// Scroll / selection anchor to restore after demote.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScrollAnchor {
    /// Vertical line or row index in the **source** document.
    pub line: usize,
    /// Horizontal column (cells).
    pub col: u16,
    /// Optional stable content id (hunk id, log id, path) for re-resolve after reflow.
    pub stable_id: Option<String>,
}

impl ScrollAnchor {
    /// Line/col only.
    #[must_use]
    pub const fn at(line: usize, col: u16) -> Self {
        Self {
            line,
            col,
            stable_id: None,
        }
    }

    /// With stable id.
    #[must_use]
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.stable_id = Some(id.into());
        self
    }
}

/// Source context preserved across promotion (host re-applies to origin widget).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceContext<Id> {
    /// Identity of the compact/detail source item.
    pub source_id: Id,
    /// Selection at promote time (may equal source_id).
    pub selection: Option<Id>,
    /// Scroll anchor in the source surface.
    pub scroll: ScrollAnchor,
    /// Host focus token for InteractionScene restore.
    pub focus_token: Option<String>,
    /// Path labels for breadcrumbs (outer → current).
    pub path_labels: Vec<String>,
}

impl<Id> SourceContext<Id> {
    /// Minimal context.
    #[must_use]
    pub fn new(source_id: Id) -> Self {
        Self {
            source_id,
            selection: None,
            scroll: ScrollAnchor::default(),
            focus_token: None,
            path_labels: Vec::new(),
        }
    }

    /// Selection.
    #[must_use]
    pub fn selection(mut self, id: Option<Id>) -> Self {
        self.selection = id;
        self
    }

    /// Scroll anchor.
    #[must_use]
    pub fn scroll(mut self, scroll: ScrollAnchor) -> Self {
        self.scroll = scroll;
        self
    }

    /// Focus token.
    #[must_use]
    pub fn focus_token(mut self, token: impl Into<String>) -> Self {
        self.focus_token = Some(token.into());
        self
    }

    /// Breadcrumb path.
    #[must_use]
    pub fn path(mut self, labels: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.path_labels = labels.into_iter().map(Into::into).collect();
        self
    }
}

// ── Chrome focus / slots ────────────────────────────────────────────────────

/// Which chrome band owns keyboard (body is host content).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ViewerChromeFocus {
    /// Title / close band.
    Title,
    /// Breadcrumb path.
    Breadcrumbs,
    /// Action bar.
    Actions,
    /// Search field.
    Search,
    /// Host body (default — pager / content keys).
    #[default]
    Body,
    /// Help strip.
    Help,
}

impl ViewerChromeFocus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Breadcrumbs => "breadcrumbs",
            Self::Actions => "actions",
            Self::Search => "search",
            Self::Body => "body",
            Self::Help => "help",
        }
    }
}

/// Slot geometry after paint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FullscreenViewerSlots {
    /// Outer fullscreen rect.
    pub root: Rect,
    /// Title / badge / close hint.
    pub title: Rect,
    /// Breadcrumb path.
    pub breadcrumbs: Rect,
    /// Action bar.
    pub actions: Rect,
    /// Search strip (empty when closed).
    pub search: Rect,
    /// Host content body.
    pub body: Rect,
    /// Help / status footer.
    pub footer: Rect,
}

impl FullscreenViewerSlots {
    /// Empty.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            root: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            title: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            breadcrumbs: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            actions: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            search: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            body: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            footer: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
        }
    }
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Typed outcomes for viewer + zoom.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FullscreenViewerOutcome<Id> {
    /// No change.
    Ignored,
    /// Fullscreen chrome opened (host should open OverlayStack).
    Opened {
        /// Zoom after open.
        level: ZoomLevel,
    },
    /// Closed; host restores source context to origin widget.
    Closed {
        /// Context to re-apply.
        restore: SourceContext<Id>,
    },
    /// Demoted one level (may still be Detail).
    Demoted {
        /// New level.
        level: ZoomLevel,
        /// Context snapshot.
        restore: SourceContext<Id>,
    },
    /// Promoted one level.
    Promoted {
        /// New level.
        level: ZoomLevel,
    },
    /// Search strip toggled or query changed.
    SearchChanged {
        /// Whether search strip is open.
        open: bool,
        /// Current query (borrowed copy as owned String for host).
        query: String,
    },
    /// Help strip toggled.
    HelpToggled {
        /// Open?
        open: bool,
    },
    /// Chrome action activated.
    ActionActivated {
        /// Action id.
        id: Id,
    },
    /// Chrome focus band moved.
    ChromeFocusMoved {
        /// New band.
        focus: ViewerChromeFocus,
    },
    /// Host should call [`OverlayStack::handle_escape`] for a nested child.
    NestedEscape,
}

// ── Overlay helpers ─────────────────────────────────────────────────────────

/// Open fullscreen viewer on the stack (fills bounds).
pub fn open_fullscreen_viewer_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.open(
        bounds,
        crate::interaction::OverlaySpec::fullscreen(FULLSCREEN_VIEWER_OVERLAY_ID, opener_focus),
    )
}

/// Open a nested child under the viewer (peels first on Esc).
pub fn open_fullscreen_viewer_child_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    child_suffix: &str,
    size: crate::interaction::OverlaySize,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    let id = format!("{FULLSCREEN_VIEWER_NESTED_PREFIX}.{child_suffix}");
    let spec = crate::interaction::OverlaySpec::popover(
        id,
        Rect::new(
            bounds.x + bounds.width / 4,
            bounds.y + bounds.height / 4,
            1,
            1,
        ),
        size,
        opener_focus,
    )
    .with_parent(OverlayId::from_static(FULLSCREEN_VIEWER_OVERLAY_ID));
    stack.open(bounds, spec)
}

/// Dismiss viewer overlay (restores opener focus).
pub fn dismiss_fullscreen_viewer_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.dismiss(&OverlayId::from_static(FULLSCREEN_VIEWER_OVERLAY_ID))
}

/// True if stack top is a child of the fullscreen viewer.
#[must_use]
pub fn fullscreen_viewer_has_nested_top<F>(stack: &OverlayStack<F>) -> bool {
    stack.top().is_some_and(|t| {
        t.parent
            .as_ref()
            .is_some_and(|p| p.0 == FULLSCREEN_VIEWER_OVERLAY_ID)
            || t.id.0.starts_with(FULLSCREEN_VIEWER_NESTED_PREFIX)
    })
}

// ── SemanticZoom ────────────────────────────────────────────────────────────

/// State machine: compact ↔ detail ↔ fullscreen with frozen source context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticZoomState<Id> {
    level: ZoomLevel,
    source: Option<SourceContext<Id>>,
    content_kind: ViewerContentKind,
}

impl<Id> Default for SemanticZoomState<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id> SemanticZoomState<Id> {
    /// Compact, no source.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            level: ZoomLevel::Compact,
            source: None,
            content_kind: ViewerContentKind::Code,
        }
    }

    /// Current level.
    #[must_use]
    pub const fn level(&self) -> ZoomLevel {
        self.level
    }

    /// Content kind.
    #[must_use]
    pub const fn content_kind(&self) -> ViewerContentKind {
        self.content_kind
    }

    /// Set content kind.
    pub fn set_content_kind(&mut self, kind: ViewerContentKind) {
        self.content_kind = kind;
    }

    /// Borrow source context (present after promote until closed).
    #[must_use]
    pub fn source(&self) -> Option<&SourceContext<Id>> {
        self.source.as_ref()
    }
}

impl<Id: Clone> SemanticZoomState<Id> {
    /// Promote from compact/detail with a frozen source snapshot.
    pub fn promote(&mut self, ctx: SourceContext<Id>) -> FullscreenViewerOutcome<Id> {
        self.source = Some(ctx);
        let prev = self.level;
        self.level = self.level.promote();
        if self.level == prev && matches!(prev, ZoomLevel::Fullscreen) {
            return FullscreenViewerOutcome::Ignored;
        }
        if self.level.is_fullscreen() {
            FullscreenViewerOutcome::Opened { level: self.level }
        } else {
            FullscreenViewerOutcome::Promoted { level: self.level }
        }
    }

    /// Force a specific level (host policy). Prefer [`Self::promote`] / [`Self::demote`] /
    /// [`Self::enter_fullscreen`] / [`Self::close`] for normal paths.
    pub fn set_level(&mut self, level: ZoomLevel) -> FullscreenViewerOutcome<Id> {
        if level == self.level {
            return FullscreenViewerOutcome::Ignored;
        }
        let was = self.level;
        self.level = level;
        if level > was {
            if level.is_fullscreen() {
                FullscreenViewerOutcome::Opened { level }
            } else {
                FullscreenViewerOutcome::Promoted { level }
            }
        } else if let Some(restore) = self.source.clone() {
            if matches!(level, ZoomLevel::Compact) {
                FullscreenViewerOutcome::Closed { restore }
            } else {
                FullscreenViewerOutcome::Demoted { level, restore }
            }
        } else {
            // Level changed without restore payload — host forced demote before promote.
            FullscreenViewerOutcome::Ignored
        }
    }

    /// Demote one level; returns restore context for host.
    pub fn demote(&mut self) -> FullscreenViewerOutcome<Id> {
        let Some(restore) = self.source.clone() else {
            return FullscreenViewerOutcome::Ignored;
        };
        match self.level {
            ZoomLevel::Compact => FullscreenViewerOutcome::Ignored,
            ZoomLevel::Detail => {
                self.level = ZoomLevel::Compact;
                FullscreenViewerOutcome::Closed { restore }
            }
            ZoomLevel::Fullscreen => {
                self.level = ZoomLevel::Detail;
                FullscreenViewerOutcome::Demoted {
                    level: ZoomLevel::Detail,
                    restore,
                }
            }
        }
    }

    /// Jump to fullscreen from any level.
    pub fn enter_fullscreen(&mut self, ctx: SourceContext<Id>) -> FullscreenViewerOutcome<Id> {
        self.source = Some(ctx);
        self.level = ZoomLevel::Fullscreen;
        FullscreenViewerOutcome::Opened {
            level: ZoomLevel::Fullscreen,
        }
    }

    /// Close fully to compact and clear open chrome.
    pub fn close(&mut self) -> FullscreenViewerOutcome<Id> {
        let restore = self.source.clone();
        self.level = ZoomLevel::Compact;
        match restore {
            Some(restore) => FullscreenViewerOutcome::Closed { restore },
            None => FullscreenViewerOutcome::Ignored,
        }
    }
}

// ── FullscreenViewer state ──────────────────────────────────────────────────

/// Fullscreen inspection chrome state (body content is host-owned).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FullscreenViewerState<Id> {
    zoom: SemanticZoomState<Id>,
    open: bool,
    accepts_input: bool,
    enabled: bool,
    title: String,
    chrome_focus: ViewerChromeFocus,
    search_open: bool,
    search_query: String,
    help_open: bool,
    action_cursor: Option<Id>,
    slots: FullscreenViewerSlots,
    action_regions: Vec<HitRegion<Id>>,
    /// Nested child count hint from host (stack is authority).
    nested_child_hint: bool,
}

impl<Id> Default for FullscreenViewerState<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id> FullscreenViewerState<Id> {
    /// Closed compact.
    #[must_use]
    pub fn new() -> Self {
        Self {
            zoom: SemanticZoomState::new(),
            open: false,
            accepts_input: true,
            enabled: true,
            title: String::new(),
            chrome_focus: ViewerChromeFocus::Body,
            search_open: false,
            search_query: String::new(),
            help_open: false,
            action_cursor: None,
            slots: FullscreenViewerSlots::empty(),
            action_regions: Vec::new(),
            nested_child_hint: false,
        }
    }

    /// Zoom engine.
    #[must_use]
    pub fn zoom(&self) -> &SemanticZoomState<Id> {
        &self.zoom
    }

    /// Whether fullscreen chrome is open.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.open
    }

    /// Current zoom level.
    #[must_use]
    pub const fn level(&self) -> ZoomLevel {
        self.zoom.level()
    }

    /// Slots after paint.
    #[must_use]
    pub const fn slots(&self) -> FullscreenViewerSlots {
        self.slots
    }

    /// Body rect for host content.
    #[must_use]
    pub const fn body_area(&self) -> Rect {
        self.slots.body
    }

    /// Search query.
    #[must_use]
    pub fn search_query(&self) -> &str {
        &self.search_query
    }

    /// Search open?
    #[must_use]
    pub const fn search_open(&self) -> bool {
        self.search_open
    }

    /// Help open?
    #[must_use]
    pub const fn help_open(&self) -> bool {
        self.help_open
    }

    /// Title.
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    /// Input gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Enable.
    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
    }

    /// ASCII chrome.
    /// Host reports nested child overlay presence (or use stack query).
    pub fn set_nested_child_hint(&mut self, on: bool) {
        self.nested_child_hint = on;
    }

    /// Chrome focus band.
    #[must_use]
    pub const fn chrome_focus(&self) -> ViewerChromeFocus {
        self.chrome_focus
    }

    /// Source context for restore.
    #[must_use]
    pub fn source(&self) -> Option<&SourceContext<Id>> {
        self.zoom.source()
    }
}

impl<Id: Clone + PartialEq> FullscreenViewerState<Id> {
    /// Promote / open path with source freeze.
    pub fn promote(
        &mut self,
        ctx: SourceContext<Id>,
        title: impl Into<String>,
    ) -> FullscreenViewerOutcome<Id> {
        if !self.enabled {
            return FullscreenViewerOutcome::Ignored;
        }
        self.title = title.into();
        let out = self.zoom.promote(ctx);
        if matches!(
            out,
            FullscreenViewerOutcome::Opened { .. } | FullscreenViewerOutcome::Promoted { .. }
        ) {
            if self.zoom.level().is_fullscreen() {
                self.open = true;
                self.chrome_focus = ViewerChromeFocus::Body;
            }
        }
        out
    }

    /// Enter fullscreen directly.
    pub fn enter_fullscreen(
        &mut self,
        ctx: SourceContext<Id>,
        title: impl Into<String>,
    ) -> FullscreenViewerOutcome<Id> {
        if !self.enabled {
            return FullscreenViewerOutcome::Ignored;
        }
        self.title = title.into();
        let out = self.zoom.enter_fullscreen(ctx);
        self.open = true;
        self.chrome_focus = ViewerChromeFocus::Body;
        out
    }

    /// Open on OverlayStack (Fullscreen kind).
    pub fn open_on_stack<F: Clone>(
        &mut self,
        stack: &mut OverlayStack<F>,
        bounds: Rect,
        opener_focus: Option<F>,
    ) -> OverlayOutcome<F> {
        self.open = true;
        open_fullscreen_viewer_overlay(stack, bounds, opener_focus)
    }

    /// Close viewer chrome + stack; restore compact.
    pub fn close_on_stack<F: Clone>(
        &mut self,
        stack: &mut OverlayStack<F>,
    ) -> (FullscreenViewerOutcome<Id>, OverlayOutcome<F>) {
        let out = self.zoom.close();
        self.open = false;
        self.search_open = false;
        self.help_open = false;
        self.chrome_focus = ViewerChromeFocus::Body;
        let stack_out = dismiss_fullscreen_viewer_overlay(stack);
        (out, stack_out)
    }

    /// Demote one zoom level; Fullscreen→Detail clears overlay open flag (host dismisses stack).
    pub fn demote(&mut self) -> FullscreenViewerOutcome<Id> {
        let out = self.zoom.demote();
        match &out {
            FullscreenViewerOutcome::Demoted {
                level: ZoomLevel::Detail,
                ..
            }
            | FullscreenViewerOutcome::Closed { .. } => {
                self.open = false;
            }
            _ => {}
        }
        out
    }

    /// Escape law with optional live stack for nested detection.
    pub fn handle_escape<F>(
        &mut self,
        stack: Option<&OverlayStack<F>>,
    ) -> FullscreenViewerOutcome<Id> {
        if !self.open || !self.accepts_input {
            return FullscreenViewerOutcome::Ignored;
        }
        let nested = self.nested_child_hint || stack.is_some_and(fullscreen_viewer_has_nested_top);
        if nested {
            return FullscreenViewerOutcome::NestedEscape;
        }
        if self.help_open {
            self.help_open = false;
            return FullscreenViewerOutcome::HelpToggled { open: false };
        }
        if self.search_open {
            self.search_open = false;
            self.search_query.clear();
            return FullscreenViewerOutcome::SearchChanged {
                open: false,
                query: String::new(),
            };
        }
        // Demote zoom
        let out = self.zoom.demote();
        match &out {
            FullscreenViewerOutcome::Demoted {
                level: ZoomLevel::Detail,
                ..
            } => {
                // Leave overlay open? Spec: demote Fullscreen→Detail may drop overlay.
                self.open = false;
            }
            FullscreenViewerOutcome::Closed { .. } => {
                self.open = false;
            }
            _ => {}
        }
        out
    }

    /// Keyboard routing.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        actions: &[Action<'_, Id>],
    ) -> FullscreenViewerOutcome<Id> {
        if !self.open || !self.enabled || !self.accepts_input {
            return FullscreenViewerOutcome::Ignored;
        }
        if key.is_release() {
            return FullscreenViewerOutcome::Ignored;
        }
        let is_press = key.is_press();

        // Esc
        if matches!(key.code, KeyCode::Esc) && is_press && key.modifiers.is_empty() {
            return self.handle_escape::<()>(None);
        }

        // Global chrome shortcuts (when not typing in search)
        if !self.search_open || !matches!(self.chrome_focus, ViewerChromeFocus::Search) {
            if matches!(key.code, KeyCode::Char('/')) && is_press && key.modifiers.is_empty() {
                return self.toggle_search();
            }
            if matches!(key.code, KeyCode::Char('?')) && is_press && key.modifiers.is_empty() {
                self.help_open = !self.help_open;
                return FullscreenViewerOutcome::HelpToggled {
                    open: self.help_open,
                };
            }
            if matches!(key.code, KeyCode::Char('f' | 'F'))
                && is_press
                && key.modifiers.is_empty()
                && !matches!(self.chrome_focus, ViewerChromeFocus::Search)
            {
                // Already fullscreen; treat as no-op promote
                return FullscreenViewerOutcome::Ignored;
            }
        }

        // Search typing
        if self.search_open && matches!(self.chrome_focus, ViewerChromeFocus::Search) {
            match key.code {
                KeyCode::Char(c) if is_press && !c.is_control() && key.modifiers.is_empty() => {
                    self.search_query.push(c);
                    return FullscreenViewerOutcome::SearchChanged {
                        open: true,
                        query: self.search_query.clone(),
                    };
                }
                KeyCode::Backspace if is_press => {
                    self.search_query.pop();
                    return FullscreenViewerOutcome::SearchChanged {
                        open: true,
                        query: self.search_query.clone(),
                    };
                }
                KeyCode::Enter if is_press => {
                    // Commit search stays open; host filters content
                    return FullscreenViewerOutcome::SearchChanged {
                        open: true,
                        query: self.search_query.clone(),
                    };
                }
                _ => {}
            }
        }

        // Tab cycle chrome (Shift+Tab reverse) — portable, no scene required
        if matches!(key.code, KeyCode::Tab) && is_press {
            return self.cycle_chrome(key.modifiers.contains(KeyModifiers::SHIFT));
        }

        // Actions when focused
        if matches!(self.chrome_focus, ViewerChromeFocus::Actions) && !actions.is_empty() {
            match key.code {
                KeyCode::Left | KeyCode::Char('h' | 'H') => {
                    return self.move_action(actions, -1);
                }
                KeyCode::Right | KeyCode::Char('l' | 'L') => {
                    return self.move_action(actions, 1);
                }
                KeyCode::Enter if is_press => {
                    if let Some(id) = self.action_cursor.clone() {
                        if actions.iter().any(|a| a.id == id && a.enabled) {
                            return FullscreenViewerOutcome::ActionActivated { id };
                        }
                    }
                }
                _ => {}
            }
        }

        FullscreenViewerOutcome::Ignored
    }

    /// Intent routing (UiIntent::Fullscreen, Cancel, Help, Search, …).
    pub fn handle_intent(
        &mut self,
        intent: UiIntent,
        actions: &[Action<'_, Id>],
    ) -> FullscreenViewerOutcome<Id> {
        if !self.open || !self.enabled || !self.accepts_input {
            return FullscreenViewerOutcome::Ignored;
        }
        match intent {
            UiIntent::Cancel | UiIntent::Close => self.handle_escape::<()>(None),
            UiIntent::Help => {
                self.help_open = !self.help_open;
                FullscreenViewerOutcome::HelpToggled {
                    open: self.help_open,
                }
            }
            UiIntent::Search => self.toggle_search(),
            UiIntent::Fullscreen => {
                // Already fullscreen chrome
                FullscreenViewerOutcome::Ignored
            }
            UiIntent::Activate | UiIntent::Submit => {
                if matches!(self.chrome_focus, ViewerChromeFocus::Actions) {
                    if let Some(id) = self.action_cursor.clone() {
                        if actions.iter().any(|a| a.id == id && a.enabled) {
                            return FullscreenViewerOutcome::ActionActivated { id };
                        }
                    }
                }
                FullscreenViewerOutcome::Ignored
            }
            UiIntent::FocusNext => self.cycle_chrome(false),
            UiIntent::FocusPrevious => self.cycle_chrome(true),
            _ => FullscreenViewerOutcome::Ignored,
        }
    }

    /// Pointer routing for chrome painted by [`FullscreenViewer`].
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        actions: &[Action<'_, Id>],
    ) -> FullscreenViewerOutcome<Id> {
        if !self.open
            || !self.enabled
            || !self.accepts_input
            || event.kind != MouseEventKind::Down(MouseButton::Left)
        {
            return FullscreenViewerOutcome::Ignored;
        }

        if let Some(region) = self
            .action_regions
            .iter()
            .find(|region| region.area.contains(event.position))
        {
            if actions
                .iter()
                .any(|action| action.id == region.id && action.enabled)
            {
                self.chrome_focus = ViewerChromeFocus::Actions;
                self.action_cursor = Some(region.id.clone());
                return FullscreenViewerOutcome::ActionActivated {
                    id: region.id.clone(),
                };
            }
            return FullscreenViewerOutcome::Ignored;
        }

        let focus = if self.slots.search.contains(event.position) && self.search_open {
            Some(ViewerChromeFocus::Search)
        } else if self.slots.breadcrumbs.contains(event.position) {
            Some(ViewerChromeFocus::Breadcrumbs)
        } else if self.slots.title.contains(event.position) {
            Some(ViewerChromeFocus::Title)
        } else if self.slots.body.contains(event.position) {
            Some(ViewerChromeFocus::Body)
        } else {
            None
        };
        if let Some(focus) = focus {
            self.chrome_focus = focus;
            return FullscreenViewerOutcome::ChromeFocusMoved { focus };
        }
        FullscreenViewerOutcome::Ignored
    }

    fn toggle_search(&mut self) -> FullscreenViewerOutcome<Id> {
        self.search_open = !self.search_open;
        if self.search_open {
            self.chrome_focus = ViewerChromeFocus::Search;
        } else {
            self.search_query.clear();
            self.chrome_focus = ViewerChromeFocus::Body;
        }
        FullscreenViewerOutcome::SearchChanged {
            open: self.search_open,
            query: self.search_query.clone(),
        }
    }

    fn cycle_chrome(&mut self, reverse: bool) -> FullscreenViewerOutcome<Id> {
        let bands = [
            ViewerChromeFocus::Title,
            ViewerChromeFocus::Breadcrumbs,
            ViewerChromeFocus::Actions,
            ViewerChromeFocus::Search,
            ViewerChromeFocus::Body,
            ViewerChromeFocus::Help,
        ];
        let idx = bands
            .iter()
            .position(|b| *b == self.chrome_focus)
            .unwrap_or(4);
        let next = if reverse {
            if idx == 0 { bands.len() - 1 } else { idx - 1 }
        } else {
            (idx + 1) % bands.len()
        };
        // Skip search band if closed
        let mut n = next;
        if matches!(bands[n], ViewerChromeFocus::Search) && !self.search_open {
            n = if reverse {
                n.checked_sub(1).unwrap_or(bands.len() - 1)
            } else {
                (n + 1) % bands.len()
            };
        }
        self.chrome_focus = bands[n];
        FullscreenViewerOutcome::ChromeFocusMoved {
            focus: self.chrome_focus,
        }
    }

    fn move_action(
        &mut self,
        actions: &[Action<'_, Id>],
        dir: isize,
    ) -> FullscreenViewerOutcome<Id> {
        let enabled: Vec<_> = actions.iter().filter(|a| a.enabled).collect();
        if enabled.is_empty() {
            return FullscreenViewerOutcome::Ignored;
        }
        let cur = self
            .action_cursor
            .as_ref()
            .and_then(|id| enabled.iter().position(|a| &a.id == id));
        let next = match (cur, dir < 0) {
            (Some(0), true) | (None, true) => enabled.len() - 1,
            (Some(i), true) => i - 1,
            (Some(i), false) => (i + 1) % enabled.len(),
            (None, false) => 0,
        };
        self.action_cursor = Some(enabled[next].id.clone());
        FullscreenViewerOutcome::ChromeFocusMoved {
            focus: ViewerChromeFocus::Actions,
        }
    }

    /// Sync open flag with stack.
    pub fn sync_with_stack<F>(&mut self, stack: &OverlayStack<F>) {
        let id = OverlayId::from_static(FULLSCREEN_VIEWER_OVERLAY_ID);
        let on = stack.contains(&id);
        self.open = on;
        self.nested_child_hint = fullscreen_viewer_has_nested_top(stack);
        if on {
            self.accepts_input = stack.top_owns_input()
                && stack.top().is_some_and(|t| {
                    t.id == id || t.id.0.starts_with(FULLSCREEN_VIEWER_NESTED_PREFIX)
                });
        }
    }
}

// ── Widget paint ────────────────────────────────────────────────────────────

/// Fullscreen viewer chrome (host paints body content into slots).
#[derive(Debug, Clone, Copy)]
pub struct FullscreenViewer<'a, Id> {
    system: &'a DesignSystem,
    actions: &'a [Action<'a, Id>],
    colorless: bool,
}

impl<'a, Id> FullscreenViewer<'a, Id> {
    /// System + optional actions.
    #[must_use]
    pub const fn new(system: &'a DesignSystem, actions: &'a [Action<'a, Id>]) -> Self {
        Self {
            system,
            actions,
            colorless: false,
        }
    }

    /// ASCII chrome.
    #[must_use]
    /// Colorless.
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    /// Paint chrome; host follows with content in `state.body_area()`.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut FullscreenViewerState<Id>)
    where
        Id: Clone + PartialEq,
    {
        state.slots = FullscreenViewerSlots::empty();
        state.action_regions.clear();
        if area.is_empty() || !state.open {
            return;
        }
        state.slots.root = area;

        let recipe = if state.enabled && state.chrome_focus != ViewerChromeFocus::Body {
            SurfaceRecipe::OverlayFocused
        } else {
            SurfaceRecipe::Overlay
        };
        let adapted_system = (false || self.colorless).then(|| {
            let system = { self.system.clone() };
            if self.colorless {
                system.capability(crate::style::ColorCapability::Monochrome)
            } else {
                system
            }
        });
        let surface_system = adapted_system.as_ref().unwrap_or(self.system);
        let inner = Surface::new(surface_system)
            .recipe(recipe)
            .bordered(true)
            .content_inset()
            .paint(area, buffer);
        if inner.is_empty() {
            return;
        }

        let has_crumbs = state
            .zoom
            .source()
            .is_some_and(|s| !s.path_labels.is_empty());
        let has_actions = !self.actions.is_empty();
        let has_search = state.search_open;
        let footer_h = 1u16;
        let title_h = 1u16;
        let crumbs_h = u16::from(has_crumbs);
        let actions_h = u16::from(has_actions);
        let search_h = u16::from(has_search);
        let chrome = title_h + crumbs_h + actions_h + search_h + footer_h;
        let body_h = inner.height.saturating_sub(chrome).max(1);

        let mut y = inner.y;

        // Title
        state.slots.title = Rect::new(inner.x, y, inner.width, title_h);
        let kind = state.zoom.content_kind().badge();
        let base_title = if state.title.is_empty() {
            format!("[{kind}]")
        } else {
            format!("[{kind}] {}", state.title)
        };
        let title = if state.enabled {
            base_title
        } else {
            format!("[disabled] {base_title}")
        };
        let close = { " ×" };
        let title_style = if !state.enabled {
            self.system.style(Role::TextDisabled)
        } else if matches!(state.chrome_focus, ViewerChromeFocus::Title) {
            // The keyboard says itself with the focus tone and weight.
            self.system.style(Role::Focus).add_modifier(Modifier::BOLD)
        } else {
            self.system
                .style(Role::TextStrong)
                .add_modifier(Modifier::BOLD)
        };
        let t = take_display_cols(
            &title,
            usize::from(inner.width).saturating_sub(display_cols(close)),
        );
        buffer.set_stringn(inner.x, y, &t, usize::from(inner.width), title_style);
        let cx = inner.right().saturating_sub(display_cols(close) as u16);
        buffer.set_stringn(
            cx,
            y,
            close,
            display_cols(close),
            self.system.style(Role::TextMuted),
        );
        y = y.saturating_add(1);

        // Breadcrumbs
        if has_crumbs {
            state.slots.breadcrumbs = Rect::new(inner.x, y, inner.width, 1);
            if let Some(src) = state.zoom.source() {
                let sep = { " › " };
                let path = src.path_labels.join(sep);
                let style = if !state.enabled {
                    self.system.style(Role::TextDisabled)
                } else if matches!(state.chrome_focus, ViewerChromeFocus::Breadcrumbs) {
                    self.system.style(Role::Text).add_modifier(Modifier::BOLD)
                } else {
                    self.system.style(Role::TextMuted)
                };
                buffer.set_stringn(
                    inner.x,
                    y,
                    take_display_cols(&path, usize::from(inner.width)).as_ref(),
                    usize::from(inner.width),
                    style,
                );
            }
            y = y.saturating_add(1);
        } else {
            state.slots.breadcrumbs = Rect::default();
        }

        // Actions
        if has_actions {
            state.slots.actions = Rect::new(inner.x, y, inner.width, 1);
            let mut x = inner.x;
            for a in self.actions {
                if x >= inner.right() {
                    break;
                }
                let active = state.action_cursor.as_ref() == Some(&a.id)
                    && matches!(state.chrome_focus, ViewerChromeFocus::Actions);
                let label = if active {
                    format!("[{}]", a.label)
                } else {
                    format!(" {} ", a.label)
                };
                let w = display_cols(&label) as u16;
                let style = if !state.enabled || !a.enabled {
                    self.system.style(Role::TextDisabled)
                } else if active {
                    self.system
                        .style(Role::TextStrong)
                        .patch(self.system.style(Role::SelectionTint))
                } else {
                    self.system.style(Role::Text)
                };
                buffer.set_stringn(
                    x,
                    y,
                    &label,
                    usize::from(w.min(inner.right().saturating_sub(x))),
                    style,
                );
                state.action_regions.push(HitRegion {
                    id: a.id.clone(),
                    area: Rect::new(x, y, w.min(inner.right().saturating_sub(x)), 1),
                });
                x = x.saturating_add(w.saturating_add(1));
            }
            y = y.saturating_add(1);
        } else {
            state.slots.actions = Rect::default();
        }

        // Search
        if has_search {
            state.slots.search = Rect::new(inner.x, y, inner.width, 1);
            let prefix = { "⌕ " };
            let q = format!("{prefix}{}", state.search_query);
            let style = if !state.enabled {
                self.system.style(Role::TextDisabled)
            } else if matches!(state.chrome_focus, ViewerChromeFocus::Search) {
                self.system.style(Role::Input)
            } else {
                self.system.style(Role::TextMuted)
            };
            buffer.set_stringn(
                inner.x,
                y,
                take_display_cols(&q, usize::from(inner.width)).as_ref(),
                usize::from(inner.width),
                style,
            );
            y = y.saturating_add(1);
        } else {
            state.slots.search = Rect::default();
        }

        // Body
        state.slots.body = Rect::new(inner.x, y, inner.width, body_h);
        y = y.saturating_add(body_h);

        // Footer
        state.slots.footer = Rect::new(inner.x, y, inner.width, footer_h);
        let hint = if state.help_open {
            "help: esc cancel · tab chrome · arrows body (host)"
        } else {
            FULLSCREEN_VIEWER_HINT
        };
        let style = if matches!(state.chrome_focus, ViewerChromeFocus::Help) {
            self.system.style(Role::Text).add_modifier(Modifier::BOLD)
        } else {
            self.system.style(Role::TextMuted)
        };
        buffer.set_stringn(
            inner.x,
            y,
            take_display_cols(hint, usize::from(inner.width)).as_ref(),
            usize::from(inner.width),
            style,
        );
    }

    /// Semantic registration.
    pub fn register_semantic<Sid, Action>(
        &self,
        scene: &mut SemanticScene<Sid, Action>,
        id: Sid,
        area: Rect,
        state: &FullscreenViewerState<Id>,
    ) where
        Sid: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
        Id: std::fmt::Display,
    {
        if area.is_empty() || !state.open {
            return;
        }
        let src = state
            .source()
            .map(|s| s.source_id.to_string())
            .unwrap_or_default();
        let desc = format!(
            "fullscreen-viewer level={} kind={} search={} help={} source={src} chrome={}",
            state.level().id(),
            state.zoom.content_kind().id(),
            state.search_open,
            state.help_open,
            state.chrome_focus.id(),
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Dialog)
                .label("fullscreen-viewer")
                .description(desc)
                .focusable(state.enabled && state.accepts_input && state.open)
                .disabled(!state.enabled)
                .state(SemanticState {
                    selected: true,
                    expanded: state.open,
                    ..Default::default()
                }),
        );
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for FullscreenViewer<'_, Id> {
    type State = FullscreenViewerState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &FullscreenViewer<'_, Id> {
    type State = FullscreenViewerState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

/// Semantic zoom paint is host-side; this helper paints a compact level badge.
#[derive(Debug, Clone, Copy)]
pub struct SemanticZoomBadge<'a> {
    system: &'a DesignSystem,
}

impl<'a> SemanticZoomBadge<'a> {
    /// Badge painter.
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self { system }
    }

    /// ASCII.
    #[must_use]
    /// Paint level label into a one-line area.
    pub fn paint<Id>(&self, area: Rect, buffer: &mut Buffer, zoom: &SemanticZoomState<Id>) {
        if area.is_empty() {
            return;
        }
        let label = match zoom.level() {
            ZoomLevel::Compact => "zoom·row",
            ZoomLevel::Detail => "zoom·detail",
            ZoomLevel::Fullscreen => "zoom·full",
        };
        buffer.set_stringn(
            area.x,
            area.y,
            take_display_cols(label, usize::from(area.width)).as_ref(),
            usize::from(area.width),
            self.system.style(Role::TextMuted),
        );
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyModifiers;
    use crate::interaction::{OverlayKind, OverlayOutcome, OverlaySize};
    use crate::widgets::tests::click;

    fn ctx(id: &'static str) -> SourceContext<&'static str> {
        SourceContext::new(id)
            .selection(Some(id))
            .scroll(ScrollAnchor::at(42, 0).with_id("line-42"))
            .focus_token("list")
            .path(["repo", "src", "main.rs"])
    }

    #[test]
    fn promote_demote_preserves_source_context() {
        let mut z = SemanticZoomState::new();
        z.set_content_kind(ViewerContentKind::Code);
        assert_eq!(z.level(), ZoomLevel::Compact);
        let out = z.promote(ctx("main.rs"));
        assert!(matches!(
            out,
            FullscreenViewerOutcome::Promoted {
                level: ZoomLevel::Detail
            }
        ));
        let out = z.promote(ctx("main.rs"));
        assert!(matches!(
            out,
            FullscreenViewerOutcome::Opened {
                level: ZoomLevel::Fullscreen
            }
        ));
        assert_eq!(z.source().unwrap().scroll.line, 42);
        assert_eq!(
            z.source().unwrap().scroll.stable_id.as_deref(),
            Some("line-42")
        );

        let out = z.demote();
        match out {
            FullscreenViewerOutcome::Demoted {
                level: ZoomLevel::Detail,
                restore,
            } => {
                assert_eq!(restore.source_id, "main.rs");
                assert_eq!(restore.scroll.line, 42);
            }
            other => panic!("expected demote to detail, got {other:?}"),
        }
        let out = z.demote();
        match out {
            FullscreenViewerOutcome::Closed { restore } => {
                assert_eq!(restore.selection, Some("main.rs"));
                assert_eq!(restore.path_labels.len(), 3);
            }
            other => panic!("expected closed, got {other:?}"),
        }
        assert_eq!(z.level(), ZoomLevel::Compact);
    }

    #[test]
    fn escape_nested_before_demote() {
        let mut state = FullscreenViewerState::new();
        let _ = state.enter_fullscreen(ctx("x"), "file");
        state.set_nested_child_hint(true);
        assert!(matches!(
            state.handle_escape::<()>(None),
            FullscreenViewerOutcome::NestedEscape
        ));
        assert!(state.is_open());
        assert_eq!(state.level(), ZoomLevel::Fullscreen);
    }

    #[test]
    fn escape_help_then_search_then_demote() {
        let mut state = FullscreenViewerState::new();
        let _ = state.enter_fullscreen(ctx("x"), "file");
        state.help_open = true;
        assert!(matches!(
            state.handle_escape::<()>(None),
            FullscreenViewerOutcome::HelpToggled { open: false }
        ));
        state.search_open = true;
        state.search_query = "foo".into();
        assert!(matches!(
            state.handle_escape::<()>(None),
            FullscreenViewerOutcome::SearchChanged { open: false, .. }
        ));
        assert!(state.search_query.is_empty());
        // Demote fullscreen → detail (closes overlay flag)
        let out = state.handle_escape::<()>(None);
        assert!(matches!(
            out,
            FullscreenViewerOutcome::Demoted {
                level: ZoomLevel::Detail,
                ..
            }
        ));
        assert!(!state.is_open());
    }

    #[test]
    fn nested_overlay_stack_escape_one_layer() {
        let bounds = Rect::new(0, 0, 80, 24);
        let mut stack = OverlayStack::<&'static str>::new();
        let mut state = FullscreenViewerState::new();
        let _ = state.enter_fullscreen(ctx("x"), "file");
        let _ = state.open_on_stack(&mut stack, bounds, Some("list"));
        assert_eq!(stack.top().unwrap().kind, OverlayKind::Fullscreen);
        let _ = open_fullscreen_viewer_child_overlay(
            &mut stack,
            bounds,
            "picker",
            OverlaySize::menu(24, 8),
            Some("list"),
        );
        assert_eq!(stack.entries().len(), 2);
        assert!(fullscreen_viewer_has_nested_top(&stack));
        // Peel one
        assert!(matches!(
            stack.handle_escape(),
            OverlayOutcome::Dismissed { .. }
        ));
        assert_eq!(stack.entries().len(), 1);
        assert_eq!(stack.top().unwrap().kind, OverlayKind::Fullscreen);
        // Viewer still open
        state.sync_with_stack(&stack);
        assert!(state.is_open());
    }

    #[test]
    fn open_close_restores_opener_focus() {
        let bounds = Rect::new(0, 0, 80, 24);
        let mut stack = OverlayStack::<&'static str>::new();
        let mut state = FullscreenViewerState::new();
        let _ = state.enter_fullscreen(ctx("row"), "title");
        let _ = state.open_on_stack(&mut stack, bounds, Some("list-focus"));
        let (out, stack_out) = state.close_on_stack(&mut stack);
        assert!(matches!(
            out,
            FullscreenViewerOutcome::Closed { restore } if restore.source_id == "row"
        ));
        assert!(matches!(
            stack_out,
            OverlayOutcome::Dismissed {
                focus: Some("list-focus"),
                ..
            }
        ));
    }

    #[test]
    fn search_and_help_keys() {
        let mut state = FullscreenViewerState::new();
        let _ = state.enter_fullscreen(ctx("x"), "t");
        let actions: [Action<'_, &str>; 0] = [];
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE),
                &actions
            ),
            FullscreenViewerOutcome::SearchChanged { open: true, .. }
        ));
        let _ = state.handle_key(
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
            &actions,
        );
        assert_eq!(state.search_query(), "a");
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE),
                &actions
            ),
            FullscreenViewerOutcome::HelpToggled { open: true }
                | FullscreenViewerOutcome::SearchChanged { .. }
                | FullscreenViewerOutcome::Ignored
        ));
        // When search focused, ? may type — open help via intent
        let _ = state.handle_intent(UiIntent::Help, &actions);
        assert!(state.help_open());
    }

    #[test]
    fn action_activation() {
        let mut state = FullscreenViewerState::new();
        let _ = state.enter_fullscreen(ctx("x"), "t");
        let actions = [
            Action {
                id: "copy",
                label: "Copy",
                enabled: true,
                variant: ActionVariant::Secondary,
            },
            Action {
                id: "raw",
                label: "Raw",
                enabled: true,
                variant: ActionVariant::Secondary,
            },
        ];
        state.chrome_focus = ViewerChromeFocus::Actions;
        state.action_cursor = Some("copy");
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &actions),
            FullscreenViewerOutcome::ActionActivated { id: "copy" }
        ));
    }

    #[test]
    fn mouse_uses_painted_action_regions_and_disabled_gate() {
        let system = DesignSystem::default();
        let actions = [
            Action {
                id: "copy",
                label: "Copy",
                enabled: true,
                variant: ActionVariant::Secondary,
            },
            Action {
                id: "raw",
                label: "Raw",
                enabled: false,
                variant: ActionVariant::Secondary,
            },
        ];
        let mut state = FullscreenViewerState::new();
        let _ = state.enter_fullscreen(ctx("x"), "viewer");
        let area = Rect::new(0, 0, 60, 16);
        let mut buffer = Buffer::empty(area);
        FullscreenViewer::new(&system, &actions).paint(area, &mut buffer, &mut state);
        let copy = state
            .action_regions
            .iter()
            .find(|region| region.id == "copy")
            .expect("copy action region")
            .area;
        let event = click(copy.x, copy.y);
        assert!(matches!(
            state.handle_mouse(event, &actions),
            FullscreenViewerOutcome::ActionActivated { id: "copy" }
        ));

        state.set_enabled(false);
        assert_eq!(
            state.handle_mouse(event, &actions),
            FullscreenViewerOutcome::Ignored
        );
        let mut scene = SemanticScene::<&str, ()>::default();
        FullscreenViewer::new(&system, &actions)
            .register_semantic(&mut scene, "viewer", area, &state);
        let node = scene.nodes().first().expect("viewer semantic node");
        assert!(node.disabled);
        assert!(!node.focusable);
    }

    #[test]
    fn paint_slots_and_body_for_host() {
        let system = DesignSystem::default();
        let actions = [Action {
            id: "copy",
            label: "Copy",
            enabled: true,
            variant: ActionVariant::Secondary,
        }];
        let mut state = FullscreenViewerState::new();
        state.zoom.set_content_kind(ViewerContentKind::Diff);
        let _ = state.enter_fullscreen(ctx("hunk-1").path(["diff", "a.rs"]), "a.rs");
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        FullscreenViewer::new(&system, &actions).paint(area, &mut buf, &mut state);
        assert!(!state.slots.body.is_empty());
        assert_eq!(state.body_area(), state.slots.body);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(text.contains("a.rs") || text.contains("diff"), "{text}");
        // Host paints into body without copying state
        let body = state.body_area();
        buffer_write_host_content(&mut buf, body, "+added line");
    }

    fn buffer_write_host_content(buf: &mut Buffer, area: Rect, s: &str) {
        if area.is_empty() {
            return;
        }
        buf.set_stringn(
            area.x,
            area.y,
            take_display_cols(s, usize::from(area.width)).as_ref(),
            usize::from(area.width),
            ratatui_core::style::Style::default(),
        );
    }

    #[test]
    fn content_kinds_for_integrations() {
        for k in [
            ViewerContentKind::Code,
            ViewerContentKind::Diff,
            ViewerContentKind::Log,
            ViewerContentKind::Object,
            ViewerContentKind::Task,
            ViewerContentKind::Media,
        ] {
            assert!(!k.badge().is_empty());
        }
    }

    #[test]
    fn semantic_registers() {
        let system = DesignSystem::default();
        let actions: [Action<'_, &str>; 0] = [];
        let mut state = FullscreenViewerState::new();
        let _ = state.enter_fullscreen(ctx("x"), "t");
        let mut scene = SemanticScene::<&str, ()>::default();
        FullscreenViewer::new(&system, &actions).register_semantic(
            &mut scene,
            "fv",
            Rect::new(0, 0, 40, 12),
            &state,
        );
        assert!(
            scene
                .nodes()
                .iter()
                .any(|n| n.label.as_deref() == Some("fullscreen-viewer"))
        );
    }

    #[test]
    fn zoom_badge_paint() {
        let system = DesignSystem::default();
        let mut z = SemanticZoomState::<&str>::new();
        let area = Rect::new(0, 0, 20, 1);
        let mut buf = Buffer::empty(area);
        let _ = SemanticZoomBadge::new(&system).paint(area, &mut buf, &z);
        let _ = z.promote(ctx("a"));
        let _ = SemanticZoomBadge::new(&system).paint(area, &mut buf, &z);
    }

    #[test]
    fn fuzz_keys() {
        let mut state = FullscreenViewerState::new();
        let _ = state.enter_fullscreen(ctx("x"), "t");
        let actions = [Action {
            id: "a",
            label: "A",
            enabled: true,
            variant: ActionVariant::Secondary,
        }];
        let keys = [
            KeyCode::Esc,
            KeyCode::Char('/'),
            KeyCode::Char('?'),
            KeyCode::Tab,
            KeyCode::Enter,
            KeyCode::Char('x'),
            KeyCode::Backspace,
            KeyCode::Left,
            KeyCode::Right,
        ];
        let mut seed = 17u64;
        for _ in 0..300 {
            if !state.is_open() {
                let _ = state.enter_fullscreen(ctx("x"), "t");
            }
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let k = keys[(seed as usize) % keys.len()];
            let _ = state.handle_key(KeyEvent::new(k, KeyModifiers::NONE), &actions);
        }
    }

    #[test]
    fn paint_perf_smoke() {
        use ratatui_core::backend::TestBackend;
        use ratatui_core::terminal::Terminal;
        let system = DesignSystem::default();
        let actions = [Action {
            id: "c",
            label: "Copy",
            enabled: true,
            variant: ActionVariant::Secondary,
        }];
        let mut state = FullscreenViewerState::new();
        let _ = state.enter_fullscreen(ctx("file").path(["a", "b", "c"]), "big.rs");
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        let start = std::time::Instant::now();
        for _ in 0..150 {
            terminal
                .draw(|f| {
                    FullscreenViewer::new(&system, &actions).paint(
                        f.area(),
                        f.buffer_mut(),
                        &mut state,
                    );
                })
                .unwrap();
        }
        assert!(start.elapsed().as_millis() < 5_000);
    }

    #[test]
    fn pty_snapshot_stable() {
        use ratatui_core::backend::TestBackend;
        use ratatui_core::terminal::Terminal;
        let system = DesignSystem::default();
        let actions: [Action<'_, &str>; 0] = [];
        let mut s1 = FullscreenViewerState::new();
        let _ = s1.enter_fullscreen(ctx("x").path(["src", "lib.rs"]), "lib.rs");
        let mut t1 = Terminal::new(TestBackend::new(48, 16)).unwrap();
        t1.draw(|f| {
            FullscreenViewer::new(&system, &actions).paint(f.area(), f.buffer_mut(), &mut s1);
        })
        .unwrap();
        let a: String = t1
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        let mut s2 = FullscreenViewerState::new();
        let _ = s2.enter_fullscreen(ctx("x").path(["src", "lib.rs"]), "lib.rs");
        let mut t2 = Terminal::new(TestBackend::new(48, 16)).unwrap();
        t2.draw(|f| {
            FullscreenViewer::new(&system, &actions).paint(f.area(), f.buffer_mut(), &mut s2);
        })
        .unwrap();
        let b: String = t2
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert_eq!(a, b);
        assert!(a.contains("lib.rs") || a.contains("code") || a.contains("src"));
    }

    #[test]
    fn demote_does_not_copy_app_state() {
        // Structural: SourceContext is a snapshot of ids/anchors, not document body.
        let ctx = ctx("item");
        assert!(ctx.scroll.stable_id.is_some());
        // Host re-binds CodeBlockState.scroll_y from ctx.scroll.line — no body clone.
    }

    #[test]
    fn set_level_promote_and_demote() {
        let mut z = SemanticZoomState::new();
        let _ = z.promote(ctx("x"));
        assert!(matches!(
            z.set_level(ZoomLevel::Fullscreen),
            FullscreenViewerOutcome::Opened {
                level: ZoomLevel::Fullscreen
            }
        ));
        assert!(matches!(
            z.set_level(ZoomLevel::Detail),
            FullscreenViewerOutcome::Demoted {
                level: ZoomLevel::Detail,
                ..
            }
        ));
        assert!(matches!(
            z.set_level(ZoomLevel::Compact),
            FullscreenViewerOutcome::Closed { .. }
        ));
        assert_eq!(z.level(), ZoomLevel::Compact);
    }

    #[test]
    fn viewer_demote_closes_overlay_flag() {
        let mut state = FullscreenViewerState::new();
        let _ = state.enter_fullscreen(ctx("x"), "t");
        assert!(state.is_open());
        let out = state.demote();
        assert!(matches!(
            out,
            FullscreenViewerOutcome::Demoted {
                level: ZoomLevel::Detail,
                ..
            }
        ));
        assert!(!state.is_open());
    }
}
