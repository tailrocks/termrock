// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **Offline** / **ReconnectingState** — specialized connectivity surfaces.
//!
//! **Mission.** Remote sessions, databases, agents, and services: connection
//! target, last successful time, retry state, queued actions, offline
//! capabilities, and manual actions. Distinguishes disconnected, reconnecting,
//! authentication required, and server unavailable. Preserves local drafts and
//! selection. Unobtrusive banner **or** full error surface. Integrates with
//! [`StatusBar`](super::StatusBar) and [`NotificationCenter`](super::NotificationCenter).
//!
//! Research: remote IDEs, database clients, SSH tools, collaborative agents.
#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Modifier,
    widgets::Widget,
};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    interaction::{
        SemanticNode, SemanticRole, SemanticScene, SemanticState, UiIntent, default_button_intent,
    },
    layout::{Center, CenterAxis, FlexSize, Stack, center_line_x},
    style::{DesignSystem, GlyphSet, Role},
    text::{display_cols, take_display_cols},
    widgets::{
        ActivityPhase, Button, ButtonState, ButtonVariant, NotificationItem, SemanticStatus,
        StatusIndicator, StatusKind, StatusRegion, StatusSlot, ToastKind, ToastPriority,
    },
};

// ── Phase ───────────────────────────────────────────────────────────────────

/// Connectivity lifecycle for a remote target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ConnectivityPhase {
    /// Connected and healthy.
    Online,
    /// Link down; not yet retrying (or gave up).
    #[default]
    Disconnected,
    /// Active reconnect attempt in flight.
    Reconnecting,
    /// Auth challenge / expired credentials.
    AuthRequired,
    /// Reachable path but server/process unavailable.
    ServerUnavailable,
}

/// Offline capabilities listed before the panel defers to `+N more`.
const OFFLINE_CAPS_SHOWN: usize = 4;

impl ConnectivityPhase {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Online => "online",
            Self::Disconnected => "disconnected",
            Self::Reconnecting => "reconnecting",
            Self::AuthRequired => "auth-required",
            Self::ServerUnavailable => "server-unavailable",
        }
    }

    /// Short verb for chrome.
    #[must_use]
    pub const fn verb(self) -> &'static str {
        match self {
            Self::Online => "connected",
            Self::Disconnected => "offline",
            Self::Reconnecting => "reconnecting",
            Self::AuthRequired => "auth required",
            Self::ServerUnavailable => "unavailable",
        }
    }

    /// Non-color glyph (Unicode).
    #[must_use]
    pub const fn glyph_unicode(self) -> &'static str {
        match self {
            Self::Online => "●",
            Self::Disconnected => "○",
            Self::Reconnecting => "◌",
            Self::AuthRequired => "⚿",
            Self::ServerUnavailable => "×",
        }
    }

    /// ASCII glyph.
    #[must_use]
    pub const fn glyph_ascii(self) -> &'static str {
        match self {
            Self::Online => "*",
            Self::Disconnected => "o",
            Self::Reconnecting => "~",
            Self::AuthRequired => "!",
            Self::ServerUnavailable => "x",
        }
    }

    /// Glyph for capability.
    #[must_use]
    pub const fn glyph(self, ascii: bool) -> &'static str {
        if ascii {
            self.glyph_ascii()
        } else {
            self.glyph_unicode()
        }
    }

    /// Shared status vocabulary.
    #[must_use]
    pub const fn semantic_status(self) -> SemanticStatus {
        match self {
            Self::Online => SemanticStatus::Online,
            Self::Disconnected => SemanticStatus::Offline,
            Self::Reconnecting => SemanticStatus::Running,
            Self::AuthRequired => SemanticStatus::Warning,
            Self::ServerUnavailable => SemanticStatus::Failed,
        }
    }

    /// Spinner phase when animating reconnect.
    #[must_use]
    pub const fn activity_phase(self) -> Option<ActivityPhase> {
        match self {
            Self::Reconnecting => Some(ActivityPhase::Reconnecting),
            Self::Online => None,
            Self::Disconnected | Self::AuthRequired | Self::ServerUnavailable => {
                Some(ActivityPhase::Waiting)
            }
        }
    }

    /// Whether local offline work is the expected mode.
    #[must_use]
    pub const fn is_offline_like(self) -> bool {
        !matches!(self, Self::Online)
    }

    /// Toast/notification kind for center ingest.
    #[must_use]
    pub const fn toast_kind(self) -> ToastKind {
        match self {
            Self::Online => ToastKind::Success,
            Self::Disconnected => ToastKind::Warning,
            Self::Reconnecting => ToastKind::Progress,
            Self::AuthRequired => ToastKind::Warning,
            Self::ServerUnavailable => ToastKind::Error,
        }
    }
}

// ── Presentation ────────────────────────────────────────────────────────────

/// How to paint connectivity chrome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ConnectivityPresentation {
    /// Single-line unobtrusive banner (default for transient blips).
    #[default]
    Banner,
    /// Full recoverable surface (long outage / auth / unavailable).
    Full,
}

impl ConnectivityPresentation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Banner => "banner",
            Self::Full => "full",
        }
    }
}

// ── Queued / capabilities ───────────────────────────────────────────────────

/// One action waiting for connectivity (host-owned id/label).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QueuedConnectivityAction {
    /// Stable id.
    pub id: String,
    /// Human label.
    pub label: String,
}

impl QueuedConnectivityAction {
    /// Construct.
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

/// Capability available while offline (or not).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OfflineCapability {
    /// Label (e.g. "Edit drafts").
    pub label: String,
    /// Whether usable offline.
    pub available: bool,
}

impl OfflineCapability {
    /// Construct.
    #[must_use]
    pub fn new(label: impl Into<String>, available: bool) -> Self {
        Self {
            label: label.into(),
            available,
        }
    }
}

// ── Outcomes / focus ────────────────────────────────────────────────────────

