// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Navigation and menu overlays: sidebar, breadcrumbs, menu, drawer, popover, tooltip (Plan 051).

#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    widgets::{StatefulWidget, Widget},
};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    interaction::{
        NavigationMove, OverlayId, OverlayKind, OverlayOutcome, OverlayPolicy, OverlaySize,
        OverlaySpec, OverlayStack, UiIntent, place_overlay,
    },
    style::{DesignSystem, Role},
    text::{display_cols, take_display_cols},
};

// ── Menu (flat adapter) ─────────────────────────────────────────────────────
// Hierarchical DropdownMenu / ContextMenu live in `dropdown_menu` module.
// Flat `MenuItem` is re-exported from there; `Menu` / `MenuState` remain here
// for simple single-panel lists.

pub use super::dropdown_menu::MenuItem;

/// Menu outcome (cursor is menu-local; scene owns surface focus).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MenuOutcome<Id> {
    /// No change.
    Ignored,
    /// Cursor moved among items.
    CursorMoved,
    /// Item activated.
    Activated(Id),
    /// Esc closed.
    Closed,
}

/// Menu state (collection cursor within the open menu).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuState {
    /// Index-based headless collection (visible indices for this open menu).
    collection: crate::interaction::CollectionState<usize>,
    open: bool,
    /// Host grants input (overlay/scene focused).
    accepts_input: bool,
    /// Painted origin for mouse hits.
    origin: (u16, u16),
    /// Painted height.
    painted_rows: u16,
}

impl Default for MenuState {
    fn default() -> Self {
        Self::new()
    }
}

impl MenuState {
    /// Open menu, cursor on first enabled item after first paint/key.
    #[must_use]
    pub fn new() -> Self {
        Self {
            collection: crate::interaction::CollectionState::new()
                .orientation(crate::interaction::RovingOrientation::Vertical),
            open: true,
            accepts_input: true,
            origin: (0, 0),
            painted_rows: 0,
        }
    }

    /// Cursor index.
    #[must_use]
    pub fn cursor_index(&self) -> usize {
        self.collection.active().copied().unwrap_or(0)
    }

    /// Deprecated name for [`Self::cursor_index`].
    #[deprecated(note = "use cursor_index")]
    #[must_use]
    pub fn focus_index(&self) -> usize {
        self.cursor_index()
    }

