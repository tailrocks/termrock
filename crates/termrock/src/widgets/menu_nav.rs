// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Navigation and menu overlays: sidebar, breadcrumbs, menu, drawer, popover, tooltip (Plan 051).

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

/// Default overlay id for drawers opened via helpers.
pub const DRAWER_OVERLAY_ID: &str = "termrock.drawer";
/// Default overlay id for popovers.
pub const POPOVER_OVERLAY_ID: &str = "termrock.popover";
/// Default overlay id for tooltips.
pub const TOOLTIP_OVERLAY_ID: &str = "termrock.tooltip";

// ── Menu ────────────────────────────────────────────────────────────────────

/// One menu row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItem<Id> {
    /// Stable id.
    pub id: Id,
    /// Label.
    pub label: String,
    /// Optional shortcut hint.
    pub shortcut: Option<String>,
    /// Disabled.
    pub enabled: bool,
    /// Checked (toggle item).
    pub checked: Option<bool>,
    /// Separator before this item.
    pub separator_before: bool,
}

impl<Id> MenuItem<Id> {
    /// Enabled item.
    #[must_use]
    pub fn new(id: Id, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            shortcut: None,
            enabled: true,
            checked: None,
            separator_before: false,
        }
    }

    /// Disabled.
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Shortcut hint text.
    #[must_use]
    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// Checked / unchecked toggle item.
    #[must_use]
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }

    /// Separator before.
    #[must_use]
    pub fn separator_before(mut self, sep: bool) -> Self {
        self.separator_before = sep;
        self
    }
}

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

/// Menu state (roving cursor within the open menu).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MenuState {
    cursor_index: usize,
    open: bool,
    /// Host grants input (overlay/scene focused).
    accepts_input: bool,
    /// Painted origin for mouse hits.
    origin: (u16, u16),
    /// Painted height.
    painted_rows: u16,
}

impl MenuState {
    /// Open menu, cursor on first enabled item after first paint/key.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            cursor_index: 0,
            open: true,
            accepts_input: true,
            origin: (0, 0),
            painted_rows: 0,
        }
    }

    /// Cursor index.
    #[must_use]
    pub const fn cursor_index(self) -> usize {
        self.cursor_index
    }

    /// Deprecated name for [`Self::cursor_index`].
    #[deprecated(note = "use cursor_index")]
    #[must_use]
    pub const fn focus_index(self) -> usize {
        self.cursor_index
    }

    /// Whether the menu is open.
    #[must_use]
    pub const fn is_open(self) -> bool {
        self.open
    }

    /// Host input gate.
    pub fn set_accepts_input(&mut self, accepts: bool) {
        self.accepts_input = accepts;
    }

    fn next_enabled<Id>(items: &[MenuItem<Id>], from: usize, dir: isize) -> usize {
        if items.is_empty() {
            return 0;
        }
        let mut i = from as isize;
        for _ in 0..items.len() {
            i = (i + dir).rem_euclid(items.len() as isize);
            if items[i as usize].enabled {
                return i as usize;
            }
        }
        from
    }

    fn ensure_cursor_enabled<Id>(&mut self, items: &[MenuItem<Id>]) {
        if items.is_empty() {
            self.cursor_index = 0;
            return;
        }
        self.cursor_index = self.cursor_index.min(items.len() - 1);
        if !items[self.cursor_index].enabled {
            self.cursor_index = Self::next_enabled(items, self.cursor_index, 1);
        }
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
        match intent {
            UiIntent::Move(NavigationMove::Next) => {
                self.cursor_index = Self::next_enabled(items, self.cursor_index, 1);
                MenuOutcome::CursorMoved
            }
            UiIntent::Move(NavigationMove::Previous) => {
                self.cursor_index = Self::next_enabled(items, self.cursor_index, -1);
                MenuOutcome::CursorMoved
            }
            UiIntent::Move(NavigationMove::First) => {
                if let Some(i) = items.iter().position(|it| it.enabled) {
                    self.cursor_index = i;
                }
                MenuOutcome::CursorMoved
            }
            UiIntent::Move(NavigationMove::Last) => {
                if let Some(i) = items.iter().rposition(|it| it.enabled) {
                    self.cursor_index = i;
                }
                MenuOutcome::CursorMoved
            }
            UiIntent::Activate | UiIntent::Submit | UiIntent::Toggle => {
                let item = &items[self.cursor_index.min(items.len() - 1)];
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
        let (ox, oy) = self.origin;
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
                        if self.cursor_index == i {
                            return MenuOutcome::Activated(item.id.clone());
                        }
                        self.cursor_index = i;
                        return MenuOutcome::CursorMoved;
                    }
                    y = y.saturating_add(1);
                }
                MenuOutcome::Ignored
            }
            MouseEventKind::ScrollDown => {
                self.cursor_index = Self::next_enabled(items, self.cursor_index, 1);
                MenuOutcome::CursorMoved
            }
            MouseEventKind::ScrollUp => {
                self.cursor_index = Self::next_enabled(items, self.cursor_index, -1);
                MenuOutcome::CursorMoved
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
            let cursor = state.cursor_index == i;
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

/// Context menu reuses Menu at pointer (placement via OverlayStack).
pub type ContextMenu<'a, Id> = Menu<'a, Id>;

// ── Sidebar ─────────────────────────────────────────────────────────────────

/// Sidebar item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarItem<Id> {
    /// Id.
    pub id: Id,
    /// Label.
    pub label: String,
    /// Enabled.
    pub enabled: bool,
}

