//! The adapter between jackin's pane transport and the library's terminal
//! session controller: the transport wrappers, the host policy and the
//! explicit mapping of the neutral status back to `LinkStatus`.

use std::sync::Arc;

use termrock::terminal::{
    ReplayTiming, SessionClipboardContext, SessionClipboardDecision, SessionClipboardRefusal,
    SessionCommand, SessionConfig, SessionDetach, SessionEnd, SessionEvent, SessionHostPolicy,
    SessionLease, SessionLink, SessionSent, SessionSize, SessionSource, SessionStatus,
};

use super::clipboard::{ClipContext, ClipDecision, ClipRefusal, decide};
use super::link::{
    DetachCause, EndKind, Lease, LinkEvent, LinkFault, LinkSource, LinkStatus, PaneCommand,
    PaneLink, PaneSize, Sent,
};
use super::policy::{PanePolicy, PaneTiming};
use super::terminal::sanitize_title;
use super::{MIN_PANE_COLS, MIN_PANE_ROWS, PaneRequest, SCROLLBACK};
use crate::domain::shown::shown;

const NOTICE_CHARS: usize = 240;

/// The controller's numbers from the console's policy and the pane's timing.
pub(super) fn config(policy: &PanePolicy, timing: PaneTiming) -> SessionConfig {
    SessionConfig {
        minimum: SessionSize {
            rows: MIN_PANE_ROWS,
            cols: MIN_PANE_COLS,
        },
        scrollback: SCROLLBACK,
        feed_budget_bytes: policy.feed_budget_bytes,
        paste_max_bytes: policy.paste_max_bytes,
        refusal_status_every_ms: policy.refusal_status_every_ms,
        timing: ReplayTiming {
            replay_cap_ms: timing.replay_cap_ms,
            nudge_ms: timing.nudge_ms,
            quiet_ms: timing.quiet_ms,
        },
    }
}

/// The clipboard permission and the text hygiene of one pane: the product
/// rule of `decide` with the request's kind.
#[derive(Debug)]
pub(super) struct JackinSessionPolicy {
    request: PaneRequest,
    policy: Arc<PanePolicy>,
}

impl JackinSessionPolicy {
    pub(super) fn new(request: &PaneRequest, policy: &Arc<PanePolicy>) -> Self {
        Self {
            request: request.clone(),
            policy: Arc::clone(policy),
        }
    }
}

impl SessionHostPolicy for JackinSessionPolicy {
    fn title(&self, raw: &str) -> String {
        sanitize_title(raw)
    }

    fn notice(&self, raw: &str) -> String {
        shown(raw, NOTICE_CHARS)
    }

    fn clipboard(
        &self,
        context: SessionClipboardContext,
        payload: &str,
    ) -> SessionClipboardDecision {
        let context = ClipContext {
            agent: self.request.agent().is_some(),
            owner: context.owner,
            focused: context.focused,
            last_gesture_ms: context.last_gesture_ms,
            now_ms: context.now_ms,
        };
        match decide(&self.policy, &context, payload) {
            ClipDecision::Allow(text) => SessionClipboardDecision::Copy(text),
            ClipDecision::Refuse(refusal) => SessionClipboardDecision::Refuse(match refusal {
                ClipRefusal::Write => SessionClipboardRefusal::Write,
                ClipRefusal::Read => SessionClipboardRefusal::Read,
                ClipRefusal::TooLarge => SessionClipboardRefusal::TooLarge,
            }),
            ClipDecision::Drop => SessionClipboardDecision::Drop,
        }
    }
}

pub(super) fn clip_refusal(refusal: SessionClipboardRefusal) -> ClipRefusal {
    match refusal {
        SessionClipboardRefusal::Write => ClipRefusal::Write,
        SessionClipboardRefusal::Read => ClipRefusal::Read,
        SessionClipboardRefusal::TooLarge => ClipRefusal::TooLarge,
    }
}

/// Opens a pane's links through the host's `LinkSource`, with the request the
/// pane was built for.
#[derive(Debug)]
pub(super) struct JackinSessionSource {
    request: PaneRequest,
    inner: Arc<dyn LinkSource>,
}

impl JackinSessionSource {
    pub(super) fn new(request: PaneRequest, inner: Arc<dyn LinkSource>) -> Self {
        Self { request, inner }
    }
}