    /// Whether the menu is open.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.open
    }

    /// Host input gate.
    pub fn set_accepts_input(&mut self, accepts: bool) {
        self.accepts_input = accepts;
    }

    fn entries<Id>(items: &[MenuItem<Id>]) -> Vec<crate::interaction::CollectionItem<usize>> {
        items
            .iter()
            .enumerate()
            .map(|(i, it)| crate::interaction::CollectionItem {
                id: i,
                enabled: it.enabled,
                label: it.label.clone(),
                parent: None,
            })
            .collect()
    }

    fn ensure_cursor_enabled<Id>(&mut self, items: &[MenuItem<Id>]) {
        let entries = Self::entries(items);
        let _ = self.collection.reconcile(&entries);
    }

    /// Keyboard navigation.
    pub fn handle_key<Id: Clone>(
        &mut self,
        key: KeyEvent,
        items: &[MenuItem<Id>],
    ) -> MenuOutcome<Id> {
        if !self.accepts_input
            || !self.open
            || items.is_empty()
            || key.kind == KeyEventKind::Release
        {
            return MenuOutcome::Ignored;
        }
        self.ensure_cursor_enabled(items);
        if let Some(intent) = crate::interaction::default_menu_intent(key) {
            let out = self.handle_intent(intent, items);
            if !matches!(out, MenuOutcome::Ignored) {
                return out;
            }
        }
        MenuOutcome::Ignored
    }

    /// Intent routing.
    pub fn handle_intent<Id: Clone>(
        &mut self,
        intent: UiIntent,
        items: &[MenuItem<Id>],
    ) -> MenuOutcome<Id> {
        if !self.accepts_input || !self.open || items.is_empty() {
            return MenuOutcome::Ignored;
        }
        self.ensure_cursor_enabled(items);
        let entries = Self::entries(items);
        match intent {
            UiIntent::Move(
                NavigationMove::Next
                | NavigationMove::Previous
                | NavigationMove::First
                | NavigationMove::Last
                | NavigationMove::Up
                | NavigationMove::Down
                | NavigationMove::Left
                | NavigationMove::Right,
            ) => {
                let out = self.collection.handle_intent(intent, &entries);
                if out.active_changed() {
                    MenuOutcome::CursorMoved
                } else {
                    MenuOutcome::Ignored
                }
            }
            UiIntent::Activate | UiIntent::Submit | UiIntent::Toggle => {
                let idx = self.cursor_index().min(items.len() - 1);
                let item = &items[idx];
                if item.enabled {
                    MenuOutcome::Activated(item.id.clone())
                } else {
                    MenuOutcome::Ignored
                }
            }
            UiIntent::Cancel | UiIntent::Close => {
                self.open = false;
                MenuOutcome::Closed
            }
            _ => MenuOutcome::Ignored,
        }
    }

    /// Click to cursor / activate.
    pub fn handle_mouse<Id: Clone>(
        &mut self,
        event: MouseEvent,
        items: &[MenuItem<Id>],
    ) -> MenuOutcome<Id> {
        if !self.accepts_input || !self.open || items.is_empty() {
            return MenuOutcome::Ignored;
        }
        let (_ox, oy) = self.origin;
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let row = usize::from(event.position.y.saturating_sub(oy));
                // Account for separators: map painted row to item index approximately
                // by walking paint order.
                let mut y = 0usize;
                for (i, item) in items.iter().enumerate() {
                    if item.separator_before {
                        y = y.saturating_add(1);
                    }
                    if y == row {
                        if !item.enabled {
                            return MenuOutcome::Ignored;
                        }
                        if self.cursor_index() == i {
                            return MenuOutcome::Activated(item.id.clone());
                        }
                        self.collection.set_active(Some(i));
                        return MenuOutcome::CursorMoved;
                    }
                    y = y.saturating_add(1);
                }
                MenuOutcome::Ignored
            }
            MouseEventKind::ScrollDown => {
                let entries = Self::entries(items);
                if self.collection.move_next(&entries).active_changed() {
                    MenuOutcome::CursorMoved
                } else {
                    MenuOutcome::Ignored
                }
            }
            MouseEventKind::ScrollUp => {
                let entries = Self::entries(items);
                if self.collection.move_previous(&entries).active_changed() {
                    MenuOutcome::CursorMoved
                } else {
                    MenuOutcome::Ignored
                }
            }
            _ => MenuOutcome::Ignored,
        }
    }
}

/// Menu list paint.
#[derive(Debug, Clone, Copy)]
pub struct Menu<'a, Id> {
    items: &'a [MenuItem<Id>],
    system: &'a DesignSystem,
    focused: bool,
    ascii: bool,
    colorless: bool,
}

impl<'a, Id> Menu<'a, Id> {
    /// Items + design system.
    #[must_use]
    pub const fn new(items: &'a [MenuItem<Id>], system: &'a DesignSystem) -> Self {
        Self {
            items,
            system,
            focused: true,
            ascii: false,
            colorless: false,
        }
    }

    /// Scene surface focus chrome.
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// ASCII check/cursor glyphs.
    #[must_use]
    pub const fn ascii(mut self, ascii: bool) -> Self {
        self.ascii = ascii;
        self
    }

    /// Reduced-color roles.
    #[must_use]
    pub const fn colorless(mut self, colorless: bool) -> Self {
        self.colorless = colorless;
        self
    }