/// Focusable control on full surface / banner actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ConnectivityFocus {
    /// Nothing.
    #[default]
    None,
    /// Retry / reconnect now.
    Retry,
    /// Open auth / re-login.
    Authenticate,
    /// Continue with offline capabilities.
    WorkOffline,
    /// View queued actions.
    ViewQueue,
    /// Dismiss banner (not the underlying problem).
    Dismiss,
}

/// Outcomes (effects host-owned).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ConnectivityOutcome {
    /// No-op.
    Ignored,
    /// Manual retry requested.
    RetryNow,
    /// Open authentication flow.
    Authenticate,
    /// User chose offline mode.
    WorkOffline,
    /// Show queue of pending actions.
    ViewQueue,
    /// Dismiss chrome (banner only).
    Dismiss,
}

// ── ReconnectingState ───────────────────────────────────────────────────────

/// Host-owned connectivity model for one remote target.
///
/// Paint via [`OfflineBanner`] or [`OfflineSurface`]. Project into StatusBar /
/// NotificationCenter with the `to_*` helpers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconnectingState {
    phase: ConnectivityPhase,
    /// Connection target (host, agent id, db dsn short form).
    target: String,
    /// Last successful contact (host clock seconds).
    last_success_secs: Option<u64>,
    /// Current host clock (for relative "last ok" paint).
    now_secs: u64,
    /// Retry attempt (1-based while reconnecting).
    attempt: u32,
    /// Optional max attempts.
    max_attempts: Option<u32>,
    /// Seconds until next auto-retry (if scheduled).
    next_retry_in_secs: Option<u64>,
    /// Actions queued while offline.
    queued: Vec<QueuedConnectivityAction>,
    /// What still works offline.
    offline_caps: Vec<OfflineCapability>,
    /// Local drafts preserved.
    drafts_preserved: bool,
    /// Selection / cursor preserved.
    selection_preserved: bool,
    /// Auto-retry enabled.
    auto_retry: bool,
    /// Preferred presentation (banner vs full).
    presentation: ConnectivityPresentation,
    /// Focus for interactive chrome.
    focus: ConnectivityFocus,
    /// Banner dismissed (full outage may still show StatusBar).
    banner_dismissed: bool,
    retry_btn: ButtonState,
    auth_btn: ButtonState,
    offline_btn: ButtonState,
    queue_btn: ButtonState,
}

impl Default for ReconnectingState {
    fn default() -> Self {
        Self::new("remote")
    }
}

impl ReconnectingState {
    /// Online-ish idle for `target`.
    #[must_use]
    pub fn new(target: impl Into<String>) -> Self {
        Self {
            phase: ConnectivityPhase::Online,
            target: target.into(),
            last_success_secs: None,
            now_secs: 0,
            attempt: 0,
            max_attempts: None,
            next_retry_in_secs: None,
            queued: Vec::new(),
            offline_caps: Vec::new(),
            drafts_preserved: true,
            selection_preserved: true,
            auto_retry: true,
            presentation: ConnectivityPresentation::Banner,
            focus: ConnectivityFocus::None,
            banner_dismissed: false,
            retry_btn: ButtonState::new(),
            auth_btn: ButtonState::new(),
            offline_btn: ButtonState::new(),
            queue_btn: ButtonState::new(),
        }
    }

