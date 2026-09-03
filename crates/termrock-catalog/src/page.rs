// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/showcase/pages/mod.rs (MIT).

//! Page trait and events.

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use termrock::input::KeyEvent;

use crate::ctx::RenderCtx;
use crate::id::WidgetId;
use crate::outcome::Route;

/// Event delivered to a page after the app has resolved hit-testing.
#[derive(Debug, Clone)]
pub enum PageEvent {
    Key(KeyEvent),
    Paste(String),
    Click {
        id: WidgetId,
        pos: Position,
    },
    Drag {
        pressed: WidgetId,
        pos: Position,
    },
    Wheel {
        id: WidgetId,
        delta: i32,
    },
    Tick,
    DialogClosed {
        id: WidgetId,
        /// Index into the dialog's actions, or none when cancelled.
        action: Option<usize>,
        value: Option<String>,
    },
}

/// Things a page may ask the app to do.
#[derive(Debug)]
pub enum Request {
    Status(String),
    FocusNext,
    FocusPrev,
}

/// Mutable page context during event handling.
pub struct PageCtx<'a> {
    pub focus: &'a mut Option<WidgetId>,
    pub requests: Vec<Request>,
}

impl PageCtx<'_> {
    pub fn status(&mut self, s: impl Into<String>) {
        self.requests.push(Request::Status(s.into()));
    }
    pub fn focus_id(&self) -> Option<WidgetId> {
        *self.focus
    }
    pub fn set_focus(&mut self, id: WidgetId) {
        *self.focus = Some(id);
    }
    pub fn focus_next(&mut self) {
        self.requests.push(Request::FocusNext);
    }
    pub fn focus_prev(&mut self) {
        self.requests.push(Request::FocusPrev);
    }
}

pub type Hint = (&'static str, &'static str);

/// One catalog page. Live and interactive — never a static Buffer snapshot.
pub trait Page {
    fn title(&self) -> &'static str;
    fn blurb(&self) -> &'static str;
    /// Whether the page accepts interaction in its current presentation.
    fn interactive(&self) -> bool {
        true
    }
    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>);
    fn handle(&mut self, ev: &PageEvent, cx: &mut PageCtx<'_>) -> Route;
    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint>;
    fn editing(&self) -> bool {
        false
    }
    fn animating(&self) -> bool {
        false
    }
    /// Browser interaction family derived from live page state.
    fn interaction_kind(&self) -> &'static str {
        if self.editing() {
            "editor-form"
        } else if self.animating() {
            "timed-state"
        } else {
            "activation"
        }
    }
    /// Whether the page currently owns literal text or paste input.
    fn captures_text_input(&self) -> bool {
        self.editing()
    }
    /// Page-owned modal already painted the footer hint row.
    fn overlaying(&self) -> bool {
        false
    }
}