impl<Id> SidebarItem<Id> {
    /// Item.
    #[must_use]
    pub fn new(id: Id, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            enabled: true,
        }
    }
}

/// Sidebar outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SidebarOutcome<Id> {
    /// No change.
    Ignored,
    /// Selection changed.
    Selected(Id),
    /// Rail/expanded toggled.
    ToggleRail {
        /// Expanded.
        expanded: bool,
    },
}

/// Sidebar state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarState<Id> {
    selected: Option<Id>,
    expanded: bool,
    cursor_index: usize,
    /// Host grants input.
    pub accepts_input: bool,
}

impl<Id: Clone + PartialEq> SidebarState<Id> {
    /// Expanded sidebar.
    #[must_use]
    pub fn new(selected: Option<Id>) -> Self {
        Self {
            selected,
            expanded: true,
            cursor_index: 0,
            accepts_input: true,
        }
    }

    #[must_use]
    /// Selected.
    pub fn selected(&self) -> Option<&Id> {
        self.selected.as_ref()
    }

    #[must_use]
    /// Expanded (false = rail).
    pub const fn is_expanded(&self) -> bool {
        self.expanded
    }

    /// Cursor index.
    #[must_use]
    pub const fn cursor_index(&self) -> usize {
        self.cursor_index
    }

    /// Keys.
    pub fn handle_key(&mut self, key: KeyEvent, items: &[SidebarItem<Id>]) -> SidebarOutcome<Id> {
        if !self.accepts_input || items.is_empty() || key.kind != KeyEventKind::Press {
            return SidebarOutcome::Ignored;
        }
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.cursor_index = (self.cursor_index + 1) % items.len();
                SidebarOutcome::Ignored
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.cursor_index = self.cursor_index.checked_sub(1).unwrap_or(items.len() - 1);
                SidebarOutcome::Ignored
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                let id = items[self.cursor_index.min(items.len() - 1)].id.clone();
                if !items[self.cursor_index.min(items.len() - 1)].enabled {
                    return SidebarOutcome::Ignored;
                }
                self.selected = Some(id.clone());
                SidebarOutcome::Selected(id)
            }
            KeyCode::Char('[') => {
                self.expanded = !self.expanded;
                SidebarOutcome::ToggleRail {
                    expanded: self.expanded,
                }
            }
            _ => SidebarOutcome::Ignored,
        }
    }
}

/// Sidebar paint.
#[derive(Debug, Clone, Copy)]
pub struct Sidebar<'a, Id> {
    items: &'a [SidebarItem<Id>],
    system: &'a DesignSystem,
    focused: bool,
    ascii: bool,
}