fn pane_size(size: SessionSize) -> PaneSize {
    PaneSize {
        rows: size.rows,
        cols: size.cols,
    }
}

fn wrap(
    link: Result<Box<dyn PaneLink>, LinkFault>,
) -> Result<Box<dyn SessionLink<LinkFault>>, LinkFault> {
    link.map(|link| Box::new(JackinSessionLink(link)) as Box<dyn SessionLink<LinkFault>>)
}

impl SessionSource<LinkFault> for JackinSessionSource {
    fn open(&self, size: SessionSize) -> Result<Box<dyn SessionLink<LinkFault>>, LinkFault> {
        wrap(self.inner.open(&self.request, pane_size(size)))
    }

    fn restart(&self, size: SessionSize) -> Result<Box<dyn SessionLink<LinkFault>>, LinkFault> {
        wrap(self.inner.restart(&self.request, pane_size(size)))
    }

    fn closed_before_attach(&self) -> LinkFault {
        LinkFault::new("")
    }
}

/// A `PaneLink` as the controller's transport.
#[derive(Debug)]
struct JackinSessionLink(Box<dyn PaneLink>);

impl SessionLink<LinkFault> for JackinSessionLink {
    fn send(&self, command: SessionCommand) -> SessionSent {
        let command = match command {
            SessionCommand::Input(bytes) => PaneCommand::Input(bytes),
            SessionCommand::Reply(bytes) => PaneCommand::Reply(bytes),
            SessionCommand::Redraw => PaneCommand::Redraw,
            SessionCommand::Detach => PaneCommand::Detach,
        };
        match self.0.send(command) {
            Sent::Queued => SessionSent::Queued,
            Sent::Full => SessionSent::Full,
            Sent::Closed => SessionSent::Closed,
        }
    }

    fn set_size(&self, size: SessionSize) {
        self.0.set_size(pane_size(size));
    }

    fn drain(&self, now_ms: i64, output_budget: usize) -> Vec<SessionEvent<LinkFault>> {
        self.0
            .drain(now_ms, output_budget)
            .into_iter()
            .map(session_event)
            .collect()
    }
}

fn session_lease(lease: Lease) -> SessionLease {
    match lease {
        Lease::Owner => SessionLease::Owner,
        Lease::ReadOnly => SessionLease::ReadOnly,
    }
}

fn session_event(event: LinkEvent) -> SessionEvent<LinkFault> {
    match event {
        LinkEvent::Attached => SessionEvent::Attached,
        LinkEvent::Lease(lease) => SessionEvent::Lease(session_lease(lease)),
        LinkEvent::Output(bytes) => SessionEvent::Output(bytes),
        LinkEvent::Notice(text) => SessionEvent::Notice(text),
        LinkEvent::Exited(code) => SessionEvent::Exited(code),
        LinkEvent::Ended(EndKind::Detached) => SessionEvent::Ended(SessionEnd::Detached),
        LinkEvent::Ended(EndKind::StreamClosed) => SessionEvent::Ended(SessionEnd::StreamClosed),
        LinkEvent::Ended(EndKind::Refused(fault)) => {
            SessionEvent::Ended(SessionEnd::Refused(fault))
        }
        LinkEvent::Ended(EndKind::Failed(fault)) => SessionEvent::Ended(SessionEnd::Failed(fault)),
    }
}

/// The host's `LinkStatus` for a controller status.
pub(super) fn link_status(status: &SessionStatus<LinkFault>) -> LinkStatus {
    match status {
        SessionStatus::Pending => LinkStatus::Pending,
        SessionStatus::Attaching => LinkStatus::Attaching,
        SessionStatus::Attached(SessionLease::Owner) => LinkStatus::Attached(Lease::Owner),
        SessionStatus::Attached(SessionLease::ReadOnly) => LinkStatus::Attached(Lease::ReadOnly),
        SessionStatus::Detached(SessionDetach::Operator) => {
            LinkStatus::Detached(DetachCause::Operator)
        }
        SessionStatus::Detached(SessionDetach::TransportLost) => {
            LinkStatus::Detached(DetachCause::GatewayLost)
        }
        SessionStatus::Exited(code) => LinkStatus::Exited(*code),
        SessionStatus::Failed(fault) => LinkStatus::Failed(fault.clone()),
    }
}
