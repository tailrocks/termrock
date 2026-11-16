//! A pane that shows a program running on the other side of a `PaneLink`.
//!
//! The lifecycle (link, lease, replay, size, input, paste, focus reports and
//! the effect queue) is `TerminalSessionController`. The pane keeps what is
//! jackin's: the words, the agent state and the status cache the host reads.

use std::sync::Arc;

use ratatui::layout::Rect;
use termrock::core::event::{Key, Outcome};
use termrock::core::hit::Axis;
use termrock::core::id::WidgetId;
use termrock::terminal::{
    SessionConfig, SessionDetach, SessionEffect, SessionKey, SessionPaste,
    TerminalSessionController, TerminalViewport,
};
use termrock::theme::Tone;
use termrock::ui::text::{clip, plural};
use termrock::widgets::viewport_ops::ViewportOps;

use super::clipboard::ClipRefusal;
use super::link::{DetachCause, Lease, LinkFault, LinkSource, LinkStatus};
use super::policy::{PanePolicy, PaneTiming};
use super::session::{JackinSessionPolicy, JackinSessionSource, clip_refusal, config, link_status};
use super::{
    PaneBody, PaneEffect, PaneFace, PaneId, PaneKey, PaneKindTag, PaneNote, PaneRequest,
    PasteOutcome,
};
use crate::domain::instance::AgentState;
use crate::domain::shown::shown;
use crate::domain::vocabulary::{PaneWords, fill_in};

const TITLE_COLUMNS: usize = 64;

pub fn sanitize_title(raw: &str) -> String {
    clip(&shown(raw, TITLE_COLUMNS), TITLE_COLUMNS)
}

#[derive(Debug)]
pub struct TerminalPane {
    request: PaneRequest,
    controller: TerminalSessionController<LinkFault>,
    /// The controller's status as the host's `LinkStatus`, so `status()` can
    /// borrow it. Only `with_controller_mut` writes it.
    status: LinkStatus,
    timing: PaneTiming,
}

impl TerminalPane {
    pub fn new(
        id: PaneId,
        request: PaneRequest,
        source: Arc<dyn LinkSource>,
        timing: PaneTiming,
        _now_ms: i64,
    ) -> Self {
        let policy = Arc::new(PanePolicy::default());
        let controller = TerminalSessionController::new(
            WidgetId::of("capsule.pane").child(id as usize),
            Arc::new(JackinSessionSource::new(request.clone(), source)),
            config(&policy, timing),
            Arc::new(JackinSessionPolicy::new(&request, &policy)),
        );
        Self {
            status: link_status(controller.status()),
            request,
            controller,
            timing,
        }
    }

    pub fn status(&self) -> &LinkStatus {
        &self.status
    }

    /// The screen of the program: the viewport that owns its grid.
    pub fn grid(&self) -> &TerminalViewport {
        self.controller.viewport()
    }

    /// Replies a full queue made the pane drop.
    pub fn dropped_replies(&self) -> usize {
        self.controller.dropped_replies()
    }

    /// The one way to change the controller: it brings the status cache back
    /// in line with the controller's before it returns.
    fn with_controller_mut<R>(
        &mut self,
        f: impl FnOnce(&mut TerminalSessionController<LinkFault>) -> R,
    ) -> R {
        let out = f(&mut self.controller);
        self.status = link_status(self.controller.status());
        out
    }

    fn label(&self, words: &PaneWords) -> String {
        self.request
            .agent()
            .map_or_else(|| words.shell_label.clone(), |a| a.label().to_owned())
    }

    fn note_text(&self, words: &PaneWords) -> Option<(String, Tone)> {
        let agent = self.label(words);
        match &self.status {
            LinkStatus::Failed(fault) => {
                return Some((words.failure(&agent, fault), Tone::Error));
            }
            LinkStatus::Exited(code) => {
                let tone = if *code == Some(0) {
                    Tone::Muted
                } else {
                    Tone::Warning
                };
                let code =
                    code.map_or_else(|| words.exited_unknown_code.clone(), |c| c.to_string());
                return Some((
                    fill_in(&words.exited, &[("agent", &agent), ("code", &code)]),
                    tone,
                ));
            }
            LinkStatus::Detached(DetachCause::Operator) => {
                return Some((
                    fill_in(&words.detached, &[("agent", &agent)]),
                    Tone::Warning,
                ));
            }
            LinkStatus::Detached(DetachCause::GatewayLost) => {
                return Some((words.connection_lost.clone(), Tone::Warning));
            }
            _ => {}
        }
        if self.controller.too_small() {
            return Some((words.too_small.clone(), Tone::Warning));
        }
        matches!(self.status, LinkStatus::Attached(Lease::ReadOnly))
            .then(|| (words.viewing.clone(), Tone::Muted))
    }

    fn paste_outcome(sent: SessionPaste) -> PasteOutcome {
        match sent {
            SessionPaste::Sent => PasteOutcome::Sent,
            SessionPaste::NeedsConfirm { lines } => PasteOutcome::NeedsConfirm { lines },
            SessionPaste::TooLarge => PasteOutcome::TooLarge,
            SessionPaste::Full => PasteOutcome::Full,
            SessionPaste::Dropped => PasteOutcome::Dropped,
        }
    }
}

impl PaneBody for TerminalPane {
    fn kind(&self) -> PaneKindTag {
        if self.request.agent().is_some() {
            PaneKindTag::Agent
        } else {
            PaneKindTag::Shell
        }
    }

    fn state(&self) -> AgentState {
        match &self.status {
            _ if self.controller.attention() => AgentState::Blocked,
            LinkStatus::Exited(Some(0)) => AgentState::Done,
            LinkStatus::Exited(Some(_)) | LinkStatus::Failed(_) => AgentState::Blocked,
            _ => AgentState::Unknown,
        }
    }

