use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Position, Rect};
use termrock_core::keymap::Actions;

use crate::menu::{ContextMenu, MenuItem};
use termrock_core::event::Key;
use termrock_core::id::WidgetId;
use termrock_core::pointer::PointerEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextKeys {
    pub shift_f10: bool,

    pub menu_key: bool,

    pub extra: Option<Key>,
}

impl ContextKeys {
    pub const DEFAULT: Self = Self {
        shift_f10: true,
        menu_key: true,
        extra: None,
    };

    pub const NONE: Self = Self {
        shift_f10: false,
        menu_key: false,
        extra: None,
    };

    pub const fn shift_f10(mut self, on: bool) -> Self {
        self.shift_f10 = on;
        self
    }
    pub const fn menu_key(mut self, on: bool) -> Self {
        self.menu_key = on;
        self
    }
    pub const fn extra(mut self, key: Option<Key>) -> Self {
        self.extra = key;
        self
    }

    pub fn matches(&self, key: &Key) -> bool {
        (self.shift_f10 && key.code == KeyCode::F(10) && key.mods == KeyModifiers::SHIFT)
            || (self.menu_key && key.code == KeyCode::Menu && key.plain())
            || self
                .extra
                .is_some_and(|k| k.code == key.code && k.mods == key.mods)
    }
}

impl Default for ContextKeys {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextConfig {
    pub keys: ContextKeys,

    pub moves_cursor: bool,
}

impl ContextConfig {
    pub const DEFAULT: Self = Self {
        keys: ContextKeys::DEFAULT,
        moves_cursor: true,
    };

    pub const fn keys(mut self, keys: ContextKeys) -> Self {
        self.keys = keys;
        self
    }
    pub const fn moves_cursor(mut self, on: bool) -> Self {
        self.moves_cursor = on;
        self
    }

    pub fn on_secondary<S: ContextSource + ?Sized>(
        &self,
        src: &mut S,
        ev: &PointerEvent,
    ) -> Option<ContextRequest<S::Target>> {
        let target = src.context_target(ev)?;
        if self.moves_cursor {
            src.context_select(target);
        }
        Some(ContextRequest {
            target,
            anchor: ContextAnchor::Pointer(ev.pos()),
        })
    }

    pub fn on_key<S: ContextSource + ?Sized>(
        &self,
        src: &S,
        key: &Key,
    ) -> Option<ContextRequest<S::Target>> {
        if !self.keys.matches(key) {
            return None;
        }
        src.context_cursor()
    }
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextAnchor {
    Pointer(Position),

    Control { id: WidgetId, fallback: Rect },
}

impl ContextAnchor {
    pub fn apply(self, menu: ContextMenu) -> ContextMenu {
        match self {
            Self::Pointer(pos) => menu.at(pos),
            Self::Control { id, fallback } => menu.anchor_control(id, fallback),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextRequest<T> {
    pub target: T,
    pub anchor: ContextAnchor,
}

impl<T> ContextRequest<T> {
    pub fn menu(&self, id: WidgetId, items: Vec<MenuItem>) -> ContextMenu {
        self.anchor.apply(ContextMenu::new(id, items))
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> ContextRequest<U> {
        ContextRequest {
            target: f(self.target),
            anchor: self.anchor,
        }
    }
}

pub trait ContextSource {
    type Target: Copy;

    fn context_target(&self, ev: &PointerEvent) -> Option<Self::Target>;

    fn context_select(&mut self, target: Self::Target);

    fn context_cursor(&self) -> Option<ContextRequest<Self::Target>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextCmd<A> {
    pub cmd: A,
    pub reason: Option<&'static str>,
    pub group_end: bool,
    pub danger: bool,
}

impl<A> ContextCmd<A> {
    pub const fn new(cmd: A) -> Self {
        Self {
            cmd,
            reason: None,
            group_end: false,
            danger: false,
        }
    }

    pub const fn unless(mut self, reason: Option<&'static str>) -> Self {
        self.reason = reason;
        self
    }

    pub const fn group_end(mut self) -> Self {
        self.group_end = true;
        self
    }

    pub const fn danger(mut self) -> Self {
        self.danger = true;
        self
    }
}

pub fn command_items<A: Actions + Copy + PartialEq>(
    map: &[(termrock_core::keymap::KeyBinding, A)],
    cmds: &[ContextCmd<A>],
) -> Vec<(A, MenuItem)> {
    cmds.iter()
        .map(|c| {
            let mut item = MenuItem::new(c.cmd.label())
                .separator_if(c.group_end)
                .danger_if(c.danger);
            if let Some(k) = termrock_core::keymap::label_for(map, c.cmd) {
                item = item.shortcut(k);
            }
            if let Some(r) = c.reason {
                item = item.reason(r);
            }
            (c.cmd, item)
        })
        .collect()
}

#[doc(hidden)]
pub fn row_rect(area: Rect, row: usize, offset: usize) -> Rect {
    let dy = row
        .saturating_sub(offset)
        .min(area.height.saturating_sub(1) as usize) as u16;
    Rect::new(area.x, area.y + dy, area.width.max(1), 1)
}