    /// Target.
    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }

    /// Phase.
    #[must_use]
    pub const fn phase(&self) -> ConnectivityPhase {
        self.phase
    }

    /// Set phase.
    pub fn set_phase(&mut self, phase: ConnectivityPhase) {
        self.phase = phase;
        if matches!(phase, ConnectivityPhase::Online) {
            self.attempt = 0;
            self.next_retry_in_secs = None;
            self.banner_dismissed = false;
        }
        if matches!(phase, ConnectivityPhase::Reconnecting) && self.attempt == 0 {
            self.attempt = 1;
        }
    }

    /// Mark disconnected and optionally start auto-retry presentation as reconnecting.
    pub fn mark_disconnected(&mut self) {
        self.phase = ConnectivityPhase::Disconnected;
        self.attempt = 0;
    }

    /// Enter reconnecting with attempt counter.
    pub fn begin_reconnect(&mut self, attempt: u32) {
        self.phase = ConnectivityPhase::Reconnecting;
        self.attempt = attempt.max(1);
        self.banner_dismissed = false;
    }

    /// Auth required.
    pub fn require_auth(&mut self) {
        self.phase = ConnectivityPhase::AuthRequired;
        self.banner_dismissed = false;
        self.presentation = ConnectivityPresentation::Full;
    }

    /// Server unavailable.
    pub fn mark_server_unavailable(&mut self) {
        self.phase = ConnectivityPhase::ServerUnavailable;
        self.banner_dismissed = false;
    }

    /// Restored online; record last success.
    pub fn mark_online(&mut self, now_secs: u64) {
        self.phase = ConnectivityPhase::Online;
        self.last_success_secs = Some(now_secs);
        self.now_secs = now_secs;
        self.attempt = 0;
        self.next_retry_in_secs = None;
        self.banner_dismissed = false;
    }

    /// Host clock.
    pub fn set_now_secs(&mut self, now: u64) {
        self.now_secs = now;
    }

    /// Last success timestamp.
    pub fn set_last_success_secs(&mut self, secs: Option<u64>) {
        self.last_success_secs = secs;
    }

    /// Last success.
    #[must_use]
    pub const fn last_success_secs(&self) -> Option<u64> {
        self.last_success_secs
    }

    /// Max attempts.
    pub fn set_max_attempts(&mut self, max: Option<u32>) {
        self.max_attempts = max;
    }

    /// Next retry countdown.
    pub fn set_next_retry_in_secs(&mut self, secs: Option<u64>) {
        self.next_retry_in_secs = secs;
    }

    /// Attempt.
    #[must_use]
    pub const fn attempt(&self) -> u32 {
        self.attempt
    }

    /// Queue replace.
    pub fn set_queued(&mut self, queued: Vec<QueuedConnectivityAction>) {
        self.queued = queued;
    }

    /// Push queued action.
    pub fn enqueue(&mut self, action: QueuedConnectivityAction) {
        self.queued.push(action);
    }

    /// Queued.
    #[must_use]
    pub fn queued(&self) -> &[QueuedConnectivityAction] {
        &self.queued
    }

    /// Offline capabilities.
    pub fn set_offline_capabilities(&mut self, caps: Vec<OfflineCapability>) {
        self.offline_caps = caps;
    }

    /// Caps.
    #[must_use]
    pub fn offline_capabilities(&self) -> &[OfflineCapability] {
        &self.offline_caps
    }

    /// Drafts preserved (default true).
    pub fn set_drafts_preserved(&mut self, on: bool) {
        self.drafts_preserved = on;
    }

    /// Selection preserved (default true).
    pub fn set_selection_preserved(&mut self, on: bool) {
        self.selection_preserved = on;
    }

    /// Drafts?
    #[must_use]
    pub const fn drafts_preserved(&self) -> bool {
        self.drafts_preserved
    }

    /// Selection?
    #[must_use]
    pub const fn selection_preserved(&self) -> bool {
        self.selection_preserved
    }

    /// Auto retry flag.
    pub fn set_auto_retry(&mut self, on: bool) {
        self.auto_retry = on;
    }

    /// Presentation.
    pub fn set_presentation(&mut self, p: ConnectivityPresentation) {
        self.presentation = p;
    }

    /// Presentation.
    #[must_use]
    pub const fn presentation(&self) -> ConnectivityPresentation {
        self.presentation
    }

    /// Force ASCII glyphs.
    /// Banner dismissed?
    #[must_use]
    pub const fn banner_dismissed(&self) -> bool {
        self.banner_dismissed
    }

    /// Focus.
    #[must_use]
    pub const fn focus(&self) -> ConnectivityFocus {
        self.focus
    }

    /// Set keyboard focus target for recovery actions.
    pub fn set_focus(&mut self, f: ConnectivityFocus) {
        self.focus = f;
    }

    // ── Formatters (StatusBar / NotificationCenter) ─────────────────────────

    /// Compact StatusBar connection content (no glyph; StatusKind supplies glyph).
    #[must_use]
    pub fn status_bar_content(&self) -> String {
        match self.phase {
            ConnectivityPhase::Online => format!("{} ok", self.target),
            ConnectivityPhase::Disconnected => format!("{} offline", self.target),
            ConnectivityPhase::Reconnecting => {
                if self.max_attempts.is_some() {
                    format!(
                        "{} reconnect {}/{}",
                        self.target,
                        self.attempt,
                        self.max_attempts.unwrap_or(0)
                    )
                } else {
                    format!("{} reconnect ·{}", self.target, self.attempt)
                }
            }
            ConnectivityPhase::AuthRequired => format!("{} auth", self.target),
            ConnectivityPhase::ServerUnavailable => format!("{} down", self.target),
        }
    }

    /// Build a right-region connection [`StatusSlot`] (host keeps `content` alive).
    ///
    /// Prefer storing `status_bar_content()` in host state, then:
    /// `StatusSlot::connection(id, content_ref).…`
    #[must_use]
    pub fn status_slot_template<'a, Id: Clone>(
        &self,
        id: Id,
        content: &'a str,
    ) -> StatusSlot<'a, Id> {
        StatusSlot::connection(id, content)
            .region(StatusRegion::Right)
            .kind(StatusKind::Connection)
            .semantic(self.phase.semantic_status())
    }

    /// Banner one-liner.
    #[must_use]
    pub fn banner_line(&self, ascii: bool) -> String {
        let (glyph, head, meta) = self.banner_parts(ascii);
        format!("{glyph} {head}{meta}")
    }

    /// The banner split into its tiers: state glyph, the sentence, the counts.
    ///
    /// One string painted in the phase role turns the whole banner into an
    /// alarm; the phase belongs to the glyph and the counts read quietly.
    fn banner_parts(&self, ascii: bool) -> (&'static str, String, String) {
        let head = format!("{} · {}", self.phase.verb(), self.target);
        let mut meta = String::new();
        if matches!(self.phase, ConnectivityPhase::Reconnecting) && self.attempt > 0 {
            meta.push_str(&format!(" · try {}", self.attempt));
        }
        if let Some(n) = self.next_retry_in_secs {
            meta.push_str(&format!(" · next {n}s"));
        }
        if !self.queued.is_empty() {
            meta.push_str(&format!(" · {} queued", self.queued.len()));
        }
        (self.phase.glyph(ascii), head, meta)
    }

    /// Relative last-success phrase.
    #[must_use]
    pub fn last_success_label(&self) -> Option<String> {
        let last = self.last_success_secs?;
        if self.now_secs == 0 || self.now_secs < last {
            return Some("last ok: known".into());
        }
        let delta = self.now_secs.saturating_sub(last);
        let text = if delta < 60 {
            format!("last ok: {delta}s ago")
        } else if delta < 3600 {
            format!("last ok: {}m ago", delta / 60)
        } else if delta < 86_400 {
            format!("last ok: {}h ago", delta / 3600)
        } else {
            format!("last ok: {}d ago", delta / 86_400)
        };
        Some(text)
    }

    /// Shared status vocab.
    #[must_use]
    pub const fn semantic_status(&self) -> SemanticStatus {
        self.phase.semantic_status()
    }

    /// Notification center item (dedup by target+phase).
    #[must_use]
    pub fn to_notification_item(&self, id: impl Into<String>) -> NotificationItem {
        let mut item = NotificationItem::new(id, self.banner_line(false), self.phase.toast_kind())
            .title(format!("Connection · {}", self.target))
            .source("connectivity")
            .group("connectivity")
            .dedup_key(format!("conn:{}:{}", self.target, self.phase.id()));
        item.priority = match self.phase {
            ConnectivityPhase::ServerUnavailable | ConnectivityPhase::AuthRequired => {
                ToastPriority::High
            }
            ConnectivityPhase::Reconnecting | ConnectivityPhase::Disconnected => {
                ToastPriority::Normal
            }
            ConnectivityPhase::Online => ToastPriority::Low,
        };
        item.created_at_secs = self.now_secs;
        if matches!(self.phase, ConnectivityPhase::Reconnecting) {
            let pct = self
                .max_attempts
                .map(|m| ((self.attempt.min(m) as u16 * 100) / m.max(1) as u16).min(99) as u8);
            item.progress = pct;
        }
        if matches!(
            self.phase,
            ConnectivityPhase::Disconnected
                | ConnectivityPhase::Reconnecting
                | ConnectivityPhase::ServerUnavailable
        ) {
            item = item.action("retry", "Retry");
        }
        if matches!(self.phase, ConnectivityPhase::AuthRequired) {
            item = item.action("auth", "Sign in");
        }
        if !self.queued.is_empty() {
            item = item.action("queue", "View queue");
        }
        item
    }

    /// Whether banner should paint (not online, not dismissed).
    #[must_use]
    pub fn should_show_banner(&self) -> bool {
        self.phase.is_offline_like()
            && !self.banner_dismissed
            && matches!(self.presentation, ConnectivityPresentation::Banner)
    }

    /// Whether full surface should paint.
    #[must_use]
    pub fn should_show_full(&self) -> bool {
        self.phase.is_offline_like() && matches!(self.presentation, ConnectivityPresentation::Full)
    }

    /// Keyboard handling for banner/full actions.
    pub fn handle_key(&mut self, key: KeyEvent) -> ConnectivityOutcome {
        if !key.is_press() || !self.phase.is_offline_like() {
            return ConnectivityOutcome::Ignored;
        }
        if matches!(key.code, KeyCode::Char('r') | KeyCode::Char('R')) {
            return ConnectivityOutcome::RetryNow;
        }
        if matches!(key.code, KeyCode::Char('a') | KeyCode::Char('A'))
            && matches!(self.phase, ConnectivityPhase::AuthRequired)
        {
            return ConnectivityOutcome::Authenticate;
        }
        if matches!(key.code, KeyCode::Char('o') | KeyCode::Char('O')) {
            return ConnectivityOutcome::WorkOffline;
        }
        if matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q')) && !self.queued.is_empty() {
            return ConnectivityOutcome::ViewQueue;
        }
        if matches!(key.code, KeyCode::Esc) {
            self.banner_dismissed = true;
            return ConnectivityOutcome::Dismiss;
        }
        if matches!(key.code, KeyCode::Tab) {
            self.cycle_focus(key.modifiers.contains(KeyModifiers::SHIFT));
            return ConnectivityOutcome::Ignored;
        }
        if matches!(default_button_intent(key), Some(UiIntent::Activate))
            || matches!(key.code, KeyCode::Enter | KeyCode::Char(' '))
        {
            return self.activate_focus();
        }
        ConnectivityOutcome::Ignored
    }

    fn cycle_focus(&mut self, reverse: bool) {
        let targets = self.focus_targets();
        if targets.is_empty() {
            return;
        }
        let cur = targets.iter().position(|&t| t == self.focus);
        self.focus = match (cur, reverse) {
            (None, false) => targets[0],
            (None, true) => *targets.last().unwrap_or(&targets[0]),
            (Some(i), false) => targets[(i + 1) % targets.len()],
            (Some(i), true) => targets[(i + targets.len() - 1) % targets.len()],
        };
    }

    fn focus_targets(&self) -> Vec<ConnectivityFocus> {
        let mut t = Vec::new();
        if !matches!(self.phase, ConnectivityPhase::AuthRequired) {
            t.push(ConnectivityFocus::Retry);
        }
        if matches!(self.phase, ConnectivityPhase::AuthRequired) {
            t.push(ConnectivityFocus::Authenticate);
        }
        t.push(ConnectivityFocus::WorkOffline);
        if !self.queued.is_empty() {
            t.push(ConnectivityFocus::ViewQueue);
        }
        if matches!(self.presentation, ConnectivityPresentation::Banner) {
            t.push(ConnectivityFocus::Dismiss);
        }
        t
    }

    fn activate_focus(&mut self) -> ConnectivityOutcome {
        match self.focus {
            ConnectivityFocus::Retry => ConnectivityOutcome::RetryNow,
            ConnectivityFocus::Authenticate => ConnectivityOutcome::Authenticate,
            ConnectivityFocus::WorkOffline => ConnectivityOutcome::WorkOffline,
            ConnectivityFocus::ViewQueue => ConnectivityOutcome::ViewQueue,
            ConnectivityFocus::Dismiss => {
                self.banner_dismissed = true;
                ConnectivityOutcome::Dismiss
            }
            ConnectivityFocus::None => {
                if matches!(self.phase, ConnectivityPhase::AuthRequired) {
                    ConnectivityOutcome::Authenticate
                } else {
                    ConnectivityOutcome::RetryNow
                }
            }
        }
    }

    /// Pointer: bottom action band on full; whole banner click = retry.
    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> ConnectivityOutcome {
        if area.is_empty()
            || !self.phase.is_offline_like()
            || !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left))
        {
            return ConnectivityOutcome::Ignored;
        }
        let pos = mouse.position;
        if !area.contains(pos) {
            return ConnectivityOutcome::Ignored;
        }
        if matches!(self.presentation, ConnectivityPresentation::Banner) || area.height <= 2 {
            return ConnectivityOutcome::RetryNow;
        }
        let rel = pos.y.saturating_sub(area.y);
        let h = area.height.max(1);
        if rel + 1 >= h {
            return ConnectivityOutcome::WorkOffline;
        }
        if rel + 2 >= h {
            if matches!(self.phase, ConnectivityPhase::AuthRequired) {
                return ConnectivityOutcome::Authenticate;
            }
            return ConnectivityOutcome::RetryNow;
        }
        ConnectivityOutcome::Ignored
    }
}