impl<'a, Id: Clone + PartialEq> Sidebar<'a, Id> {
    /// Items + design system.
    #[must_use]
    pub const fn new(items: &'a [SidebarItem<Id>], system: &'a DesignSystem) -> Self {
        Self {
            items,
            system,
            focused: true,
            ascii: false,
        }
    }

    /// Scene surface focus.
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// ASCII rail glyph fallback.
    #[must_use]
    pub const fn ascii(mut self, ascii: bool) -> Self {
        self.ascii = ascii;
        self
    }

    /// Render rail or full labels.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &SidebarState<Id>) {
        if area.is_empty() {
            return;
        }
        let surface = self.focused && state.accepts_input;
        let mut y = area.y;
        for (i, item) in self.items.iter().enumerate() {
            if y >= area.bottom() {
                break;
            }
            let selected = state.selected.as_ref() == Some(&item.id);
            let cursor = state.cursor_index == i;
            let style = if !item.enabled {
                self.system.style(Role::TextDisabled)
            } else if selected {
                self.system.style(Role::Selection)
            } else if cursor && surface {
                self.system.style(Role::Focus)
            } else {
                self.system.style(Role::Text)
            };
            let gutter = if cursor && surface {
                if self.ascii { ">" } else { "›" }
            } else if selected {
                if self.ascii { "*" } else { "•" }
            } else {
                " "
            };
            let text = if state.expanded {
                take_display_cols(&format!("{gutter} {}", item.label), usize::from(area.width))
            } else {
                let ch = item
                    .label
                    .chars()
                    .next()
                    .unwrap_or(if self.ascii { '.' } else { '·' });
                format!("{gutter}{ch}")
            };
            buffer.set_stringn(area.x, y, &text, usize::from(area.width), style);
            y = y.saturating_add(1);
        }
    }
}

// ── Breadcrumbs ─────────────────────────────────────────────────────────────

/// Crumb item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreadcrumbItem<Id> {
    /// Id.
    pub id: Id,
    /// Label.
    pub label: String,
}

impl<Id> BreadcrumbItem<Id> {
    /// Crumb.
    #[must_use]
    pub fn new(id: Id, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
        }
    }
}

/// Breadcrumb outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum BreadcrumbsOutcome<Id> {
    /// No change.
    Ignored,
    /// Navigate to crumb.
    Navigate(Id),
    /// Overflow menu requested.
    OpenOverflow,
}

/// Breadcrumbs state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BreadcrumbsState {
    focus_index: usize,
}

impl BreadcrumbsState {
    /// Keys.
    pub fn handle_key<Id: Clone>(
        &mut self,
        key: KeyEvent,
        items: &[BreadcrumbItem<Id>],
    ) -> BreadcrumbsOutcome<Id> {
        if items.is_empty() || key.kind != KeyEventKind::Press {
            return BreadcrumbsOutcome::Ignored;
        }
        match key.code {
            KeyCode::Left => {
                self.focus_index = self.focus_index.saturating_sub(1);
                BreadcrumbsOutcome::Ignored
            }
            KeyCode::Right => {
                self.focus_index = (self.focus_index + 1).min(items.len().saturating_sub(1));
                BreadcrumbsOutcome::Ignored
            }
            KeyCode::Enter => {
                let id = items[self.focus_index.min(items.len() - 1)].id.clone();
                BreadcrumbsOutcome::Navigate(id)
            }
            _ => BreadcrumbsOutcome::Ignored,
        }
    }
}

/// Breadcrumbs with middle collapse on narrow width.
#[derive(Debug, Clone, Copy)]
pub struct Breadcrumbs<'a, Id> {
    items: &'a [BreadcrumbItem<Id>],
    tokens: &'a DesignSystem,
}

impl<'a, Id> Breadcrumbs<'a, Id> {
    /// Items root→leaf.
    #[must_use]
    pub const fn new(items: &'a [BreadcrumbItem<Id>], tokens: &'a DesignSystem) -> Self {
        Self { items, tokens }
    }