    /// Render.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut MenuState) {
        if area.is_empty() {
            state.painted_rows = 0;
            return;
        }
        state.origin = (area.x, area.y);
        let surface = self.focused && state.accepts_input;
        let mut y = area.y;
        let mut rows = 0u16;
        for (i, item) in self.items.iter().enumerate() {
            if y >= area.bottom() {
                break;
            }
            if item.separator_before {
                let line = if self.ascii {
                    "-".repeat(usize::from(area.width))
                } else {
                    "─".repeat(usize::from(area.width))
                };
                buffer.set_stringn(
                    area.x,
                    y,
                    &line,
                    usize::from(area.width),
                    self.system.style(Role::Border),
                );
                y = y.saturating_add(1);
                rows = rows.saturating_add(1);
                if y >= area.bottom() {
                    break;
                }
            }
            let cursor = state.cursor_index() == i;
            let style = if self.colorless {
                if !item.enabled {
                    self.system.style(Role::TextMuted)
                } else if cursor && surface {
                    self.system.style(Role::TextStrong)
                } else {
                    self.system.style(Role::Text)
                }
            } else if !item.enabled {
                self.system.style(Role::TextDisabled)
            } else if cursor && surface {
                self.system.style(Role::Selection)
            } else {
                self.system.style(Role::Text)
            };
            let check = match item.checked {
                Some(true) if self.ascii => "[x] ",
                Some(true) => "✓ ",
                Some(false) if self.ascii => "[ ] ",
                Some(false) => "  ",
                None => "",
            };
            let cursor_g = if cursor && surface {
                if self.ascii { "> " } else { "› " }
            } else {
                "  "
            };
            let disabled_mark = if !item.enabled {
                if self.ascii { " #" } else { " ⊘" }
            } else {
                ""
            };
            let mut line = format!("{cursor_g}{check}{}{disabled_mark}", item.label);
            if let Some(sc) = &item.shortcut {
                line.push(' ');
                line.push_str(sc);
            }
            let text = take_display_cols(&line, usize::from(area.width));
            buffer.set_stringn(area.x, y, &text, usize::from(area.width), style);
            y = y.saturating_add(1);
            rows = rows.saturating_add(1);
        }
        state.painted_rows = rows;
    }
}

impl<Id> StatefulWidget for Menu<'_, Id> {
    type State = MenuState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        Menu::render(&self, area, buffer, state);
    }
}

impl<Id> StatefulWidget for &Menu<'_, Id> {
    type State = MenuState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        Menu::render(self, area, buffer, state);
    }
}

// ContextMenu → `dropdown_menu::{DropdownMenu, ContextMenuState, open_context_menu_overlay}`.

// Sidebar / NavigationList live in `sidebar.rs` (0153 redesign).

// Breadcrumbs live in breadcrumbs.rs (0155 redesign).

