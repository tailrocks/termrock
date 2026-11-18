use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::clock::Moment;
use crate::core::event::{Input, MouseKind};
use crate::theme::Tone;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Message<P> {
    text: String,
    tone: Tone,
    at: Moment,
    payload: P,

    job: Option<JobKey>,

    sticky: bool,

    notice: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JobKey(&'static str);

impl JobKey {
    pub const fn new(name: &'static str) -> Self {
        Self(name)
    }

    pub const fn name(self) -> &'static str {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct StatusLine<P = ()> {
    message: Option<Message<P>>,
    ttl_ms: u64,

    now: Moment,

    revision: u64,
}

impl<P: PartialEq> PartialEq for StatusLine<P> {
    fn eq(&self, other: &Self) -> bool {
        self.message == other.message && self.ttl_ms == other.ttl_ms && self.now == other.now
    }
}

impl<P: Eq> Eq for StatusLine<P> {}

impl<P> Default for StatusLine<P> {
    fn default() -> Self {
        Self::new()
    }
}

impl<P> StatusLine<P> {
    pub const DEFAULT_TTL_MS: u64 = 5_000;

    pub const fn new() -> Self {
        Self {
            message: None,
            ttl_ms: Self::DEFAULT_TTL_MS,
            now: Moment::ZERO,
            revision: 0,
        }
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }

    pub const fn ttl_ms(mut self, ms: u64) -> Self {
        self.ttl_ms = ms;
        self
    }

    pub fn observe(&mut self, now: Moment) {
        self.now = self.now.max(now);
    }

    pub fn is_sticky(&self) -> bool {
        self.message.as_ref().is_some_and(|m| m.sticky)
    }

    pub fn end_job(&mut self, job: JobKey) -> bool {
        let showing = self.message.as_ref().is_some_and(|m| m.job == Some(job));
        if showing {
            self.message = None;
            self.revision += 1;
        }
        showing
    }

    pub fn job(&self) -> Option<JobKey> {
        self.message.as_ref().and_then(|m| m.job)
    }

    pub fn clear(&mut self) -> bool {
        let cleared = self.message.take().is_some();
        self.revision += u64::from(cleared);
        cleared
    }

    pub fn get(&self) -> Option<(&str, Tone)> {
        self.message.as_ref().map(|m| (m.text.as_str(), m.tone))
    }

    pub fn entry(&self) -> Option<(&str, Tone, &P)> {
        self.message
            .as_ref()
            .map(|m| (m.text.as_str(), m.tone, &m.payload))
    }

    pub fn payload(&self) -> Option<&P> {
        self.message.as_ref().map(|m| &m.payload)
    }

    pub fn text(&self) -> Option<&str> {
        self.get().map(|(text, _)| text)
    }

    pub fn is_some(&self) -> bool {
        self.message.is_some()
    }

    pub fn is_none(&self) -> bool {
        self.message.is_none()
    }

    pub fn is_error(&self) -> bool {
        self.message.as_ref().is_some_and(|m| m.tone == Tone::Error)
    }

    pub fn expire(&mut self, now: Moment) -> bool {
        self.observe(now);
        let due = self
            .message
            .as_ref()
            .is_some_and(|m| m.tone != Tone::Error && !m.sticky && now.since(m.at) >= self.ttl_ms);
        if due {
            self.message = None;
            self.revision += 1;
        }
        due
    }

    pub fn on_input(&mut self, input: &Input) -> bool {
        let acts = match input {
            Input::Key(_) | Input::Paste(_) => true,
            Input::Mouse(m) => matches!(
                m.kind,
                MouseKind::Down | MouseKind::Secondary | MouseKind::Middle
            ),
            Input::Tick | Input::Resize(..) | Input::External(_) => false,
        };
        if acts && (self.is_error() || self.is_sticky()) {
            self.message = None;
            self.revision += 1;
            return true;
        }
        false
    }
}

impl<P: PartialEq> StatusLine<P> {
    pub fn set_with(&mut self, text: impl Into<String>, tone: Tone, payload: P) {
        let text = text.into();
        let shown = self
            .message
            .as_ref()
            .is_some_and(|m| m.text == text && m.tone == tone && m.payload == payload);
        self.revision += u64::from(!shown);
        self.message = Some(Message {
            text,
            tone,
            at: self.now,
            payload,
            job: None,
            sticky: false,
            notice: false,
        });
    }

    pub fn set_sticky_with(&mut self, text: impl Into<String>, payload: P) {
        self.set_with(text, Tone::Secondary, payload);
        if let Some(m) = self.message.as_mut() {
            m.sticky = true;
        }
    }

    pub fn set_with_at(&mut self, text: impl Into<String>, tone: Tone, payload: P, now: Moment) {
        self.observe(now);
        self.set_with(text, tone, payload)
    }

    /// Posts a message about something the user did not just do, such as a
    /// periodic refresh failing. It replaces nothing but another notice, so it
    /// never hides a confirmation the user has not read yet. It is dropped
    /// when a regular message is on screen, so keep the lasting state of a
    /// failure somewhere other than this line.
    pub fn notice_with(&mut self, text: impl Into<String>, tone: Tone, payload: P) {
        if self.message.as_ref().is_some_and(|m| !m.notice) {
            return;
        }
        self.set_with(text, tone, payload);
        if let Some(m) = self.message.as_mut() {
            m.notice = true;
        }
    }

    pub fn notice_with_at(&mut self, text: impl Into<String>, tone: Tone, payload: P, now: Moment) {
        self.observe(now);
        self.notice_with(text, tone, payload)
    }
}

impl<P: Default + PartialEq> StatusLine<P> {
    pub fn set(&mut self, text: impl Into<String>, tone: Tone) {
        self.set_with(text, tone, P::default())
    }

    pub fn set_at(&mut self, text: impl Into<String>, tone: Tone, now: Moment) {
        self.observe(now);
        self.set(text, tone)
    }

    pub fn notice(&mut self, text: impl Into<String>, tone: Tone) {
        self.notice_with(text, tone, P::default())
    }

    pub fn notice_at(&mut self, text: impl Into<String>, tone: Tone, now: Moment) {
        self.observe(now);
        self.notice(text, tone)
    }

    pub fn sticky(&mut self, text: impl Into<String>) {
        self.set_sticky_with(text, P::default())
    }

    pub fn sticky_at(&mut self, text: impl Into<String>, now: Moment) {
        self.observe(now);
        self.sticky(text)
    }

    pub fn error(&mut self, text: impl Into<String>) {
        self.set(text, Tone::Error)
    }

    pub fn progress(&mut self, job: JobKey, text: impl Into<String>) {
        self.set(text, Tone::Secondary);
        if let Some(m) = self.message.as_mut() {
            m.job = Some(job);
        }
    }

    pub fn progress_at(&mut self, job: JobKey, text: impl Into<String>, now: Moment) {
        self.observe(now);
        self.progress(job, text)
    }
}

#[derive(Debug, Clone, Default)]
pub struct StopHandle(Arc<AtomicBool>);

impl StopHandle {
    pub fn stop(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_stopped(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}
