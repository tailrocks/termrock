// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Typed event results — domain messages + framework coordination.
//!
//! Components report what happened after input without running side effects.
//! Hosts apply redraw, focus, and overlay requests; domain `M` stays product-owned.
//!
//! Not an Elm/Bubble Tea runtime: no global command executor, no forced app loop.

use crate::interaction::{OverlayId, OverlayKind};

/// Whether the frame should be painted again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Redraw {
    /// No paint required for this result alone.
    #[default]
    None,
    /// Host should schedule a redraw (full frame is fine; partial is host policy).
    Now,
}

impl Redraw {
    /// Combines two redraw flags (`Now` wins).
    #[must_use]
    pub const fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Now, _) | (_, Self::Now) => Self::Now,
            (Self::None, Self::None) => Self::None,
        }
    }

    /// Whether a paint is requested.
    #[must_use]
    pub const fn needs_paint(self) -> bool {
        matches!(self, Self::Now)
    }
}

/// How the event propagates through nested composites.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Propagation {
    /// Not handled (or intentionally open); parent may handle.
    #[default]
    Bubble,
    /// Consumed; stop bubbling to parents.
    Stop,
}

impl Propagation {
    /// Whether input was consumed (stop further bubble handlers).
    #[must_use]
    pub const fn is_consumed(self) -> bool {
        matches!(self, Self::Stop)
    }

    /// Merge for composites: `Stop` wins over `Bubble`.
    #[must_use]
    pub const fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Stop, _) | (_, Self::Stop) => Self::Stop,
            (Self::Bubble, Self::Bubble) => Self::Bubble,
        }
    }
}

/// Framework focus coordination (not a domain message).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FocusRequest<Id = ()> {
    /// Focus this identity (scene / host applies).
    Set(Id),
    /// Clear focus if the host owns it.
    Clear,
    /// Linear focus next (Tab contract).
    Next,
    /// Linear focus previous.
    Previous,
}

impl<Id> FocusRequest<Id> {
    /// Maps the focused identity type.
    #[must_use]
    pub fn map_id<J>(self, f: impl FnOnce(Id) -> J) -> FocusRequest<J> {
        match self {
            Self::Set(id) => FocusRequest::Set(f(id)),
            Self::Clear => FocusRequest::Clear,
            Self::Next => FocusRequest::Next,
            Self::Previous => FocusRequest::Previous,
        }
    }
}

/// Framework overlay coordination (host owns [`crate::interaction::OverlayStack`]).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum OverlayRequest {
    /// Dismiss the top overlay layer.
    DismissTop,
    /// Dismiss a named overlay.
    Dismiss(OverlayId),
    /// Open jump-mode chrome (host pairs with JumpOverlay).
    OpenJump,
    /// Open command-palette chrome.
    OpenCommandPalette,
    /// Open a named layer; host builds full [`crate::interaction::OverlaySpec`].
    OpenNamed {
        /// Overlay identity.
        id: OverlayId,
        /// Kind for policy defaults.
        kind: OverlayKind,
    },
}

/// Standard result envelope for component input handlers.
///
/// # Domain vs framework
///
/// - **`message`**: typed domain outcome (activated id, query changed, …).
/// - **`propagation` / `redraw` / `focus` / `overlay`**: coordination only.
///
/// # Examples
///
/// ```
/// use termrock::interaction::{EventResult, Propagation, Redraw};
///
/// #[derive(Debug, PartialEq)]
/// enum Msg { Saved }
///
/// let r: EventResult<Msg> = EventResult::emit(Msg::Saved);
/// assert!(r.consumed());
/// assert_eq!(r.message(), Some(&Msg::Saved));
/// assert_eq!(r.redraw(), Redraw::Now);
/// assert_eq!(r.propagation(), Propagation::Stop);
///
/// let ignored = EventResult::<Msg>::ignored();
/// assert!(!ignored.consumed());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventResult<M, FocusId = ()> {
    propagation: Propagation,
    message: Option<M>,
    redraw: Redraw,
    focus: Option<FocusRequest<FocusId>>,
    overlay: Option<OverlayRequest>,
}

impl<M, FocusId> Default for EventResult<M, FocusId> {
    fn default() -> Self {
        Self::ignored()
    }
}

impl<M, FocusId> EventResult<M, FocusId> {
    /// Not handled; bubble to parent. No redraw.
    #[must_use]
    pub const fn ignored() -> Self {
        Self {
            propagation: Propagation::Bubble,
            message: None,
            redraw: Redraw::None,
            focus: None,
            overlay: None,
        }
    }