// ── OfflineBanner ───────────────────────────────────────────────────────────

/// Unobtrusive single-line connectivity banner.
#[derive(Debug, Clone, Copy)]
pub struct OfflineBanner<'a> {
    state: &'a ReconnectingState,
    system: &'a DesignSystem,
}

impl<'a> OfflineBanner<'a> {
    /// Bind state + system.
    #[must_use]
    pub const fn new(state: &'a ReconnectingState, system: &'a DesignSystem) -> Self {
        Self { state, system }
    }

    /// Paint (no-op if online or dismissed).
    pub fn paint(&self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() || !self.state.should_show_banner() {
            return;
        }
        let (_, head, meta) = self.state.banner_parts(false);
        let status = StatusIndicator::new(self.state.phase.semantic_status(), self.system)
            .label(&head)
            .strong(true);
        let line = format!("{}{meta}", status.text(None));
        let clipped = take_display_cols(&line, usize::from(area.width));
        buffer.set_stringn(
            area.x,
            area.y,
            &clipped,
            usize::from(area.width),
            self.system.style(Role::Text),
        );
        status.paint(Rect::new(area.x, area.y, area.width, 1), buffer);
    }

    /// Semantic.
    pub fn register_semantic<Sid, Act>(
        &self,
        scene: &mut SemanticScene<Sid, Act>,
        id: Sid,
        area: Rect,
    ) where
        Sid: Clone + PartialEq + std::fmt::Display,
        Act: Clone,
    {
        if area.is_empty() || !self.state.should_show_banner() {
            return;
        }
        let desc = format!(
            "offline-banner phase={} target={} queued={} drafts={} selection={}",
            self.state.phase.id(),
            self.state.target,
            self.state.queued.len(),
            self.state.drafts_preserved,
            self.state.selection_preserved,
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Status)
                .label("offline-banner")
                .description(desc)
                .focusable(false)
                .state(SemanticState {
                    busy: matches!(self.state.phase, ConnectivityPhase::Reconnecting),
                    ..Default::default()
                }),
        );
    }
}