    fn title(&self) -> Option<&str> {
        self.controller.title()
    }

    fn note(&self, words: &PaneWords) -> Option<PaneNote> {
        self.note_text(words)
            .map(|(text, tone)| PaneNote { text, tone })
    }

    fn suffix(&self, words: &PaneWords) -> Option<String> {
        matches!(self.status, LinkStatus::Attached(Lease::ReadOnly))
            .then(|| words.viewing_suffix.clone())
    }

    fn face(&self, words: &PaneWords) -> PaneFace {
        let agent = self.label(words);
        match &self.status {
            LinkStatus::Pending => PaneFace::Empty {
                title: words.waiting_title.clone(),
                hint: Some(fill_in(&words.waiting_hint, &[("agent", &agent)])),
            },
            LinkStatus::Attaching => PaneFace::Empty {
                title: fill_in(&words.attaching_title, &[("agent", &agent)]),
                hint: Some(words.attaching_hint.clone()),
            },
            LinkStatus::Detached(_) => PaneFace::Dimmed,
            LinkStatus::Failed(_) if !self.controller.has_output() => PaneFace::Empty {
                title: fill_in(&words.unattached_title, &[("agent", &agent)]),
                hint: None,
            },
            LinkStatus::Attached(_) | LinkStatus::Exited(_) | LinkStatus::Failed(_) => {
                PaneFace::Grid
            }
        }
    }

    fn viewport(&self) -> &dyn ViewportOps {
        self.controller.viewport()
    }

    fn accepts_input(&self) -> bool {
        self.controller.live()
    }

    fn viewport_mut(&mut self) -> &mut dyn ViewportOps {
        // The viewport holds no lifecycle state, so this one borrow cannot
        // leave the status cache behind and stays outside the gateway.
        self.controller.viewport_mut()
    }

    fn key(&mut self, key: &Key, now_ms: i64) -> PaneKey {
        match self.with_controller_mut(|c| c.key(key, now_ms)) {
            SessionKey::Consumed => PaneKey::Consumed,
            SessionKey::Forwarded => PaneKey::Forwarded,
        }
    }

    fn gesture(&mut self, now_ms: i64) {
        self.with_controller_mut(|c| c.gesture(now_ms));
    }

    fn wheel(&mut self, axis: Axis, delta: i32) -> Option<Outcome> {
        self.with_controller_mut(|c| c.wheel(axis, delta))
    }

    fn paste(&mut self, text: &str, now_ms: i64) -> PasteOutcome {
        Self::paste_outcome(self.with_controller_mut(|c| c.paste(text, now_ms)))
    }

    fn paste_confirmed(&mut self, text: &str, now_ms: i64) -> PasteOutcome {
        Self::paste_outcome(self.with_controller_mut(|c| c.paste_confirmed(text, now_ms)))
    }

    fn tick(&mut self, now_ms: i64) -> bool {
        self.with_controller_mut(|c| c.pump(now_ms))
    }

    fn take_effects(&mut self, words: &PaneWords) -> Vec<PaneEffect> {
        let agent = self.label(words);
        self.with_controller_mut(|c| c.take_effects())
            .into_iter()
            .map(|effect| match effect {
                SessionEffect::Detached(SessionDetach::Operator) => PaneEffect::Status {
                    text: fill_in(&words.detached, &[("agent", &agent)]),
                    tone: Tone::Warning,
                },
                SessionEffect::Detached(SessionDetach::TransportLost) => PaneEffect::Status {
                    text: words.connection_lost.clone(),
                    tone: Tone::Warning,
                },
                SessionEffect::Failed(fault) => PaneEffect::Status {
                    text: words.failure(&agent, &fault),
                    tone: Tone::Error,
                },
                SessionEffect::Notice(text) => PaneEffect::Status {
                    text,
                    tone: Tone::Secondary,
                },
                SessionEffect::ClipboardRefused(refusal) => {
                    let template = match clip_refusal(refusal) {
                        ClipRefusal::Write => &words.clip_write_blocked,
                        ClipRefusal::Read => &words.clip_read_blocked,
                        ClipRefusal::TooLarge => &words.clip_too_large,
                    };
                    PaneEffect::Status {
                        text: fill_in(template, &[("source", &agent)]),
                        tone: Tone::Warning,
                    }
                }
                SessionEffect::Copy(text) => {
                    let chars = plural(text.chars().count(), "character", "characters");
                    PaneEffect::Copy {
                        what: fill_in(&words.copied_from, &[("source", &agent), ("chars", &chars)]),
                        text,
                    }
                }
            })
            .collect()
    }

    fn set_policy(&mut self, policy: &Arc<PanePolicy>) {
        let config: SessionConfig = config(policy, self.timing);
        let host = Arc::new(JackinSessionPolicy::new(&self.request, policy));
        self.with_controller_mut(|c| {
            c.set_config(config);
            c.set_host_policy(host);
        });
    }

    fn sync_size(&mut self, inner: Rect) {
        self.with_controller_mut(|c| c.sync_size(inner));
    }

    fn focus_changed(&mut self, focused: bool, _now_ms: i64) {
        self.with_controller_mut(|c| c.focus_changed(focused));
    }

    fn redraw(&mut self) {
        self.with_controller_mut(|c| c.redraw());
    }

    fn clear(&mut self) {
        self.with_controller_mut(|c| c.clear());
    }

    fn touched(&self) -> &[String] {
        &[]
    }

    fn sends_prefix(&self) -> bool {
        true
    }

    fn close_needs_confirm(&self) -> bool {
        self.controller.close_needs_confirm()
    }
}