    /// Consumed with no domain message and no redraw (e.g. key arm only).
    #[must_use]
    pub const fn stop() -> Self {
        Self {
            propagation: Propagation::Stop,
            message: None,
            redraw: Redraw::None,
            focus: None,
            overlay: None,
        }
    }

    /// Consumed state change that needs paint, no domain message.
    #[must_use]
    pub const fn changed() -> Self {
        Self {
            propagation: Propagation::Stop,
            message: None,
            redraw: Redraw::Now,
            focus: None,
            overlay: None,
        }
    }

    /// Consumed domain message + redraw.
    #[must_use]
    pub fn emit(message: M) -> Self {
        Self {
            propagation: Propagation::Stop,
            message: Some(message),
            redraw: Redraw::Now,
            focus: None,
            overlay: None,
        }
    }

    /// Domain message that still bubbles (parent may also act). Rare.
    #[must_use]
    pub fn emit_bubble(message: M) -> Self {
        Self {
            propagation: Propagation::Bubble,
            message: Some(message),
            redraw: Redraw::Now,
            focus: None,
            overlay: None,
        }
    }

    /// Whether propagation is [`Propagation::Stop`] (input consumed).
    #[must_use]
    pub const fn consumed(&self) -> bool {
        self.propagation.is_consumed()
    }

    /// Propagation policy.
    #[must_use]
    pub const fn propagation(&self) -> Propagation {
        self.propagation
    }

    /// Redraw flag.
    #[must_use]
    pub const fn redraw(&self) -> Redraw {
        self.redraw
    }

    /// Domain message borrow.
    #[must_use]
    pub const fn message(&self) -> Option<&M> {
        self.message.as_ref()
    }

    /// Focus coordination borrow.
    #[must_use]
    pub const fn focus(&self) -> Option<&FocusRequest<FocusId>> {
        self.focus.as_ref()
    }

    /// Overlay coordination borrow.
    #[must_use]
    pub const fn overlay(&self) -> Option<&OverlayRequest> {
        self.overlay.as_ref()
    }

    /// Takes the domain message, leaving `None`.
    pub fn take_message(&mut self) -> Option<M> {
        self.message.take()
    }

    /// Overrides redraw.
    #[must_use]
    pub const fn with_redraw(mut self, redraw: Redraw) -> Self {
        self.redraw = redraw;
        self
    }

    /// Overrides propagation.
    #[must_use]
    pub const fn with_propagation(mut self, propagation: Propagation) -> Self {
        self.propagation = propagation;
        self
    }

    /// Attaches a focus request (replaces prior).
    #[must_use]
    pub fn with_focus(mut self, focus: FocusRequest<FocusId>) -> Self {
        self.focus = Some(focus);
        self
    }

    /// Attaches an overlay request (replaces prior).
    #[must_use]
    pub fn with_overlay(mut self, overlay: OverlayRequest) -> Self {
        self.overlay = Some(overlay);
        self
    }

    /// Sets or replaces the domain message without forcing redraw/stop.
    #[must_use]
    pub fn with_message(mut self, message: M) -> Self {
        self.message = Some(message);
        self
    }

    /// Maps the domain message type.
    #[must_use]
    pub fn map<N>(self, f: impl FnOnce(M) -> N) -> EventResult<N, FocusId> {
        EventResult {
            propagation: self.propagation,
            message: self.message.map(f),
            redraw: self.redraw,
            focus: self.focus,
            overlay: self.overlay,
        }
    }

    /// Maps focus identity type.
    #[must_use]
    pub fn map_focus<J>(self, f: impl FnOnce(FocusId) -> J) -> EventResult<M, J> {
        EventResult {
            propagation: self.propagation,
            message: self.message,
            redraw: self.redraw,
            focus: self.focus.map(|req| req.map_id(f)),
            overlay: self.overlay,
        }
    }

    /// If this result bubbles, replace with `fallback`; otherwise keep self.
    #[must_use]
    pub fn or_else(self, fallback: impl FnOnce() -> Self) -> Self {
        if self.propagation.is_consumed() {
            self
        } else {
            // Prefer fallback coordination; keep earlier message only if fallback empty.
            let mut next = fallback();
            if next.message.is_none() {
                next.message = self.message;
            }
            next.redraw = self.redraw.merge(next.redraw);
            if next.focus.is_none() {
                next.focus = self.focus;
            }
            if next.overlay.is_none() {
                next.overlay = self.overlay;
            }
            next
        }
    }

    /// Merges two results from sibling/child composition (child/`other` preferred for message).
    ///
    /// - Propagation: `Stop` wins.
    /// - Message: `other` wins if present, else `self`.
    /// - Redraw: merged.
    /// - Focus/overlay: `other` wins if present, else `self`.
    #[must_use]
    pub fn merge(self, other: Self) -> Self {
        Self {
            propagation: self.propagation.merge(other.propagation),
            message: other.message.or(self.message),
            redraw: self.redraw.merge(other.redraw),
            focus: other.focus.or(self.focus),
            overlay: other.overlay.or(self.overlay),
        }
    }
}