impl Widget for &OfflineBanner<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        self.paint(area, buffer);
    }
}

impl Widget for OfflineBanner<'_> {
    #[expect(
        clippy::needless_borrows_for_generic_args,
        reason = "explicitly delegate the owned contract to the borrowed renderer"
    )]
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

// ── OfflineSurface (full) ───────────────────────────────────────────────────

/// Full connectivity / offline recovery surface.
#[derive(Debug, Clone, Copy)]
pub struct OfflineSurface<'a> {
    system: &'a DesignSystem,
}

impl<'a> OfflineSurface<'a> {
    /// System only (state passed to paint for mutability).
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self { system }
    }

    /// Paint full surface (no-op when online).
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut ReconnectingState) {
        if area.is_empty() || !state.phase.is_offline_like() {
            return;
        }
        // Allow painting full even if presentation is Banner when host forces Full paint
        let status_label = format!("{} · {}", state.phase.verb(), state.target);
        let status = StatusIndicator::new(state.phase.semantic_status(), self.system)
            .label(&status_label)
            .strong(true);
        let mut rows: Vec<(String, Role, bool)> = Vec::new();
        rows.push((status.text(None), Role::Text, false));
        if let Some(last) = state.last_success_label() {
            rows.push((last, Role::TextMuted, false));
        }
        if matches!(state.phase, ConnectivityPhase::Reconnecting) {
            let mut retry = format!("retry attempt {}", state.attempt);
            if let Some(m) = state.max_attempts {
                retry.push_str(&format!("/{m}"));
            }
            if let Some(n) = state.next_retry_in_secs {
                retry.push_str(&format!(" · next in {n}s"));
            }
            if state.auto_retry {
                retry.push_str(" · auto");
            }
            rows.push((retry, Role::TextSecondary, false));
        }
        if !state.queued.is_empty() {
            rows.push((
                format!("{} action(s) queued", state.queued.len()),
                Role::Warning,
                false,
            ));
            for q in state.queued.iter().take(3) {
                rows.push((format!("  · {}", q.label), Role::TextDisabled, false));
            }
            if let Some(note) = crate::text::more_note(state.queued.len().saturating_sub(3)) {
                rows.push((format!("  · {note}"), Role::TextDisabled, false));
            }
        }
        if !state.offline_caps.is_empty() {
            rows.push(("offline capabilities".into(), Role::TextMuted, false));
            for c in state.offline_caps.iter().take(OFFLINE_CAPS_SHOWN) {
                let mark = if c.available { "+" } else { "-" };
                rows.push((
                    format!("  {mark} {}", c.label),
                    if c.available {
                        Role::TextStrong
                    } else {
                        Role::TextDisabled
                    },
                    false,
                ));
            }
            // Say what was held back, the way the queued list above does.
            let hidden = state.offline_caps.len().saturating_sub(OFFLINE_CAPS_SHOWN);
            if let Some(note) = crate::text::more_note(hidden) {
                rows.push((format!("  {note}"), Role::TextDisabled, false));
            }
        }
        // Preserve cues
        if state.drafts_preserved || state.selection_preserved {
            let mut parts = Vec::new();
            if state.drafts_preserved {
                parts.push("drafts");
            }
            if state.selection_preserved {
                parts.push("selection");
            }
            rows.push((
                format!("✓ {} preserved", parts.join(" + ")),
                Role::TextStrong,
                false,
            ));
        }

        let action_rows = 2u16; // primary + secondary line
        let content = rows.len() as u16;
        let total = content.saturating_add(action_rows).max(1);
        let block = Center::new(area.width, total)
            .axis(CenterAxis::Vertical)
            .layout(area)
            .child;
        let sizes: Vec<FlexSize> = (0..total).map(|_| FlexSize::Fixed(1)).collect();
        let stack = Stack::new().layout(block, &sizes);

        let mut idx = 0usize;
        for (text, role, bold) in &rows {
            if let Some(r) = stack.get(idx) {
                let row_area = Rect::new(area.x, r.y, area.width, 1);
                if idx == 0 {
                    let x = center_line_x(row_area, display_cols(text) as u16);
                    status.paint(
                        Rect::new(x, r.y, row_area.right().saturating_sub(x), 1),
                        buffer,
                    );
                } else {
                    let mut style = self.system.style(*role);
                    if *bold {
                        style = style.add_modifier(Modifier::BOLD);
                    }
                    paint_centered(row_area, buffer, r.y, text, style);
                }
            }
            idx += 1;
        }

        // Actions
        if let Some(r) = stack.get(idx) {
            let (label, primary_focus) = if matches!(state.phase, ConnectivityPhase::AuthRequired) {
                ("Sign in", ConnectivityFocus::Authenticate)
            } else {
                ("Retry now", ConnectivityFocus::Retry)
            };
            paint_action(
                self.system,
                Rect::new(area.x, r.y, area.width, 1),
                buffer,
                label,
                Some("r"),
                true,
                state.focus == primary_focus,
                if matches!(state.phase, ConnectivityPhase::AuthRequired) {
                    &mut state.auth_btn
                } else {
                    &mut state.retry_btn
                },
                false,
            );
        }
        idx += 1;
        if let Some(r) = stack.get(idx) {
            let label = if state.queued.is_empty() {
                "Work offline"
            } else {
                "Work offline · view queue"
            };
            paint_action(
                self.system,
                Rect::new(area.x, r.y, area.width, 1),
                buffer,
                label,
                Some("o"),
                false,
                matches!(
                    state.focus,
                    ConnectivityFocus::WorkOffline | ConnectivityFocus::ViewQueue
                ),
                &mut state.offline_btn,
                false,
            );
        }
        let _ = idx;
    }

    /// Semantic.
    pub fn register_semantic<Sid, Act>(
        &self,
        scene: &mut SemanticScene<Sid, Act>,
        id: Sid,
        area: Rect,
        state: &ReconnectingState,
    ) where
        Sid: Clone + PartialEq + std::fmt::Display,
        Act: Clone,
    {
        if area.is_empty() || !state.phase.is_offline_like() {
            return;
        }
        let desc = format!(
            "offline-surface phase={} target={} attempt={} queued={} presentation={}",
            state.phase.id(),
            state.target,
            state.attempt,
            state.queued.len(),
            state.presentation.id(),
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Status)
                .label("offline-surface")
                .description(desc)
                .focusable(true)
                .state(SemanticState {
                    busy: matches!(state.phase, ConnectivityPhase::Reconnecting),
                    ..Default::default()
                }),
        );
    }
}