// Drawer / Sheet: canonical home is `drawer` module.
// Popover / Tooltip: canonical homes are `popover` and `tooltip` modules.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyModifiers;
    use crate::widgets::drawer::{
        DRAWER_OVERLAY_ID, DrawerOutcome, DrawerState, open_drawer_overlay,
    };
    use crate::widgets::popover::{open_popover_overlay, place_popover};
    use crate::widgets::tooltip::{TooltipState, open_tooltip_overlay};

    #[test]
    fn menu_cursor_moved_not_focus_changed() {
        let items = [MenuItem::new("a", "A"), MenuItem::new("b", "B")];
        let mut state = MenuState::new();
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &items),
            MenuOutcome::CursorMoved
        ));
        assert_eq!(state.cursor_index(), 1);
    }

    #[test]
    fn menu_accepts_input_gate() {
        let items = [MenuItem::new("a", "A")];
        let mut state = MenuState::new();
        state.set_accepts_input(false);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &items),
            MenuOutcome::Ignored
        ));
    }

    #[test]
    fn menu_jk_and_intent_activate() {
        let items = [MenuItem::new("a", "A"), MenuItem::new("b", "B")];
        let mut state = MenuState::new();
        let _ = state.handle_key(
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
            &items,
        );
        assert_eq!(state.cursor_index(), 1);
        assert!(matches!(
            state.handle_intent(UiIntent::Activate, &items),
            MenuOutcome::Activated("b")
        ));
    }

    #[test]
    fn menu_skips_disabled_on_roving() {
        let items = [
            MenuItem::new("a", "A"),
            MenuItem::new("b", "B").enabled(false),
            MenuItem::new("c", "C"),
        ];
        let mut state = MenuState::new();
        let _ = state.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &items);
        assert_eq!(state.cursor_index(), 2);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &items),
            MenuOutcome::Activated("c")
        ));
    }

    #[test]
    fn menu_esc_closes() {
        let items = [MenuItem::new("a", "A")];
        let mut state = MenuState::new();
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &items),
            MenuOutcome::Closed
        ));
    }

    #[test]
    fn tooltip_delay() {
        let mut state = TooltipState::new();
        state.tick_hover(200, true);
        assert!(!state.is_visible());
        state.tick_hover(250, true);
        assert!(state.is_visible());
        state.tick_hover(0, false);
        assert!(!state.is_visible());
    }

    #[test]
    fn drawer_esc() {
        let mut state = DrawerState::new();
        state.open();
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            DrawerOutcome::Closed
        ));
    }

    #[test]
    fn drawer_popover_tooltip_open_on_overlay_stack() {
        let bounds = Rect::new(0, 0, 80, 24);
        let mut stack = OverlayStack::<&'static str>::new();
        let drawer = open_drawer_overlay(
            &mut stack,
            bounds,
            DRAWER_OVERLAY_ID,
            OverlaySize {
                width: 28,
                height: 24,
                min_width: 12,
                min_height: 3,
                max_width: 40,
                max_height: 0,
            },
            Some("sidebar"),
        );
        assert!(matches!(drawer, OverlayOutcome::Opened { .. }));
        assert_eq!(stack.top().unwrap().kind, OverlayKind::Drawer);
        assert_eq!(
            stack.handle_escape(),
            OverlayOutcome::Dismissed {
                id: OverlayId::from_static(DRAWER_OVERLAY_ID),
                focus: Some("sidebar"),
            }
        );

        let anchor = Rect::new(10, 10, 8, 1);
        let pop = open_popover_overlay(
            &mut stack,
            bounds,
            anchor,
            OverlaySize::menu(24, 6),
            Some("trigger"),
        );
        assert!(matches!(pop, OverlayOutcome::Opened { .. }));
        assert_eq!(stack.top().unwrap().kind, OverlayKind::Popover);
        let placed = place_popover(bounds, anchor, OverlaySize::menu(24, 6));
        assert_eq!(stack.top().unwrap().rect, placed);
        assert!(matches!(
            stack.handle_outside_click(ratatui_core::layout::Position::new(0, 0)),
            OverlayOutcome::Dismissed { .. }
        ));

        let tip = open_tooltip_overlay(&mut stack, bounds, anchor, OverlaySize::menu(16, 1), None);
        assert!(matches!(tip, OverlayOutcome::Opened { .. }));
        assert_eq!(stack.top().unwrap().kind, OverlayKind::Tooltip);
        assert!(!stack.top_owns_input());
    }

    #[test]
    fn no_menu_focus_changed_in_production() {
        let src = include_str!("menu_nav.rs");
        let head = src
            .split("#[cfg(test)]")
            .next()
            .unwrap_or(src)
            .lines()
            .filter(|l| {
                let t = l.trim_start();
                !t.starts_with("//") && !t.starts_with("//!")
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!head.contains("FocusChanged"));
        assert!(head.contains("CursorMoved"));
        assert!(head.contains("cursor_index"));
    }

    #[test]
    fn menu_paint_cursor_gutter_and_narrow() {
        use ratatui_core::backend::TestBackend;
        use ratatui_core::terminal::Terminal;
        let system = DesignSystem::default();
        let items = [
            MenuItem::new("a", "Alpha"),
            MenuItem::new("b", "Beta").checked(true),
            MenuItem::new("c", "Gamma").enabled(false),
        ];
        let mut state = MenuState::new();
        for (w, h, ascii) in [(40, 8, false), (12, 5, true)] {
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            terminal
                .draw(|f| {
                    Menu::new(&items, &system)
                        .focused(true)
                        .ascii(ascii)
                        .render(f.area(), f.buffer_mut(), &mut state);
                })
                .unwrap();
            let text: String = terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .map(|c| c.symbol().to_string())
                .collect();
            assert!(text.contains("Alpha") || text.contains("A"));
        }
    }
}