/// Child-first bubble: if `child` stopped, return it; else run `parent` and merge.
#[must_use]
pub fn compose_bubble<M, FocusId>(
    child: EventResult<M, FocusId>,
    parent: impl FnOnce() -> EventResult<M, FocusId>,
) -> EventResult<M, FocusId> {
    if child.consumed() {
        child
    } else {
        let p = parent();
        // Child may carry a bubbling message; parent may stop.
        child.merge(p)
    }
}

/// Parent-first capture: if `parent` stopped, return it; else run `child` and merge.
#[must_use]
pub fn compose_capture<M, FocusId>(
    parent: EventResult<M, FocusId>,
    child: impl FnOnce() -> EventResult<M, FocusId>,
) -> EventResult<M, FocusId> {
    if parent.consumed() {
        parent
    } else {
        parent.merge(child())
    }
}

impl<M, FocusId> EventResult<M, FocusId> {
    /// Builds from consume flag + optional message (test/host adapters).
    #[must_use]
    pub fn from_parts(
        consumed: bool,
        message: Option<M>,
        redraw: Redraw,
        focus: Option<FocusRequest<FocusId>>,
        overlay: Option<OverlayRequest>,
    ) -> Self {
        Self {
            propagation: if consumed {
                Propagation::Stop
            } else {
                Propagation::Bubble
            },
            message,
            redraw,
            focus,
            overlay,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Msg {
        A,
        B,
    }

    #[test]
    fn ignored_bubbles_without_paint() {
        let r = EventResult::<Msg>::ignored();
        assert!(!r.consumed());
        assert_eq!(r.redraw(), Redraw::None);
        assert!(r.message().is_none());
    }

    #[test]
    fn emit_stops_and_redraws() {
        let r: EventResult<Msg> = EventResult::emit(Msg::A);
        assert!(r.consumed());
        assert_eq!(r.message(), Some(&Msg::A));
        assert!(r.redraw().needs_paint());
    }

    #[test]
    fn or_else_runs_only_when_bubble() {
        let stopped: EventResult<Msg> =
            EventResult::emit(Msg::A).or_else(|| EventResult::emit(Msg::B));
        assert_eq!(stopped.message(), Some(&Msg::A));

        let bubbled = EventResult::<Msg>::ignored().or_else(|| EventResult::emit(Msg::B));
        assert_eq!(bubbled.message(), Some(&Msg::B));
        assert!(bubbled.consumed());
    }

    #[test]
    fn compose_bubble_prefers_consuming_child() {
        let child: EventResult<Msg> = EventResult::emit(Msg::A);
        let out = compose_bubble(child, || EventResult::emit(Msg::B));
        assert_eq!(out.message(), Some(&Msg::A));
    }

    #[test]
    fn compose_bubble_parent_when_child_ignores() {
        let out = compose_bubble(EventResult::<Msg>::ignored(), || EventResult::emit(Msg::B));
        assert_eq!(out.message(), Some(&Msg::B));
    }

    #[test]
    fn compose_capture_parent_stop_skips_child() {
        let mut child_ran = false;
        let out = compose_capture(EventResult::<Msg>::emit(Msg::A), || {
            child_ran = true;
            EventResult::emit(Msg::B)
        });
        assert!(!child_ran);
        assert_eq!(out.message(), Some(&Msg::A));
    }

    #[test]
    fn merge_unions_redraw_and_prefers_other_message() {
        let a: EventResult<Msg> = EventResult::changed().with_message(Msg::A);
        let b = EventResult::<Msg>::ignored().with_redraw(Redraw::None);
        let m = a.merge(b);
        assert_eq!(m.message(), Some(&Msg::A));
        assert!(m.redraw().needs_paint());
        assert!(m.consumed()); // changed is Stop
    }

    #[test]
    fn map_transforms_message() {
        let r: EventResult<i32> = EventResult::<Msg>::emit(Msg::A).map(|m| match m {
            Msg::A => 1,
            Msg::B => 2,
        });
        assert_eq!(r.message(), Some(&1));
    }

    #[test]
    fn focus_and_overlay_attach() {
        let r = EventResult::<Msg, &str>::changed()
            .with_focus(FocusRequest::Set("list"))
            .with_overlay(OverlayRequest::DismissTop);
        assert_eq!(r.focus(), Some(&FocusRequest::Set("list")));
        assert_eq!(r.overlay(), Some(&OverlayRequest::DismissTop));
    }
}