fn paint_centered(
    area: Rect,
    buffer: &mut Buffer,
    y: u16,
    text: &str,
    style: ratatui_core::style::Style,
) {
    let width = display_cols(text).min(usize::from(area.width));
    if width == 0 {
        return;
    }
    let clipped = take_display_cols(text, width);
    let x = center_line_x(area, width as u16);
    buffer.set_stringn(x, y, &clipped, width, style);
}

fn paint_action(
    system: &DesignSystem,
    area: Rect,
    buffer: &mut Buffer,
    label: &str,
    shortcut: Option<&str>,
    primary: bool,
    focused: bool,
    btn_state: &mut ButtonState,
    _ascii: bool,
) {
    if area.is_empty() {
        return;
    }
    let variant = if primary {
        ButtonVariant::Primary
    } else {
        ButtonVariant::Quiet
    };
    let mut btn = Button::new(label, system).variant(variant);
    if let Some(sc) = shortcut {
        btn = btn.trailing(sc);
    }
    let measure = display_cols(label)
        .saturating_add(shortcut.map(display_cols).unwrap_or(0))
        .saturating_add(6)
        .min(usize::from(area.width)) as u16;
    let x = center_line_x(area, measure.max(1));
    let hit = Rect::new(x, area.y, measure.max(1), 1);
    btn_state.activation.set_accepts_input(focused || primary);
    let _ = btn.paint(hit, buffer, btn_state);
}

// ── Unified paint helper ────────────────────────────────────────────────────

/// Choose banner vs full from state presentation.
pub struct OfflineChrome;

impl OfflineChrome {
    /// Paint preferred presentation.
    pub fn paint(
        area: Rect,
        buffer: &mut Buffer,
        state: &mut ReconnectingState,
        system: &DesignSystem,
    ) {
        if !state.phase.is_offline_like() {
            return;
        }
        if state.should_show_full() || matches!(state.presentation, ConnectivityPresentation::Full)
        {
            OfflineSurface::new(system).paint(area, buffer, state);
        } else if state.should_show_banner() {
            OfflineBanner::new(state, system).paint(area, buffer);
        } else if state.phase.is_offline_like() && state.banner_dismissed {
            // still paint nothing; StatusBar carries the cue
        }
    }
}

// ── Examples ────────────────────────────────────────────────────────────────

