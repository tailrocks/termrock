use crate::core::keymap::Actions;
use crate::core::list::Navigable;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Position, Rect};

use crate::core::event::Key;
use crate::core::id::{Part, WidgetId};
use crate::core::pointer::PointerEvent;
use crate::widgets::grid::DataGrid;
use crate::widgets::list::ListBox;
use crate::widgets::menu::{ContextMenu, MenuItem};
use crate::widgets::table::DataTable;
use crate::widgets::tabs::Tabs;
use crate::widgets::tree::TreeView;

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
    map: &[(crate::core::keymap::KeyBinding, A)],
    cmds: &[ContextCmd<A>],
) -> Vec<(A, MenuItem)> {
    cmds.iter()
        .map(|c| {
            let mut item = MenuItem::new(c.cmd.label())
                .separator_if(c.group_end)
                .danger_if(c.danger);
            if let Some(k) = crate::core::keymap::label_for(map, c.cmd) {
                item = item.shortcut(k);
            }
            if let Some(r) = c.reason {
                item = item.reason(r);
            }
            (c.cmd, item)
        })
        .collect()
}

fn row_rect(area: Rect, row: usize, offset: usize) -> Rect {
    let dy = row
        .saturating_sub(offset)
        .min(area.height.saturating_sub(1) as usize) as u16;
    Rect::new(area.x, area.y + dy, area.width.max(1), 1)
}

impl ContextSource for ListBox {
    type Target = usize;
    fn context_target(&self, ev: &PointerEvent) -> Option<usize> {
        ev.on(self.id).then(|| ev.index_of(Part::ROW))?
    }
    fn context_select(&mut self, row: usize) {
        if let Some(i) = Navigable::cursor(self).select(row, self).index() {
            self.cursor = i;
        }
    }
    fn context_cursor(&self) -> Option<ContextRequest<usize>> {
        (self.cursor < self.items.len()).then(|| ContextRequest {
            target: self.cursor,
            anchor: ContextAnchor::Control {
                id: self.row_id(self.cursor),
                fallback: row_rect(self.area, self.cursor, self.scroll.offset),
            },
        })
    }
}

impl ContextSource for TreeView {
    type Target = usize;
    fn context_target(&self, ev: &PointerEvent) -> Option<usize> {
        self.row_at(ev)
    }
    fn context_select(&mut self, row: usize) {
        if let Some(i) = Navigable::cursor(self).select(row, self).index() {
            self.cursor = i;
        }
    }
    fn context_cursor(&self) -> Option<ContextRequest<usize>> {
        self.cursor_path()?;
        Some(ContextRequest {
            target: self.cursor,
            anchor: ContextAnchor::Control {
                id: self.row_id(self.cursor),
                fallback: row_rect(self.area, self.cursor, self.scroll.offset),
            },
        })
    }
}

impl ContextSource for DataTable {
    type Target = (usize, usize);
    fn context_target(&self, ev: &PointerEvent) -> Option<(usize, usize)> {
        if !ev.on(self.id) {
            return None;
        }
        if let Some(r) = ev.index_of(Part::ROW) {
            return Some((r, self.cursor_col));
        }
        let r = ev.index_of(Part::CELL)?;
        Some((r, self.column_at(ev.pos().x).unwrap_or(self.cursor_col)))
    }

    fn context_select(&mut self, (row, col): (usize, usize)) {
        if let Some(r) = Navigable::cursor(self).select(row, self).index() {
            self.cursor_row = r;
            if self.cell_nav && col < self.columns.len() {
                self.cursor_col = col;
            }
            self.context_selection(r);
        }
    }
    fn context_cursor(&self) -> Option<ContextRequest<(usize, usize)>> {
        (self.cursor_row < self.rows.len()).then(|| {
            let id = if self.cell_nav {
                self.cell_id(self.cursor_row, self.cursor_col)
            } else {
                self.row_id(self.cursor_row)
            };
            ContextRequest {
                target: (self.cursor_row, self.cursor_col),
                anchor: ContextAnchor::Control {
                    id,

                    fallback: row_rect(
                        Rect {
                            y: self.area.y.saturating_add(1),
                            height: self.area.height.saturating_sub(1),
                            ..self.area
                        },
                        self.cursor_row,
                        self.scroll.offset,
                    ),
                },
            }
        })
    }
}

impl ContextSource for DataGrid {
    type Target = (usize, usize);
    fn context_target(&self, ev: &PointerEvent) -> Option<(usize, usize)> {
        if !ev.on(self.id) {
            return None;
        }
        if let Some(r) = ev.index_of(Part::ROW_NUMBER) {
            return Some((r, self.cursor.1));
        }
        let r = ev.index_of(Part::CELL)?;
        Some((r, self.column_at(ev.pos().x)?))
    }

    fn context_select(&mut self, cell: (usize, usize)) {
        if cell.1 < self.columns.len()
            && let Some(r) = Navigable::cursor(self).select(cell.0, self).index()
        {
            self.context_selection(r, cell.1);
        }
    }
    fn context_cursor(&self) -> Option<ContextRequest<(usize, usize)>> {
        (self.cursor.0 < self.len()).then(|| ContextRequest {
            target: self.cursor,
            anchor: ContextAnchor::Control {
                id: self.cell_id(self.cursor.0, self.cursor.1),
                fallback: row_rect(
                    Rect {
                        y: self.area.y.saturating_add(1),
                        height: self.area.height.saturating_sub(1),
                        ..self.area
                    },
                    self.cursor.0,
                    self.scroll.offset,
                ),
            },
        })
    }
}

impl ContextSource for Tabs {
    type Target = usize;
    fn context_target(&self, ev: &PointerEvent) -> Option<usize> {
        (self.layout_valid() && ev.on(self.id)).then(|| ev.index_of(Part::TAB))?
    }
    fn context_select(&mut self, tab: usize) {
        if let Some(i) = Navigable::cursor(self).select(tab, self).index() {
            self.cursor = i;
        }
    }
    fn context_cursor(&self) -> Option<ContextRequest<usize>> {
        (self.cursor < self.items().len()).then(|| ContextRequest {
            target: self.cursor,
            anchor: ContextAnchor::Control {
                id: self.tab_id(self.cursor),
                fallback: self.areas.get(self.cursor).copied().unwrap_or_default(),
            },
        })
    }
}
