use std::any::Any;

use termrock_core::focus::{Focus, FocusRing};
use termrock_overlays::context::ContextConfig;
use termrock_overlays::layer::{Layer, LayerTag};
use termrock_runtime::popup::Dismiss;
use termrock_theme::{StatusKind, Tone};

pub type TopFn<R, W> = Box<dyn FnOnce(&mut dyn Layer<R, W>)>;

pub enum Request<R, W: ?Sized = (), G = ()> {
    Status(String, Tone),

    StatusAs(String, StatusKind),

    Notice(String, Tone),

    Sticky(String),

    Progress(String, super::JobKey),

    Open(Box<dyn Layer<R, W>>, LayerTag),

    Close,

    Top(TopFn<R, W>),

    Go(G),

    Back,

    Copy(String, Option<String>),

    Help,

    RefusedOutsideClick(Dismiss),
}

impl<R, W: ?Sized, G> Request<R, W, G> {
    pub fn allowed_in_background(&self) -> bool {
        matches!(
            self,
            Self::Status(..)
                | Self::StatusAs(..)
                | Self::Notice(..)
                | Self::Sticky(..)
                | Self::Progress(..)
        )
    }
}

pub fn background<R, W: ?Sized, G>(
    requests: Vec<Request<R, W, G>>,
) -> impl Iterator<Item = Request<R, W, G>> {
    requests.into_iter().filter(Request::allowed_in_background)
}

pub struct Cx<'a, R, W: ?Sized = (), G = ()> {
    pub focus: &'a mut Focus,
    pub ring: &'a FocusRing,

    pub context: ContextConfig,
    pub requests: Vec<Request<R, W, G>>,
}

impl<'a, R, W: ?Sized, G> Cx<'a, R, W, G> {
    pub fn new(focus: &'a mut Focus, ring: &'a FocusRing) -> Self {
        Self {
            focus,
            ring,
            context: ContextConfig::DEFAULT,
            requests: Vec::new(),
        }
    }

    pub fn context(mut self, context: ContextConfig) -> Self {
        self.context = context;
        self
    }

    pub fn take(&mut self) -> Vec<Request<R, W, G>> {
        std::mem::take(&mut self.requests)
    }

    pub fn focus_next(&mut self) {
        self.focus.next(self.ring);
    }

    pub fn focus_prev(&mut self) {
        self.focus.prev(self.ring);
    }

    pub fn status(&mut self, s: impl Into<String>) {
        self.status_in(s, Tone::Secondary);
    }

    pub fn status_in(&mut self, s: impl Into<String>, tone: Tone) {
        self.requests.push(Request::Status(s.into(), tone));
    }

    pub fn notice(&mut self, s: impl Into<String>, tone: Tone) {
        self.requests.push(Request::Notice(s.into(), tone));
    }

    pub fn status_as(&mut self, kind: StatusKind, s: impl Into<String>) {
        self.requests.push(Request::StatusAs(s.into(), kind));
    }

    pub fn sticky(&mut self, s: impl Into<String>) {
        self.requests.push(Request::Sticky(s.into()));
    }

    pub fn refused(&mut self, reason: termrock_core::refusal::Reason) {
        self.status(reason.text);
    }

    pub fn progress(&mut self, job: super::JobKey, s: impl Into<String>) {
        self.requests.push(Request::Progress(s.into(), job));
    }

    pub fn error(&mut self, s: impl Into<String>) {
        self.status_in(s, Tone::Error);
    }

    pub fn open(&mut self, layer: impl Layer<R, W>, tag: LayerTag) {
        self.requests.push(Request::Open(Box::new(layer), tag));
    }

    pub fn close(&mut self) {
        self.requests.push(Request::Close);
    }

    pub fn go(&mut self, g: G) {
        self.requests.push(Request::Go(g));
    }

    pub fn back(&mut self) {
        self.requests.push(Request::Back);
    }

    pub fn copy(&mut self, s: impl Into<String>) {
        self.requests.push(Request::Copy(s.into(), None));
    }

    pub fn copy_as(&mut self, s: impl Into<String>, what: impl Into<String>) {
        self.requests
            .push(Request::Copy(s.into(), Some(what.into())));
    }

    pub fn help(&mut self) {
        self.requests.push(Request::Help);
    }

    pub fn refuse_outside_click(&mut self, dismiss: Dismiss) {
        self.requests.push(Request::RefusedOutsideClick(dismiss));
    }
}

impl<'a, R: 'static, W: ?Sized + 'static, G> Cx<'a, R, W, G> {
    pub fn with_top<L: Layer<R, W>>(&mut self, f: impl FnOnce(&mut L) + 'static) {
        self.requests
            .push(Request::Top(Box::new(move |layer: &mut dyn Layer<R, W>| {
                let any: &mut dyn Any = layer;
                if let Some(l) = any.downcast_mut::<L>() {
                    f(l);
                }
            })));
    }
}