/// Reconnecting agent with queue and offline caps.
#[must_use]
pub fn example_reconnecting_agent() -> ReconnectingState {
    let mut s = ReconnectingState::new("agent://prod-1");
    s.set_now_secs(1_700_000_400);
    s.set_last_success_secs(Some(1_700_000_000));
    s.begin_reconnect(3);
    s.set_max_attempts(Some(8));
    s.set_next_retry_in_secs(Some(12));
    s.set_queued(vec![
        QueuedConnectivityAction::new("send-1", "Send prompt"),
        QueuedConnectivityAction::new("tool-2", "Apply patch"),
    ]);
    s.set_offline_capabilities(vec![
        OfflineCapability::new("Edit drafts", true),
        OfflineCapability::new("Browse history", true),
        OfflineCapability::new("Run remote tools", false),
    ]);
    s.set_presentation(ConnectivityPresentation::Banner);
    s
}

/// Auth required full surface.
#[must_use]
pub fn example_auth_required() -> ReconnectingState {
    let mut s = ReconnectingState::new("ssh://bastion");
    s.set_now_secs(1_700_000_100);
    s.set_last_success_secs(Some(1_700_000_000));
    s.require_auth();
    s.set_drafts_preserved(true);
    s.set_selection_preserved(true);
    s
}

/// Server unavailable full.
#[must_use]
pub fn example_server_unavailable() -> ReconnectingState {
    let mut s = ReconnectingState::new("db://analytics");
    s.set_now_secs(1_700_000_200);
    s.set_last_success_secs(Some(1_699_999_000));
    s.mark_server_unavailable();
    s.set_presentation(ConnectivityPresentation::Full);
    s.set_queued(vec![QueuedConnectivityAction::new("q1", "Run query")]);
    s.set_offline_capabilities(vec![
        OfflineCapability::new("View cached result", true),
        OfflineCapability::new("Write", false),
    ]);
    s
}