    /// Paint; when width < 40 and len > 3, middle becomes `…`.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, _state: &BreadcrumbsState) {
        if area.is_empty() || self.items.is_empty() {
            return;
        }
        let narrow = area.width < 40 && self.items.len() > 3;
        let parts: Vec<&str> = if narrow {
            let first = self.items[0].label.as_str();
            let last = self.items[self.items.len() - 1].label.as_str();
            vec![first, "…", last]
        } else {
            self.items.iter().map(|i| i.label.as_str()).collect()
        };
        let line = parts.join(" / ");
        let text = take_display_cols(&line, usize::from(area.width));
        buffer.set_stringn(
            area.x,
            area.y,
            &text,
            usize::from(area.width),
            self.tokens.style(Role::TextMuted),
        );
        let _ = display_cols(&line);
    }
}

// ── Drawer / Popover / Tooltip ──────────────────────────────────────────────

/// Drawer outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum DrawerOutcome {
    /// No change.
    #[default]
    Ignored,
    /// Closed.
    Closed,
}

/// Drawer open state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DrawerState {
    open: bool,
}

impl DrawerState {
    /// Closed.
    #[must_use]
    pub const fn new() -> Self {
        Self { open: false }
    }

    #[must_use]
    /// Open.
    pub const fn is_open(self) -> bool {
        self.open
    }

    /// Open drawer.
    pub const fn open(&mut self) {
        self.open = true;
    }

    /// Esc closes.
    pub fn handle_key(&mut self, key: KeyEvent) -> DrawerOutcome {
        if self.open && key.kind == KeyEventKind::Press && key.code == KeyCode::Esc {
            self.open = false;
            DrawerOutcome::Closed
        } else {
            DrawerOutcome::Ignored
        }
    }

    /// Open drawer overlay on stack (modal-like edge panel).
    pub fn open_on_stack<F: Clone + Eq>(
        &mut self,
        stack: &mut OverlayStack<F>,
        bounds: Rect,
        id: &'static str,
    ) -> OverlayOutcome<F> {
        self.open = true;
        open_drawer_overlay(
            stack,
            bounds,
            id,
            OverlaySize {
                width: 32,
                height: bounds.height.max(3),
                min_width: 16,
                min_height: 3,
                max_width: 48,
                max_height: 0,
            },
            None,
        )
    }
}

/// Places a drawer using [`OverlayKind::Drawer`] policy.
#[must_use]
pub fn place_drawer(bounds: Rect, size: OverlaySize) -> Rect {
    if bounds.is_empty() {
        return Rect::default();
    }
    place_overlay(
        bounds,
        None,
        size,
        OverlayPolicy::for_kind(OverlayKind::Drawer),
    )
}

/// Opens (or replaces) a drawer overlay.
pub fn open_drawer_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    id: impl Into<OverlayId>,
    size: OverlaySize,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.open(bounds, OverlaySpec::drawer(id, size, opener_focus))
}

/// Dismisses the default drawer overlay when present.
pub fn dismiss_drawer_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.dismiss(&OverlayId::from_static(DRAWER_OVERLAY_ID))
}

/// Places a popover under `anchor` (flip/clamp via stack policy).
#[must_use]
pub fn place_popover(bounds: Rect, anchor: Rect, size: OverlaySize) -> Rect {
    if bounds.is_empty() || size.width == 0 || size.height == 0 {
        return Rect::default();
    }
    place_overlay(
        bounds,
        Some(anchor),
        size,
        OverlayPolicy::for_kind(OverlayKind::Popover),
    )
}

/// Opens an anchored popover on the stack.
pub fn open_popover_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    anchor: Rect,
    size: OverlaySize,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.open(
        bounds,
        OverlaySpec::popover(POPOVER_OVERLAY_ID, anchor, size, opener_focus),
    )
}

/// Places a tooltip above `anchor` (may hide on tiny terminals).
#[must_use]
pub fn place_tooltip(bounds: Rect, anchor: Rect, size: OverlaySize) -> Rect {
    if bounds.is_empty() || size.width == 0 || size.height == 0 {
        return Rect::default();
    }
    place_overlay(
        bounds,
        Some(anchor),
        size,
        OverlayPolicy::for_kind(OverlayKind::Tooltip),
    )
}