/// Disconnected banner.
#[must_use]
pub fn example_disconnected() -> ReconnectingState {
    let mut s = ReconnectingState::new("wss://collab");
    s.set_now_secs(50);
    s.set_last_success_secs(Some(10));
    s.mark_disconnected();
    s.set_presentation(ConnectivityPresentation::Banner);
    s
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_core::backend::TestBackend;
    use ratatui_core::terminal::Terminal;

    fn system() -> DesignSystem {
        DesignSystem::default()
    }

    fn painted(area: Rect, mut f: impl FnMut(Rect, &mut Buffer)) -> String {
        let mut buf = Buffer::empty(area);
        f(area, &mut buf);
        let mut s = String::new();
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                s.push_str(buf[(x, y)].symbol());
            }
            s.push('\n');
        }
        s
    }

    #[test]
    fn phases_map_to_semantic_and_toast() {
        assert_eq!(
            ConnectivityPhase::Disconnected.semantic_status(),
            SemanticStatus::Offline
        );
        assert_eq!(
            ConnectivityPhase::Reconnecting.activity_phase(),
            Some(ActivityPhase::Reconnecting)
        );
        assert_eq!(
            ConnectivityPhase::AuthRequired.toast_kind(),
            ToastKind::Warning
        );
    }

    #[test]
    fn status_bar_content_and_slot() {
        let mut s = example_reconnecting_agent();
        let content = s.status_bar_content();
        assert!(content.contains("reconnect"), "{content}");
        let slot = s.status_slot_template("c", &content);
        assert_eq!(slot.kind, StatusKind::Connection);
        assert_eq!(slot.region, StatusRegion::Right);
        s.mark_online(99);
        assert!(s.status_bar_content().contains("ok"));
    }

    #[test]
    fn notification_item_integration() {
        let s = example_reconnecting_agent();
        let n = s.to_notification_item("n1");
        assert_eq!(n.kind, ToastKind::Progress);
        assert!(n.title.as_deref().is_some_and(|t| t.contains("Connection")));
        assert!(n.actions.iter().any(|(id, _)| id == "retry"));
        assert_eq!(n.group_id.as_deref(), Some("connectivity"));
    }

    #[test]
    fn banner_paints_unobtrusive() {
        let system = system();
        let s = example_disconnected();
        let text = painted(Rect::new(0, 0, 48, 1), |a, b| {
            OfflineBanner::new(&s, &system).paint(a, b);
        });
        assert!(
            text.contains("offline") || text.contains("collab"),
            "{text}"
        );
    }

    #[test]
    fn banner_hidden_when_dismissed() {
        let system = system();
        let mut s = example_disconnected();
        let esc = KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crate::input::KeyEventState::NONE,
        };
        assert_eq!(s.handle_key(esc), ConnectivityOutcome::Dismiss);
        assert!(s.banner_dismissed());
        assert!(!s.should_show_banner());
        let text = painted(Rect::new(0, 0, 40, 1), |a, b| {
            OfflineBanner::new(&s, &system).paint(a, b);
        });
        assert!(!text.contains("offline"), "{text}");
    }

    #[test]
    fn full_surface_shows_queue_caps_preserve() {
        let system = system();
        let mut s = example_server_unavailable();
        let text = painted(Rect::new(0, 0, 50, 16), |a, b| {
            OfflineSurface::new(&system).paint(a, b, &mut s);
        });
        assert!(
            text.contains("unavailable") || text.contains("analytics") || text.contains("down"),
            "{text}"
        );
        assert!(text.contains("queued") || text.contains("query"), "{text}");
        assert!(
            text.contains("preserved") || text.contains("drafts"),
            "{text}"
        );
        assert!(text.contains("Retry") || text.contains("offline"), "{text}");
    }

    #[test]
    fn auth_required_prefers_sign_in() {
        let system = system();
        let mut s = example_auth_required();
        let text = painted(Rect::new(0, 0, 48, 12), |a, b| {
            OfflineSurface::new(&system).paint(a, b, &mut s);
        });
        assert!(text.contains("auth") || text.contains("Sign"), "{text}");
        let key = KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crate::input::KeyEventState::NONE,
        };
        assert_eq!(s.handle_key(key), ConnectivityOutcome::Authenticate);
    }

    #[test]
    fn retry_key_and_preserve_defaults() {
        let mut s = example_reconnecting_agent();
        assert!(s.drafts_preserved());
        assert!(s.selection_preserved());
        let key = KeyEvent {
            code: KeyCode::Char('r'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crate::input::KeyEventState::NONE,
        };
        assert_eq!(s.handle_key(key), ConnectivityOutcome::RetryNow);
    }

    #[test]
    fn last_success_relative() {
        let mut s = ReconnectingState::new("t");
        s.set_now_secs(120);
        s.set_last_success_secs(Some(60));
        let label = s.last_success_label().unwrap();
        assert!(label.contains("60s") || label.contains("1m"), "{label}");
    }

    #[test]
    fn distinguish_all_phases() {
        for p in [
            ConnectivityPhase::Online,
            ConnectivityPhase::Disconnected,
            ConnectivityPhase::Reconnecting,
            ConnectivityPhase::AuthRequired,
            ConnectivityPhase::ServerUnavailable,
        ] {
            assert!(!p.id().is_empty());
            assert!(!p.verb().is_empty());
            assert!(!p.glyph_unicode().is_empty());
        }
    }

    #[test]
    fn semantic_registers() {
        let system = system();
        let s = example_reconnecting_agent();
        let mut scene = SemanticScene::<&str, ()>::default();
        OfflineBanner::new(&s, &system).register_semantic(&mut scene, "b", Rect::new(0, 0, 40, 1));
        assert!(
            scene
                .nodes()
                .iter()
                .any(|n| n.label.as_deref() == Some("offline-banner"))
        );
    }

    #[test]
    fn tiny_empty_safe() {
        let system = system();
        let mut s = example_disconnected();
        let mut buf = Buffer::empty(Rect::new(0, 0, 8, 2));
        OfflineBanner::new(&s, &system).paint(Rect::new(0, 0, 1, 1), &mut buf);
        OfflineSurface::new(&system).paint(Rect::new(0, 0, 0, 0), &mut buf, &mut s);
    }

    #[test]
    fn offline_surfaces_resize_cjk_combining_and_ascii_safe() {
        let system = system();
        let target = "東京 🛰 Cafe\u{301}";
        for _ in [false, true] {
            let mut state = ReconnectingState::new(target);
            state.mark_server_unavailable();
            for (width, height) in [(48, 12), (12, 2), (1, 1), (0, 0)] {
                let area = Rect::new(0, 0, width, height);

                let mut banner = Buffer::empty(area);
                OfflineBanner::new(&state, &system).paint(area, &mut banner);

                let mut surface = Buffer::empty(area);
                OfflineSurface::new(&system).paint(area, &mut surface, &mut state);

                let mut chrome = Buffer::empty(area);
                OfflineChrome::paint(area, &mut chrome, &mut state, &system);

                if width == 48 {
                    let text: String = banner.content().iter().map(|cell| cell.symbol()).collect();
                    assert!(text.contains('東'), "{text:?}");
                    assert!(text.contains('🛰'), "{text:?}");
                    assert!(text.contains("Cafe\u{301}"), "{text:?}");
                }
            }
        }
    }

    #[test]
    fn fuzz_phases() {
        let system = system();
        let mut seed = 17u64;
        for _ in 0..40 {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let mut s = ReconnectingState::new("fuzz");
            s.set_now_secs(seed % 10_000);
            s.set_last_success_secs(Some(seed % 5_000));
            match seed % 5 {
                0 => s.mark_online(seed % 10_000),
                1 => s.mark_disconnected(),
                2 => s.begin_reconnect((seed % 9) as u32 + 1),
                3 => s.require_auth(),
                _ => s.mark_server_unavailable(),
            }
            if seed % 2 == 0 {
                s.set_presentation(ConnectivityPresentation::Full);
            }
            if seed % 3 == 0 {
                s.enqueue(QueuedConnectivityAction::new("x", "act"));
            }
            let w = (seed % 40) as u16 + 1;
            let h = (seed % 14) as u16 + 1;
            let area = Rect::new(0, 0, w, h);
            let mut buf = Buffer::empty(area);
            OfflineChrome::paint(area, &mut buf, &mut s, &system);
        }
    }

    #[test]
    fn pty_snapshot_stable() {
        let system = system();
        let paint = || {
            let mut t = Terminal::new(TestBackend::new(48, 12)).unwrap();
            let mut s = example_server_unavailable();
            t.draw(|f| {
                OfflineSurface::new(&system).paint(f.area(), f.buffer_mut(), &mut s);
            })
            .unwrap();
            t.backend()
                .buffer()
                .content()
                .iter()
                .map(|c| c.symbol().to_string())
                .collect::<String>()
        };
        assert_eq!(paint(), paint());
    }

    #[test]
    fn paint_perf_smoke() {
        let system = system();
        let mut terminal = Terminal::new(TestBackend::new(50, 14)).unwrap();
        let mut s = example_reconnecting_agent();
        s.set_presentation(ConnectivityPresentation::Full);
        let start = std::time::Instant::now();
        for _ in 0..100 {
            terminal
                .draw(|f| {
                    OfflineSurface::new(&system).paint(f.area(), f.buffer_mut(), &mut s);
                })
                .unwrap();
        }
        assert!(start.elapsed().as_millis() < 5_000);
    }

    #[test]
    fn mouse_retry_on_banner() {
        let mut s = example_disconnected();
        let area = Rect::new(0, 0, 40, 1);
        let mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position { x: 2, y: 0 },
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(s.handle_mouse(mouse, area), ConnectivityOutcome::RetryNow);
    }

    #[test]
    fn online_skips_banner() {
        let system = system();
        let mut s = ReconnectingState::new("x");
        s.mark_online(1);
        assert!(!s.should_show_banner());
        let text = painted(Rect::new(0, 0, 20, 1), |a, b| {
            OfflineBanner::new(&s, &system).paint(a, b);
        });
        assert!(text.trim().is_empty() || !text.contains("offline"));
    }
}