/// Opens a tooltip overlay (no input ownership; outside-click dismissible).
pub fn open_tooltip_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    anchor: Rect,
    size: OverlaySize,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.open(
        bounds,
        OverlaySpec::tooltip(TOOLTIP_OVERLAY_ID, anchor, size, opener_focus),
    )
}

/// Drawer chrome (edge panel).
#[derive(Debug, Clone, Copy)]
pub struct Drawer<'a> {
    title: &'a str,
    tokens: &'a DesignSystem,
}

impl<'a> Drawer<'a> {
    /// Title.
    #[must_use]
    pub const fn new(title: &'a str, tokens: &'a DesignSystem) -> Self {
        Self { title, tokens }
    }
}

impl Widget for &Drawer<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let style = self.tokens.style(Role::Elevated);
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                buffer[(x, y)].set_style(style);
            }
        }
        let text = take_display_cols(self.title, usize::from(area.width));
        buffer.set_stringn(
            area.x,
            area.y,
            &text,
            usize::from(area.width),
            self.tokens.style(Role::TextStrong),
        );
    }
}

/// Popover non-modal chrome.
#[derive(Debug, Clone, Copy)]
pub struct Popover<'a> {
    title: &'a str,
    tokens: &'a DesignSystem,
}

impl<'a> Popover<'a> {
    /// Title.
    #[must_use]
    pub const fn new(title: &'a str, tokens: &'a DesignSystem) -> Self {
        Self { title, tokens }
    }
}

impl Widget for &Popover<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let text = take_display_cols(self.title, usize::from(area.width));
        buffer.set_stringn(
            area.x,
            area.y,
            &text,
            usize::from(area.width),
            self.tokens.style(Role::Elevated),
        );
    }
}

/// Tooltip delay state (FrameTick driven by consumer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TooltipState {
    visible: bool,
    /// Accumulated ms hovering (consumer advances).
    hover_ms: u64,
    delay_ms: u64,
}

impl TooltipState {
    /// Default 400ms delay.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            visible: false,
            hover_ms: 0,
            delay_ms: 400,
        }
    }

    /// Advance hover clock; shows after delay.
    pub fn tick_hover(&mut self, delta_ms: u64, hovering: bool) {
        if !hovering {
            self.hover_ms = 0;
            self.visible = false;
            return;
        }
        self.hover_ms = self.hover_ms.saturating_add(delta_ms);
        if self.hover_ms >= self.delay_ms {
            self.visible = true;
        }
    }

    #[must_use]
    /// Visible.
    pub const fn is_visible(self) -> bool {
        self.visible
    }
}

/// Tooltip paint (never steals focus).
#[derive(Debug, Clone, Copy)]
pub struct Tooltip<'a> {
    text: &'a str,
    tokens: &'a DesignSystem,
}

impl<'a> Tooltip<'a> {
    /// Help text.
    #[must_use]
    pub const fn new(text: &'a str, tokens: &'a DesignSystem) -> Self {
        Self { text, tokens }
    }
}

impl Tooltip<'_> {
    /// Paint when visible (never steals focus).
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &TooltipState) {
        if !state.visible || area.is_empty() {
            return;
        }
        let text = take_display_cols(self.text, usize::from(area.width));
        buffer.set_stringn(
            area.x,
            area.y,
            &text,
            usize::from(area.width),
            self.tokens.style(Role::TextMuted),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyModifiers;

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
    fn sidebar_select() {
        let items = [SidebarItem::new("x", "X"), SidebarItem::new("y", "Y")];
        let mut state = SidebarState::new(None);
        let _ = state.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &items);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &items),
            SidebarOutcome::Selected("y")
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
    fn breadcrumbs_navigate() {
        let items = [
            BreadcrumbItem::new("r", "root"),
            BreadcrumbItem::new("a", "a"),
        ];
        let mut state = BreadcrumbsState::default();
        let _ = state.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &items);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &items),
            BreadcrumbsOutcome::Navigate("a")
        ));
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
